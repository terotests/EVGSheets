// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rgr::{DgNative, OpcPackage};
use crate::cell_style::{CellBorder, CellStyle};
use crate::date_serial::DateSerial;
use crate::formats::ods_formula::OdsFormula;
use crate::formats::sheet_bytes::SheetBytes;
use crate::spreadsheet_model::SpreadsheetModel;
use crate::workbook_model::WorkbookModel;
use crate::xlsx::xml_lite::{XmlLite, XmlTag};
/// OdsLoader: an OpenDocument spreadsheet (.ods) into a WorkbookModel.
///
/// An .ods is a ZIP whose content.xml holds every sheet:
///
///   <table:table table:name="Sheet1">
///     <table:table-column table:style-name="co1" table:number-columns-repeated="3"/>
///     <table:table-row table:style-name="ro1">
///       <table:table-cell office:value-type="float" office:value="3"
///                         table:formula="of:=[.A1]+1" table:style-name="ce1">
///         <text:p>3</text:p>
///       </table:table-cell>
///       <table:covered-table-cell/>          (under a merge)
///
/// Rows and cells are run-length coded (`number-rows-repeated`,
/// `number-columns-repeated`), and a sheet usually ends in one empty row
/// repeated a million times — so repeats are honoured only for what has
/// content, and an empty run is a jump.
///
/// Read: values (numbers, text, booleans, dates, times, percentages,
/// currencies), formulas (OpenFormula turned into Excel text, `OdsFormula`),
/// cell styles (font, colour, fill, borders, alignment, wrap, number format),
/// merges, column widths, row heights, hidden rows, columns and sheets,
/// named ranges. Not read: charts, pictures, comments, conditional formats.
/// A table-cell style as read, before its parent is folded in.
#[derive(Clone)]
pub struct OdsCellStyle {
    pub name: String,
    pub parent: String,
    pub dataStyle: String,
    pub st: Rc<RefCell<CellStyle>>,
    /// Which properties this style itself stated, so a parent fills the rest.
    pub marks: HashMap<String,bool>,
}

impl OdsCellStyle {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            parent: String::new(),
            dataStyle: String::new(),
            st: Rc::new(RefCell::new(CellStyle::new())),
            marks: HashMap::new(),
        }
    }
    pub fn mark(&mut self, k: &str) {
        self.marks.insert(k.to_string(), true);
    }
    pub fn stated(&self, k: &str) -> bool {
        self.marks.contains_key(k)
    }
}

#[derive(Clone)]
pub struct OdsLoader {
    pub lastError: String,
    pub notes: Vec<String>,
    pub cellStyles: HashMap<String,Rc<RefCell<OdsCellStyle>>>,
    pub numFormats: HashMap<String,String>,
    pub numMaps: HashMap<String,String>,
    pub colWidths: HashMap<String,i64>,
    pub rowHeights: HashMap<String,i64>,
    pub tableHidden: HashMap<String,bool>,
    pub resolved: HashMap<String,Rc<RefCell<CellStyle>>>,
    pub styleIds: HashMap<String,i64>,
    /// Index 0 is what a cell with no style is drawn in.
    pub styleList: Vec<Rc<RefCell<CellStyle>>>,
}

