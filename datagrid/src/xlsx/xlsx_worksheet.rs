// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rgr::{DgNative, OpcPackage, OpcRelationship};
use crate::cell_style::CellStyle;
use crate::data_validation::DataValidation;
use crate::formula_engine::FormulaParser;
use crate::rich_text::RichText;
use crate::spreadsheet_model::{CfRule, SheetOpaque, Sparkline, SpreadsheetModel};
use crate::xlsx::xlsx_shared_strings::XlsxSharedStrings;
use crate::xlsx::xml_lite::{XmlLite, XmlTag};
/// XlsxWorksheet - parse one worksheet XML into a SpreadsheetModel.
///
/// Viewer v1: values, shared/inline strings, booleans, cached formula results,
/// column widths, row heights, merge ranges. No formula engine.
#[derive(Clone)]
pub struct XlsxWorksheet {
    /// The sheet's own relationship part, and the comments part it points at.
    /// Both are optional and both are set by the loader before `parse`, because
    /// a hyperlink's target and a note's text live outside the sheet XML.
    pub relsXml: String,
    /// sheetView zoomScale, applied by the loader after the row heights settle.
    pub fileZoom: f64,
    pub commentsXml: String,
    /// Shared formulas. Excel does not repeat a formula it can derive: it writes
    /// the text once, on the first cell of a run —
    ///   <f t="shared" ref="O10:O17" si="3">SUM(C10:N10)</f>
    /// — and every other cell in that run carries only <f t="shared" si="3"/>.
    /// Reading just the tags that have text left those cells with their cached
    /// number and no formula at all, which is what "the sums stop delegating"
    /// looks like from the outside: the totals were no longer formulas, so
    /// nothing downstream of an edit moved. `si` is the run's id; the run's
    /// anchor is the cell the text was on, and every other member is that
    /// formula translated by its offset from the anchor.
    pub sharedText: HashMap<String,String>,
    pub sharedRow: HashMap<String,String>,
    pub sharedCol: HashMap<String,String>,
    /// Row auto-fit. A file states a row's height only when it is NOT the
    /// automatic one, so a row it says nothing about is as tall as its tallest
    /// text needs — which is why 12pt text sits comfortably in Excel inside a
    /// sheet whose stated default is 12.75pt, and was clipped and overlapping
    /// here.
    ///
    /// The sizes themselves cannot be read while parsing: a cell names a style
    /// by index and the style table is not attached to the sheet until the
    /// whole part has been read. So parsing records only WHERE the content is,
    /// per row, and `autoFitRows` — run by the loader once the styles are in
    /// place — reads the sizes and grows the rows.
    pub rowMinCol: Vec<i64>,
    pub rowMaxCol: Vec<i64>,
    pub rowStated: Vec<bool>,
    /// True when any cell carried a style index. Unstyled sheets (common for
    /// large CSV-style exports) can auto-fit from `defaultFontPt` alone — the
    /// per-cell `getCell` walk is what made C++ hang for minutes on 500k rows
    /// while the browser finished in seconds (JS string maps share values;
    /// C++ copies every key lookup).
    pub sawCellStyle: bool,
    /// Scratch for the hot `parseCell` path so we do not allocate two int
    /// vectors per cell just to read A1-style refs.
    pub refRow: i64,
    pub refCol: i64,
}

