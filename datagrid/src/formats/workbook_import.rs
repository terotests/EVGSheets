// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::{DgNative, OpcPackage};
use crate::formats::cfb::Cfb;
use crate::formats::ods_loader::OdsLoader;
use crate::formats::sheet_bytes::SheetBytes;
use crate::formats::text_workbook::TextWorkbook;
use crate::formats::xls_loader::XlsLoader;
use crate::formats::xlsb_loader::XlsbLoader;
use crate::formula_engine::FormulaEngine;
use crate::workbook_model::WorkbookModel;
/// WorkbookImport: which kind of workbook a file is, and the readers for the
/// kinds that are not .xlsx.
///
/// The contents decide, not the name: a .xls renamed .xlsx is still a compound
/// file, and a .xlsm is the same package as a .xlsx. The name only decides
/// between the text formats, which have no signature, and is the fallback when
/// bytes say nothing.
///
///   xlsx       a SpreadsheetML package (.xlsx .xlsm .xltx .xltm) — XlsxLoader
///   encrypted  a password-protected OOXML package — XlsxLoader decrypts it
///   xls        Excel 97-2003 (.xls .xlt) — XlsLoader
///   xlsb       Excel binary workbook — XlsbLoader (values only)
///   ods        OpenDocument spreadsheet — OdsLoader
///   csv, tsv   text — TextWorkbook
///
/// An import is a new workbook: it has no package to write back over, so Save
/// writes .xlsx (or .ods, which OdsWriter can write) — see `saveNameFor`.
#[derive(Clone)]
pub struct WorkbookImport {
    pub lastError: String,
    pub notes: Vec<String>,
    pub engine: Rc<RefCell<FormulaEngine>>,
}

