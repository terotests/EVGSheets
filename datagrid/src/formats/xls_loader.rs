// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rgr::{DgNative, OfficeColorMath};
use crate::cell_style::CellStyle;
use crate::formats::cfb::Cfb;
use crate::formats::sheet_bytes::SheetBytes;
use crate::formats::xls_formula::XlsFormula;
use crate::spreadsheet_model::SpreadsheetModel;
use crate::workbook_model::WorkbookModel;
use crate::xlsx_number_format::XlsxNumberFormat;
/// XlsLoader: an Excel 97-2003 workbook (.xls, BIFF8) into a WorkbookModel.
///
/// The file is a compound file (`Cfb`) whose "Workbook" stream is a run of
/// records: [id u16][length u16][data]. A record longer than 8224 bytes goes on
/// in CONTINUE records, and a string that crosses into one restarts with a new
/// flag byte saying whether its characters are one byte or two — so strings are
/// read through `XlsCursor`, which knows where the pieces join.
///
/// The stream opens with the workbook globals (sheet list, shared strings,
/// fonts, formats, cell formats, names, the EXTERNSHEET table) and then one
/// substream per sheet, each between a BOF and an EOF, found by the offsets the
/// BOUNDSHEET records give.
///
/// Read: values, formulas (as text, see `XlsFormula`; a formula it cannot
/// decode keeps its cached value), shared and array formulas, number formats,
/// fonts, fills, borders and alignment, merges, column widths, row heights,
/// hidden rows, columns and sheets, freeze panes, defined names, the 1904 date
/// system. Not read: charts, pictures, comments, conditional formats,
/// validation, hyperlinks. What was skipped is listed in `notes`.
///
/// The design follows GridCraft's clean-room reader (crates/xlsx/src/xls,
/// MIT/Apache-2.0), itself written from [MS-CFB] and [MS-XLS].
/// A record's data, in the pieces CONTINUE records split it into.
#[derive(Clone)]
pub struct XlsCursor {
    pub parts: Vec<Vec<u8>>,
    pub part: i64,
    pub pos: i64,
}

impl XlsCursor {
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
            part: 0,
            pos: 0,
        }
    }
    pub fn settle(&mut self) -> bool {
        let mut moved: bool = false;
        while self.part < (self.parts.len() as i64) - 1 && self.pos >= (self.parts[(self.part) as usize].clone().len() as i64) {
            self.part += 1;
            self.pos = 0;
            moved = true;
        }
        moved
    }
    pub fn atEnd(&mut self) -> bool {
        self.settle();
        if self.part >= (self.parts.len() as i64) {
            return true;
        }
        self.pos >= (self.parts[(self.part) as usize].clone().len() as i64)
    }
    pub fn remaining(&self) -> i64 {
        let mut n: i64 = 0;
        let mut i: i64 = self.part;
        while i < (self.parts.len() as i64) {
            let __len: i64 = self.parts[i as usize].clone().len() as i64;
            if i == self.part {
                n += __len - self.pos;
            } else {
                n += __len;
            }
            i += 1;
        }
        n
    }
    pub fn u8(&mut self) -> i64 {
        self.settle();
        if self.part >= (self.parts.len() as i64) {
            return 0;
        }
        let b: Vec<u8> = self.parts[(self.part) as usize].clone();
        if self.pos >= (b.len() as i64) {
            return 0;
        }
        let v: i64 = b[(self.pos) as usize] as i64;
        self.pos += 1;
        v
    }
    pub fn u16(&mut self) -> i64 {
        let lo: i64 = self.u8();
        lo + self.u8() * 256
    }
    pub fn i32(&mut self) -> i64 {
        let lo: i64 = self.u16();
        let mut hi: i64 = self.u16();
        if hi >= 32768 {
            hi -= 65536;
        }
        hi * 65536 + lo
    }
    pub fn f64(&mut self) -> f64 {
        let mut d: Vec<u8> = vec![0u8; 8];
        let mut i: i64 = 0;
        while i < 8 {
            d[i as usize] = (self.u8()) as u8;
            i += 1;
        }
        SheetBytes::f64(&d, 0)
    }
    pub fn skip(&mut self, n: i64) {
        let mut i: i64 = 0;
        while i < n {
            self.u8();
            i += 1;
        }
    }
    /// Everything left, as one buffer: formula tokens never cross a CONTINUE
    /// boundary in a way that matters, so they are read flat.
    pub fn rest(&mut self) -> Vec<u8> {
        let n: i64 = self.remaining();
        let mut out: Vec<u8> = vec![0u8; (n) as usize];
        let mut w: i64 = 0;
        while self.part < (self.parts.len() as i64) && w < n {
            let b: Vec<u8> = self.parts[(self.part) as usize].clone();
            let take: i64 = (b.len() as i64) - self.pos;
            if take > 0 {
                out[(w) as usize..(w+take) as usize].copy_from_slice(&b[(self.pos) as usize..(self.pos+take) as usize]);
                w += take;
            }
            self.part += 1;
            self.pos = 0;
        }
        out
    }
    /// `cch` characters. At the start of each CONTINUE piece the width flag is
    /// stated again.
    pub fn chars(&mut self, cch: i64, wide: bool) -> String {
        let mut units: Vec<i64> = Vec::new();
        let mut high: bool = wide;
        let mut i: i64 = 0;
        while i < cch {
            let mut here: bool = false;
            if self.part < (self.parts.len() as i64) {
                here = self.pos < (self.parts[(self.part) as usize].clone().len() as i64);
            }
            if !here {
                if self.settle() {
                    high = ((self.u8()) & 1) != 0;
                } else {
                    i = cch;
                    continue;
                }
            }
            if high {
                units.push(self.u16());
            } else {
                units.push(self.u8());
            }
            i += 1;
        }
        SheetBytes::unitsToString(&units)
    }
    pub fn shortString(&mut self) -> String {
        let cch: i64 = self.u8();
        let wide: bool = ((self.u8()) & 1) != 0;
        self.chars(cch, wide)
    }
    pub fn string(&mut self) -> String {
        let cch: i64 = self.u16();
        let wide: bool = ((self.u8()) & 1) != 0;
        self.chars(cch, wide)
    }
    /// XLUnicodeRichExtendedString: the text, then formatting runs and phonetic
    /// data that are skipped.
    pub fn richString(&mut self) -> String {
        let cch: i64 = self.u16();
        let flags: i64 = self.u8();
        let mut runs: i64 = 0;
        if (flags & 8) != 0 {
            runs = self.u16();
        }
        let mut ext: i64 = 0;
        if (flags & 4) != 0 {
            ext = self.i32();
        }
        let text: String = self.chars(cch, (flags & 1) != 0);
        self.skip(runs * 4 + ext);
        text
    }
}

