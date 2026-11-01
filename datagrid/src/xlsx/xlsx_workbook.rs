// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rgr::{DgNative, OpcPackage, OpcRelationship};
use crate::xlsx::xlsx_package::XlsxPackage;
use crate::xlsx::xml_lite::{XmlLite, XmlTag};

/// XlsxWorkbook - workbook.xml + workbook.xml.rels → sheet name / part paths.
#[derive(Clone)]
pub struct XlsxSheetInfo {
    pub name: String,
    pub rid: String,
    pub path: String,
    pub hidden: bool,
}

impl XlsxSheetInfo {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            rid: String::new(),
            path: String::new(),
            hidden: false,
        }
    }
}

#[derive(Clone)]
pub struct XlsxWorkbook {
    pub sheets: Vec<Rc<RefCell<XlsxSheetInfo>>>,
    pub rels: HashMap<String,String>,
    /// The workbook part itself — what sheet targets are resolved against. Read
    /// from the package when it opens rather than assumed.
    pub mainPart: String,
    /// Workbook-wide defined names (not Excel's own _xlnm.*), as name / ref.
    pub nameKeys: Vec<String>,
    pub nameRefs: Vec<String>,
}

impl XlsxWorkbook {
    pub fn new() -> Self {
        Self {
            sheets: Vec::new(),
            rels: HashMap::new(),
            mainPart: "xl/workbook.xml".to_string(),
            nameKeys: Vec::new(),
            nameRefs: Vec::new(),
        }
    }
    /// The workbook's own relationships: rId → target, exactly as written. The
    /// scan is `OpcPackage`'s, the same one the .docx and .pptx readers use.
    pub fn parseRels(&mut self, xml: &str) {
        let empty: HashMap<String,String> = HashMap::new();
        self.rels = empty.clone();
        if xml.is_empty() {
            return;
        }
        let list: Vec<Rc<RefCell<OpcRelationship>>> = OpcPackage::parse_relationships(xml);
        let mut i: i64 = 0;
        while i < (list.len() as i64) {
            let rel: Rc<RefCell<OpcRelationship>> = list[i as usize].clone();
            if !rel.borrow().target.is_empty() {
                if !OpcRelationship::is_external(&rel) {
                    self.rels.insert(rel.borrow().id.clone(), rel.borrow().target.clone());
                }
            }
            i += 1;
        }
    }
    /// A sheet's target is written relative to the WORKBOOK part that names it,
    /// so that is what it is resolved against — rather than against a literal
    /// "xl/" this class used to prepend, which is the same answer only as long
    /// as the workbook is where everyone expects it to be.
    pub fn resolvePath(&self, target: &str) -> String {
        OpcPackage::join_part_path(&self.mainPart.clone(), target)
    }
    pub fn parseWorkbook(&mut self, xml: &str) {
        self.sheets.clear();
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
            if (t.borrow().name == "definedName" && !t.borrow().isEnd) && !t.borrow().selfClose {
                let dn: String = t.borrow().attr("name");
                let local: bool = t.borrow().has("localSheetId");
                let body: String = p.borrow_mut().readTextUntilEnd("definedName");
                if (DgNative::index_of(&dn, "_xlnm.") != 0 && !local) && !dn.is_empty() {
                    self.nameKeys.push(dn.clone());
                    self.nameRefs.push(body.clone());
                }
                continue;
            }
            if t.borrow().name == "sheet" && !t.borrow().isEnd {
                let info: Rc<RefCell<XlsxSheetInfo>> = Rc::new(RefCell::new(XlsxSheetInfo::new()));
                info.borrow_mut().name = t.borrow().attr("name");
                info.borrow_mut().rid = t.borrow().attrAny("r:id", "id");
                let st: String = t.borrow().attr("state");
                if st == "hidden" {
                    info.borrow_mut().hidden = true;
                }
                if st == "veryHidden" {
                    info.borrow_mut().hidden = true;
                }
                if self.rels.contains_key(&info.borrow().rid) {
                    let target: String = self.rels.get(&info.borrow().rid).cloned().unwrap();
                    info.borrow_mut().path = self.resolvePath(&target);
                }
                self.sheets.push(info.clone());
            }
        }
    }
    pub fn load(&mut self, pkg: &Rc<RefCell<XlsxPackage>>) {
        self.mainPart = pkg.borrow().workbookPart();
        self.parseRels(&pkg.borrow().entryText(&OpcPackage::rels_path_for(&self.mainPart)).clone());
        self.parseWorkbook(&pkg.borrow().entryText(&self.mainPart).clone());
    }
    pub fn sheetCount(&self) -> i64 {
        self.sheets.len() as i64
    }
    pub fn sheetAt(&self, i: i64) -> Rc<RefCell<XlsxSheetInfo>> {
        self.sheets[i as usize].clone()
    }
}