impl WorkbookImport {
    pub fn new() -> Self {
        Self {
            lastError: String::new(),
            notes: Vec::new(),
            engine: Rc::new(RefCell::new(FormulaEngine::new())),
        }
    }
    pub fn isZip(b: &[u8]) -> bool {
        if (b.len() as i64) < 4 {
            return false;
        }
        (((b[0] as i64) == 80 && (b[1] as i64) == 75) && (b[2] as i64) == 3) && (b[3] as i64) == 4
    }
    pub fn textKindOf(name: &str) -> String {
        let ext: String = SheetBytes::extOf(name);
        if ext == "tsv" || ext == "tab" {
            return "tsv".to_string();
        }
        if (ext == "csv" || ext == "txt") || ext == "prn" {
            return "csv".to_string();
        }
        String::new()
    }
    pub fn kindOf(bytes: &[u8], name: &str) -> String {
        if WorkbookImport::isZip(bytes) {
            let pkg: Rc<RefCell<OpcPackage>> = Rc::new(RefCell::new(OpcPackage::new()));
            if OpcPackage::open_bytes(&pkg, bytes, name.to_string()) {
                let mut kind: &str = "zip";
                if OpcPackage::has_part(&pkg, "xl/workbook.xml") {
                    kind = "xlsx";
                } else {
                    if OpcPackage::has_part(&pkg, "xl/workbook.bin") {
                        kind = "xlsb";
                    } else {
                        if OpcPackage::has_part(&pkg, "content.xml") {
                            let mt: String = OpcPackage::read_xml(&pkg, "mimetype");
                            if DgNative::index_of(&mt, "opendocument.spreadsheet") >= 0 || mt.is_empty() {
                                kind = "ods";
                            }
                        }
                    }
                }
                OpcPackage::close(&pkg);
                if kind != "zip" {
                    return kind.to_string();
                }
            }
            // A ZIP that is none of these: let the .xlsx reader say what is
            // wrong with it.
            return "xlsx".to_string();
        }
        if Cfb::isCompound(bytes) {
            let cf: Rc<RefCell<Cfb>> = Rc::new(RefCell::new(Cfb::new()));
            if cf.borrow_mut().open(bytes.to_vec()) {
                if cf.borrow().hasStream("EncryptionInfo") || cf.borrow().hasStream("EncryptedPackage") {
                    return "encrypted".to_string();
                }
                if cf.borrow().hasStream("Workbook") || cf.borrow().hasStream("Book") {
                    return "xls".to_string();
                }
            }
            return "encrypted".to_string();
        }
        let text: String = WorkbookImport::textKindOf(name);
        if !text.is_empty() {
            return text;
        }
        let ext: String = SheetBytes::extOf(name);
        if ((ext == "xlsx" || ext == "xlsm") || ext == "xltx") || ext == "xltm" {
            return "xlsx".to_string();
        }
        if WorkbookImport::looksLikeText(bytes) {
            return "csv".to_string();
        }
        "unknown".to_string()
    }
    /// No NUL bytes in the first few kilobytes, and something in it.
    pub fn looksLikeText(b: &[u8]) -> bool {
        let mut n: i64 = b.len() as i64;
        if n == 0 {
            return false;
        }
        if n > 4096 {
            n = 4096;
        }
        let mut i: i64 = 0;
        while i < n {
            if (b[i as usize] as i64) == 0 {
                return false;
            }
            i += 1;
        }
        true
    }
    /// Is this a kind `load` reads (the rest go to XlsxLoader)?
    pub fn isImportKind(kind: &str) -> bool {
        (((kind == "xls" || kind == "xlsb") || kind == "ods") || kind == "csv") || kind == "tsv"
    }
    /// What a Save should be called after opening `source`: the formats this
    /// editor can write keep their name, the rest become .xlsx beside it.
    pub fn saveNameFor(source: &str) -> String {
        let ext: String = SheetBytes::extOf(source);
        if (((ext == "xlsx" || ext == "xlsm") || ext == "xltx") || ext == "xltm") || ext == "ods" {
            return source.to_string();
        }
        if source.is_empty() {
            return source.to_string();
        }
        SheetBytes::withExt(source, "xlsx")
    }
    pub fn load(&mut self, bytes: &[u8], name: &str, book: &Rc<RefCell<WorkbookModel>>) -> bool {
        self.lastError = String::new();
        self.notes.clear();
        let kind: String = WorkbookImport::kindOf(bytes, name);
        let mut ok: bool = false;
        if kind == "xls" {
            let xl: Rc<RefCell<XlsLoader>> = Rc::new(RefCell::new(XlsLoader::new()));
            ok = xl.borrow_mut().loadBytes(bytes, book);
            self.lastError = xl.borrow().lastError.clone();
            self.takeNotes(&xl.borrow().notes.clone());
            if ok {
                self.note("opened an Excel 97-2003 workbook; charts, pictures, notes and conditional formats are not imported; Save writes .xlsx");
            }
        }
        if kind == "ods" {
            let od: Rc<RefCell<OdsLoader>> = Rc::new(RefCell::new(OdsLoader::new()));
            ok = od.borrow_mut().loadBytes(bytes, book);
            self.lastError = od.borrow().lastError.clone();
            self.takeNotes(&od.borrow().notes.clone());
            if ok {
                self.note("opened an OpenDocument spreadsheet; charts, pictures, notes and conditional formats are not imported");
            }
        }
        if kind == "xlsb" {
            let xb: Rc<RefCell<XlsbLoader>> = Rc::new(RefCell::new(XlsbLoader::new()));
            ok = xb.borrow_mut().loadBytes(bytes, book);
            self.lastError = xb.borrow().lastError.clone();
            self.takeNotes(&xb.borrow().notes.clone());
            if ok {
                self.note("opened an Excel binary workbook: values and number formats only; Save writes .xlsx");
            }
        }
        if kind == "csv" || kind == "tsv" {
            let tw: Rc<RefCell<TextWorkbook>> = Rc::new(RefCell::new(TextWorkbook::new()));
            ok = tw.borrow_mut().loadBytes(bytes, name, &kind, book);
            self.lastError = tw.borrow().lastError.clone();
        }
        if (kind == "unknown" || kind == "xlsx") || kind == "encrypted" {
            self.lastError = "not a workbook this reader opens".to_string();
            return false;
        }
        if ok {
            book.borrow_mut().sourceBytes = vec![0u8; 0];
            self.engine.borrow_mut().attach(book.clone());
            self.engine.borrow_mut().rebuildAll();
        }
        ok
    }
    pub fn note(&mut self, msg: &str) {
        self.notes.push(msg.to_string());
    }
    pub fn takeNotes(&mut self, list: &[String]) {
        let mut i: i64 = 0;
        while i < (list.len() as i64) {
            self.notes.push(list[i as usize].clone());
            i += 1;
        }
    }
}
