// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rgr::OpcPackage;
use crate::cell_style::CellStyle;
use crate::formats::sheet_bytes::SheetBytes;
use crate::formats::xls_formula::XlsFormula;
use crate::spreadsheet_model::SpreadsheetModel;
use crate::workbook_model::WorkbookModel;
use crate::xlsx_number_format::XlsxNumberFormat;
/// XlsbLoader: the data of an Excel binary workbook (.xlsb).
///
/// An .xlsb is the .xlsx package with its parts in BIFF12 records instead of
/// XML: [type: 1-2 bytes, 7 bits each][size: 1-4 bytes, 7 bits each][data].
/// The relationships and content types are still XML, so the package is opened
/// the same way and only the parts are different.
///
/// Numbers, text, booleans, errors, formulas, number formats (so dates read as
/// dates), merges, column widths and sheet visibility. Formulas are BIFF12
/// parsed tokens, turned back into text by XlsFormula in its wide mode; one it
/// cannot read keeps its saved result, and the note on open says which kinds.
#[derive(Clone)]
pub struct XlsbLoader {
    pub lastError: String,
    pub notes: Vec<String>,
    pub data: Vec<u8>,
    pub pos: i64,
    pub recId: i64,
    pub recStart: i64,
    pub recEnd: i64,
    pub date1904: bool,
    pub sst: Vec<String>,
    pub formats: HashMap<i64,String>,
    pub styles: Vec<Rc<RefCell<CellStyle>>>,
    pub sawFormula: bool,
    pub formula: Rc<RefCell<XlsFormula>>,
    pub failures: Vec<String>,
    /// Shared formulas of the sheet being read: their ranges and tokens.
    pub shRange: Vec<i64>,
    pub shRgce: Vec<Vec<u8>>,
    pub shRgcb: Vec<Vec<u8>>,
    /// Cells that point at a shared formula, resolved when the sheet ends:
    /// the BrtShrFmla record comes after the first cell that uses it.
    pub expCells: Vec<i64>,
}