impl XlsxWorksheet {
    pub fn new() -> Self {
        Self {
            relsXml: String::new(),
            fileZoom: 1.0,
            commentsXml: String::new(),
            sharedText: HashMap::new(),
            sharedRow: HashMap::new(),
            sharedCol: HashMap::new(),
            rowMinCol: Vec::new(),
            rowMaxCol: Vec::new(),
            rowStated: Vec::new(),
            sawCellStyle: false,
            refRow: 0,
            refCol: 0,
        }
    }
    /// rId → Target, out of a sheet's own `.rels` part. The scan is
    /// `OpcPackage`'s, which is the same one the .docx and .pptx readers use;
    /// this file used to have two of its own. The RAW target comes back here
    /// rather than a package path, because a hyperlink's target is a URL and the
    /// caller resolves the rest against the part that named it.
    pub fn relTarget(rels: &str, id: &str) -> String {
        if rels.is_empty() {
            return String::new();
        }
        if id.is_empty() {
            return String::new();
        }
        let list: Vec<Rc<RefCell<OpcRelationship>>> = OpcPackage::parse_relationships(rels);
        let mut i: i64 = 0;
        while i < (list.len() as i64) {
            let rel: Rc<RefCell<OpcRelationship>> = list[i as usize].clone();
            if rel.borrow().id == id {
                return rel.borrow().target.clone();
            }
            i += 1;
        }
        String::new()
    }
    /// The first relationship of a given type — how the comments part and the
    /// drawing part are found. The suffix has to match the END of the type:
    /// "/drawing" is not "/drawingHF", and a header picture is not a picture on
    /// the sheet.
    pub fn relTargetOfType(rels: &str, type_suffix: &str) -> String {
        if rels.is_empty() {
            return String::new();
        }
        let list: Vec<Rc<RefCell<OpcRelationship>>> = OpcPackage::parse_relationships(rels);
        let mut i: i64 = 0;
        while i < (list.len() as i64) {
            let rel: Rc<RefCell<OpcRelationship>> = list[i as usize].clone();
            if OpcPackage::type_ends_with(&rel.borrow().r#type.clone(), type_suffix) {
                if !OpcRelationship::is_external(&rel) {
                    return rel.borrow().target.clone();
                }
            }
            i += 1;
        }
        String::new()
    }
    /// The first whitespace-separated token of a reference list. `sqref` may
    /// name several ranges — "A1:A5 C1:C5" — and one rule per first range is
    /// what the model stores.
    pub fn firstToken(s: &str) -> String {
        let n: i64 = DgNative::str_len(&s);
        let mut i: i64 = 0;
        while i < n {
            if DgNative::char_at(&s, i) == 32 {
                return DgNative::substr(&s, 0, i);
            }
            i += 1;
        }
        s.to_string()
    }
    /// A1 / AA17 → 0-based `[row, col]`.
    pub fn cellRef(r#ref: &str) -> Vec<i64> {
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&r#ref);
        let mut col: i64 = 0;
        let mut saw_col: bool = false;
        while i < n {
            let ch: i64 = DgNative::char_at(&r#ref, i);
            if ch >= 65 && ch <= 90 {
                col = col * 26 + ((ch - 65) + 1);
                saw_col = true;
                i += 1;
            } else {
                if ch >= 97 && ch <= 122 {
                    col = col * 26 + ((ch - 97) + 1);
                    saw_col = true;
                    i += 1;
                } else {
                    break;
                }
            }
        }
        if saw_col {
            col -= 1;
        }
        let mut row: i64 = 0;
        let mut saw_row: bool = false;
        while i < n {
            let ch2: i64 = DgNative::char_at(&r#ref, i);
            if ch2 >= 48 && ch2 <= 57 {
                row = row * 10 + (ch2 - 48);
                saw_row = true;
                i += 1;
            } else {
                break;
            }
        }
        if saw_row {
            row -= 1;
        }
        vec![row, col]
    }
    /// A1:D2 → 0-based `[r1, c1, r2, c2]`; a single cell is a range of one.
    pub fn mergeRange(r#ref: &str) -> Vec<i64> {
        let mut colon: i64 = -1;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(r#ref);
        while i < n {
            if DgNative::char_at(r#ref, i) == 58 {
                colon = i;
            }
            i += 1;
        }
        if colon < 0 {
            let p: Vec<i64> = XlsxWorksheet::cellRef(r#ref);
            return vec![p[0], p[1], p[0], p[1]];
        }
        let a: Vec<i64> = XlsxWorksheet::cellRef(&DgNative::substr(r#ref, 0, colon));
        let b: Vec<i64> = XlsxWorksheet::cellRef(&DgNative::substr(r#ref, colon + 1, n));
        vec![a[0], a[1], b[0], b[1]]
    }
    /// Excel column width (character units) → CSS pixels.
    ///
    /// The unit is "how many digits of the default font fit", and the conversion
    /// Excel documents is `round(w * MDW) + padding`, where MDW is the width of
    /// the widest digit — 7 pixels for the 11-point default. This used 8, which
    /// made every column with a stated width about a seventh wider than the one
    /// the file describes; over a sheet it reads as a zoomed-in grid.
    pub fn colWidthToPx(w: f64) -> i64 {
        let mut px: i64 = ((w * 7.0 + 5.0) + 0.5).floor() as i64;
        if px < 16 {
            px = 16;
        }
        px
    }
    /// Row height in points → pixels (96 DPI).
    pub fn rowHeightToPx(pt: f64) -> i64 {
        let mut px: i64 = ((pt * 96.0) / 72.0).floor() as i64;
        if px < 8 {
            px = 8;
        }
        px
    }
    pub fn parseDouble(s: &str) -> f64 {
        if s.is_empty() {
            return 0.0;
        }
        let parsed: Option<f64> = s.parse::<f64>().ok();
        if parsed.is_some() {
            return parsed.unwrap();
        }
        0.0
    }
    pub fn parseInt0(s: &str) -> i64 {
        let parsed: Option<i64> = s.parse::<i64>().ok();
        if parsed.is_some() {
            return parsed.unwrap();
        }
        0
    }
    /// The worksheet children `XlsxWriter.sheetXml` does not write.
    ///
    /// An allowlist rather than "anything unknown": the writer regenerates
    /// `cols`, `mergeCells`, `autoFilter`, `dataValidations`, `hyperlinks` and
    /// `tableParts` from the model, and preserving those too would put each of
    /// them in the file twice. This names what is genuinely unrepresented, and a
    /// name that is not on it is either modelled or deliberately dropped.
    pub fn isPreserved(name: &str) -> bool {
        if name == "sheetPr" {
            return true;
        }
        if name == "sheetCalcPr" {
            return true;
        }
        if name == "sheetProtection" {
            return true;
        }
        if name == "protectedRanges" {
            return true;
        }
        if name == "scenarios" {
            return true;
        }
        if name == "phoneticPr" {
            return true;
        }
        // The loader DOES read conditional formatting into `cfRules`, and the
        // writer does not write it. Preserving the source is what keeps it in
        // the file until the writer learns to state it.
        if name == "conditionalFormatting" {
            return true;
        }
        if name == "printOptions" {
            return true;
        }
        if name == "pageMargins" {
            return true;
        }
        if name == "pageSetup" {
            return true;
        }
        if name == "headerFooter" {
            return true;
        }
        if name == "rowBreaks" {
            return true;
        }
        if name == "colBreaks" {
            return true;
        }
        if name == "customProperties" {
            return true;
        }
        if name == "cellWatches" {
            return true;
        }
        if name == "ignoredErrors" {
            return true;
        }
        if name == "smartTags" {
            return true;
        }
        // The two that matter most: the pictures and the comments are in the
        // package, and these are the only thing that says so.
        if name == "drawing" {
            return true;
        }
        if name == "drawingHF" {
            return true;
        }
        if name == "legacyDrawing" {
            return true;
        }
        if name == "legacyDrawingHF" {
            return true;
        }
        if name == "picture" {
            return true;
        }
        if name == "oleObjects" {
            return true;
        }
        if name == "controls" {
            return true;
        }
        if name == "webPublishItems" {
            return true;
        }
        if name == "extLst" {
            return true;
        }
        false
    }
    /// The element `t` just opened, whole, as it appears in `xml` — and the
    /// parser left standing just past it.
    ///
    /// A self-closing tag is its own span. Anything else is scanned to its
    /// matching end tag, counting nesting, because `<extLst>` contains an
    /// `<ext>` that may contain another `<extLst>`.
    pub fn spanOf(p: &Rc<RefCell<XmlLite>>, xml: &str, t: &Rc<RefCell<XmlTag>>) -> String {
        let start: i64 = t.borrow().startPos;
        if t.borrow().selfClose {
            return DgNative::substr(&xml, start, t.borrow().endPos);
        }
        let want: String = t.borrow().name.clone();
        let mut depth: i64 = 1;
        while p.borrow().pos < p.borrow().srcLen {
            let n: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !n.borrow().valid {
                return String::new();
            }
            if n.borrow().name == want {
                if n.borrow().isEnd {
                    depth -= 1;
                    if depth == 0 {
                        return DgNative::substr(&xml, start, n.borrow().endPos);
                    }
                } else {
                    if !n.borrow().selfClose {
                        depth += 1;
                    }
                }
            }
        }
        // Ran off the end: the file is truncated. Preserving half an element
        // would produce a package Excel refuses, so preserve none of it.
        String::new()
    }
    /// --- conditional formatting -------------------------------------------------
    ///
    /// Read from the element's own SOURCE rather than from the worksheet parser,
    /// because by the time this runs the element has already been consumed.
    ///
    /// `isPreserved` lists `conditionalFormatting`: the writer does not generate
    /// it, so the loader keeps the raw XML and hands it back at save. That capture
    /// takes the whole element and skips the parser past it — which meant the
    /// branch that READ the rules, further down the same loop, stopped being
    /// reached. A workbook with a colour scale in it round-tripped perfectly and
    /// displayed nothing, and neither half looked wrong on its own.
    ///
    /// So both happen, from one place: the span is kept for the writer AND parsed
    /// here for the screen.
    /// <x14:sparklineGroup type="column"><x14:colorSeries rgb="FF1F4E79"/>
    ///   <x14:sparklines><x14:sparkline><xm:f>Sales!C10:F10</xm:f><xm:sqref>I10</xm:sqref>
    /// The extLst itself stays opaque for the writer; this only reads it for
    /// the screen. Tag names arrive without their namespace prefix.
    pub fn readSparklines(src: &str, model: &Rc<RefCell<SpreadsheetModel>>) {
        if DgNative::index_of(&src, "sparklineGroup") < 0 {
            return;
        }
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(src.to_string());
        let mut kind: String = "line".to_string();
        let mut color: String = "#1F4E79".to_string();
        let mut f: String = String::new();
        loop {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            if t.borrow().isEnd {
                continue;
            }
            if t.borrow().name == "sparklineGroup" {
                kind = "line".to_string();
                if t.borrow().has("type") {
                    kind = t.borrow().attr("type");
                }
                color = "#1F4E79".to_string();
            }
            if t.borrow().name == "colorSeries" {
                let rgb: String = t.borrow().attr("rgb");
                if DgNative::str_len(&rgb) == 8 {
                    color = format!("#{}", DgNative::substr(&rgb, 2, 8));
                }
            }
            if t.borrow().name == "f" && !t.borrow().selfClose {
                f = p.borrow_mut().readTextUntilEnd("f");
            }
            if t.borrow().name == "sqref" && !t.borrow().selfClose {
                let at: String = p.borrow_mut().readTextUntilEnd("sqref").trim().to_string();
                let mut r#ref: String = f.clone();
                let mut bang: i64 = -1;
                let mut bi: i64 = 0;
                while bi < DgNative::str_len(&r#ref) {
                    if DgNative::char_at(&r#ref, bi) == 33 {
                        bang = bi;
                    }
                    bi += 1;
                }
                if bang >= 0 {
                    r#ref = DgNative::substr(&r#ref, bang + 1, DgNative::str_len(&r#ref));
                }
                r#ref = XlsxWorksheet::stripDollars(&r#ref);
                let mut rr1: Vec<i64> = Vec::new();
                let mut cc1: Vec<i64> = Vec::new();
                let mut rr2: Vec<i64> = Vec::new();
                let mut cc2: Vec<i64> = Vec::new();
                let range: Vec<i64> = XlsxWorksheet::mergeRange(&r#ref);
                rr1 = vec![range[0]];
                cc1 = vec![range[1]];
                rr2 = vec![range[2]];
                cc2 = vec![range[3]];
                let mut ar: Vec<i64> = Vec::new();
                let mut ac: Vec<i64> = Vec::new();
                let pos: Vec<i64> = XlsxWorksheet::cellRef(&at);
                ar = vec![pos[0]];
                ac = vec![pos[1]];
                if !rr1.is_empty() && !ar.is_empty() {
                    let sp: Rc<RefCell<Sparkline>> = Rc::new(RefCell::new(Sparkline::new()));
                    sp.borrow_mut().row = ar[0];
                    sp.borrow_mut().col = ac[0];
                    sp.borrow_mut().r1 = rr1[0];
                    sp.borrow_mut().c1 = cc1[0];
                    sp.borrow_mut().r2 = rr2[0];
                    sp.borrow_mut().c2 = cc2[0];
                    sp.borrow_mut().kind = kind.clone();
                    sp.borrow_mut().color = color.clone();
                    model.borrow_mut().sparklines.push(sp.clone());
                }
            }
        }
    }
    pub fn stripDollars(s: &str) -> String {
        let mut out: String = String::new();
        let mut i: i64 = 0;
        while i < DgNative::str_len(&s) {
            if DgNative::char_at(&s, i) != 36 {
                out = format!("{}{}", out, DgNative::substr(&s, i, i + 1));
            }
            i += 1;
        }
        out
    }
    pub fn readConditionalFormatting(src: &str, model: &Rc<RefCell<SpreadsheetModel>>) {
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(src.to_string());
        let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
        if !t.borrow().valid {
            return;
        }
        if t.borrow().name != "conditionalFormatting" {
            return;
        }
        let sq: String = t.borrow().attr("sqref");
        let mut rr1: Vec<i64> = Vec::new();
        let mut cc1: Vec<i64> = Vec::new();
        let mut rr2: Vec<i64> = Vec::new();
        let mut cc2: Vec<i64> = Vec::new();
        rr1.push(0);
        cc1.push(0);
        rr2.push(0);
        cc2.push(0);
        if !sq.is_empty() {
            // take first ref token
            let mut sp: i64 = 0;
            while sp < DgNative::str_len(&sq) {
                let sch: i64 = DgNative::char_at(&sq, sp);
                if sch == 32 {
                    break;
                }
                sp += 1;
            }
            let one: String = DgNative::substr(&sq, 0, sp);
            rr1.clear();
            cc1.clear();
            rr2.clear();
            cc2.clear();
            let range: Vec<i64> = XlsxWorksheet::mergeRange(&one);
            rr1 = vec![range[0]];
            cc1 = vec![range[1]];
            rr2 = vec![range[2]];
            cc2 = vec![range[3]];
        }
        let mut color_idx: i64 = 0;
        // scan children for cfRule until </conditionalFormatting>
        let mut depth: i64 = 1;
        while p.borrow().pos < p.borrow().srcLen && depth > 0 {
            let ct: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !ct.borrow().valid {
                break;
            }
            if ct.borrow().name == "conditionalFormatting" {
                if ct.borrow().isEnd {
                    depth -= 1;
                } else {
                    if !ct.borrow().selfClose {
                        depth += 1;
                    }
                }
            } else {
                if ct.borrow().name == "cfRule" && !ct.borrow().isEnd {
                    let typ: String = ct.borrow().attr("type");
                    let rule: Rc<RefCell<CfRule>> = Rc::new(RefCell::new(CfRule::new()));
                    rule.borrow_mut().r0 = rr1[0];
                    rule.borrow_mut().c0 = cc1[0];
                    rule.borrow_mut().r1 = rr2[0];
                    rule.borrow_mut().c1 = cc2[0];
                    if typ == "colorScale" {
                        rule.borrow_mut().kind = 0;
                        model.borrow_mut().cfRules.push(rule.clone());
                        color_idx = 0;
                    }
                    if typ == "dataBar" {
                        rule.borrow_mut().kind = 2;
                        rule.borrow_mut().color0 = "FF638EC6".to_string();
                        model.borrow_mut().cfRules.push(rule.clone());
                    }
                    if typ == "iconSet" {
                        rule.borrow_mut().kind = 3;
                        rule.borrow_mut().iconSet = "3TrafficLights1".to_string();
                        model.borrow_mut().cfRules.push(rule.clone());
                    }
                    if typ == "cellIs" {
                        rule.borrow_mut().kind = 1;
                        rule.borrow_mut().op = ct.borrow().attr("operator");
                        rule.borrow_mut().fillRgb = "#C6EFCE".to_string();
                        model.borrow_mut().cfRules.push(rule.clone());
                    }
                }
                if ct.borrow().name == "iconSet" && !ct.borrow().isEnd {
                    let nis: i64 = model.borrow().cfRules.len() as i64;
                    if nis > 0 {
                        let last_i: Rc<RefCell<CfRule>> = model.borrow().cfRules[(nis - 1) as usize].clone();
                        let set_name: String = ct.borrow().attr("iconSet");
                        if last_i.borrow().kind == 3 && !set_name.is_empty() {
                            last_i.borrow_mut().iconSet = set_name.clone();
                        }
                    }
                }
                if ct.borrow().name == "cfvo" && !ct.borrow().isEnd {
                    let ncv: i64 = model.borrow().cfRules.len() as i64;
                    if ncv > 0 {
                        let last_v: Rc<RefCell<CfRule>> = model.borrow().cfRules[(ncv - 1) as usize].clone();
                        if last_v.borrow().kind == 3 {
                            let cv: Option<f64> = ct.borrow().attr("val").parse::<f64>().ok();
                            if cv.is_some() {
                                last_v.borrow_mut().thresholds.push(cv.unwrap());
                            }
                        }
                    }
                }
                if ct.borrow().name == "color" && !ct.borrow().isEnd {
                    let ncf: i64 = model.borrow().cfRules.len() as i64;
                    if ncf > 0 {
                        let last: Rc<RefCell<CfRule>> = model.borrow().cfRules[(ncf - 1) as usize].clone();
                        if last.borrow().kind == 2 {
                            let bar_rgb: String = ct.borrow().attr("rgb");
                            if DgNative::str_len(&bar_rgb) >= 6 {
                                last.borrow_mut().color0 = bar_rgb.clone();
                            }
                        }
                        if last.borrow().kind == 0 {
                            let rgb: String = ct.borrow().attr("rgb");
                            if DgNative::str_len(&rgb) >= 6 {
                                if color_idx == 0 {
                                    last.borrow_mut().color0 = rgb.clone();
                                }
                                if color_idx == 1 {
                                    last.borrow_mut().color1 = rgb.clone();
                                }
                                if color_idx == 2 {
                                    last.borrow_mut().color2 = rgb.clone();
                                }
                                color_idx += 1;
                            }
                        }
                    }
                }
                if (ct.borrow().name == "formula" && !ct.borrow().isEnd) && !ct.borrow().selfClose {
                    let ncf2: i64 = model.borrow().cfRules.len() as i64;
                    if ncf2 > 0 {
                        let last2: Rc<RefCell<CfRule>> = model.borrow().cfRules[(ncf2 - 1) as usize].clone();
                        if last2.borrow().kind == 1 {
                            let ftxt: String = p.borrow_mut().readTextUntilEnd("formula");
                            let pv: Option<f64> = ftxt.parse::<f64>().ok();
                            if pv.is_some() {
                                if last2.borrow().v1 == 0.0 {
                                    last2.borrow_mut().v1 = pv.unwrap();
                                } else {
                                    last2.borrow_mut().v2 = pv.unwrap();
                                }
                            }
                        }
                    }
                }
                if ct.borrow().name == "dxf" && !ct.borrow().isEnd {
                    // fill may appear later; default green highlight for cellIs
                    let ncf3: i64 = model.borrow().cfRules.len() as i64;
                    if ncf3 > 0 {
                        let last3: Rc<RefCell<CfRule>> = model.borrow().cfRules[(ncf3 - 1) as usize].clone();
                        if last3.borrow().kind == 1 {
                            if last3.borrow().fillRgb.is_empty() {
                                last3.borrow_mut().fillRgb = "#C6EFCE".to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    /// Notes, out of `xl/comments1.xml`. The author list is skipped: what a
    /// spreadsheet shows in the little box is the text, and our note is one
    /// string rather than a run list.
    pub fn parseComments(&self, model: &Rc<RefCell<SpreadsheetModel>>, xml: &str) {
        if xml.is_empty() {
            return;
        }
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(xml.to_string());
        while p.borrow().pos < p.borrow().srcLen {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            if t.borrow().name == "comment" && !t.borrow().isEnd {
                let r#ref: String = t.borrow().attr("ref");
                let mut r_out: Vec<i64> = Vec::new();
                let mut c_out: Vec<i64> = Vec::new();
                let pos: Vec<i64> = XlsxWorksheet::cellRef(&r#ref);
                r_out = vec![pos[0]];
                c_out = vec![pos[1]];
                let body: String = p.borrow_mut().collectTTextUntil("comment");
                if !r_out.is_empty() {
                    if !body.is_empty() {
                        model.borrow_mut().setNote(r_out[0], c_out[0], &body);
                    }
                }
            }
        }
    }
    pub fn applyDimension(&self, model: &Rc<RefCell<SpreadsheetModel>>, r#ref: &str) {
        if DgNative::str_len(&r#ref) == 0 {
            return;
        }
        let mut r1: Vec<i64> = Vec::new();
        let mut c1: Vec<i64> = Vec::new();
        let mut r2: Vec<i64> = Vec::new();
        let mut c2: Vec<i64> = Vec::new();
        let range: Vec<i64> = XlsxWorksheet::mergeRange(r#ref);
        r1 = vec![range[0]];
        c1 = vec![range[1]];
        r2 = vec![range[2]];
        c2 = vec![range[3]];
        let mut rows: i64 = r2[0] + 1;
        let mut cols: i64 = c2[0] + 1;
        if rows < 1 {
            rows = 1;
        }
        if cols < 1 {
            cols = 1;
        }
        if rows < model.borrow().rowCount {
            rows = model.borrow().rowCount;
        }
        if cols < model.borrow().colCount {
            cols = model.borrow().colCount;
        }
        model.borrow_mut().resize(rows, cols);
    }
    pub fn ensureSize(&self, model: &Rc<RefCell<SpreadsheetModel>>, row: i64, col: i64) {
        let mut rows: i64 = model.borrow().rowCount;
        let mut cols: i64 = model.borrow().colCount;
        if row + 1 > rows {
            rows = row + 1;
        }
        if col + 1 > cols {
            cols = col + 1;
        }
        if rows != model.borrow().rowCount || cols != model.borrow().colCount {
            // growTo, NOT resize: `resize` rebuilds the width and height arrays
            // from the defaults, and this runs in the middle of parsing — after
            // <cols> has already said how wide column A is. A file that widens
            // an early column and then mentions a later one lost the first
            // width entirely.
            model.borrow_mut().growTo(rows, cols);
        }
    }
    pub fn parse(&mut self, xml: &str, model: &Rc<RefCell<SpreadsheetModel>>, sst: &Rc<RefCell<XlsxSharedStrings>>) {
        model.borrow_mut().clearSheet();
        // One parser serves every sheet in the book, and `si` is numbered from
        // scratch in each of them.
        let no_text: HashMap<String,String> = HashMap::new();
        let no_row: HashMap<String,String> = HashMap::new();
        let no_col: HashMap<String,String> = HashMap::new();
        self.sharedText = no_text.clone();
        self.sharedRow = no_row.clone();
        self.sharedCol = no_col.clone();
        self.rowMinCol.clear();
        self.rowMaxCol.clear();
        self.rowStated.clear();
        self.sawCellStyle = false;
        if xml.is_empty() {
            return;
        }
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(xml.to_string());
        // Depth-of-sheetData, so an `<extLst>` inside a cell is never mistaken
        // for the worksheet's own. Everything captured below is a DIRECT child
        // of `<worksheet>`; nothing inside `<sheetData>` is.
        let mut in_sheet_data: bool = false;
        while p.borrow().pos < p.borrow().srcLen {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            if t.borrow().name == "sheetData" {
                in_sheet_data = !t.borrow().isEnd;
                if t.borrow().selfClose {
                    in_sheet_data = false;
                }
            }
            // A worksheet child this writer does not generate. Taken whole,
            // source and all, so it can go back exactly as it came — see
            // `SheetOpaque`. Without this a saved workbook keeps the pictures
            // in the package and points at none of them.
            if !in_sheet_data {
                if !t.borrow().isEnd {
                    if XlsxWorksheet::isPreserved(&t.borrow().name.clone()) {
                        let src: String = XlsxWorksheet::spanOf(&p, xml, &t);
                        if !src.is_empty() {
                            let keep: Rc<RefCell<SheetOpaque>> = Rc::new(RefCell::new(SheetOpaque::new()));
                            keep.borrow_mut().name = t.borrow().name.clone();
                            keep.borrow_mut().source = src.clone();
                            model.borrow_mut().opaque.push(keep.clone());
                            // Kept for the writer AND read for the screen. The
                            // capture consumed the element, so anything that
                            // wants to look inside one has to do it here.
                            if t.borrow().name == "conditionalFormatting" {
                                XlsxWorksheet::readConditionalFormatting(&src, model);
                            }
                            if t.borrow().name == "extLst" {
                                XlsxWorksheet::readSparklines(&src, model);
                            }
                        }
                        continue;
                    }
                }
            }
            // <sheetFormatPr defaultRowHeight="12.75" defaultColWidth="12.63"/>
            // — how big a row or column is when the file does not say. Most
            // columns in a real sheet have no <col> entry at all, so ignoring
            // this was ignoring most of the sheet's geometry.
            if t.borrow().name == "sheetFormatPr" && !t.borrow().isEnd {
                let mut def_h: i64 = 0;
                let mut def_w: i64 = 0;
                if t.borrow().has("defaultRowHeight") {
                    let hpt: f64 = XlsxWorksheet::parseDouble(&t.borrow().attr("defaultRowHeight"));
                    if hpt > 0.0 {
                        def_h = XlsxWorksheet::rowHeightToPx(hpt);
                    }
                }
                if t.borrow().has("defaultColWidth") {
                    let wch: f64 = XlsxWorksheet::parseDouble(&t.borrow().attr("defaultColWidth"));
                    if wch > 0.0 {
                        def_w = XlsxWorksheet::colWidthToPx(wch);
                    }
                }
                if def_h > 0 || def_w > 0 {
                    model.borrow_mut().setDefaultGeometry(def_h, def_w);
                }
            }
            if t.borrow().name == "dimension" && !t.borrow().isEnd {
                self.applyDimension(model, &t.borrow().attr("ref").clone());
            }
            if t.borrow().name == "sheetView" && !t.borrow().isEnd {
                if t.borrow().attr("showGridLines") == "0" {
                    model.borrow_mut().showGridlines = false;
                }
                self.fileZoom = 1.0;
                if t.borrow().attr("view") == "pageBreakPreview" {
                    model.borrow_mut().viewMode = "pageBreak".to_string();
                }
                if t.borrow().attr("view") == "pageLayout" {
                    model.borrow_mut().viewMode = "pageLayout".to_string();
                }
                let zs: String = t.borrow().attr("zoomScale");
                if !zs.is_empty() {
                    let zv: f64 = XlsxWorksheet::parseDouble(&zs);
                    if zv >= 10.0 && zv <= 400.0 {
                        self.fileZoom = zv / 100.0;
                    }
                }
                if t.borrow().attr("showRowColHeaders") == "0" {
                    model.borrow_mut().showHeadings = false;
                }
                if t.borrow().attr("showFormulas") == "1" {
                    model.borrow_mut().showFormulas = true;
                }
            }
            if t.borrow().name == "pane" && !t.borrow().isEnd {
                // `xSplit` / `ySplit` mean two different things and the `state`
                // attribute is what says which.
                //
                //   state="frozen" / "frozenSplit"  counts of columns and rows
                //   state="split", or absent        TWIPS — a position on the
                //                                   page, twentieths of a point
                //
                // Read without looking at `state`, a sheet split a third of the
                // way down reports two thousand frozen rows. Nothing on screen
                // says so, because a freeze that large is clamped when it is
                // drawn — but everything that asks the model how many rows are
                // pinned gets two thousand, and sorting was one of them: every
                // row it could see was a heading, so a sort marked the column
                // and moved nothing.
                //
                // A split pane is not implemented here, so it is read as no
                // freeze at all rather than as a freeze in the wrong unit.
                let st: String = t.borrow().attr("state");
                let mut frozen: bool = false;
                if st == "frozen" {
                    frozen = true;
                }
                if st == "frozenSplit" {
                    frozen = true;
                }
                if frozen {
                    let xs: String = t.borrow().attr("xSplit");
                    let ys: String = t.borrow().attr("ySplit");
                    let mut fc: i64 = 0;
                    let mut fr: i64 = 0;
                    if !xs.is_empty() {
                        fc = XlsxWorksheet::parseInt0(&xs);
                    }
                    if !ys.is_empty() {
                        fr = XlsxWorksheet::parseInt0(&ys);
                    }
                    model.borrow_mut().setFreeze(fr, fc);
                }
            }
            if t.borrow().name == "col" && !t.borrow().isEnd {
                let min_c: i64 = XlsxWorksheet::parseInt0(&t.borrow().attr("min")) - 1;
                let mut max_c: i64 = XlsxWorksheet::parseInt0(&t.borrow().attr("max")) - 1;
                let w: f64 = XlsxWorksheet::parseDouble(&t.borrow().attr("width"));
                let px: i64 = XlsxWorksheet::colWidthToPx(w);
                if max_c < min_c {
                    max_c = min_c;
                }
                self.ensureSize(model, 0, max_c);
                let mut hide_col: bool = false;
                if t.borrow().has("hidden") {
                    let hv: String = t.borrow().attr("hidden");
                    if hv == "1" || hv == "true" {
                        hide_col = true;
                    }
                }
                let mut c: i64 = min_c;
                while c <= max_c {
                    model.borrow_mut().setColWidth(c, px);
                    if hide_col {
                        model.borrow_mut().setColHidden(c, true);
                    }
                    c += 1;
                }
            }
            if t.borrow().name == "autoFilter" && !t.borrow().isEnd {
                // record filter range start col for SheetView (basic)
                let r#ref: String = t.borrow().attr("ref");
                if DgNative::str_len(&r#ref) > 0 {
                    let mut r1: Vec<i64> = Vec::new();
                    let mut c1: Vec<i64> = Vec::new();
                    let mut r2: Vec<i64> = Vec::new();
                    let mut c2: Vec<i64> = Vec::new();
                    let range: Vec<i64> = XlsxWorksheet::mergeRange(&r#ref);
                    r1 = vec![range[0]];
                    c1 = vec![range[1]];
                    r2 = vec![range[2]];
                    c2 = vec![range[3]];
                    model.borrow_mut().filterCol = c1[0];
                    // enabled after user applies; presence marks autofilter exists
                    model.borrow_mut().filterEnabled = false;
                }
            }
            if (t.borrow().name == "row" && !t.borrow().isEnd) && !t.borrow().selfClose {
                let r_attr: String = t.borrow().attr("r");
                let mut row_idx: i64 = 0;
                if !r_attr.is_empty() {
                    row_idx = XlsxWorksheet::parseInt0(&r_attr) - 1;
                }
                self.ensureSize(model, row_idx, 0);
                if t.borrow().has("ht") {
                    let ht: f64 = XlsxWorksheet::parseDouble(&t.borrow().attr("ht"));
                    model.borrow_mut().setRowHeight(row_idx, XlsxWorksheet::rowHeightToPx(ht));
                    self.noteRowStated(row_idx);
                }
                if t.borrow().has("hidden") {
                    let rh: String = t.borrow().attr("hidden");
                    if rh == "1" || rh == "true" {
                        model.borrow_mut().setRowHidden(row_idx, true);
                    }
                }
                // walk children until </row>
                let mut depth: i64 = 1;
                while p.borrow().pos < p.borrow().srcLen && depth > 0 {
                    let ct: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
                    if !ct.borrow().valid {
                        break;
                    }
                    if ct.borrow().name == "row" {
                        if ct.borrow().isEnd {
                            depth -= 1;
                        } else {
                            if !ct.borrow().selfClose {
                                depth += 1;
                            }
                        }
                    } else {
                        if ct.borrow().name == "c" && !ct.borrow().isEnd {
                            self.parseCell(&p, &ct, model, sst, row_idx);
                        }
                    }
                }
            }
            if t.borrow().name == "dataValidation" && !t.borrow().isEnd {
                let dv: Rc<RefCell<DataValidation>> = Rc::new(RefCell::new(DataValidation::new()));
                dv.borrow_mut().kind = t.borrow().attr("type");
                let dop: String = t.borrow().attr("operator");
                if !dop.is_empty() {
                    dv.borrow_mut().op = dop.clone();
                }
                let ab: String = t.borrow().attr("allowBlank");
                if ab == "0" || ab == "false" {
                    dv.borrow_mut().allowBlank = false;
                }
                dv.borrow_mut().errorTitle = t.borrow().attr("errorTitle");
                dv.borrow_mut().errorText = t.borrow().attr("error");
                // `sqref` may name several ranges; the first is taken, which is
                // what a single rule almost always is.
                let sq: String = t.borrow().attr("sqref");
                let mut vr1: Vec<i64> = Vec::new();
                let mut vc1: Vec<i64> = Vec::new();
                let mut vr2: Vec<i64> = Vec::new();
                let mut vc2: Vec<i64> = Vec::new();
                let range: Vec<i64> = XlsxWorksheet::mergeRange(&XlsxWorksheet::firstToken(&sq));
                vr1 = vec![range[0]];
                vc1 = vec![range[1]];
                vr2 = vec![range[2]];
                vc2 = vec![range[3]];
                if !vr1.is_empty() {
                    dv.borrow_mut().r0 = vr1[0];
                    dv.borrow_mut().c0 = vc1[0];
                    dv.borrow_mut().r1 = vr2[0];
                    dv.borrow_mut().c1 = vc2[0];
                }
                if !t.borrow().selfClose {
                    // formula1 / formula2 are elements rather than attributes.
                    let mut depth_v: i64 = 1;
                    while p.borrow().pos < p.borrow().srcLen && depth_v > 0 {
                        let vt: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
                        if !vt.borrow().valid {
                            break;
                        }
                        if vt.borrow().name == "dataValidation" {
                            if vt.borrow().isEnd {
                                depth_v -= 1;
                            }
                        }
                        if vt.borrow().name == "formula1" && !vt.borrow().isEnd {
                            dv.borrow_mut().f1 = p.borrow_mut().readTextUntilEnd("formula1");
                        }
                        if vt.borrow().name == "formula2" && !vt.borrow().isEnd {
                            dv.borrow_mut().f2 = p.borrow_mut().readTextUntilEnd("formula2");
                        }
                    }
                }
                if !dv.borrow().kind.is_empty() {
                    model.borrow_mut().addValidation(dv.clone());
                }
            }
            if t.borrow().name == "hyperlink" && !t.borrow().isEnd {
                let href: String = t.borrow().attr("ref");
                let mut hr: Vec<i64> = Vec::new();
                let mut hc: Vec<i64> = Vec::new();
                let pos: Vec<i64> = XlsxWorksheet::cellRef(&href);
                hr = vec![pos[0]];
                hc = vec![pos[1]];
                if !hr.is_empty() {
                    // An external link is a relationship; an internal one names
                    // a place in this workbook and has no target at all.
                    let mut target: String = XlsxWorksheet::relTarget(&self.relsXml.clone(), &t.borrow().attr("r:id"));
                    if target.is_empty() {
                        let loc: String = t.borrow().attr("location");
                        if !loc.is_empty() {
                            target = format!("#{}", loc);
                        }
                    }
                    if !target.is_empty() {
                        model.borrow_mut().setHyperlink(hr[0], hc[0], &target);
                    }
                }
            }
            if t.borrow().name == "mergeCell" && !t.borrow().isEnd {
                let mref: String = t.borrow().attr("ref");
                let mut r1_1: Vec<i64> = Vec::new();
                let mut c1_1: Vec<i64> = Vec::new();
                let mut r2_1: Vec<i64> = Vec::new();
                let mut c2_1: Vec<i64> = Vec::new();
                let range: Vec<i64> = XlsxWorksheet::mergeRange(&mref);
                r1_1 = vec![range[0]];
                c1_1 = vec![range[1]];
                r2_1 = vec![range[2]];
                c2_1 = vec![range[3]];
                model.borrow_mut().addMerge(r1_1[0], c1_1[0], r2_1[0], c2_1[0]);
            }
            // Conditional formatting is read where it is PRESERVED — see
            // `readConditionalFormatting`. It cannot be read here: the capture
            // above has already consumed the element.
        }
        model.borrow().history.clone().borrow_mut().reset();
        model.borrow_mut().version = model.borrow().version + 1;
    }
    pub fn noteRowStated(&mut self, row: i64) {
        while (self.rowStated.len() as i64) <= row {
            self.rowStated.push(false);
        }
        self.rowStated[row as usize] = true;
    }
    pub fn noteRowContent(&mut self, row: i64, col: i64) {
        while (self.rowMinCol.len() as i64) <= row {
            self.rowMinCol.push(-1);
            self.rowMaxCol.push(-1);
        }
        let lo: i64 = self.rowMinCol[row as usize];
        if lo < 0 || col < lo {
            self.rowMinCol[row as usize] = col;
        }
        let hi: i64 = self.rowMaxCol[row as usize];
        if col > hi {
            self.rowMaxCol[row as usize] = col;
        }
    }
    /// The largest point size among the cells that have something in them, on
    /// one row. Only the span the file actually wrote to is looked at, so a
    /// sheet with a hundred thousand empty columns costs nothing here.
    ///
    /// Uses style ids + `defaultFontPt` rather than `getCellStyle`, which would
    /// allocate a fresh default CellStyle for every unstyled cell.
    pub fn tallestIn(&self, model: &Rc<RefCell<SpreadsheetModel>>, row: i64) -> f64 {
        if row >= (self.rowMinCol.len() as i64) {
            return 0.0;
        }
        let lo: i64 = self.rowMinCol[row as usize];
        if lo < 0 {
            return 0.0;
        }
        let mut hi: i64 = self.rowMaxCol[row as usize];
        if hi >= model.borrow().colCount {
            hi = model.borrow().colCount - 1;
        }
        let mut best: f64 = 0.0;
        let style_n: i64 = model.borrow().styles.len() as i64;
        let mut c: i64 = lo;
        while c <= hi {
            let txt: String = model.borrow().getCell(row, c);
            if !txt.is_empty() {
                let mut pt: f64 = model.borrow().defaultFontPt;
                let sid: i64 = model.borrow().getCellStyleId(row, c);
                if sid >= 0 && sid < style_n {
                    let st: Rc<RefCell<CellStyle>> = model.borrow().styles[sid as usize].clone();
                    pt = st.borrow().fontSizePt;
                }
                if pt > best {
                    best = pt;
                }
            }
            c += 1;
        }
        best
    }
    /// A1 / AA17 → 0-based row/col into `refRow` / `refCol` (no heap arrays).
    pub fn parseCellRefFast(&mut self, r#ref: &str) {
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&r#ref);
        let mut col: i64 = 0;
        let mut saw_col: bool = false;
        while i < n {
            let ch: i64 = DgNative::char_at(&r#ref, i);
            if ch >= 65 && ch <= 90 {
                col = col * 26 + ((ch - 65) + 1);
                saw_col = true;
                i += 1;
            } else {
                if ch >= 97 && ch <= 122 {
                    col = col * 26 + ((ch - 97) + 1);
                    saw_col = true;
                    i += 1;
                } else {
                    break;
                }
            }
        }
        if saw_col {
            col -= 1;
        }
        let mut row: i64 = 0;
        let mut saw_row: bool = false;
        while i < n {
            let ch2: i64 = DgNative::char_at(&r#ref, i);
            if ch2 >= 48 && ch2 <= 57 {
                row = row * 10 + (ch2 - 48);
                saw_row = true;
                i += 1;
            } else {
                break;
            }
        }
        if saw_row {
            row -= 1;
        }
        self.refRow = row;
        self.refCol = col;
    }
    /// Grow every row the file did not size to fit the largest text in it. The
    /// factor is Excel's own: a row is about 1.275 times the point size of its
    /// text, which is exactly how 10pt text yields the familiar 12.75pt row.
    /// Rows only ever grow — a height the file stated is what the author chose,
    /// and clipping is then their decision, not ours.
    pub fn autoFitRows(&self, model: &Rc<RefCell<SpreadsheetModel>>) {
        let mut i: i64 = 0;
        let n: i64 = self.rowMinCol.len() as i64;
        // Unstyled sheet: every content row needs the same height. Do not call
        // `getCell` millions of times — that dominates native loads of large
        // workbooks (web profile: setCellRaw + getCell; C++ copies string keys).
        if !self.sawCellStyle {
            let uni_px: i64 = CellStyle::rowHeightForPt(model.borrow().defaultFontPt.clone());
            while i < n {
                if i >= model.borrow().rowCount {
                    break;
                }
                let mut stated_u: bool = false;
                if i < (self.rowStated.len() as i64) {
                    stated_u = self.rowStated[i as usize];
                }
                if !stated_u {
                    if self.rowMinCol[i as usize] >= 0 {
                        let have_u: i64 = model.borrow_mut().rowHeight(i);
                        if uni_px > have_u {
                            model.borrow_mut().setRowHeight(i, uni_px);
                        }
                    }
                }
                i += 1;
            }
            return;
        }
        while i < n {
            if i >= model.borrow().rowCount {
                break;
            }
            let mut stated: bool = false;
            if i < (self.rowStated.len() as i64) {
                stated = self.rowStated[i as usize];
            }
            if !stated {
                let pt: f64 = self.tallestIn(model, i);
                if pt > 0.0 {
                    let px: i64 = CellStyle::rowHeightForPt(pt);
                    let have: i64 = model.borrow_mut().rowHeight(i);
                    if px > have {
                        model.borrow_mut().setRowHeight(i, px);
                    }
                }
            }
            i += 1;
        }
    }
    /// The formula for a member of shared run `si`, sitting at (row, col).
    /// Returns "" when the run's text has not been seen — Excel writes the
    /// anchor first, but a file that does not gets its cached value rather than
    /// a guess.
    pub fn sharedAt(&self, si: &str, row: i64, col: i64) -> String {
        if !self.sharedText.contains_key(si) {
            return String::new();
        }
        let master: String = self.sharedText.get(si).cloned().unwrap();
        let ar: i64 = XlsxWorksheet::parseInt0(&self.sharedRow.get(si).cloned().unwrap());
        let ac: i64 = XlsxWorksheet::parseInt0(&self.sharedCol.get(si).cloned().unwrap());
        let dy: i64 = row - ar;
        let dx: i64 = col - ac;
        if dx == 0 && dy == 0 {
            return master;
        }
        FormulaParser::translateFormula(&master, dx, dy)
    }
    pub fn parseCell(&mut self, p: &Rc<RefCell<XmlLite>>, open: &Rc<RefCell<XmlTag>>, model: &Rc<RefCell<SpreadsheetModel>>, sst: &Rc<RefCell<XlsxSharedStrings>>, fall_back_row: i64) {
        let r#ref: String = open.borrow().attr("r");
        let mut row: i64 = fall_back_row;
        let mut col: i64 = 0;
        if DgNative::str_len(&r#ref) > 0 {
            self.parseCellRefFast(&r#ref);
            row = self.refRow;
            col = self.refCol;
        }
        let cell_type: String = open.borrow().attr("t");
        let style_attr: String = open.borrow().attr("s");
        let mut formula: String = String::new();
        let mut value: String = String::new();
        let mut inline: String = String::new();
        let mut inline_rich: Option<Rc<RefCell<RichText>>> = None;
        if open.borrow().selfClose {
            // An empty but FORMATTED cell is written self-closing, e.g.
            // <c r="B3" s="2"/>. Returning here dropped its fill and borders —
            // which is exactly how a "background colour" row is stored.
            if !style_attr.is_empty() {
                self.ensureSize(model, row, col);
                model.borrow_mut().setCellStyleId(row, col, XlsxWorksheet::parseInt0(&style_attr));
                self.sawCellStyle = true;
            }
            return;
        }
        let mut depth: i64 = 1;
        while p.borrow().pos < p.borrow().srcLen && depth > 0 {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            if t.borrow().name == "c" {
                if t.borrow().isEnd {
                    depth -= 1;
                } else {
                    if !t.borrow().selfClose {
                        depth += 1;
                    }
                }
            } else {
                if t.borrow().name == "f" && !t.borrow().isEnd {
                    let fkind: String = t.borrow().attr("t");
                    let si: String = t.borrow().attr("si");
                    let mut body: String = String::new();
                    if !t.borrow().selfClose {
                        body = p.borrow_mut().readTextUntilEnd("f");
                    }
                    if !body.is_empty() {
                        formula = body.clone();
                        if fkind == "shared" {
                            if !si.is_empty() {
                                self.sharedText.insert(si.clone(), body.clone());
                                self.sharedRow.insert(si.clone(), row.to_string());
                                self.sharedCol.insert(si.clone(), col.to_string());
                            }
                        }
                    } else {
                        // No text: either a member of a shared run, or a
                        // formula this reader cannot make sense of. The first
                        // can be reconstructed; the second keeps its cached
                        // value, as it did before.
                        if fkind == "shared" {
                            if !si.is_empty() {
                                formula = self.sharedAt(&si, row, col);
                            }
                        }
                    }
                }
                if (t.borrow().name == "v" && !t.borrow().isEnd) && !t.borrow().selfClose {
                    value = p.borrow_mut().readTextUntilEnd("v");
                }
                if (t.borrow().name == "is" && !t.borrow().isEnd) && !t.borrow().selfClose {
                    // An inline string can be written in styled pieces too, so
                    // it is read the same way a shared one is.
                    let rich: Rc<RefCell<RichText>> = XlsxSharedStrings::parseRuns(p, "is");
                    inline = rich.borrow().text();
                    inline_rich = Some(rich);
                }
            }
        }
        self.ensureSize(model, row, col);
        let mut display: String = String::new();
        if cell_type == "s" {
            let idx: i64 = XlsxWorksheet::parseInt0(&value);
            display = sst.borrow().at(idx);
            // Several styles inside one string: keep the pieces, not only what
            // they spell. Skip the rich-table probe when the SST has none.
            if sst.borrow().anyRich {
                let shared: Rc<RefCell<RichText>> = sst.borrow().richAt(idx);
                if shared.borrow().isRich() {
                    model.borrow_mut().setRichText(row, col, shared.borrow().copy());
                }
            }
        } else {
            if cell_type == "inlineStr" {
                display = inline.clone();
                if inline_rich.is_some() {
                    let ir: Rc<RefCell<RichText>> = inline_rich.clone().unwrap();
                    if ir.borrow().isRich() {
                        model.borrow_mut().setRichText(row, col, ir.clone());
                    }
                }
            } else {
                if cell_type == "b" {
                    if value == "1" || value == "true" {
                        display = "TRUE".to_string();
                    } else {
                        display = "FALSE".to_string();
                    }
                } else {
                    if cell_type == "str" {
                        display = value.clone();
                    } else {
                        display = value.clone();
                    }
                }
            }
        }
        model.borrow_mut().setCellRaw(row, col, &display);
        if !formula.is_empty() {
            model.borrow_mut().setFormula(row, col, &formula);
        }
        if !style_attr.is_empty() {
            model.borrow_mut().setCellStyleId(row, col, XlsxWorksheet::parseInt0(&style_attr));
            self.sawCellStyle = true;
        }
        // Only a cell with something in it can make a row taller; an empty one
        // that merely carries a fill should not.
        if !display.is_empty() {
            self.noteRowContent(row, col);
        }
    }
}