impl OdsLoader {
    pub fn new() -> Self {
        let mut me = Self {
            lastError: String::new(),
            notes: Vec::new(),
            cellStyles: HashMap::new(),
            numFormats: HashMap::new(),
            numMaps: HashMap::new(),
            colWidths: HashMap::new(),
            rowHeights: HashMap::new(),
            tableHidden: HashMap::new(),
            resolved: HashMap::new(),
            styleIds: HashMap::new(),
            styleList: Vec::new(),
        };
        me.styleList.push(Rc::new(RefCell::new(CellStyle::new())));
        me
    }
    /// "0.889in", "2.26cm", "12pt" → pixels at 96 per inch.
    pub fn lengthPx(s: &str) -> f64 {
        let n: i64 = DgNative::str_len(&s);
        let mut i: i64 = 0;
        while i < n && ((DgNative::char_at(&s, i) >= 48 && DgNative::char_at(&s, i) <= 57 || DgNative::char_at(&s, i) == 46) || DgNative::char_at(&s, i) == 45) {
            i += 1;
        }
        let v: Option<f64> = DgNative::substr(&s, 0, i).parse::<f64>().ok();
        if v.is_none() {
            return 0.0;
        }
        let x: f64 = v.unwrap();
        let unit: String = DgNative::substr(&s, i, n);
        if unit == "in" {
            return x * 96.0;
        }
        if unit == "cm" {
            return (x * 96.0) / 2.54;
        }
        if unit == "mm" {
            return (x * 96.0) / 25.4;
        }
        if unit == "pt" {
            return (x * 96.0) / 72.0;
        }
        if unit == "pc" {
            return x * 16.0;
        }
        x
    }
    pub fn lengthPt(s: &str) -> f64 {
        (OdsLoader::lengthPx(s) * 72.0) / 96.0
    }
    pub fn toInt(s: &str, fallback: i64) -> i64 {
        let v: Option<i64> = s.parse::<i64>().ok();
        if v.is_none() {
            return fallback;
        }
        v.unwrap()
    }
    pub fn numberPattern(min_int: i64, dec: i64, grouping: bool) -> String {
        let mut ip: String = String::new();
        let mut k: i64 = 0;
        while k < min_int {
            ip = format!("{}0", ip);
            k += 1;
        }
        if ip.is_empty() {
            ip = "#".to_string();
        }
        if grouping {
            while DgNative::str_len(&ip) < 4 {
                ip = format!("#{}", ip);
            }
            ip = format!("{},{}", DgNative::substr(&ip, 0, (DgNative::str_len(&ip)) - 3), DgNative::substr(&ip, DgNative::str_len(&ip) - 3, DgNative::str_len(&ip)));
        }
        if dec > 0 {
            ip = format!("{}.", ip);
            let mut d: i64 = 0;
            while d < dec {
                ip = format!("{}0", ip);
                d += 1;
            }
        }
        ip
    }
    /// Literal text in a format code: punctuation as it is, the rest quoted.
    pub fn literal(s: &str) -> String {
        if s.is_empty() {
            return String::new();
        }
        let mut plain: bool = true;
        let mut i: i64 = 0;
        while i < DgNative::str_len(&s) {
            let ch: i64 = DgNative::char_at(&s, i);
            if (((((((ch != 45 && ch != 47) && ch != 58) && ch != 46) && ch != 44) && ch != 32) && ch != 37) && ch != 40) && ch != 41 {
                plain = false;
            }
            i += 1;
        }
        if plain {
            return s.to_string();
        }
        format!("\"{}\"", (s.split("\"").map(|s| s.to_string()).collect::<Vec<String>>()).join(""))
    }
    /// "2024-01-31" or "2024-01-31T12:30:00" → a serial.
    pub fn dateSerial(s: &str) -> f64 {
        if DgNative::str_len(&s) < 10 {
            return 0.0;
        }
        let y: i64 = OdsLoader::toInt(&DgNative::substr(&s, 0, 4), 1900);
        let m: i64 = OdsLoader::toInt(&DgNative::substr(&s, 5, 7), 1);
        let d: i64 = OdsLoader::toInt(&DgNative::substr(&s, 8, 10), 1);
        let mut v: f64 = DateSerial::fromYMD(y, m, d);
        let t_at: i64 = DgNative::index_of(&s, "T");
        if t_at > 0 {
            let tp: String = DgNative::substr(&s, t_at + 1, DgNative::str_len(&s));
            let hms: Vec<String> = tp.split(":").map(|s| s.to_string()).collect::<Vec<String>>();
            let mut h: f64 = 0.0;
            let mut mi: f64 = 0.0;
            let mut se: f64 = 0.0;
            if !hms.is_empty() {
                h = OdsLoader::dbl(&hms[0].clone());
            }
            if (hms.len() as i64) > 1 {
                mi = OdsLoader::dbl(&hms[1].clone());
            }
            if (hms.len() as i64) > 2 {
                se = OdsLoader::dbl(&hms[2].clone());
            }
            v += ((h * 3600.0 + mi * 60.0) + se) / 86400.0;
        }
        v
    }
    pub fn dbl(s: &str) -> f64 {
        let v: Option<f64> = s.parse::<f64>().ok();
        if v.is_none() {
            return 0.0;
        }
        v.unwrap()
    }
    /// "PT12H30M05S" (or "P1DT2H") → a fraction of a day.
    pub fn durationDays(s: &str) -> f64 {
        let mut total: f64 = 0.0;
        let mut num: String = String::new();
        let mut in_time: bool = false;
        let mut i: i64 = 0;
        while i < DgNative::str_len(&s) {
            let ch: i64 = DgNative::char_at(&s, i);
            if (ch >= 48 && ch <= 57) || ch == 46 {
                num = format!("{}{}", num, DgNative::substr(&s, i, i + 1));
            } else {
                let v: f64 = OdsLoader::dbl(&num);
                if ch == 84 {
                    in_time = true;
                }
                if ch == 68 && !in_time {
                    total += v;
                }
                if ch == 72 && in_time {
                    total += v / 24.0;
                }
                if ch == 77 && in_time {
                    total += v / 1440.0;
                }
                if ch == 83 && in_time {
                    total += v / 86400.0;
                }
                num = String::new();
            }
            i += 1;
        }
        total
    }
    /// "$Sheet1.$A$1:.$B$2" as a cell-range-address is written without
    /// brackets; the same text works inside them.
    pub fn absolute(s: &str) -> String {
        s.to_string()
    }
    pub fn note(&mut self, msg: &str) {
        let mut i: i64 = 0;
        while i < (self.notes.len() as i64) {
            if self.notes[i as usize].clone() == msg {
                return;
            }
            i += 1;
        }
        self.notes.push(msg.to_string());
    }
    pub fn loadBytes(&mut self, bytes: &[u8], book: &Rc<RefCell<WorkbookModel>>) -> bool {
        self.lastError = String::new();
        let pkg: Rc<RefCell<OpcPackage>> = Rc::new(RefCell::new(OpcPackage::new()));
        if !OpcPackage::open_bytes(&pkg, bytes, "book.ods".to_string()) {
            self.lastError = "could not open the OpenDocument file: not a zip".to_string();
            return false;
        }
        if OpcPackage::has_part(&pkg, "META-INF/manifest.xml") {
            let man: String = OpcPackage::read_xml(&pkg, "META-INF/manifest.xml");
            if DgNative::index_of(&man, "encryption-data") >= 0 {
                OpcPackage::close(&pkg);
                self.lastError = "this .ods file is password-protected; remove the password in LibreOffice and try again".to_string();
                return false;
            }
        }
        let content: String = OpcPackage::read_xml(&pkg, "content.xml");
        if content.is_empty() {
            OpcPackage::close(&pkg);
            self.lastError = "the OpenDocument file has no content.xml".to_string();
            return false;
        }
        // Common styles first: automatic ones name them as parents.
        if OpcPackage::has_part(&pkg, "styles.xml") {
            self.readStyles(&OpcPackage::read_xml(&pkg, "styles.xml").clone());
        }
        self.readStyles(&content);
        OpcPackage::close(&pkg);
        if self.cellStyles.contains_key("Default") {
            self.styleList[0] = self.resolve("Default", 0);
        }
        book.borrow_mut().clear();
        self.readBody(&content, book);
        if book.borrow().sheetCount() == 0 {
            self.lastError = "the OpenDocument file has no sheets".to_string();
            return false;
        }
        let mut first: i64 = 0;
        let mut k: i64 = 0;
        while k < book.borrow().sheetCount() {
            let sh: Rc<RefCell<SpreadsheetModel>> = book.borrow().sheetAt(k);
            if !sh.borrow().sheetHidden {
                first = k;
                break;
            }
            k += 1;
        }
        book.borrow_mut().setActive(first);
        true
    }
    /// --- styles -----------------------------------------------------------------
    pub fn readStyles(&mut self, xml: &str) {
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(xml.to_string());
        let mut cur: Rc<RefCell<OdsCellStyle>> = Rc::new(RefCell::new(OdsCellStyle::new()));
        let mut in_cell: bool = false;
        let mut family: String = String::new();
        let mut style_name: String = String::new();
        // number styles
        let mut num_name: String = String::new();
        let mut num_parts: Vec<String> = Vec::new();
        let mut num_kind: String = String::new();
        let mut num_map: String = String::new();
        loop {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            let nm: String = t.borrow().name.clone();
            if nm == "style" && !t.borrow().isEnd {
                family = t.borrow().attr("style:family");
                style_name = t.borrow().attr("style:name");
                if family == "table-cell" {
                    let cs: Rc<RefCell<OdsCellStyle>> = Rc::new(RefCell::new(OdsCellStyle::new()));
                    cs.borrow_mut().name = style_name.clone();
                    cs.borrow_mut().parent = t.borrow().attr("style:parent-style-name");
                    cs.borrow_mut().dataStyle = t.borrow().attr("style:data-style-name");
                    self.cellStyles.insert(style_name.clone(), cs.clone());
                    cur = cs.clone();
                    in_cell = true;
                } else {
                    in_cell = false;
                }
                if t.borrow().selfClose {
                    family = String::new();
                }
                continue;
            }
            if nm == "style" && t.borrow().isEnd {
                family = String::new();
                continue;
            }
            if nm == "table-column-properties" && family == "table-column" {
                let w: String = t.borrow().attr("style:column-width");
                if !w.is_empty() {
                    self.colWidths.insert(style_name.clone(), (OdsLoader::lengthPx(&w) + 0.5).floor() as i64);
                }
                continue;
            }
            if nm == "table-row-properties" && family == "table-row" {
                let h: String = t.borrow().attr("style:row-height");
                let optimal: String = t.borrow().attr("style:use-optimal-row-height");
                if !h.is_empty() && optimal != "true" {
                    self.rowHeights.insert(style_name.clone(), (OdsLoader::lengthPx(&h) + 0.5).floor() as i64);
                }
                continue;
            }
            if nm == "table-properties" && family == "table" {
                if t.borrow().attr("table:display") == "false" {
                    self.tableHidden.insert(style_name.clone(), true);
                }
                continue;
            }
            if family == "table-cell" && in_cell {
                let cs2: Rc<RefCell<OdsCellStyle>> = cur.clone();
                if nm == "table-cell-properties" {
                    self.cellProps(&t, &cs2);
                }
                if nm == "paragraph-properties" {
                    let ta: String = t.borrow().attr("fo:text-align");
                    if !ta.is_empty() {
                        if ta == "start" || ta == "left" {
                            cs2.borrow_mut().st.borrow_mut().align = "left".to_string();
                        }
                        if ta == "center" {
                            cs2.borrow_mut().st.borrow_mut().align = "center".to_string();
                        }
                        if ta == "end" || ta == "right" {
                            cs2.borrow_mut().st.borrow_mut().align = "right".to_string();
                        }
                        if ta == "justify" {
                            cs2.borrow_mut().st.borrow_mut().align = "justify".to_string();
                        }
                        cs2.borrow_mut().st.borrow_mut().alignSet = true;
                        cs2.borrow_mut().mark("align");
                    }
                    let ml: String = t.borrow().attr("fo:margin-left");
                    if !ml.is_empty() {
                        let ind: i64 = (OdsLoader::lengthPx(&ml) / 9.0 + 0.5).floor() as i64;
                        if ind > 0 {
                            cs2.borrow_mut().st.borrow_mut().indent = ind;
                            cs2.borrow_mut().mark("indent");
                        }
                    }
                }
                if nm == "text-properties" {
                    self.textProps(&t, &cs2);
                }
                continue;
            }
            // --- number styles
            if ((((((nm == "number-style" || nm == "percentage-style") || nm == "currency-style") || nm == "date-style") || nm == "time-style") || nm == "text-style") || nm == "boolean-style") && !t.borrow().isEnd {
                num_name = t.borrow().attr("style:name");
                num_kind = nm.clone();
                num_parts.clear();
                num_map = String::new();
                if nm == "time-style" {
                    if t.borrow().attr("number:truncate-on-overflow") == "false" {
                        num_kind = "duration".to_string();
                    }
                }
                if t.borrow().selfClose {
                    self.endNumStyle(&num_name, &num_kind, &num_parts, &num_map);
                    num_name = String::new();
                }
                continue;
            }
            if ((((((nm == "number-style" || nm == "percentage-style") || nm == "currency-style") || nm == "date-style") || nm == "time-style") || nm == "text-style") || nm == "boolean-style") && t.borrow().isEnd {
                if !num_name.is_empty() {
                    self.endNumStyle(&num_name, &num_kind, &num_parts, &num_map);
                }
                num_name = String::new();
                continue;
            }
            if !num_name.is_empty() {
                self.numPart(&p, &t, &mut num_parts, &num_kind);
                if nm == "map" && t.borrow().attr("style:condition") == "value()>=0" {
                    num_map = t.borrow().attr("style:apply-style-name");
                }
            }
        }
    }
    pub fn cellProps(&self, t: &Rc<RefCell<XmlTag>>, cs: &Rc<RefCell<OdsCellStyle>>) {
        let bg: String = t.borrow().attr("fo:background-color");
        if DgNative::str_len(&bg) == 7 && DgNative::char_at(&bg, 0) == 35 {
            cs.borrow_mut().st.borrow_mut().hasFill = true;
            cs.borrow_mut().st.borrow_mut().fillRgb = bg.to_uppercase();
            cs.borrow_mut().mark("fill");
        }
        if bg == "transparent" {
            cs.borrow_mut().st.borrow_mut().hasFill = false;
            cs.borrow_mut().mark("fill");
        }
        let wrap: String = t.borrow().attr("fo:wrap-option");
        if wrap == "wrap" {
            cs.borrow_mut().st.borrow_mut().wrapText = true;
            cs.borrow_mut().mark("wrap");
        }
        let va: String = t.borrow().attr("style:vertical-align");
        if va == "top" {
            cs.borrow_mut().st.borrow_mut().valign = "top".to_string();
            cs.borrow_mut().mark("valign");
        }
        if va == "middle" {
            cs.borrow_mut().st.borrow_mut().valign = "center".to_string();
            cs.borrow_mut().mark("valign");
        }
        if va == "bottom" {
            cs.borrow_mut().st.borrow_mut().valign = "bottom".to_string();
            cs.borrow_mut().mark("valign");
        }
        let rot: String = t.borrow().attr("style:rotation-angle");
        if !rot.is_empty() {
            let deg: i64 = OdsLoader::toInt(&rot, 0);
            if deg > 0 && deg <= 90 {
                cs.borrow_mut().st.borrow_mut().textRotation = deg;
                cs.borrow_mut().mark("rotation");
            }
            if deg >= 270 && deg < 360 {
                cs.borrow_mut().st.borrow_mut().textRotation = 90 + (360 - deg);
                cs.borrow_mut().mark("rotation");
            }
        }
        let all: String = t.borrow().attr("fo:border");
        if !all.is_empty() {
            self.border(&all, &cs.borrow().st.borrow().borderTop.clone());
            self.border(&all, &cs.borrow().st.borrow().borderRight.clone());
            self.border(&all, &cs.borrow().st.borrow().borderBottom.clone());
            self.border(&all, &cs.borrow().st.borrow().borderLeft.clone());
            cs.borrow_mut().mark("border");
        }
        if t.borrow().has("fo:border-top") {
            let mut _tmp_918 = cs.borrow().st.borrow().borderTop.clone();
            self.border(&t.borrow().attr("fo:border-top").clone(), &_tmp_918);
            cs.borrow_mut().mark("border");
        }
        if t.borrow().has("fo:border-right") {
            let mut _tmp_919 = cs.borrow().st.borrow().borderRight.clone();
            self.border(&t.borrow().attr("fo:border-right").clone(), &_tmp_919);
            cs.borrow_mut().mark("border");
        }
        if t.borrow().has("fo:border-bottom") {
            let mut _tmp_920 = cs.borrow().st.borrow().borderBottom.clone();
            self.border(&t.borrow().attr("fo:border-bottom").clone(), &_tmp_920);
            cs.borrow_mut().mark("border");
        }
        if t.borrow().has("fo:border-left") {
            let mut _tmp_921 = cs.borrow().st.borrow().borderLeft.clone();
            self.border(&t.borrow().attr("fo:border-left").clone(), &_tmp_921);
            cs.borrow_mut().mark("border");
        }
        cs.borrow_mut().st.borrow_mut().hasBorder = ((cs.borrow().st.borrow().borderTop.borrow().has() || cs.borrow().st.borrow().borderRight.borrow().has()) || cs.borrow().st.borrow().borderBottom.borrow().has()) || cs.borrow().st.borrow().borderLeft.borrow().has();
    }
    /// "0.74pt solid #000000" → thin, black.
    pub fn border(&self, spec: &str, b: &Rc<RefCell<CellBorder>>) {
        if spec == "none" || spec.is_empty() {
            b.borrow_mut().style = String::new();
            return;
        }
        let parts: Vec<String> = spec.split(" ").map(|s| s.to_string()).collect::<Vec<String>>();
        let mut width: f64 = 0.75;
        let mut kind: String = "solid".to_string();
        let mut rgb: String = "#000000".to_string();
        let mut i: i64 = 0;
        while i < (parts.len() as i64) {
            let p: String = parts[i as usize].clone();
            if !p.is_empty() {
                if DgNative::char_at(&p, 0) == 35 {
                    rgb = p.to_uppercase();
                } else {
                    if DgNative::char_at(&p, 0) >= 48 && DgNative::char_at(&p, 0) <= 57 {
                        width = OdsLoader::lengthPt(&p);
                    } else {
                        kind = p.clone();
                    }
                }
            }
            i += 1;
        }
        let mut style: &str = "thin";
        if width > 1.6 {
            style = "medium";
        }
        if width > 2.6 {
            style = "thick";
        }
        if kind == "double" {
            style = "double";
        }
        if kind == "dashed" || kind == "dash" {
            style = "dashed";
            if width > 1.6 {
                style = "mediumDashed";
            }
        }
        if kind == "dotted" {
            style = "dotted";
        }
        if width < 0.3 {
            style = "hair";
        }
        b.borrow_mut().style = style.to_string();
        b.borrow_mut().rgb = rgb.clone();
    }
    pub fn textProps(&self, t: &Rc<RefCell<XmlTag>>, cs: &Rc<RefCell<OdsCellStyle>>) {
        if t.borrow().attr("fo:font-weight") == "bold" {
            cs.borrow_mut().st.borrow_mut().bold = true;
            cs.borrow_mut().mark("bold");
        }
        if t.borrow().attr("fo:font-weight") == "normal" {
            cs.borrow_mut().st.borrow_mut().bold = false;
            cs.borrow_mut().mark("bold");
        }
        if t.borrow().attr("fo:font-style") == "italic" {
            cs.borrow_mut().st.borrow_mut().italic = true;
            cs.borrow_mut().mark("italic");
        }
        let ul: String = t.borrow().attr("style:text-underline-style");
        if !ul.is_empty() {
            cs.borrow_mut().st.borrow_mut().underline = ul != "none";
            cs.borrow_mut().mark("underline");
        }
        let lt: String = t.borrow().attr("style:text-line-through-style");
        if !lt.is_empty() {
            cs.borrow_mut().st.borrow_mut().strike = lt != "none";
            cs.borrow_mut().mark("strike");
        }
        let col: String = t.borrow().attr("fo:color");
        if DgNative::str_len(&col) == 7 && DgNative::char_at(&col, 0) == 35 {
            cs.borrow_mut().st.borrow_mut().textRgb = col.to_uppercase();
            cs.borrow_mut().mark("color");
        }
        let fs: String = t.borrow().attr("fo:font-size");
        if !fs.is_empty() {
            if DgNative::index_of(&fs, "%") < 0 {
                cs.borrow_mut().st.borrow_mut().fontSizePt = OdsLoader::lengthPt(&fs);
                cs.borrow_mut().mark("size");
            }
        }
        let face: String = t.borrow().attr("style:font-name");
        if !face.is_empty() {
            cs.borrow_mut().st.borrow_mut().fontName = face.clone();
            cs.borrow_mut().mark("font");
        }
    }
    /// One element of a number style, as Excel format-code text.
    pub fn numPart(&self, p: &Rc<RefCell<XmlLite>>, t: &Rc<RefCell<XmlTag>>, parts: &mut Vec<String>, kind: &str) {
        let nm: String = t.borrow().name.clone();
        if t.borrow().isEnd {
            return;
        }
        let is_long: bool = t.borrow().attr("number:style") == "long";
        if nm == "number" {
            let mut dec: i64 = OdsLoader::toInt(&t.borrow().attr("number:decimal-places"), 0);
            if t.borrow().has("number:min-decimal-places") {
                let mind: i64 = OdsLoader::toInt(&t.borrow().attr("number:min-decimal-places"), dec);
                if mind < dec {
                    dec = mind;
                }
            }
            let min_int: i64 = OdsLoader::toInt(&t.borrow().attr("number:min-integer-digits"), 1);
            let grouping: bool = t.borrow().attr("number:grouping") == "true";
            parts.push(OdsLoader::numberPattern(min_int, dec, grouping));
            return;
        }
        if nm == "scientific-number" {
            let sdec: i64 = OdsLoader::toInt(&t.borrow().attr("number:decimal-places"), 2);
            let ed: i64 = OdsLoader::toInt(&t.borrow().attr("number:min-exponent-digits"), 2);
            let mut exp: String = String::new();
            let mut k: i64 = 0;
            while k < ed {
                exp = format!("{}0", exp);
                k += 1;
            }
            parts.push(format!("{}E+{}", OdsLoader::numberPattern(1, sdec, false), exp));
            return;
        }
        if nm == "fraction" {
            let dd: i64 = OdsLoader::toInt(&t.borrow().attr("number:min-denominator-digits"), 1);
            let mut q: String = String::new();
            let mut k2: i64 = 0;
            while k2 < dd {
                q = format!("{}?", q);
                k2 += 1;
            }
            parts.push(format!("# {}/{}", q, q));
            return;
        }
        if nm == "day" {
            if is_long {
                parts.push("dd".to_string().to_string());
            } else {
                parts.push("d".to_string().to_string());
            }
            return;
        }
        if nm == "month" {
            if t.borrow().attr("number:textual") == "true" {
                if is_long {
                    parts.push("mmmm".to_string().to_string());
                } else {
                    parts.push("mmm".to_string().to_string());
                }
            } else {
                if is_long {
                    parts.push("mm".to_string().to_string());
                } else {
                    parts.push("m".to_string().to_string());
                }
            }
            return;
        }
        if nm == "year" {
            if is_long {
                parts.push("yyyy".to_string().to_string());
            } else {
                parts.push("yy".to_string().to_string());
            }
            return;
        }
        if nm == "day-of-week" {
            if is_long {
                parts.push("dddd".to_string().to_string());
            } else {
                parts.push("ddd".to_string().to_string());
            }
            return;
        }
        if nm == "hours" {
            let mut hh: String = "h".to_string();
            if is_long {
                hh = "hh".to_string();
            }
            if kind == "duration" {
                hh = format!("[{}]", hh);
            }
            parts.push(hh.clone());
            return;
        }
        if nm == "minutes" {
            if is_long {
                parts.push("mm".to_string().to_string());
            } else {
                parts.push("m".to_string().to_string());
            }
            return;
        }
        if nm == "seconds" {
            let mut ss: String = "s".to_string();
            if is_long {
                ss = "ss".to_string();
            }
            let sd: i64 = OdsLoader::toInt(&t.borrow().attr("number:decimal-places"), 0);
            if sd > 0 {
                ss = format!("{}.", ss);
                let mut k3: i64 = 0;
                while k3 < sd {
                    ss = format!("{}0", ss);
                    k3 += 1;
                }
            }
            parts.push(ss.clone());
            return;
        }
        if nm == "am-pm" {
            parts.push("AM/PM".to_string().to_string());
            return;
        }
        if nm == "text-content" {
            parts.push("@".to_string().to_string());
            return;
        }
        if nm == "text" && !t.borrow().selfClose {
            parts.push(OdsLoader::literal(&p.borrow_mut().readTextUntilEnd("text")));
            return;
        }
        if nm == "currency-symbol" && !t.borrow().selfClose {
            let sym: String = p.borrow_mut().readTextUntilEnd("currency-symbol");
            // Quoted rather than Excel's [$€-40B] locale form: the symbol
            // is what the cell shows, and every formatter reads a literal.
            parts.push(format!("\"{}\"", sym));
            return;
        }
    }
    pub fn endNumStyle(&mut self, name: &str, kind: &str, parts: &[String], map_name: &str) {
        let mut code: String = parts.join("");
        if kind == "boolean-style" {
            code = "General".to_string();
        }
        if kind == "text-style" {
            if code.is_empty() {
                code = "@".to_string();
            }
        }
        if code.is_empty() {
            code = "General".to_string();
        }
        self.numFormats.insert(name.to_string(), code.clone());
        if !map_name.is_empty() {
            self.numMaps.insert(name.to_string(), map_name.to_string());
        }
    }
    /// The Excel code for a data style, with its positive section in front
    /// when it is the negative half of a pair (`style:map`).
    pub fn numFmtFor(&self, data_style: &str) -> String {
        if data_style.is_empty() {
            return "General".to_string();
        }
        if !self.numFormats.contains_key(data_style) {
            return "General".to_string();
        }
        let code: String = self.numFormats.get(data_style).cloned().unwrap();
        if self.numMaps.contains_key(data_style) {
            let pos: String = self.numMaps.get(data_style).cloned().unwrap();
            if self.numFormats.contains_key(&pos) {
                return format!("{};{}", self.numFormats.get(&pos).cloned().unwrap(), code);
            }
        }
        code
    }
    /// A cell style's CellStyle with its parents folded in, as an index into
    /// the sheet's style list; -1 for none.
    pub fn styleIdOf(&mut self, name: &str) -> i64 {
        if name.is_empty() {
            return -1;
        }
        if self.styleIds.contains_key(name) {
            return self.styleIds.get(name).cloned().unwrap();
        }
        let st: Rc<RefCell<CellStyle>> = self.resolve(name, 0);
        let mut id: i64 = -1;
        if !st.borrow().isPlain() {
            id = self.styleList.len() as i64;
            self.styleList.push(st.clone());
        }
        self.styleIds.insert(name.to_string(), id);
        id
    }
    pub fn resolve(&self, name: &str, depth: i64) -> Rc<RefCell<CellStyle>> {
        if !self.cellStyles.contains_key(name) {
            return Rc::new(RefCell::new(CellStyle::new()));
        }
        let cs: Rc<RefCell<OdsCellStyle>> = self.cellStyles.get(name)
            .cloned()
            .unwrap();
        let mut out: Rc<RefCell<CellStyle>> = Rc::new(RefCell::new(CellStyle::new()));
        if !cs.borrow().parent.is_empty() && depth < 8 {
            out = self.resolve(&cs.borrow().parent.clone(),
                depth + 1
            ).borrow().copy();
        }
        let s: Rc<RefCell<CellStyle>> = cs.borrow().st.clone().clone();
        if cs.borrow().stated("fill") {
            out.borrow_mut().hasFill = s.borrow().hasFill;
            out.borrow_mut().fillRgb = s.borrow().fillRgb.clone();
        }
        if cs.borrow().stated("wrap") {
            out.borrow_mut().wrapText = s.borrow().wrapText;
        }
        if cs.borrow().stated("valign") {
            out.borrow_mut().valign = s.borrow().valign.clone();
        }
        if cs.borrow().stated("rotation") {
            out.borrow_mut().textRotation = s.borrow().textRotation;
        }
        if cs.borrow().stated("border") {
            let mut _tmp_936 = s.borrow().borderTop.clone();
            out.borrow().borderTop.clone().borrow_mut().copyFrom(&_tmp_936);
            let mut _tmp_938 = s.borrow().borderRight.clone();
            out.borrow().borderRight.clone().borrow_mut().copyFrom(&_tmp_938);
            let mut _tmp_940 = s.borrow().borderBottom.clone();
            out.borrow().borderBottom.clone().borrow_mut().copyFrom(&_tmp_940);
            let mut _tmp_942 = s.borrow().borderLeft.clone();
            out.borrow().borderLeft.clone().borrow_mut().copyFrom(&_tmp_942);
            out.borrow_mut().hasBorder = s.borrow().hasBorder;
        }
        if cs.borrow().stated("align") {
            out.borrow_mut().align = s.borrow().align.clone();
            out.borrow_mut().alignSet = true;
        }
        if cs.borrow().stated("indent") {
            out.borrow_mut().indent = s.borrow().indent;
        }
        if cs.borrow().stated("bold") {
            out.borrow_mut().bold = s.borrow().bold;
        }
        if cs.borrow().stated("italic") {
            out.borrow_mut().italic = s.borrow().italic;
        }
        if cs.borrow().stated("underline") {
            out.borrow_mut().underline = s.borrow().underline;
        }
        if cs.borrow().stated("strike") {
            out.borrow_mut().strike = s.borrow().strike;
        }
        if cs.borrow().stated("color") {
            out.borrow_mut().textRgb = s.borrow().textRgb.clone();
        }
        if cs.borrow().stated("size") {
            out.borrow_mut().fontSizePt = s.borrow().fontSizePt;
        }
        if cs.borrow().stated("font") {
            out.borrow_mut().fontName = s.borrow().fontName.clone();
        }
        if !cs.borrow().dataStyle.is_empty() {
            out.borrow_mut().numFmt = self.numFmtFor(&cs.borrow().dataStyle.clone());
        }
        out
    }
    /// --- the sheets -------------------------------------------------------------
    pub fn ensure(&self, sheet: &Rc<RefCell<SpreadsheetModel>>, row: i64, col: i64) {
        if row + 1 > sheet.borrow().rowCount || col + 1 > sheet.borrow().colCount {
            let mut nr: i64 = sheet.borrow().rowCount;
            let mut nc: i64 = sheet.borrow().colCount;
            if row + 1 > nr {
                nr = row + 1;
            }
            if col + 1 > nc {
                nc = col + 1;
            }
            sheet.borrow_mut().growTo(nr, nc);
        }
    }
    pub fn readBody(&mut self, xml: &str, book: &Rc<RefCell<WorkbookModel>>) {
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(xml.to_string());
        let mut sheet: Rc<RefCell<SpreadsheetModel>> = Rc::new(RefCell::new(SpreadsheetModel::new()));
        let mut in_sheet: bool = false;
        let mut row: i64 = 0;
        let mut col: i64 = 0;
        let mut row_repeat: i64 = 1;
        let mut row_start: i64 = 0;
        let mut col_at: i64 = 0;
        let mut col_default: HashMap<i64,String> = HashMap::new();
        loop {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            let nm: String = t.borrow().name.clone();
            if nm == "table" && !t.borrow().isEnd {
                let sh: Rc<RefCell<SpreadsheetModel>> = Rc::new(RefCell::new(SpreadsheetModel::new()));
                sh.borrow_mut().name = t.borrow().attr("table:name");
                sh.borrow_mut().quietWrites = true;
                let ts: String = t.borrow().attr("table:style-name");
                if self.tableHidden.contains_key(&ts) {
                    sh.borrow_mut().sheetHidden = true;
                }
                sheet = sh.clone();
                in_sheet = true;
                row = 0;
                col_at = 0;
                let no_defaults: HashMap<i64,String> = HashMap::new();
                col_default = no_defaults.clone();
                continue;
            }
            if nm == "table" && t.borrow().isEnd {
                if in_sheet {
                    self.finishSheet(&sheet);
                    book.borrow_mut().addSheet(sheet.clone());
                }
                in_sheet = false;
                continue;
            }
            if !in_sheet {
                continue;
            }
            let sh2: Rc<RefCell<SpreadsheetModel>> = sheet.clone();
            if nm == "table-column" {
                let rep: i64 = OdsLoader::toInt(&t.borrow().attr("table:number-columns-repeated"), 1);
                let cst: String = t.borrow().attr("table:style-name");
                let hidden: bool = t.borrow().attr("table:visibility") == "collapse";
                let dcs: String = t.borrow().attr("table:default-cell-style-name");
                let mut k: i64 = 0;
                while k < rep && col_at < 1024 {
                    if self.colWidths.contains_key(&cst) {
                        self.ensure(&sh2, 0, col_at);
                        sh2.borrow_mut().setColWidth(col_at, self.colWidths.get(&cst).cloned().unwrap());
                    }
                    if hidden {
                        self.ensure(&sh2, 0, col_at);
                        sh2.borrow_mut().setColHidden(col_at, true);
                    }
                    if !dcs.is_empty() && dcs != "Default" {
                        col_default.insert(col_at, dcs.clone());
                    }
                    col_at += 1;
                    k += 1;
                }
                if k < rep {
                    col_at += rep - k;
                }
                continue;
            }
            if nm == "table-row" && !t.borrow().isEnd {
                row_repeat = OdsLoader::toInt(&t.borrow().attr("table:number-rows-repeated"), 1);
                row_start = row;
                col = 0;
                let rst: String = t.borrow().attr("table:style-name");
                let rhide: bool = t.borrow().attr("table:visibility") == "collapse";
                // Height and hiding only for the first few repeats: a run of a
                // million rows is the sheet's empty tail.
                let mut rk: i64 = 0;
                while rk < row_repeat && rk < 1024 {
                    if self.rowHeights.contains_key(&rst) {
                        self.ensure(&sh2, row + rk, 0);
                        sh2.borrow_mut().setRowHeight(row + rk, self.rowHeights.get(&rst).cloned().unwrap());
                    }
                    if rhide {
                        self.ensure(&sh2, row + rk, 0);
                        sh2.borrow_mut().setRowHidden(row + rk, true);
                    }
                    rk += 1;
                }
                if t.borrow().selfClose {
                    row += row_repeat;
                }
                continue;
            }
            if nm == "table-row" && t.borrow().isEnd {
                row = row_start + row_repeat;
                continue;
            }
            if (nm == "table-cell" || nm == "covered-table-cell") && !t.borrow().isEnd {
                // Read before the cell: reading its text moves the scanner,
                // and the tag is the scanner's.
                let crep: i64 = OdsLoader::toInt(&t.borrow().attr("table:number-columns-repeated"), 1);
                self.readCell(&p, &t, &sh2, row, col, row_repeat, &col_default);
                col += crep;
                continue;
            }
            if nm == "named-range" || nm == "named-expression" {
                let nn: String = t.borrow().attr("table:name");
                let mut expr: String = t.borrow().attr("table:cell-range-address");
                if expr.is_empty() {
                    expr = t.borrow().attr("table:expression");
                }
                if !nn.is_empty() && !expr.is_empty() {
                    let mut r#ref: String = OdsFormula::toExcel(&format!("[{}]", OdsLoader::absolute(&expr)));
                    if DgNative::index_of(&expr, "[") >= 0 {
                        r#ref = OdsFormula::toExcel(&expr);
                    }
                    book.borrow_mut().setName(&nn, &r#ref);
                }
                continue;
            }
        }
    }
    /// Text in a <text:p>, with ODF's coded whitespace expanded. Annotations
    /// (comments) carry their own <text:p> and are left out.
    pub fn cellText(&self, p: &Rc<RefCell<XmlLite>>) -> String {
        let mut paras: Vec<String> = Vec::new();
        let mut cur: String = String::new();
        let mut in_p: i64 = 0;
        let mut skip: i64 = 0;
        let mut any_p: bool = false;
        let mut depth: i64 = 1;
        let mut last: i64 = p.borrow().pos;
        loop {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            if in_p > 0 && skip == 0 {
                cur = format!("{}{}", cur, XmlLite::decodeEntities(&DgNative::substr(&p.borrow().src, last, t.borrow().startPos)));
            }
            last = t.borrow().endPos;
            let nm: String = t.borrow().name.clone();
            if nm == "table-cell" || nm == "covered-table-cell" {
                if t.borrow().isEnd {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if !t.borrow().selfClose {
                        depth += 1;
                    }
                }
                continue;
            }
            if nm == "annotation" || nm == "frame" {
                if t.borrow().isEnd {
                    skip -= 1;
                } else {
                    if !t.borrow().selfClose {
                        skip += 1;
                    }
                }
                continue;
            }
            if skip > 0 {
                continue;
            }
            if nm == "p" || nm == "h" {
                if t.borrow().isEnd {
                    in_p -= 1;
                    paras.push(cur.clone());
                    cur = String::new();
                } else {
                    any_p = true;
                    if t.borrow().selfClose {
                        paras.push(String::new().to_string());
                    } else {
                        in_p += 1;
                    }
                }
                continue;
            }
            if in_p > 0 && nm == "s" {
                let cnt: i64 = OdsLoader::toInt(&t.borrow().attr("text:c"), 1);
                let mut k: i64 = 0;
                while k < cnt {
                    cur = format!("{} ", cur);
                    k += 1;
                }
            }
            if in_p > 0 && nm == "tab" {
                cur = format!("{}\t", cur);
            }
            if in_p > 0 && nm == "line-break" {
                cur = format!("{}\n", cur);
            }
        }
        if !any_p {
            return String::new();
        }
        paras.join("\n")
    }
    pub fn readCell(&mut self, p: &Rc<RefCell<XmlLite>>, t: &Rc<RefCell<XmlTag>>, sheet: &Rc<RefCell<SpreadsheetModel>>, row: i64, col: i64, row_repeat: i64, col_default: &HashMap<i64,String>) {
        let covered: bool = t.borrow().name == "covered-table-cell";
        let rep: i64 = OdsLoader::toInt(&t.borrow().attr("table:number-columns-repeated"), 1);
        let vtype: String = t.borrow().attr("office:value-type");
        let formula: String = t.borrow().attr("table:formula");
        let style_name: String = t.borrow().attr("table:style-name");
        let span_c: i64 = OdsLoader::toInt(&t.borrow().attr("table:number-columns-spanned"), 1);
        let span_r: i64 = OdsLoader::toInt(&t.borrow().attr("table:number-rows-spanned"), 1);
        let mut value: String = String::new();
        if (vtype == "float" || vtype == "percentage") || vtype == "currency" {
            value = SheetBytes::numText(OdsLoader::dbl(&t.borrow().attr("office:value")));
        }
        if vtype == "date" {
            value = SheetBytes::numText(OdsLoader::dateSerial(&t.borrow().attr("office:date-value")));
        }
        if vtype == "time" {
            value = SheetBytes::numText(OdsLoader::durationDays(&t.borrow().attr("office:time-value")));
        }
        if vtype == "boolean" {
            let bv: String = t.borrow().attr("office:boolean-value");
            if bv == "true" || bv == "1" {
                value = "TRUE".to_string();
            } else {
                value = "FALSE".to_string();
            }
        }
        let mut text: String = String::new();
        if !t.borrow().selfClose {
            text = self.cellText(p);
        }
        if vtype == "string" || vtype.is_empty() {
            if t.borrow().has("office:string-value") {
                value = t.borrow().attr("office:string-value");
                if value.is_empty() && !text.is_empty() {
                    value = text.clone();
                }
            } else {
                value = text.clone();
            }
        }
        let mut excel_f: String = String::new();
        if !formula.is_empty() {
            excel_f = OdsFormula::toExcel(&formula);
        }
        if covered {
            return;
        }
        let has_content: bool = !value.is_empty() || !excel_f.is_empty();
        let mut sid: i64 = -1;
        if !style_name.is_empty() {
            sid = self.styleIdOf(&style_name);
        }
        if sid < 0 && col_default.contains_key(&col) {
            sid = self.styleIdOf(&col_default.get(&col).cloned().unwrap());
        }
        if !has_content && sid < 0 {
            return;
        }
        // A styled empty run is drawn only when it is short; LibreOffice
        // repeats a styled blank to the last column.
        let mut max_rep: i64 = rep;
        let mut max_rows: i64 = row_repeat;
        if !has_content {
            if max_rep > 256 {
                return;
            }
            if max_rows > 64 {
                max_rows = 1;
            }
        } else {
            if max_rep > 1024 {
                max_rep = 1024;
            }
            if max_rows > 1024 {
                max_rows = 1024;
            }
        }
        let mut rr: i64 = 0;
        while rr < max_rows {
            let mut k: i64 = 0;
            while k < max_rep {
                let r: i64 = row + rr;
                let c: i64 = col + k;
                self.ensure(sheet, r, c);
                if !value.is_empty() {
                    sheet.borrow_mut().setCellRaw(r, c, &value);
                }
                if !excel_f.is_empty() {
                    sheet.borrow_mut().setFormula(r, c, &excel_f);
                }
                if sid >= 0 {
                    sheet.borrow_mut().setCellStyleId(r, c, sid);
                }
                k += 1;
            }
            rr += 1;
        }
        if span_c > 1 || span_r > 1 {
            self.ensure(sheet, (row + span_r) - 1, (col + span_c) - 1);
            sheet.borrow_mut().addMerge(row, col, (row + span_r) - 1, (col + span_c) - 1);
        }
    }
    pub fn finishSheet(&self, sheet: &Rc<RefCell<SpreadsheetModel>>) {
        sheet.borrow_mut().setStylesFrom(&self.styleList.clone());
        let base: Rc<RefCell<CellStyle>> = self.styleList[0].clone();
        sheet.borrow_mut().defaultFontName = base.borrow().fontName.clone();
        sheet.borrow_mut().quietWrites = false;
        sheet.borrow_mut().version = sheet.borrow().version + 1;
    }
}
