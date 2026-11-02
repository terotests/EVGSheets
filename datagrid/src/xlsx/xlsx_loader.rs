// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::{DgNative, OpcPackage};
use crate::formula_engine::FormulaEngine;
use crate::script::grid_script::GridScript;
use crate::sheet_image::SheetImage;
use crate::sheet_table::SheetTable;
use crate::spreadsheet_model::SpreadsheetModel;
use crate::workbook_model::WorkbookModel;
use crate::xlsx::xlsx_drawing::XlsxDrawing;
use crate::xlsx::xlsx_package::XlsxPackage;
use crate::xlsx::xlsx_shared_strings::XlsxSharedStrings;
use crate::xlsx::xlsx_styles::XlsxStyles;
use crate::xlsx::xlsx_table::XlsxTable;
use crate::xlsx::xlsx_workbook::{XlsxSheetInfo, XlsxWorkbook};
use crate::xlsx::xlsx_worksheet::XlsxWorksheet;
use crate::xlsx::xml_lite::XmlLite;
/// XlsxLoader - book.xlsx → WorkbookModel (sheets → SpreadsheetModel).
#[derive(Clone)]
pub struct XlsxLoader {
    pub lastError: String,
    pub engine: Rc<RefCell<FormulaEngine>>,
}

impl XlsxLoader {
    pub fn new() -> Self {
        Self {
            lastError: String::new(),
            engine: Rc::new(RefCell::new(FormulaEngine::new())),
        }
    }
    /// Where a part's relationships live, and what a relationship target means
    /// — both `OpcPackage`'s, so the spreadsheet resolves paths the same way the
    /// .docx and .pptx readers do. This file used to carry its own pair, and the
    /// second said out loud that it handled "only the two forms a spreadsheet
    /// actually writes"; a chart's own relationships write a third.
    pub fn relsPathFor(part_path: &str) -> String {
        OpcPackage::rels_path_for(part_path)
    }
    pub fn resolveRelPath(part_path: &str, target: &str) -> String {
        if target.is_empty() {
            return String::new();
        }
        OpcPackage::join_part_path(part_path, target)
    }
    /// The text of the first <tag>…</tag> in a small part, entities decoded.
    pub fn tagText(xml: &str, tag: &str) -> String {
        let open: i64 = DgNative::index_of(&xml, &format!("<{}>", tag));
        if open < 0 {
            return String::new();
        }
        let from: i64 = (open + DgNative::str_len(&tag)) + 2;
        let close: i64 = DgNative::index_of(&xml, &format!("</{}>", tag));
        if close < from {
            return String::new();
        }
        XmlLite::decodeEntities(&DgNative::substr(&xml, from, close))
    }
    pub fn loadPath(&mut self, full_path: &str, book: &Rc<RefCell<WorkbookModel>>) -> bool {
        self.loadPathPassword(full_path, book, "")
    }
    pub fn loadPathPassword(&mut self, full_path: &str, book: &Rc<RefCell<WorkbookModel>>, password: &str) -> bool {
        self.lastError = String::new();
        let pkg: Rc<RefCell<XlsxPackage>> = Rc::new(RefCell::new(XlsxPackage::new()));
        if !pkg.borrow_mut().openPathPassword(full_path, password) {
            self.lastError = pkg.borrow().lastError.clone();
            if self.lastError.is_empty() {
                self.lastError = format!("could not open zip: {}", full_path);
            }
            return false;
        }
        let ok: bool = self.loadPackage(&pkg, book);
        pkg.borrow_mut().close();
        ok
    }
    /// The same, from bytes. `loadPath` is this with a file read in front of it;
    /// the browser build has no file read and calls straight in here.
    pub fn loadBytes(&mut self, bytes: &[u8], book: &Rc<RefCell<WorkbookModel>>) -> bool {
        self.loadBytesPassword(bytes, book, "")
    }
    pub fn loadBytesPassword(&mut self, bytes: &[u8], book: &Rc<RefCell<WorkbookModel>>, password: &str) -> bool {
        self.lastError = String::new();
        let pkg: Rc<RefCell<XlsxPackage>> = Rc::new(RefCell::new(XlsxPackage::new()));
        if !pkg.borrow_mut().openBytesPassword(bytes, password) {
            self.lastError = pkg.borrow().lastError.clone();
            if self.lastError.is_empty() {
                self.lastError = "could not open the workbook: not a zip".to_string();
            }
            return false;
        }
        let ok: bool = self.loadPackage(&pkg, book);
        pkg.borrow_mut().close();
        ok
    }
    pub fn loadPackage(&mut self, pkg: &Rc<RefCell<XlsxPackage>>, book: &Rc<RefCell<WorkbookModel>>) -> bool {
        book.borrow_mut().clear();
        // Kept so a later save can preserve the parts nothing here models —
        // see `WorkbookModel.sourceBytes` and `XlsxWriter.writeOver`.
        book.borrow_mut().sourceBytes = pkg.borrow().plainBytes.clone();
        let wb: Rc<RefCell<XlsxWorkbook>> = Rc::new(RefCell::new(XlsxWorkbook::new()));
        wb.borrow_mut().load(pkg);
        let mut ni: i64 = 0;
        while ni < (wb.borrow().nameKeys.len() as i64) {
            book.borrow_mut().setName(&wb.borrow().nameKeys[ni as usize].clone(), &wb.borrow().nameRefs[ni as usize].clone());
            ni += 1;
        }
        if wb.borrow().sheetCount() == 0 {
            self.lastError = "workbook has no sheets".to_string();
            return false;
        }
        if pkg.borrow().hasEntry("docProps/core.xml") {
            let core: String = pkg.borrow().entryText("docProps/core.xml");
            book.borrow_mut().propTitle = XlsxLoader::tagText(&core, "dc:title");
            book.borrow_mut().propSubject = XlsxLoader::tagText(&core, "dc:subject");
            book.borrow_mut().propAuthor = XlsxLoader::tagText(&core, "dc:creator");
            book.borrow_mut().propKeywords = XlsxLoader::tagText(&core, "cp:keywords");
        }
        let sst: Rc<RefCell<XlsxSharedStrings>> = Rc::new(RefCell::new(XlsxSharedStrings::new()));
        if pkg.borrow().hasEntry("xl/sharedStrings.xml") {
            sst.borrow_mut().parse(&pkg.borrow().entryText("xl/sharedStrings.xml"));
        }
        let sty: Rc<RefCell<XlsxStyles>> = Rc::new(RefCell::new(XlsxStyles::new()));
        // The theme first: a `<color theme="4"/>` in the styles has nothing to
        // resolve to without it, and that is most of the colour in a workbook
        // Excel styled from its own palette.
        let theme_part: String = pkg.borrow().findRelByType(&pkg.borrow().workbookPart(), "/theme");
        if !theme_part.is_empty() {
            sty.borrow().parseTheme(&pkg.borrow().entryText(&theme_part));
        }
        if pkg.borrow().hasEntry("xl/styles.xml") {
            sty.borrow_mut().parse(&pkg.borrow().entryText("xl/styles.xml"));
        } else {
            sty.borrow_mut().parse("");
        }
        let parser: Rc<RefCell<XlsxWorksheet>> = Rc::new(RefCell::new(XlsxWorksheet::new()));
        let mut first_visible: i64 = -1;
        let mut i: i64 = 0;
        let n: i64 = wb.borrow().sheetCount();
        while i < n {
            let info: Rc<RefCell<XlsxSheetInfo>> = wb.borrow().sheetAt(i);
            let sheet: Rc<RefCell<SpreadsheetModel>> = Rc::new(RefCell::new(SpreadsheetModel::new()));
            sheet.borrow_mut().name = info.borrow().name.clone();
            sheet.borrow_mut().sheetHidden = info.borrow().hidden;
            // Where it came from, so a save-over can put it back there.
            sheet.borrow_mut().partPath = info.borrow().path.clone();
            sheet.borrow_mut().quietWrites = true;
            let xml: String = pkg.borrow().entryText(&info.borrow().path.clone());
            if xml.is_empty() {
                self.lastError = format!("missing sheet part: {}", info.borrow().path);
                return false;
            }
            // A hyperlink's target and a note's text live OUTSIDE the sheet
            // part: one behind a relationship id, the other in a comments part
            // the relationships point at. Both are read here and handed to the
            // parser, so the parser still only walks XML it was given.
            parser.borrow_mut().relsXml = pkg.borrow().entryText(&XlsxLoader::relsPathFor(&info.borrow().path.clone()));
            parser.borrow_mut().commentsXml = String::new();
            let comments_rel: String = XlsxWorksheet::relTargetOfType(&parser.borrow().relsXml.clone(), "/comments");
            if !comments_rel.is_empty() {
                parser.borrow_mut().commentsXml = pkg.borrow().entryText(&XlsxLoader::resolveRelPath(&info.borrow().path.clone(), &comments_rel));
            }
            parser.borrow_mut().parse(&xml, &sheet, &sst);
            parser.borrow().parseComments(&sheet, &parser.borrow().commentsXml.clone());
            // Pictures are three hops away: the sheet points at a drawing, the
            // drawing at a relationship, the relationship at the media file.
            let drawing_rel: String = XlsxWorksheet::relTargetOfType(&parser.borrow().relsXml.clone(), "/drawing");
            if !drawing_rel.is_empty() {
                let drawing_path: String = XlsxLoader::resolveRelPath(&info.borrow().path.clone(), &drawing_rel);
                let drawing_xml: String = pkg.borrow().entryText(&drawing_path);
                if !drawing_xml.is_empty() {
                    let drawing: Rc<RefCell<XlsxDrawing>> = Rc::new(RefCell::new(XlsxDrawing::new()));
                    // The drawing has to know its own path: its relationship
                    // targets are written relative to it, not to the sheet.
                    drawing.borrow_mut().partPath = drawing_path.clone();
                    drawing.borrow_mut().parse(&drawing_xml, &pkg.borrow().entryText(&XlsxLoader::relsPathFor(&drawing_path)));
                    let mut di: i64 = 0;
                    while di < (drawing.borrow().images.len() as i64) {
                        let pic: Rc<RefCell<SheetImage>> = drawing.borrow().images[di as usize].clone();
                        // The bytes have to be taken now: the package is closed
                        // as soon as loading finishes, and a picture decoded on
                        // first paint would have nothing left to read.
                        pic.borrow_mut().bytes = pkg.borrow().entryBytes(&pic.borrow().src.clone());
                        sheet.borrow_mut().addImage(pic.clone());
                        di += 1;
                    }
                    let mut ci: i64 = 0;
                    while ci < (drawing.borrow().charts.len() as i64) {
                        let ch: Rc<RefCell<SheetImage>> = drawing.borrow().charts[ci as usize].clone();
                        let chart_xml: String = pkg.borrow().entryText(&ch.borrow().chartPart.clone());
                        if !chart_xml.is_empty() {
                            XlsxDrawing::readChart(&ch, &chart_xml);
                            sheet.borrow_mut().fileCharts.push(ch.clone());
                        }
                        ci += 1;
                    }
                }
            }
            // Tables. A sheet can carry several, each behind a relationship of
            // its own, and their names are what formulas like
            // `Table1[[#This Row],[CategoryID]]` are written against — so a
            // workbook whose tables were not read is a workbook where those
            // formulas can only sit there as text.
            let table_targets: Vec<String> = XlsxTable::tableTargets(&parser.borrow().relsXml);
            let mut tt: i64 = 0;
            while tt < (table_targets.len() as i64) {
                let table_path: String = XlsxLoader::resolveRelPath(&info.borrow().path.clone(), &table_targets[tt as usize].clone());
                let table_xml: String = pkg.borrow().entryText(&table_path);
                if !table_xml.is_empty() {
                    let tbl: Rc<RefCell<SheetTable>> = XlsxTable::parse(&table_xml);
                    if !tbl.borrow().name.is_empty() {
                        XlsxTable::fillColumnsFromSheet(&tbl, &sheet);
                        sheet.borrow_mut().addTable(tbl.clone());
                    }
                }
                tt += 1;
            }
            sheet.borrow_mut().setStylesFrom(&sty.borrow().styles.clone());
            // Only now can a row be measured: until the style table is on the
            // sheet, every cell reports the default size rather than its own.
            parser.borrow().autoFitRows(&sheet);
            if parser.borrow().fileZoom != 1.0 {
                sheet.borrow_mut().setZoom(parser.borrow().fileZoom.clone());
            }
            sheet.borrow_mut().quietWrites = false;
            sheet.borrow_mut().version = sheet.borrow().version + 1;
            book.borrow_mut().addSheet(sheet.clone());
            if !info.borrow().hidden {
                if first_visible < 0 {
                    first_visible = i;
                }
            }
            i += 1;
        }
        if first_visible < 0 {
            first_visible = 0;
        }
        book.borrow_mut().setActive(first_visible);
        self.loadScripts(pkg, book);
        self.engine.borrow_mut().attach(book.clone());
        self.engine.borrow_mut().rebuildAll();
        true
    }
    /// Scripts ride along in the package under `ranger/`. Nothing here parses
    /// them — the part is read as text and handed to the workbook, so a build
    /// that never opens the script engine still round-trips the script.
    pub fn loadScripts(&self, pkg: &Rc<RefCell<XlsxPackage>>, book: &Rc<RefCell<WorkbookModel>>) {
        let names: Vec<String> = pkg.borrow().listFiles();
        let mut i: i64 = 0;
        let n: i64 = names.len() as i64;
        while i < n {
            let path: String = names[i as usize].clone();
            if GridScript::isScriptPart(&path) {
                let src: String = pkg.borrow().entryText(&path);
                let sc: Rc<RefCell<GridScript>> = Rc::new(RefCell::new(GridScript::new()));
                sc.borrow_mut().name = GridScript::nameOfPart(&path);
                sc.borrow_mut().source = src.clone();
                sc.borrow_mut().part = path.clone();
                book.borrow_mut().addScript(sc.clone());
            }
            i += 1;
        }
    }
}