impl XlsbLoader {
    pub fn new() -> Self {
        Self {
            lastError: String::new(),
            notes: Vec::new(),
            data: vec![0u8; 0],
            pos: 0,
            recId: -1,
            recStart: 0,
            recEnd: 0,
            date1904: false,
            sst: Vec::new(),
            formats: HashMap::new(),
            styles: Vec::new(),
            sawFormula: false,
            formula: Rc::new(RefCell::new(XlsFormula::new())),
            failures: Vec::new(),
            shRange: Vec::new(),
            shRgce: Vec::new(),
            shRgcb: Vec::new(),
            expCells: Vec::new(),
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
    /// Bytes [from, from + n) of the record data, or empty when out of range.
    pub fn slice(&self, from: i64, n: i64) -> Vec<u8> {
        if n <= 0 || from < 0 || from + n > (self.data.len() as i64) {
            return vec![0u8; 0];
        }
        self.data[(from) as usize..(from + n) as usize].to_vec()
    }
    /// One formula's text from tokens at `at` (cce, rgce, cb, rgcb), or "".
    pub fn formulaAt(&mut self, at: i64, row: i64, col: i64) -> String {
        let cce: i64 = SheetBytes::i32(&self.data.clone(), at);
        if cce <= 0 || at + 4 + cce > self.recEnd {
            return String::new();
        }
        let rgce: Vec<u8> = self.slice(at + 4, cce);
        let mut rgcb: Vec<u8> = vec![0u8; 0];
        let cb_at: i64 = at + 4 + cce;
        if cb_at + 4 <= self.recEnd {
            let cb: i64 = SheetBytes::i32(&self.data.clone(), cb_at);
            if cb > 0 && cb_at + 4 + cb <= self.recEnd {
                rgcb = self.slice(cb_at + 4, cb);
            }
        }
        if (rgce[0] as i64) == 1 {
            self.expCells.push(row);
            self.expCells.push(col);
            return String::new();
        }
        let text: String = self.formula.borrow_mut().decode(&rgce, &rgcb, row, col);
        if text.is_empty() {
            let why: String = self.formula.borrow().failure.clone();
            self.failed(&why);
        }
        text
    }
    /// --- records ----------------------------------------------------------------
    pub fn next(&mut self) -> bool {
        let n: i64 = self.data.len() as i64;
        if self.pos >= n {
            return false;
        }
        let b0: i64 = self.data[(self.pos) as usize] as i64;
        self.pos += 1;
        let mut id: i64 = b0 & 127;
        if (b0 & 128) != 0 {
            let mut _tmp_1099 = self.data.clone();
            let b1: i64 = SheetBytes::u8(&_tmp_1099, self.pos.clone());
            self.pos += 1;
            id += (b1 & 127) * 128;
        }
        let mut size: i64 = 0;
        let mut mult: i64 = 1;
        let mut k: i64 = 0;
        while k < 4 {
            let mut _tmp_1101 = self.data.clone();
            let b: i64 = SheetBytes::u8(&_tmp_1101, self.pos.clone());
            self.pos += 1;
            size += (b & 127) * mult;
            mult *= 128;
            if (b & 128) == 0 {
                k = 4;
            }
            k += 1;
        }
        self.recId = id;
        self.recStart = self.pos;
        self.recEnd = self.pos + size;
        if self.recEnd > n {
            self.recEnd = n;
        }
        self.pos = self.recEnd;
        true
    }
    /// XLWideString: a u32 count of UTF-16 units, then the units.
    pub fn wideAt(&self, at: i64) -> String {
        let cch: i64 = SheetBytes::i32(&self.data.clone(), at);
        if cch < 0 || (at + 4) + cch * 2 > self.recEnd {
            return String::new();
        }
        SheetBytes::utf16(&self.data.clone(), at + 4, cch, true)
    }
    /// --- the workbook -------------------------------------------------------------
    pub fn loadBytes(&mut self, bytes: &[u8], book: &Rc<RefCell<WorkbookModel>>) -> bool {
        self.lastError = String::new();
        let pkg: Rc<RefCell<OpcPackage>> = Rc::new(RefCell::new(OpcPackage::new()));
        if !OpcPackage::open_bytes(&pkg, bytes, "book.xlsb".to_string()) {
            self.lastError = "could not open the binary workbook: not a zip".to_string();
            return false;
        }
        let mut wb_part: String = "xl/workbook.bin".to_string();
        let main: String = OpcPackage::main_document_part(&pkg);
        if !main.is_empty() {
            wb_part = main.clone();
        }
        if !OpcPackage::has_part(&pkg, &wb_part) {
            OpcPackage::close(&pkg);
            self.lastError = "the binary workbook has no workbook part".to_string();
            return false;
        }
        let mut names: Vec<String> = Vec::new();
        let mut rels: Vec<String> = Vec::new();
        let mut hidden: Vec<bool> = Vec::new();
        self.data = OpcPackage::read_binary(&pkg, &wb_part);
        self.pos = 0;
        while self.next() {
            if self.recId == 153 {
                // BrtWbProp: bit 0 is the 1904 date system.
                let mut _tmp_1109 = self.data.clone();
                self.date1904 = ((SheetBytes::u8(&_tmp_1109, self.recStart.clone())) & 1) != 0;
            }
            if self.recId == 39 {
                // BrtName: flags, chKey, itab, then the name.
                let nm: String = self.wideAt(self.recStart + 9);
                self.formula.borrow_mut().links.borrow_mut().names.push(nm.clone());
            }
            if self.recId == 357 || self.recId == 358 {
                // BrtSupSelf / BrtSupSame: this workbook.
                self.formula.borrow_mut().links.borrow_mut().bookInternal.push(true);
                self.formula.borrow_mut().links.borrow_mut().bookNames.push(Vec::new());
            }
            if self.recId == 355 || self.recId == 667 {
                // BrtSupBookSrc / BrtSupAddin: another workbook or an add-in.
                self.formula.borrow_mut().links.borrow_mut().bookInternal.push(false);
                self.formula.borrow_mut().links.borrow_mut().bookNames.push(Vec::new());
            }
            if self.recId == 362 {
                // BrtExternSheet: count, then (book, first sheet, last sheet).
                let cnt: i64 = SheetBytes::i32(&self.data.clone(), self.recStart);
                let mut xi: i64 = 0;
                while xi < cnt && self.recStart + 4 + (xi + 1) * 12 <= self.recEnd {
                    let base: i64 = self.recStart + 4 + xi * 12;
                    let bk: i64 = SheetBytes::i32(&self.data.clone(), base);
                    let f1: i64 = SheetBytes::i32(&self.data.clone(), base + 4);
                    let f2: i64 = SheetBytes::i32(&self.data.clone(), base + 8);
                    self.formula.borrow_mut().links.borrow_mut().xtiBook.push(bk);
                    self.formula.borrow_mut().links.borrow_mut().xtiFirst.push(f1);
                    self.formula.borrow_mut().links.borrow_mut().xtiLast.push(f2);
                    xi += 1;
                }
            }
            if self.recId == 156 {
                // BrtBundleSh: state, id, relationship id, name.
                let mut _tmp_1112 = self.data.clone();
                let state: i64 = SheetBytes::i32(&_tmp_1112, self.recStart.clone());
                let rel_len: i64 = SheetBytes::i32(&self.data.clone(), self.recStart + 8);
                let mut rid: String = String::new();
                let mut name_at: i64 = self.recStart + 12;
                if rel_len >= 0 {
                    rid = self.wideAt(self.recStart + 8);
                    name_at = (self.recStart + 12) + rel_len * 2;
                }
                hidden.push(state != 0);
                rels.push(rid.clone());
                names.push(self.wideAt(name_at));
            }
        }
        // Shared strings and styles are found through the workbook's
        // relationships, as in the XML form.
        let sst_part: String = OpcPackage::find_relationship_by_type(&pkg, &wb_part, "/sharedStrings");
        if !sst_part.is_empty() {
            self.readSst(OpcPackage::read_binary(&pkg, &sst_part).clone());
        }
        let style_part: String = OpcPackage::find_relationship_by_type(&pkg, &wb_part, "/styles");
        if !style_part.is_empty() {
            self.readStyles(OpcPackage::read_binary(&pkg, &style_part).clone());
        }
        if self.styles.is_empty() {
            self.styles.push(Rc::new(RefCell::new(CellStyle::new())));
        }
        book.borrow_mut().clear();
        self.formula.borrow_mut().wide = true;
        self.formula.borrow_mut().links.borrow_mut().sheets = names.clone();
        let mut i: i64 = 0;
        while i < (names.len() as i64) {
            let part: String = OpcPackage::resolve_relationship(
                &pkg,
                &wb_part,
                &rels[i as usize].clone()
            );
            if !part.is_empty() {
                if OpcPackage::has_part(&pkg, &part) {
                    let sheet: Rc<RefCell<SpreadsheetModel>> = Rc::new(RefCell::new(SpreadsheetModel::new()));
                    sheet.borrow_mut().name = names[i as usize].clone();
                    sheet.borrow_mut().sheetHidden = hidden[i as usize];
                    sheet.borrow_mut().quietWrites = true;
                    self.readSheet(OpcPackage::read_binary(&pkg, &part).clone(), &sheet);
                    sheet.borrow_mut().setStylesFrom(&self.styles.clone());
                    sheet.borrow_mut().quietWrites = false;
                    sheet.borrow_mut().version = sheet.borrow().version + 1;
                    book.borrow_mut().addSheet(sheet.clone());
                }
            }
            i += 1;
        }
        OpcPackage::close(&pkg);
        if book.borrow().sheetCount() == 0 {
            self.lastError = "the binary workbook has no worksheets that can be read".to_string();
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
        if !self.failures.is_empty() {
            self.notes.push(format!("some formulas use {}; their saved results were kept as values", self.failures.join(", ")));
        }
        true
    }
    pub fn readSst(&mut self, bytes: Vec<u8>) {
        self.data = bytes.clone();
        self.pos = 0;
        while self.next() {
            if self.recId == 19 {
                // BrtSSTItem: flags, then the text; runs and phonetics after.
                self.sst.push(self.wideAt(self.recStart + 1));
            }
        }
    }
    pub fn readStyles(&mut self, bytes: Vec<u8>) {
        self.data = bytes.clone();
        self.pos = 0;
        let mut in_xfs: bool = false;
        while self.next() {
            if self.recId == 44 {
                let mut _tmp_1119 = self.data.clone();
                let ifmt: i64 = SheetBytes::u16(&_tmp_1119, self.recStart.clone());
                self.formats.insert(ifmt, self.wideAt(self.recStart + 2));
            }
            if self.recId == 617 {
                in_xfs = true;
            }
            if self.recId == 618 {
                in_xfs = false;
            }
            if self.recId == 47 && in_xfs {
                // BrtXF: parent, number format, font, …
                let st: Rc<RefCell<CellStyle>> = Rc::new(RefCell::new(CellStyle::new()));
                let fid: i64 = SheetBytes::u16(&self.data.clone(), self.recStart + 2);
                if self.formats.contains_key(&fid) {
                    st.borrow_mut().numFmt = self.formats.get(&fid).cloned().unwrap();
                } else {
                    st.borrow_mut().numFmt = SheetBytes::builtinNumFmt(fid);
                }
                self.styles.push(st.clone());
            }
        }
    }
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
    pub fn readSheet(&mut self, bytes: Vec<u8>, sheet: &Rc<RefCell<SpreadsheetModel>>) {
        self.data = bytes.clone();
        self.pos = 0;
        self.shRange = Vec::new();
        self.shRgce = Vec::new();
        self.shRgcb = Vec::new();
        self.expCells = Vec::new();
        let mut formulas: Vec<i64> = Vec::new();
        let mut formula_text: Vec<String> = Vec::new();
        let mut row: i64 = 0;
        while self.next() {
            let id: i64 = self.recId;
            let at: i64 = self.recStart;
            if id == 0 {
                row = SheetBytes::i32(&self.data, at);
                continue;
            }
            if (id >= 1 && id <= 11) || id == 62 {
                let col: i64 = SheetBytes::i32(&self.data.clone(), at);
                let xf: i64 = (SheetBytes::i32(&self.data.clone(), at + 4)) & 16777215;
                let v: i64 = at + 8;
                let mut text: String = String::new();
                if id == 2 {
                    text = self.numAt(xf, SheetBytes::rk(&self.data, v));
                }
                if id == 3 || id == 11 {
                    text = XlsFormula::errorText(SheetBytes::u8(&self.data, v));
                }
                if id == 4 || id == 10 {
                    if SheetBytes::u8(&self.data.clone(), v) != 0 {
                        text = "TRUE".to_string();
                    } else {
                        text = "FALSE".to_string();
                    }
                }
                if id == 5 || id == 9 {
                    text = self.numAt(xf, SheetBytes::f64(&self.data, v));
                }
                if id == 6 || id == 8 {
                    text = self.wideAt(v);
                }
                if id == 62 {
                    text = self.wideAt(v + 1);
                }
                if id == 7 {
                    let si: i64 = SheetBytes::i32(&self.data.clone(), v);
                    if si >= 0 && si < (self.sst.len() as i64) {
                        text = self.sst[si as usize].clone();
                    }
                }
                let mut ftext: String = String::new();
                if id >= 8 && id <= 11 {
                    self.sawFormula = true;
                    // The value, then grbitFlags (2 bytes), then the tokens.
                    let mut after: i64 = v + 1;
                    if id == 8 {
                        after = (v + 4) + SheetBytes::i32(&self.data.clone(), v) * 2;
                    }
                    if id == 9 {
                        after = v + 8;
                    }
                    ftext = self.formulaAt(after + 2, row, col);
                }
                if ((row >= 0 && row < 1048576) && col >= 0) && col < 16384 {
                    if !text.is_empty() || xf > 0 {
                        self.ensure(sheet, row, col);
                    }
                    if !text.is_empty() {
                        sheet.borrow_mut().setCellRaw(row, col, &text);
                    }
                    if !ftext.is_empty() {
                        formulas.push(row);
                        formulas.push(col);
                        formula_text.push(ftext.clone());
                    }
                    if xf > 0 && xf < (self.styles.len() as i64) {
                        sheet.borrow_mut().setCellStyleId(row, col, xf);
                    }
                }
                continue;
            }
            if id == 425 {
                // BrtShrFmla: the range it covers, then the shared tokens.
                let sr0: i64 = SheetBytes::i32(&self.data.clone(), at);
                let sr1: i64 = SheetBytes::i32(&self.data.clone(), at + 4);
                let sc0: i64 = SheetBytes::i32(&self.data.clone(), at + 8);
                let sc1: i64 = SheetBytes::i32(&self.data.clone(), at + 12);
                let scce: i64 = SheetBytes::i32(&self.data.clone(), at + 16);
                if scce > 0 && at + 20 + scce <= self.recEnd {
                    self.shRange.push(sr0);
                    self.shRange.push(sr1);
                    self.shRange.push(sc0);
                    self.shRange.push(sc1);
                    self.shRgce.push(self.slice(at + 20, scce));
                    let mut srgcb: Vec<u8> = vec![0u8; 0];
                    let scb_at: i64 = at + 20 + scce;
                    if scb_at + 4 <= self.recEnd {
                        let scb: i64 = SheetBytes::i32(&self.data.clone(), scb_at);
                        if scb > 0 && scb_at + 4 + scb <= self.recEnd {
                            srgcb = self.slice(scb_at + 4, scb);
                        }
                    }
                    self.shRgcb.push(srgcb);
                }
                continue;
            }
            if id == 176 {
                // BrtMergeCell: first row, last row, first column, last column.
                let r0: i64 = SheetBytes::i32(&self.data.clone(), at);
                let r1: i64 = SheetBytes::i32(&self.data.clone(), at + 4);
                let c0: i64 = SheetBytes::i32(&self.data.clone(), at + 8);
                let c1: i64 = SheetBytes::i32(&self.data.clone(), at + 12);
                if (((r0 >= 0 && r1 >= r0) && c0 >= 0) && c1 >= c0) && c1 < 16384 {
                    self.ensure(sheet, r1, c1);
                    sheet.borrow_mut().addMerge(r0, c0, r1, c1);
                }
                continue;
            }
            if id == 60 {
                // BrtColInfo: first, last, width in 1/256 characters, style,
                // flags (bit 0 hidden).
                let cf: i64 = SheetBytes::i32(&self.data.clone(), at);
                let mut cl: i64 = SheetBytes::i32(&self.data.clone(), at + 4);
                let w: i64 = SheetBytes::i32(&self.data.clone(), at + 8);
                let flags: i64 = SheetBytes::u16(&self.data.clone(), at + 16);
                if cl > 1023 {
                    cl = 1023;
                }
                let mut px: i64 = (((w as f64) / 256.0) * 7.0 + 0.5).floor() as i64;
                if px < 16 {
                    px = 16;
                }
                let mut c: i64 = cf;
                while c >= 0 && c <= cl {
                    self.ensure(sheet, 0, c);
                    sheet.borrow_mut().setColWidth(c, px);
                    if (flags & 1) != 0 {
                        sheet.borrow_mut().setColHidden(c, true);
                    }
                    c += 1;
                }
                continue;
            }
        }
        // Cells that use a shared formula, now that every BrtShrFmla is read.
        let mut ei: i64 = 0;
        while ei + 1 < (self.expCells.len() as i64) {
            let er: i64 = self.expCells[ei as usize];
            let ec: i64 = self.expCells[(ei + 1) as usize];
            let mut si: i64 = 0;
            let mut found: bool = false;
            while si < (self.shRgce.len() as i64) && !found {
                let b: i64 = si * 4;
                if er >= self.shRange[b as usize] && er <= self.shRange[(b + 1) as usize] && ec >= self.shRange[(b + 2) as usize] && ec <= self.shRange[(b + 3) as usize] {
                    found = true;
                    let rg: Vec<u8> = self.shRgce[si as usize].clone();
                    let rb: Vec<u8> = self.shRgcb[si as usize].clone();
                    let st: String = self.formula.borrow_mut().decode(&rg, &rb, er, ec);
                    if st.is_empty() {
                        let why: String = self.formula.borrow().failure.clone();
                        self.failed(&why);
                    } else {
                        formulas.push(er);
                        formulas.push(ec);
                        formula_text.push(st.clone());
                    }
                }
                si += 1;
            }
            if !found {
                self.failed("a shared formula that is missing");
            }
            ei += 2;
        }
        let mut fi: i64 = 0;
        while fi < (formula_text.len() as i64) {
            let fr: i64 = formulas[(fi * 2) as usize];
            let fc: i64 = formulas[(fi * 2 + 1) as usize];
            sheet.borrow_mut().setFormula(fr, fc, &formula_text[fi as usize].clone());
            fi += 1;
        }
    }
}
