// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;
use crate::rich_text::{CellRun, RichText};
use crate::xlsx::xml_lite::{XmlLite, XmlTag};

/// XlsxSharedStrings - xl/sharedStrings.xml → string table.
pub struct XlsxSharedStrings {
    pub strings: Vec<String>,
    /// The same entries, in pieces. A shared string is either one plain run or
    /// several styled ones, and the styled case is the whole point of rich text —
    /// keeping only the concatenation throws away what the file said.
    pub rich: Vec<Rc<RefCell<RichText>>>,
    /// True when any shared entry has more than one styled run. Load paths that
    /// only need the plain string skip `richAt` when this is false.
    pub anyRich: bool,
}

impl XlsxSharedStrings {
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
            rich: Vec::new(),
            anyRich: false,
        }
    }
    /// The runs of one <si> or <is>, up to its closing tag. A bare <t> is one
    /// unstyled run; an <r> carries an optional <rPr> saying how its piece
    /// differs from the cell.
    pub fn parseRuns(p: &Rc<RefCell<XmlLite>>, end_name: &str) -> Rc<RefCell<RichText>> {
        let out: Rc<RefCell<RichText>> = Rc::new(RefCell::new(RichText::new()));
        let mut depth: i64 = 1;
        let mut cur: Rc<RefCell<CellRun>> = Rc::new(RefCell::new(CellRun::new()));
        let mut in_run: bool = false;
        while p.borrow().pos < p.borrow().srcLen && depth > 0 {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            if t.borrow().name == end_name {
                if t.borrow().isEnd {
                    depth -= 1;
                } else {
                    if !t.borrow().selfClose {
                        depth += 1;
                    }
                }
            } else {
                if t.borrow().name == "r" {
                    if t.borrow().isEnd {
                        in_run = false;
                    } else {
                        cur = Rc::new(RefCell::new(CellRun::new()));
                        in_run = true;
                    }
                }
                // The element being THERE is what states the property; its
                // `val` says which way. `<b/>` and `<b val="1"/>` state bold,
                // `<b val="0"/>` states not-bold, and no `<b>` states nothing.
                if t.borrow().name == "b" && !t.borrow().isEnd {
                    cur.borrow_mut().setBold(XlsxSharedStrings::onOff(&t));
                }
                if t.borrow().name == "i" && !t.borrow().isEnd {
                    cur.borrow_mut().setItalic(XlsxSharedStrings::onOff(&t));
                }
                if t.borrow().name == "u" && !t.borrow().isEnd {
                    cur.borrow_mut().setUnderline(XlsxSharedStrings::onOff(&t));
                }
                if t.borrow().name == "strike" && !t.borrow().isEnd {
                    cur.borrow_mut().setStrike(XlsxSharedStrings::onOff(&t));
                }
                if t.borrow().name == "sz" && !t.borrow().isEnd {
                    let sv: String = t.borrow().attr("val");
                    if !sv.is_empty() {
                        let d: Option<f64> = sv.parse::<f64>().ok();
                        if d.is_some() {
                            cur.borrow_mut().sizePt = d.unwrap();
                        }
                    }
                }
                if t.borrow().name == "color" && !t.borrow().isEnd {
                    let cv: String = t.borrow().attr("rgb");
                    if DgNative::str_len(&cv) >= 6 {
                        // ARGB in the file, CSS in the model.
                        cur.borrow_mut().rgb = format!("#{}", DgNative::substr(&cv, DgNative::str_len(&cv) - 6, DgNative::str_len(&cv)));
                    }
                }
                if t.borrow().name == "t" && !t.borrow().isEnd {
                    if !t.borrow().selfClose {
                        let body: String = p.borrow_mut().readTextUntilEnd("t");
                        cur.borrow_mut().text = format!("{}{}", cur.borrow().text, body);
                        out.borrow_mut().add(cur.clone());
                        let next: Rc<RefCell<CellRun>> = Rc::new(RefCell::new(CellRun::new()));
                        if in_run {
                            // A run's properties belong to its own <t> only.
                            cur = next.clone();
                        } else {
                            cur = next.clone();
                        }
                    }
                }
            }
        }
        out
    }
    pub fn onOff(t: &Rc<RefCell<XmlTag>>) -> bool {
        let v: String = t.borrow().attr("val");
        if v.is_empty() {
            return true;
        }
        if v == "0" {
            return false;
        }
        v != "false"
    }
    pub fn count(&self) -> i64 {
        self.strings.len() as i64
    }
    pub fn at(&self, i: i64) -> String {
        if i < 0 {
            return String::new();
        }
        if i >= (self.strings.len() as i64) {
            return String::new();
        }
        self.strings[i as usize].clone()
    }
    pub fn richAt(&self, i: i64) -> Rc<RefCell<RichText>> {
        if i < 0 {
            return Rc::new(RefCell::new(RichText::new()));
        }
        if i >= (self.rich.len() as i64) {
            return Rc::new(RefCell::new(RichText::new()));
        }
        self.rich[i as usize].clone()
    }
    pub fn parse(&mut self, xml: &str) {
        self.strings.clear();
        self.rich.clear();
        self.anyRich = false;
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
            if (t.borrow().name == "si" && !t.borrow().isEnd) && !t.borrow().selfClose {
                let rt: Rc<RefCell<RichText>> = XlsxSharedStrings::parseRuns(&p, "si");
                self.rich.push(rt.clone());
                self.strings.push(rt.borrow().text());
                if rt.borrow().isRich() {
                    self.anyRich = true;
                }
            }
        }
    }
}