#[derive(Clone)]
pub struct XlsRecord {
    pub id: i64,
    pub cur: Rc<RefCell<XlsCursor>>,
}

impl XlsRecord {
    pub fn new() -> Self {
        Self {
            id: 0,
            cur: Rc::new(RefCell::new(XlsCursor::new())),
        }
    }
}

#[derive(Clone)]
pub struct XlsFont {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub sizePt: f64,
    pub rgb: String,
    pub name: String,
    pub vertAlign: String,
}

impl XlsFont {
    pub fn new() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            sizePt: 10.0,
            rgb: String::new(),
            name: String::new(),
            vertAlign: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct XlsLoader {
    pub lastError: String,
    pub notes: Vec<String>,
    pub stream: Vec<u8>,
    pub pos: i64,
    pub date1904: bool,
    pub sst: Vec<String>,
    pub fonts: Vec<Rc<RefCell<XlsFont>>>,
    pub formats: HashMap<i64,String>,
    pub styles: Vec<Rc<RefCell<CellStyle>>>,
    pub palette: Vec<String>,
    pub sheetNames: Vec<String>,
    pub sheetPos: Vec<i64>,
    pub sheetKind: Vec<i64>,
    pub sheetState: Vec<i64>,
    pub nameText: Vec<String>,
    pub nameRgce: Vec<Vec<u8>>,
    pub nameItab: Vec<i64>,
    pub nameBuiltin: Vec<i64>,
    pub formula: Rc<RefCell<XlsFormula>>,
    /// Per sheet, while it is read.
    pub lastFormulaKey: String,
    pub pendingStringRow: i64,
    pub pendingStringCol: i64,
    pub sharedRgce: HashMap<String,Vec<u8>>,
    pub sharedRgcb: HashMap<String,Vec<u8>>,
    pub sharedCells: Vec<String>,
    pub sharedAnchors: Vec<String>,
    pub failures: Vec<String>,
}

impl XlsLoader {
    pub fn new() -> Self {
        Self {
            lastError: String::new(),
            notes: Vec::new(),
            stream: vec![0u8; 0],
            pos: 0,
            date1904: false,
            sst: Vec::new(),
            fonts: Vec::new(),
            formats: HashMap::new(),
            styles: Vec::new(),
            palette: Vec::new(),
            sheetNames: Vec::new(),
            sheetPos: Vec::new(),
            sheetKind: Vec::new(),
            sheetState: Vec::new(),
            nameText: Vec::new(),
            nameRgce: Vec::new(),
            nameItab: Vec::new(),
            nameBuiltin: Vec::new(),
            formula: Rc::new(RefCell::new(XlsFormula::new())),
            lastFormulaKey: String::new(),
            pendingStringRow: -1,
            pendingStringCol: -1,
            sharedRgce: HashMap::new(),
            sharedRgcb: HashMap::new(),
            sharedCells: Vec::new(),
            sharedAnchors: Vec::new(),
            failures: Vec::new(),
        }
    }
    pub fn isXls(b: &[u8]) -> bool {
        if !Cfb::isCompound(b) {
            return false;
        }
        let cf: Rc<RefCell<Cfb>> = Rc::new(RefCell::new(Cfb::new()));
        if !cf.borrow_mut().open(b.to_vec()) {
            return false;
        }
        cf.borrow().hasStream("Workbook") || cf.borrow().hasStream("Book")
    }
    pub fn hex2(v: i64) -> String {
        let digits: &str = "0123456789ABCDEF";
        let hi: i64 = (v as f64 / 16.0).floor() as i64;
        let lo: i64 = v % 16;
        format!("{}{}", DgNative::substr(&digits, hi, hi + 1), DgNative::substr(&digits, lo, lo + 1))
    }
    pub fn borderStyle(dg: i64) -> String {
        let names: Vec<String> = vec![String::new(), "thin".to_string(), "medium".to_string(), "dashed".to_string(), "dotted".to_string(), "thick".to_string(), "double".to_string(), "hair".to_string(), "mediumDashed".to_string(), "dashDot".to_string(), "mediumDashDot".to_string(), "dashDotDot".to_string(), "mediumDashDotDot".to_string(), "slantDashDot".to_string()];
        if dg < 0 || dg >= (names.len() as i64) {
            return String::new();
        }
        names[dg as usize].clone()
    }
    /// "Sheet1!$A$1:$D$9" → "$A$1:$D$9" (the first range of a list).
    pub fn dropSheet(text: &str) -> String {
        let mut s: String = text.to_string();
        let comma: i64 = DgNative::index_of(&s, ",");
        if comma >= 0 {
            s = DgNative::substr(&s, 0, comma);
        }
        let mut bang: i64 = -1;
        let mut i: i64 = 0;
        while i < DgNative::str_len(&s) {
            if DgNative::char_at(&s, i) == 33 {
                bang = i;
            }
            i += 1;
        }
        DgNative::substr(&s, bang + 1, DgNative::str_len(&s))
    }
    pub fn colWidthToPx(w: f64) -> i64 {
        // COLINFO counts in characters including the padding Excel adds, so
        // this is a touch narrower than the .xlsx conversion.
        let mut px: i64 = (w * 7.0 + 0.5).floor() as i64;
        if px < 16 {
            px = 16;
        }
        px
    }
    pub fn toInt(s: &str) -> i64 {
        let v: Option<i64> = s.parse::<i64>().ok();
        if v.is_none() {
            return 0;
        }
        v.unwrap()
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
    /// --- records ----------------------------------------------------------------
    pub fn nextRecord(&mut self) -> Rc<RefCell<XlsRecord>> {
        let rec: Rc<RefCell<XlsRecord>> = Rc::new(RefCell::new(XlsRecord::new()));
        rec.borrow_mut().id = -1;
        let n: i64 = self.stream.len() as i64;
        if self.pos + 4 > n {
            return rec;
        }
        let mut _tmp_983 = self.stream.clone();
        rec.borrow_mut().id = SheetBytes::u16(&_tmp_983, self.pos.clone());
        let mut __len: i64 = SheetBytes::u16(&self.stream.clone(), self.pos + 2);
        let start: i64 = self.pos + 4;
        if start + __len > n {
            __len = n - start;
        }
        rec.borrow_mut()
            .cur.borrow_mut()
            .parts.push(self.stream[(start) as usize..(start + __len) as usize].to_vec());
        self.pos = start + __len;
        // CONTINUE (0x003C) pieces belong to the record before them.
        while self.pos + 4 <= n {
            let mut _tmp_987 = self.stream.clone();
            if SheetBytes::u16(&_tmp_987, self.pos.clone()) != 60 {
                break;
            }
            let mut clen: i64 = SheetBytes::u16(&self.stream.clone(), self.pos + 2);
            let cs: i64 = self.pos + 4;
            if cs + clen > n {
                clen = n - cs;
            }
            rec.borrow_mut()
                .cur.borrow_mut()
                .parts.push(self.stream[(cs) as usize..(cs + clen) as usize].to_vec());
            self.pos = cs + clen;
        }
        rec
    }
    /// --- the workbook -------------------------------------------------------------
    pub fn loadBytes(&mut self, bytes: &[u8], book: &Rc<RefCell<WorkbookModel>>) -> bool {
        self.lastError = String::new();
        let cf: Rc<RefCell<Cfb>> = Rc::new(RefCell::new(Cfb::new()));
        if !cf.borrow_mut().open(bytes.to_vec()) {
            self.lastError = format!("XLS: {}", cf.borrow().lastError);
            return false;
        }
        if !cf.borrow().hasStream("Workbook") {
            if cf.borrow().hasStream("Book") {
                self.lastError = "Excel 5.0/95 workbooks (BIFF5) are not supported; save the file as .xlsx in Excel and try again".to_string();
            } else {
                self.lastError = "the file has no Workbook stream, so it is not an Excel workbook".to_string();
            }
            return false;
        }
        self.stream = cf.borrow_mut().stream("Workbook");
        if !cf.borrow().lastError.is_empty() {
            self.lastError = format!("XLS: {}", cf.borrow().lastError);
            return false;
        }
        self.initPalette();
        if !self.readGlobals() {
            return false;
        }
        self.buildStyles();
        self.formula.borrow_mut().links.borrow_mut().sheets = self.sheetNames.clone();
        book.borrow_mut().clear();
        // BOUNDSHEET index → model index, -1 for sheets that were skipped.
        let mut model_index: Vec<i64> = Vec::new();
        let mut i: i64 = 0;
        while i < (self.sheetNames.len() as i64) {
            let kind: i64 = self.sheetKind[i as usize];
            if kind != 0 {
                if kind == 2 {
                    self.note("chart sheets were skipped");
                } else {
                    self.note("macro and module sheets were skipped");
                }
                model_index.push(-1);
            } else {
                let sheet: Rc<RefCell<SpreadsheetModel>> = Rc::new(RefCell::new(SpreadsheetModel::new()));
                sheet.borrow_mut().name = self.sheetNames[i as usize].clone();
                sheet.borrow_mut().sheetHidden = self.sheetState[i as usize] != 0;
                sheet.borrow_mut().quietWrites = true;
                if self.readSheet(self.sheetPos[i as usize], &sheet) {
                    sheet.borrow_mut().setStylesFrom(&self.styles.clone());
                    if !self.fonts.is_empty() {
                        let f0: Rc<RefCell<XlsFont>> = self.fonts[0].clone();
                        sheet.borrow_mut().defaultFontName = f0.borrow().name.clone();
                    }
                    sheet.borrow_mut().quietWrites = false;
                    sheet.borrow_mut().version = sheet.borrow().version + 1;
                    model_index.push(book.borrow().sheetCount());
                    book.borrow_mut().addSheet(sheet.clone());
                } else {
                    self.note("dialog sheets and unreadable sheets were skipped");
                    model_index.push(-1);
                }
            }
            i += 1;
        }
        if book.borrow().sheetCount() == 0 {
            self.lastError = "the workbook has no worksheets that can be read".to_string();
            return false;
        }
        self.readNames(book, &model_index);
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
        if !self.failures.is_empty() {
            self.note(&format!("some formulas use {}; their saved results were kept as values", self.failures.join(", ")).clone());
        }
        true
    }
    pub fn initPalette(&mut self) {
        self.palette.clear();
        let mut i: i64 = 0;
        while i < 64 {
            self.palette.push(format!("#{}", OfficeColorMath::indexed_color(i)));
            i += 1;
        }
    }
    pub fn colorOf(&self, icv: i64) -> String {
        if icv >= 0 && icv < (self.palette.len() as i64) {
            return self.palette[icv as usize].clone();
        }
        String::new()
    }
    pub fn readGlobals(&mut self) -> bool {
        self.pos = 0;
        let first: Rc<RefCell<XlsRecord>> = self.nextRecord();
        if first.borrow().id != 2057 {
            self.lastError = "XLS: the Workbook stream does not start with a BOF record".to_string();
            return false;
        }
        let vers: i64 = (first.borrow().cur).borrow_mut().u16();
        let dt: i64 = (first.borrow().cur).borrow_mut().u16();
        if vers != 1536 {
            if vers == 1280 {
                self.lastError = "Excel 5.0/95 workbooks (BIFF5) are not supported; save the file as .xlsx in Excel and try again".to_string();
            } else {
                self.lastError = "unsupported BIFF version (only Excel 97-2003 BIFF8 files are read)".to_string();
            }
            return false;
        }
        if dt != 5 {
            self.lastError = "XLS: the Workbook stream does not start with the workbook globals".to_string();
            return false;
        }
        let mut guard: i64 = 0;
        loop {
            let rec: Rc<RefCell<XlsRecord>> = self.nextRecord();
            if rec.borrow().id < 0 || rec.borrow().id == 10 {
                break;
            }
            guard += 1;
            if guard > 10000000 {
                break;
            }
            let c: Rc<RefCell<XlsCursor>> = rec.borrow().cur.clone().clone();
            if rec.borrow().id == 47 {
                self.lastError = "this .xls workbook is password-protected (encrypted); remove the password in Excel and try again".to_string();
                return false;
            }
            if rec.borrow().id == 133 {
                // BOUNDSHEET8: where the sheet's substream starts, its state,
                // its kind, its name.
                self.sheetPos.push(c.borrow_mut().i32());
                self.sheetState.push((c.borrow_mut().u8()) & 3);
                self.sheetKind.push(c.borrow_mut().u8());
                self.sheetNames.push(c.borrow_mut().shortString());
            }
            if rec.borrow().id == 34 {
                self.date1904 = c.borrow_mut().u16() == 1;
            }
            if rec.borrow().id == 1054 {
                let ifmt: i64 = c.borrow_mut().u16();
                self.formats.insert(ifmt, c.borrow_mut().string());
            }
            if rec.borrow().id == 49 {
                self.fonts.push(self.readFont(&c));
            }
            if rec.borrow().id == 146 {
                // PALETTE: colours 8 onward replaced.
                let cnt: i64 = c.borrow_mut().u16();
                let mut p: i64 = 0;
                while p < cnt {
                    let r: i64 = c.borrow_mut().u8();
                    let g: i64 = c.borrow_mut().u8();
                    let b: i64 = c.borrow_mut().u8();
                    c.borrow_mut().u8();
                    if p + 8 < (self.palette.len() as i64) {
                        self.palette[(p + 8) as usize] = format!("#{}{}{}", XlsLoader::hex2(r), XlsLoader::hex2(g), XlsLoader::hex2(b));
                    }
                    p += 1;
                }
            }
            if rec.borrow().id == 224 {
                self.styles.push(self.readXf(&c));
            }
            if rec.borrow().id == 252 {
                self.readSst(&c);
            }
            if rec.borrow().id == 430 {
                // SUPBOOK: 0x0401 marks this workbook, 0x3A01 an add-in.
                c.borrow_mut().u16();
                let cch: i64 = c.borrow_mut().u16();
                self.formula.borrow_mut()
                    .links.borrow_mut()
                    .bookInternal.push(cch == 1025 || cch == 14849);
                let none: Vec<String> = Vec::new();
                self.formula.borrow_mut()
                    .links.borrow_mut()
                    .bookNames.push(none.clone());
            }
            if rec.borrow().id == 35 {
                // EXTERNNAME: a name in the last SUPBOOK (add-in functions).
                c.borrow_mut().u16();
                c.borrow_mut().skip(4);
                let en: String = c.borrow_mut().shortString();
                let nb: i64 = self.formula.borrow().links.borrow().bookNames.len() as i64;
                if nb > 0 {
                    let mut list: Vec<String> = self.formula.borrow()
                        .links.borrow()
                        .bookNames[(nb - 1) as usize].clone();
                    list.push(en.clone());
                    self.formula.borrow_mut()
                        .links.borrow_mut()
                        .bookNames[(nb - 1) as usize] = list.clone();
                }
            }
            if rec.borrow().id == 23 {
                let nx: i64 = c.borrow_mut().u16();
                let mut xi: i64 = 0;
                while xi < nx && c.borrow().remaining() >= 6 {
                    self.formula.borrow_mut().links.borrow_mut().xtiBook.push(c.borrow_mut().u16());
                    let mut f1: i64 = c.borrow_mut().u16();
                    let mut f2: i64 = c.borrow_mut().u16();
                    if f1 >= 32768 {
                        f1 -= 65536;
                    }
                    if f2 >= 32768 {
                        f2 -= 65536;
                    }
                    self.formula.borrow_mut().links.borrow_mut().xtiFirst.push(f1);
                    self.formula.borrow_mut().links.borrow_mut().xtiLast.push(f2);
                    xi += 1;
                }
            }
            if rec.borrow().id == 24 {
                self.readLbl(&c);
            }
        }
        true
    }
    pub fn readFont(&self, c: &Rc<RefCell<XlsCursor>>) -> Rc<RefCell<XlsFont>> {
        let f: Rc<RefCell<XlsFont>> = Rc::new(RefCell::new(XlsFont::new()));
        let height: i64 = c.borrow_mut().u16();
        let grbit: i64 = c.borrow_mut().u16();
        let icv: i64 = c.borrow_mut().u16();
        let bls: i64 = c.borrow_mut().u16();
        let sss: i64 = c.borrow_mut().u16();
        let uls: i64 = c.borrow_mut().u8();
        c.borrow_mut().skip(3);
        f.borrow_mut().sizePt = (height as f64) / 20.0;
        f.borrow_mut().italic = (grbit & 2) != 0;
        f.borrow_mut().strike = (grbit & 8) != 0;
        f.borrow_mut().bold = bls >= 700;
        f.borrow_mut().underline = uls != 0;
        if sss == 1 {
            f.borrow_mut().vertAlign = "superscript".to_string();
        }
        if sss == 2 {
            f.borrow_mut().vertAlign = "subscript".to_string();
        }
        if icv != 32767 && icv != 8 {
            f.borrow_mut().rgb = self.colorOf(icv);
        }
        f.borrow_mut().name = c.borrow_mut().shortString();
        f
    }
    /// XF: font, number format, alignment, borders, fill — all a CellStyle has
    /// room for.
    pub fn readXf(&self, c: &Rc<RefCell<XlsCursor>>) -> Rc<RefCell<CellStyle>> {
        let st: Rc<RefCell<CellStyle>> = Rc::new(RefCell::new(CellStyle::new()));
        let ifnt: i64 = c.borrow_mut().u16();
        let ifmt: i64 = c.borrow_mut().u16();
        c.borrow_mut().u16();
        let al: i64 = c.borrow_mut().u8();
        let rot: i64 = c.borrow_mut().u8();
        let ind: i64 = c.borrow_mut().u8();
        c.borrow_mut().u8();
        let b1: i64 = c.borrow_mut().u16();
        let b2: i64 = c.borrow_mut().u16();
        let b3: i64 = c.borrow_mut().u16();
        let b4: i64 = c.borrow_mut().u16();
        let fill_colors: i64 = c.borrow_mut().u16();
        // Font index 4 does not exist in BIFF: the indexes after it are one
        // higher than their place in the list.
        let mut fi: i64 = ifnt;
        if fi > 4 {
            fi -= 1;
        }
        if fi >= 0 && fi < (self.fonts.len() as i64) {
            let f: Rc<RefCell<XlsFont>> = self.fonts[fi as usize].clone();
            st.borrow_mut().bold = f.borrow().bold;
            st.borrow_mut().italic = f.borrow().italic;
            st.borrow_mut().underline = f.borrow().underline;
            st.borrow_mut().strike = f.borrow().strike;
            st.borrow_mut().fontSizePt = f.borrow().sizePt;
            st.borrow_mut().vertAlign = f.borrow().vertAlign.clone();
            if !f.borrow().rgb.is_empty() {
                st.borrow_mut().textRgb = f.borrow().rgb.clone();
            }
            st.borrow_mut().fontName = f.borrow().name.clone();
        }
        if self.formats.contains_key(&ifmt) {
            st.borrow_mut().numFmt = self.formats.get(&ifmt).cloned().unwrap();
        } else {
            st.borrow_mut().numFmt = SheetBytes::builtinNumFmt(ifmt);
        }
        let alc: i64 = al & 7;
        let hnames: Vec<String> = vec!["general".to_string(), "left".to_string(), "center".to_string(), "right".to_string(), "fill".to_string(), "justify".to_string(), "centerContinuous".to_string(), "distributed".to_string()];
        st.borrow_mut().align = hnames[alc as usize].clone();
        if alc != 0 {
            st.borrow_mut().alignSet = true;
        } else {
            st.borrow_mut().align = "left".to_string();
        }
        st.borrow_mut().wrapText = (al & 8) != 0;
        let alv: i64 = ((al as f64 / 16.0).floor() as i64) & 7;
        let vnames: Vec<String> = vec!["top".to_string(), "center".to_string(), "bottom".to_string(), "justify".to_string(), "distributed".to_string(), "bottom".to_string(), "bottom".to_string(), "bottom".to_string()];
        st.borrow_mut().valign = vnames[alv as usize].clone();
        if rot > 0 && rot <= 180 {
            st.borrow_mut().textRotation = rot;
        }
        st.borrow_mut().indent = ind & 15;
        // Borders: four 4-bit styles, then 7-bit colour indexes.
        let dg_left: i64 = b1 & 15;
        let dg_right: i64 = ((b1 as f64 / 16.0).floor() as i64) & 15;
        let dg_top: i64 = ((b1 as f64 / 256.0).floor() as i64) & 15;
        let dg_bottom: i64 = ((b1 as f64 / 4096.0).floor() as i64) & 15;
        let icv_left: i64 = b2 & 127;
        let icv_right: i64 = ((b2 as f64 / 128.0).floor() as i64) & 127;
        let icv_top: i64 = b3 & 127;
        let icv_bottom: i64 = ((b3 as f64 / 128.0).floor() as i64) & 127;
        st.borrow_mut().borderLeft.borrow_mut().style = XlsLoader::borderStyle(dg_left);
        st.borrow_mut().borderRight.borrow_mut().style = XlsLoader::borderStyle(dg_right);
        st.borrow_mut().borderTop.borrow_mut().style = XlsLoader::borderStyle(dg_top);
        st.borrow_mut().borderBottom.borrow_mut().style = XlsLoader::borderStyle(dg_bottom);
        st.borrow_mut().borderLeft.borrow_mut().rgb = self.borderRgb(icv_left);
        st.borrow_mut().borderRight.borrow_mut().rgb = self.borderRgb(icv_right);
        st.borrow_mut().borderTop.borrow_mut().rgb = self.borderRgb(icv_top);
        st.borrow_mut().borderBottom.borrow_mut().rgb = self.borderRgb(icv_bottom);
        st.borrow_mut().hasBorder = ((dg_left + dg_right) + dg_top) + dg_bottom > 0;
        // The fill pattern is the top six bits of the second border word.
        let fls: i64 = (b4 as f64 / 1024.0).floor() as i64;
        if fls > 0 {
            let icv_fore: i64 = fill_colors & 127;
            let icv_back: i64 = ((fill_colors as f64 / 128.0).floor() as i64) & 127;
            let mut rgb: String = self.colorOf(icv_fore);
            if fls != 1 && rgb.is_empty() {
                rgb = self.colorOf(icv_back);
            }
            if !rgb.is_empty() {
                st.borrow_mut().hasFill = true;
                st.borrow_mut().fillRgb = rgb.clone();
            }
        }
        st
    }
    pub fn borderRgb(&self, icv: i64) -> String {
        let rgb: String = self.colorOf(icv);
        if rgb.is_empty() {
            return "#000000".to_string();
        }
        rgb
    }
    pub fn buildStyles(&mut self) {
        // Cell formats are the XFs as listed; nothing to do but keep the
        // list's order, which is what the cells index.
        if self.styles.is_empty() {
            self.styles.push(Rc::new(RefCell::new(CellStyle::new())));
        }
    }
    pub fn readSst(&mut self, c: &Rc<RefCell<XlsCursor>>) {
        c.borrow_mut().i32();
        let unique: i64 = c.borrow_mut().i32();
        let mut i: i64 = 0;
        while i < unique {
            if c.borrow_mut().atEnd() {
                break;
            }
            self.sst.push(c.borrow_mut().richString());
            i += 1;
        }
    }
    pub fn readLbl(&mut self, c: &Rc<RefCell<XlsCursor>>) {
        let flags: i64 = c.borrow_mut().u16();
        c.borrow_mut().u8();
        let cch: i64 = c.borrow_mut().u8();
        let cce: i64 = c.borrow_mut().u16();
        c.borrow_mut().skip(2);
        let itab: i64 = c.borrow_mut().u16();
        c.borrow_mut().skip(4);
        let wide: bool = ((c.borrow_mut().u8()) & 1) != 0;
        let raw: String = c.borrow_mut().chars(cch, wide);
        let mut builtin: i64 = -1;
        let mut name: String = raw.clone();
        if (flags & 32) != 0 {
            builtin = 0;
            if !raw.is_empty() {
                builtin = DgNative::char_at(&raw, 0);
            }
            let bn: Vec<String> = vec!["Consolidate_Area".to_string(), "Auto_Open".to_string(), "Auto_Close".to_string(), "Extract".to_string(), "Database".to_string(), "Criteria".to_string(), "Print_Area".to_string(), "Print_Titles".to_string(), "Recorder".to_string(), "Data_Form".to_string(), "Auto_Activate".to_string(), "Auto_Deactivate".to_string(), "Sheet_Title".to_string(), "_FilterDatabase".to_string()];
            if builtin >= 0 && builtin < (bn.len() as i64) {
                name = bn[builtin as usize].clone();
            }
        }
        if (flags & 10) != 0 {
            // A macro name: kept in the list so later indexes line up.
            builtin = 99;
        }
        let all: Vec<u8> = c.borrow_mut().rest();
        let mut take: i64 = cce;
        if take > (all.len() as i64) {
            take = all.len() as i64;
        }
        self.formula.borrow_mut().links.borrow_mut().names.push(name.clone());
        self.nameText.push(name);
        self.nameRgce.push(all[0..(take) as usize].to_vec());
        self.nameItab.push(itab);
        self.nameBuiltin.push(builtin);
    }
    pub fn readNames(&mut self, book: &Rc<RefCell<WorkbookModel>>, model_index: &[i64]) {
        let mut i: i64 = 0;
        while i < (self.nameText.len() as i64) {
            let b: i64 = self.nameBuiltin[i as usize];
            let rg: Vec<u8> = self.nameRgce[i as usize].clone();
            if !rg.is_empty() {
                let text: String = self.formula.borrow_mut().decode(&rg, &vec![0u8; 0], 0, 0);
                if !text.is_empty() {
                    let itab: i64 = self.nameItab[i as usize];
                    let mut target: Option<Rc<RefCell<SpreadsheetModel>>> = None;
                    if itab > 0 && itab <= (model_index.len() as i64) {
                        let mi: i64 = model_index[(itab - 1) as usize];
                        if mi >= 0 {
                            target = Some(book.borrow().sheetAt(mi));
                        }
                    }
                    if b == 6 {
                        if target.is_some() {
                            let sh: Rc<RefCell<SpreadsheetModel>> = target.clone().unwrap();
                            sh.borrow_mut().printArea = XlsLoader::dropSheet(&text);
                        }
                    } else {
                        if b == 7 {
                            if target.is_some() {
                                let sh2: Rc<RefCell<SpreadsheetModel>> = target.clone().unwrap();
                                sh2.borrow_mut().printTitleRows = XlsLoader::dropSheet(&text);
                            }
                        } else {
                            if b < 0 {
                                if itab == 0 {
                                    book.borrow_mut().setName(&self.nameText[i as usize].clone(), &text);
                                } else {
                                    // Sheet-scoped names have no place of their own in
                                    // the model; the workbook-wide one is better than none.
                                    if !book.borrow().hasName(&self.nameText[i as usize].clone()) {
                                        book.borrow_mut().setName(&self.nameText[i as usize].clone(), &text);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    if b < 0 {
                        self.note("some defined names could not be read and were skipped");
                    }
                }
            }
            i += 1;
        }
    }
    /// --- one sheet ----------------------------------------------------------------
    pub fn putCell(&self, sheet: &Rc<RefCell<SpreadsheetModel>>, row: i64, col: i64, xf: i64, value: &str) {
        if ((row < 0 || row >= 65536) || col < 0) || col >= 256 {
            return;
        }
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
        if !value.is_empty() {
            sheet.borrow_mut().setCellRaw(row, col, value);
        }
        // XF 15 is the default cell format, which is what a cell with no
        // style is drawn in already.
        if (xf >= 0 && xf != 15) && xf < (self.styles.len() as i64) {
            sheet.borrow_mut().setCellStyleId(row, col, xf);
        }
    }
    /// A number as the cell's text. In a 1904 workbook a date's serial is
    /// 1462 days behind the 1900 count the model uses, so date-formatted
    /// numbers are moved; other numbers are not dates and stay as they are.
    pub fn numAt(&self, xf: i64, v: f64) -> String {
        let mut x: f64 = v;
        if (self.date1904 && xf >= 0) && xf < (self.styles.len() as i64) {
            let st: Rc<RefCell<CellStyle>> = self.styles[xf as usize].clone();
            if XlsxNumberFormat::isDatePattern(&st.borrow().numFmt.clone()) {
                x = v + 1462.0;
            }
        }
        SheetBytes::numText(x)
    }
    pub fn readSheet(&mut self, at: i64, sheet: &Rc<RefCell<SpreadsheetModel>>) -> bool {
        if at < 0 || at >= (self.stream.len() as i64) {
            return false;
        }
        self.pos = at;
        let first: Rc<RefCell<XlsRecord>> = self.nextRecord();
        if first.borrow().id != 2057 {
            return false;
        }
        let vers: i64 = (first.borrow().cur).borrow_mut().u16();
        let dt: i64 = (first.borrow().cur).borrow_mut().u16();
        if vers != 1536 || dt != 16 {
            return false;
        }
        self.lastFormulaKey = String::new();
        self.pendingStringRow = -1;
        let no_r: HashMap<String,Vec<u8>> = HashMap::new();
        self.sharedRgce = no_r.clone();
        let no_b: HashMap<String,Vec<u8>> = HashMap::new();
        self.sharedRgcb = no_b.clone();
        self.sharedCells.clear();
        self.sharedAnchors.clear();
        let mut depth: i64 = 1;
        let mut guard: i64 = 0;
        loop {
            let rec: Rc<RefCell<XlsRecord>> = self.nextRecord();
            if rec.borrow().id < 0 {
                break;
            }
            guard += 1;
            if guard > 10000000 {
                break;
            }
            if rec.borrow().id == 2057 {
                depth += 1;
                continue;
            }
            if rec.borrow().id == 10 {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                continue;
            }
            if depth > 1 {
                continue;
            }
            if rec.borrow().id == 129 {
                // WSBOOL: a dialog sheet shares the worksheet BOF.
                if (((rec.borrow().cur).borrow_mut().u8()) & 16) != 0 {
                    return false;
                }
                continue;
            }
            self.sheetRecord(&rec, sheet);
        }
        self.resolveShared(sheet);
        true
    }
    pub fn sheetRecord(&mut self, rec: &Rc<RefCell<XlsRecord>>, sheet: &Rc<RefCell<SpreadsheetModel>>) {
        let c: Rc<RefCell<XlsCursor>> = rec.borrow().cur.clone().clone();
        let id: i64 = rec.borrow().id;
        if ((id != 519 && id != 1212) && id != 545) && id != 566 {
            self.pendingStringRow = -1;
        }
        if id == 515 {
            // NUMBER
            let r: i64 = c.borrow_mut().u16();
            let col: i64 = c.borrow_mut().u16();
            let xf: i64 = c.borrow_mut().u16();
            self.putCell(sheet, r, col, xf, &self.numAt(xf, c.borrow_mut().f64()));
            return;
        }
        if id == 638 {
            // RK
            let r2: i64 = c.borrow_mut().u16();
            let c2: i64 = c.borrow_mut().u16();
            let xf2: i64 = c.borrow_mut().u16();
            let raw: Vec<u8> = c.borrow_mut().rest();
            self.putCell(sheet, r2, c2, xf2, &self.numAt(xf2, SheetBytes::rk(&raw, 0)));
            return;
        }
        if id == 189 {
            // MULRK: one row, consecutive columns, (xf, rk) each.
            let all: Vec<u8> = c.borrow_mut().rest();
            let mr: i64 = SheetBytes::u16(&all, 0);
            let mc: i64 = SheetBytes::u16(&all, 2);
            let cnt: i64 = (((all.len() as i64) - 6) as f64 / 6.0).floor() as i64;
            let mut k: i64 = 0;
            while k < cnt {
                let base: i64 = 4 + k * 6;
                let mxf: i64 = SheetBytes::u16(&all, base);
                self.putCell(sheet, mr, mc + k, mxf, &self.numAt(mxf, SheetBytes::rk(&all, base + 2)));
                k += 1;
            }
            return;
        }
        if id == 253 {
            // LABELSST
            let r3: i64 = c.borrow_mut().u16();
            let c3: i64 = c.borrow_mut().u16();
            let xf3: i64 = c.borrow_mut().u16();
            let si: i64 = c.borrow_mut().i32();
            if si >= 0 && si < (self.sst.len() as i64) {
                self.putCell(sheet, r3, c3, xf3, &self.sst[si as usize].clone().clone());
            }
            return;
        }
        if id == 516 {
            // LABEL (older writers)
            let r4: i64 = c.borrow_mut().u16();
            let c4: i64 = c.borrow_mut().u16();
            let xf4: i64 = c.borrow_mut().u16();
            self.putCell(sheet, r4, c4, xf4, &c.borrow_mut().string().clone());
            return;
        }
        if id == 517 {
            // BOOLERR
            let r5: i64 = c.borrow_mut().u16();
            let c5: i64 = c.borrow_mut().u16();
            let xf5: i64 = c.borrow_mut().u16();
            let v: i64 = c.borrow_mut().u8();
            let is_err: i64 = c.borrow_mut().u8();
            if is_err != 0 {
                self.putCell(sheet, r5, c5, xf5, &XlsFormula::errorText(v).clone());
            } else {
                if v != 0 {
                    self.putCell(sheet, r5, c5, xf5, "TRUE");
                } else {
                    self.putCell(sheet, r5, c5, xf5, "FALSE");
                }
            }
            return;
        }
        if id == 513 {
            // BLANK: only its format
            let r6: i64 = c.borrow_mut().u16();
            let c6: i64 = c.borrow_mut().u16();
            self.putCell(sheet, r6, c6, c.borrow_mut().u16(), "");
            return;
        }
        if id == 190 {
            // MULBLANK
            let all2: Vec<u8> = c.borrow_mut().rest();
            let br: i64 = SheetBytes::u16(&all2, 0);
            let bc: i64 = SheetBytes::u16(&all2, 2);
            let bn: i64 = (((all2.len() as i64) - 6) as f64 / 2.0).floor() as i64;
            let mut bk: i64 = 0;
            while bk < bn {
                self.putCell(sheet, br, bc + bk, SheetBytes::u16(&all2, 4 + bk * 2), "");
                bk += 1;
            }
            return;
        }
        if id == 6 {
            self.formulaCell(&c, sheet);
            return;
        }
        if id == 519 {
            // STRING: the text result of the formula just before it.
            if self.pendingStringRow >= 0 {
                let mut _tmp_1037 = self.pendingStringRow.clone();
                sheet.borrow_mut().setCellRaw(_tmp_1037, self.pendingStringCol.clone(), &c.borrow_mut().string());
                self.pendingStringRow = -1;
            }
            return;
        }
        if id == 1212 {
            // SHRFMLA: the formula shared by the cells whose FORMULA record
            // points at the cell just read.
            c.borrow_mut().skip(8);
            let cce: i64 = c.borrow_mut().u16();
            let all3: Vec<u8> = c.borrow_mut().rest();
            let mut take: i64 = cce;
            if take > (all3.len() as i64) {
                take = all3.len() as i64;
            }
            if !self.lastFormulaKey.is_empty() {
                self.sharedRgce.insert(self.lastFormulaKey.clone(), all3[0..(take) as usize].to_vec());
                self.sharedRgcb.insert(self.lastFormulaKey.clone(), all3[(take) as usize..(all3.len() as i64) as usize].to_vec());
            }
            return;
        }
        if id == 545 {
            // ARRAY: the formula of an array range; it goes on its top-left
            // cell, which is the one just read.
            let ar0: i64 = c.borrow_mut().u16();
            let _ar1: i64 = c.borrow_mut().u16();
            let ac0: i64 = c.borrow_mut().u8();
            let _ac1: i64 = c.borrow_mut().u8();
            c.borrow_mut().skip(6);
            let acce: i64 = c.borrow_mut().u16();
            let all4: Vec<u8> = c.borrow_mut().rest();
            let mut atake: i64 = acce;
            if atake > (all4.len() as i64) {
                atake = all4.len() as i64;
            }
            let text: String = self.formula.borrow_mut().decode(&all4[0..(atake) as usize].to_vec(), &all4[(atake) as usize..(all4.len() as i64) as usize].to_vec(), ar0, ac0);
            if !text.is_empty() {
                sheet.borrow_mut().setFormula(ar0, ac0, &text);
            } else {
                let failure: String = self.formula.borrow().failure.clone();
                self.failed(&failure);
            }
            return;
        }
        if id == 229 {
            // MERGECELLS
            let mn: i64 = c.borrow_mut().u16();
            let mut mi: i64 = 0;
            while mi < mn && c.borrow().remaining() >= 8 {
                let m0: i64 = c.borrow_mut().u16();
                let m1: i64 = c.borrow_mut().u16();
                let n0: i64 = c.borrow_mut().u16();
                let n1: i64 = c.borrow_mut().u16();
                if ((m1 >= m0 && n1 >= n0) && n1 < 256) && (m1 - m0) + (n1 - n0) > 0 {
                    let need: i64 = m1 + 1;
                    let need_c: i64 = n1 + 1;
                    if need > sheet.borrow().rowCount || need_c > sheet.borrow().colCount {
                        let mut gr: i64 = sheet.borrow().rowCount;
                        let mut gc: i64 = sheet.borrow().colCount;
                        if need > gr {
                            gr = need;
                        }
                        if need_c > gc {
                            gc = need_c;
                        }
                        sheet.borrow_mut().growTo(gr, gc);
                    }
                    sheet.borrow_mut().addMerge(m0, n0, m1, n1);
                }
                mi += 1;
            }
            return;
        }
        if id == 125 {
            // COLINFO: widths in 1/256 of a character.
            let cf: i64 = c.borrow_mut().u16();
            let mut cl: i64 = c.borrow_mut().u16();
            let width: i64 = c.borrow_mut().u16();
            c.borrow_mut().u16();
            let flags: i64 = c.borrow_mut().u16();
            if cl > 255 {
                cl = 255;
            }
            if cf > cl {
                return;
            }
            let px: i64 = XlsLoader::colWidthToPx((width as f64) / 256.0);
            if cl + 1 > sheet.borrow().colCount {
                // A COLINFO that runs to the last column does not make the
                // sheet that wide; only the columns that already exist are
                // sized.
                cl = sheet.borrow().colCount - 1;
                if cl < cf {
                    if cf < 64 {
                        sheet.borrow_mut().growTo(sheet.borrow().rowCount.clone(), cf + 1);
                        cl = cf;
                    } else {
                        return;
                    }
                }
            }
            let mut cc: i64 = cf;
            while cc <= cl {
                sheet.borrow_mut().setColWidth(cc, px);
                if (flags & 1) != 0 {
                    sheet.borrow_mut().setColHidden(cc, true);
                }
                cc += 1;
            }
            return;
        }
        if id == 520 {
            // ROW: height in twips, flags.
            let rr: i64 = c.borrow_mut().u16();
            c.borrow_mut().skip(4);
            let height: i64 = c.borrow_mut().u16();
            c.borrow_mut().skip(4);
            let rflags: i64 = c.borrow_mut().i32();
            let custom: bool = (rflags & 64) != 0;
            let hidden: bool = (rflags & 32) != 0;
            if custom || hidden {
                if rr + 1 > sheet.borrow().rowCount {
                    sheet.borrow_mut().growTo(rr + 1, sheet.borrow().colCount.clone());
                }
                if custom {
                    let mut hpx: i64 = (((((height & 32767) as f64) / 20.0) * 96.0) / 72.0).floor() as i64;
                    if hpx < 8 {
                        hpx = 8;
                    }
                    sheet.borrow_mut().setRowHeight(rr, hpx);
                }
                if hidden {
                    sheet.borrow_mut().setRowHidden(rr, true);
                }
            }
            return;
        }
        if id == 574 {
            // WINDOW2: gridlines, headings, formulas shown.
            if c.borrow().remaining() >= 10 {
                let wflags: i64 = c.borrow_mut().u16();
                sheet.borrow_mut().showFormulas = (wflags & 1) != 0;
                sheet.borrow_mut().showGridlines = (wflags & 2) != 0;
                sheet.borrow_mut().showHeadings = (wflags & 4) != 0;
            }
            return;
        }
        if id == 65 {
            // PANE: frozen rows and columns (x is columns, y is rows).
            let px2: i64 = c.borrow_mut().u16();
            let py2: i64 = c.borrow_mut().u16();
            if px2 < 256 && py2 < 65536 {
                sheet.borrow_mut().freezeCols = px2;
                sheet.borrow_mut().freezeRows = py2;
            }
            return;
        }
        if id == 28 {
            self.note("cell notes were not imported");
            return;
        }
        if id == 236 {
            self.note("pictures, charts and shapes were not imported");
            return;
        }
        if id == 432 || id == 2169 {
            self.note("conditional formatting was not imported");
            return;
        }
        if id == 434 {
            self.note("data validation was not imported");
            return;
        }
        if id == 440 {
            self.note("hyperlinks were not imported");
            return;
        }
        if id == 176 {
            self.note("PivotTables were not imported; their values were kept");
            return;
        }
    }
    pub fn failed(&mut self, why: &str) {
        if why.is_empty() {
            return;
        }
        let mut i: i64 = 0;
        while i < (self.failures.len() as i64) {
            if self.failures[i as usize].clone() == why {
                return;
            }
            i += 1;
        }
        self.failures.push(why.to_string());
    }
    pub fn formulaCell(&mut self, c: &Rc<RefCell<XlsCursor>>, sheet: &Rc<RefCell<SpreadsheetModel>>) {
        let r: i64 = c.borrow_mut().u16();
        let col: i64 = c.borrow_mut().u16();
        let xf: i64 = c.borrow_mut().u16();
        let mut val: Vec<u8> = vec![0u8; 8];
        let mut i: i64 = 0;
        while i < 8 {
            val[i as usize] = (c.borrow_mut().u8()) as u8;
            i += 1;
        }
        c.borrow_mut().u16();
        c.borrow_mut().skip(4);
        let cce: i64 = c.borrow_mut().u16();
        let all: Vec<u8> = c.borrow_mut().rest();
        let mut take: i64 = cce;
        if take > (all.len() as i64) {
            take = all.len() as i64;
        }
        let rgce: Vec<u8> = all[0..(take) as usize].to_vec();
        let rgcb: Vec<u8> = all[(take) as usize..(all.len() as i64) as usize].to_vec();
        // The cached result: a double, or a tagged value when the top two
        // bytes are 0xFFFF.
        let mut shown: String = String::new();
        let mut string_result: bool = false;
        if (val[6] as i64) == 255 && (val[7] as i64) == 255 {
            let kind: i64 = val[0] as i64;
            if kind == 0 {
                string_result = true;
            }
            if kind == 1 {
                if (val[2] as i64) != 0 {
                    shown = "TRUE".to_string();
                } else {
                    shown = "FALSE".to_string();
                }
            }
            if kind == 2 {
                shown = XlsFormula::errorText(val[2] as i64);
            }
        } else {
            shown = self.numAt(xf, SheetBytes::f64(&val, 0));
        }
        self.putCell(sheet, r, col, xf, &shown);
        self.lastFormulaKey = format!("{},{}", r, col);
        if string_result {
            self.pendingStringRow = r;
            self.pendingStringCol = col;
        }
        // tExp: the cell is one of a shared (or array) formula's cells; the
        // formula itself comes later, so it is resolved at the end.
        if take == 5 && (rgce[0] as i64) == 1 {
            let ar: i64 = SheetBytes::u16(&rgce, 1);
            let ac: i64 = SheetBytes::u16(&rgce, 3);
            self.sharedCells.push(self.lastFormulaKey.clone());
            self.sharedAnchors.push(format!("{},{}", ar, ac));
            return;
        }
        let text: String = self.formula.borrow_mut().decode(&rgce, &rgcb, r, col);
        if !text.is_empty() {
            sheet.borrow_mut().setFormula(r, col, &text);
        } else {
            let failure: String = self.formula.borrow().failure.clone();
            self.failed(&failure);
        }
    }
    pub fn resolveShared(&mut self, sheet: &Rc<RefCell<SpreadsheetModel>>) {
        let mut i: i64 = 0;
        while i < (self.sharedCells.len() as i64) {
            let key: String = self.sharedCells[i as usize].clone();
            let anchor: String = self.sharedAnchors[i as usize].clone();
            if self.sharedRgce.contains_key(&anchor) {
                let parts: Vec<String> = key.split(",").map(|s| s.to_string()).collect::<Vec<String>>();
                let r: i64 = XlsLoader::toInt(&parts[0].clone());
                let c: i64 = XlsLoader::toInt(&parts[1].clone());
                let text: String = self.formula.borrow_mut().decode(&self.sharedRgce.get(&anchor).cloned().unwrap(), &self.sharedRgcb.get(&anchor).cloned().unwrap(), r, c);
                if !text.is_empty() {
                    sheet.borrow_mut().setFormula(r, c, &text);
                } else {
                    let failure: String = self.formula.borrow().failure.clone();
                    self.failed(&failure);
                }
            }
            // An array formula's other cells point at its corner too; they
            // keep their values, and the corner already has the formula.
            i += 1;
        }
    }
}
