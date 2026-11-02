// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::{DgNative, OoxmlText};

/// XmlLite - tiny OpenXML-oriented XML scanner (not a full DOM).
///
/// Enough to walk SpreadsheetML parts: tag names, attributes (incl. r:id), and
/// element text. Modeled after lib/evg/SvgParser's SvgTag/SvgAttr scan, but
/// kept independent so DataGrid does not import the SVG IR stack.
#[derive(Clone)]
pub struct XmlAttr {
    pub name: String,
    pub value: String,
}

impl XmlAttr {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            value: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct XmlTag {
    pub name: String,
    pub isEnd: bool,
    pub selfClose: bool,
    pub attrs: Vec<Rc<RefCell<XmlAttr>>>,
    pub valid: bool,
    pub startPos: i64,
    pub endPos: i64,
}

impl XmlTag {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            isEnd: false,
            selfClose: false,
            attrs: Vec::new(),
            valid: false,
            startPos: 0,
            endPos: 0,
        }
    }
    /// Reset for reuse. Callers must not keep a tag across `nextTag` /
    /// `readTag` — those hand back the same instance.
    pub fn clearTag(&mut self) {
        self.name = String::new();
        self.isEnd = false;
        self.selfClose = false;
        self.valid = false;
        self.startPos = 0;
        self.endPos = 0;
        self.attrs.clear();
    }
    pub fn has(&self, n: &str) -> bool {
        let mut k: i64 = 0;
        while k < (self.attrs.len() as i64) {
            let a: Rc<RefCell<XmlAttr>> = self.attrs[k as usize].clone();
            if a.borrow().name == n {
                return true;
            }
            k += 1;
        }
        false
    }
    pub fn attr(&self, n: &str) -> String {
        let mut k: i64 = 0;
        while k < (self.attrs.len() as i64) {
            let a: Rc<RefCell<XmlAttr>> = self.attrs[k as usize].clone();
            if a.borrow().name == n {
                return a.borrow().value.clone();
            }
            k += 1;
        }
        String::new()
    }
    /// Prefer local name when prefixed attrs differ (r:id vs id).
    pub fn attrAny(&self, a: &str, b: &str) -> String {
        if self.has(a) {
            return self.attr(a);
        }
        self.attr(b)
    }
}

#[derive(Clone)]
pub struct XmlLite {
    pub src: String,
    pub pos: i64,
    pub srcLen: i64,
    /// One tag instance for the whole scan. Large sheets create tens of millions
    /// of tags if each `nextTag` allocates; native builds then spend minutes in
    /// the allocator while the JS host finishes in seconds.
    pub cur: Rc<RefCell<XmlTag>>,
    pub attrPool: Vec<Rc<RefCell<XmlAttr>>>,
    pub attrPoolN: i64,
}

impl XmlLite {
    pub fn new() -> Self {
        Self {
            src: String::new(),
            pos: 0,
            srcLen: 0,
            cur: Rc::new(RefCell::new(XmlTag::new())),
            attrPool: Vec::new(),
            attrPoolN: 0,
        }
    }
    /// `&lt;` and friends back into the characters they stand for.
    ///
    /// Everything that is NOT an entity is copied with `substring`, in runs, and
    /// that is the point of the shape below rather than a character at a time.
    /// `(strfromcode (charAt s i))` looks like an identity and is one in
    /// JavaScript, where a string is code units and `charAt` hands one back. In
    /// C++ a string is BYTES: `charAt` gives 0xC3, and `strfromcode` — whose job
    /// is to turn a code POINT into text — dutifully encodes 195 as UTF-8, two
    /// bytes. So every non-ASCII character came out of here encoded twice, and
    /// "Hämeenlinna" was loaded as "HÃ¤meenlinna". `substring` is byte-exact in
    /// both, and copying a run of text is what this loop is doing anyway.
    /// The decoder moved to `OoxmlText`, which .docx and .pptx read through
    /// too — three copies of it is how the spreadsheet came to understand
    /// `&#228;` while the other two still showed the digits.
    pub fn decodeEntities(s: &str) -> String {
        OoxmlText::decode_entities(s)
    }
    pub fn entityText(ent: &str) -> String {
        OoxmlText::entity_text(ent)
    }
    pub fn entityNumber(ent: &str) -> i64 {
        OoxmlText::entity_number(ent)
    }
    pub fn reset(&mut self, s: String) {
        self.srcLen = DgNative::str_len(&s);
        self.src = s;
        self.pos = 0;
        self.attrPoolN = 0;
        self.cur.clone().borrow_mut().clearTag();
    }
    pub fn takeAttr(&mut self) -> Rc<RefCell<XmlAttr>> {
        if self.attrPoolN < (self.attrPool.len() as i64) {
            let a: Rc<RefCell<XmlAttr>> = self.attrPool[(self.attrPoolN) as usize].clone();
            self.attrPoolN += 1;
            a.borrow_mut().name = String::new();
            a.borrow_mut().value = String::new();
            return a;
        }
        let fresh: Rc<RefCell<XmlAttr>> = Rc::new(RefCell::new(XmlAttr::new()));
        self.attrPool.push(fresh.clone());
        self.attrPoolN += 1;
        fresh
    }
    pub fn skipWs(&mut self) {
        while self.pos < self.srcLen {
            let ch: i64 = DgNative::char_at(&self.src, self.pos);
            if ((ch == 32 || ch == 9) || ch == 10) || ch == 13 {
                self.pos += 1;
            } else {
                return;
            }
        }
    }
    pub fn skipUntil(&mut self, c: i64) -> bool {
        while self.pos < self.srcLen {
            if DgNative::char_at(&self.src, self.pos) == c {
                return true;
            }
            self.pos += 1;
        }
        false
    }
    pub fn readName(&mut self) -> String {
        let start: i64 = self.pos;
        while self.pos < self.srcLen {
            let ch: i64 = DgNative::char_at(&self.src, self.pos);
            // letter digit _ : - .
            if ((((((ch >= 65 && ch <= 90) || (ch >= 97 && ch <= 122)) || (ch >= 48 && ch <= 57)) || ch == 95) || ch == 58) || ch == 45) || ch == 46 {
                self.pos += 1;
            } else {
                break;
            }
        }
        DgNative::substr(&self.src, start, self.pos)
    }
    pub fn localName(&self, raw: &str) -> String {
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(raw);
        let mut colon: i64 = -1;
        while i < n {
            if DgNative::char_at(raw, i) == 58 {
                colon = i;
            }
            i += 1;
        }
        if colon >= 0 {
            return DgNative::substr(raw, colon + 1, n);
        }
        raw.to_string()
    }
    pub fn readTag(&mut self) -> Rc<RefCell<XmlTag>> {
        self.cur.clone().borrow_mut().clearTag();
        self.attrPoolN = 0;
        self.skipWs();
        if self.pos >= self.srcLen {
            return self.cur.clone();
        }
        if DgNative::char_at(&self.src, self.pos) != 60 {
            return self.cur.clone();
        }
        self.cur.borrow_mut().startPos = self.pos;
        self.pos += 1;
        if self.pos >= self.srcLen {
            return self.cur.clone();
        }
        // comment / PI / doctype — skip
        let peek: i64 = DgNative::char_at(&self.src, self.pos);
        if peek == 33 || peek == 63 {
            self.skipUntil(62);
            if self.pos < self.srcLen {
                self.pos += 1;
            }
            return self.readTag();
        }
        if peek == 47 {
            self.cur.borrow_mut().isEnd = true;
            self.pos += 1;
        }
        let raw = self.readName();
        self.cur.borrow_mut().name = self.localName(&raw);
        self.skipWs();
        while self.pos < self.srcLen {
            let ch: i64 = DgNative::char_at(&self.src, self.pos);
            if ch == 62 {
                self.pos += 1;
                self.cur.borrow_mut().endPos = self.pos;
                self.cur.borrow_mut().valid = true;
                return self.cur.clone();
            }
            if ch == 47 {
                self.pos += 1;
                self.skipWs();
                if self.pos < self.srcLen && DgNative::char_at(&self.src, self.pos) == 62 {
                    self.pos += 1;
                    self.cur.borrow_mut().selfClose = true;
                    self.cur.borrow_mut().endPos = self.pos;
                    self.cur.borrow_mut().valid = true;
                    return self.cur.clone();
                }
            }
            let an: String = self.readName();
            if an.is_empty() {
                self.pos += 1;
            } else {
                self.skipWs();
                let mut av: String = String::new();
                if self.pos < self.srcLen && DgNative::char_at(&self.src, self.pos) == 61 {
                    self.pos += 1;
                    self.skipWs();
                    if self.pos < self.srcLen {
                        let q: i64 = DgNative::char_at(&self.src, self.pos);
                        if q == 34 || q == 39 {
                            self.pos += 1;
                            let vs: i64 = self.pos;
                            while self.pos < self.srcLen && DgNative::char_at(&self.src, self.pos) != q {
                                self.pos += 1;
                            }
                            av = XmlLite::decodeEntities(&DgNative::substr(&self.src, vs, self.pos));
                            if self.pos < self.srcLen {
                                self.pos += 1;
                            }
                        }
                    }
                }
                let a: Rc<RefCell<XmlAttr>> = self.takeAttr();
                a.borrow_mut().name = self.localName(&an);
                // also keep prefixed form for r:id lookups via attrAny
                if an != a.borrow().name {
                    let a2: Rc<RefCell<XmlAttr>> = self.takeAttr();
                    a2.borrow_mut().name = an.clone();
                    a2.borrow_mut().value = av.clone();
                    self.cur.borrow_mut().attrs.push(a2.clone());
                }
                a.borrow_mut().value = av.clone();
                self.cur.borrow_mut().attrs.push(a.clone());
                self.skipWs();
            }
        }
        self.cur.clone()
    }
    /// Advance to next start/end tag; returns invalid at EOF.
    pub fn nextTag(&mut self) -> Rc<RefCell<XmlTag>> {
        while self.pos < self.srcLen {
            if DgNative::char_at(&self.src, self.pos) == 60 {
                return self.readTag();
            }
            self.pos += 1;
        }
        self.cur.clone().borrow_mut().clearTag();
        self.cur.clone()
    }
    /// Text content from current pos until the matching end tag of `name`.
    /// Assumes caller is just after the open tag. Handles simple nesting depth.
    pub fn readTextUntilEnd(&mut self, name: &str) -> String {
        let start: i64 = self.pos;
        let mut depth: i64 = 1;
        while self.pos < self.srcLen && depth > 0 {
            if DgNative::char_at(&self.src, self.pos) != 60 {
                self.pos += 1;
            } else {
                let save: i64 = self.pos;
                let t: Rc<RefCell<XmlTag>> = self.readTag();
                if t.borrow().valid {
                    if t.borrow().name == name {
                        if t.borrow().isEnd {
                            depth -= 1;
                            if depth == 0 {
                                let raw: String = DgNative::substr(&self.src, start, save);
                                return XmlLite::decodeEntities(&raw);
                            }
                        } else {
                            if !t.borrow().selfClose {
                                depth += 1;
                            }
                        }
                    }
                } else {
                    self.pos = save + 1;
                }
            }
        }
        XmlLite::decodeEntities(&DgNative::substr(&self.src, start, self.pos))
    }
    /// Collect concatenated <t>…</t> text under the current element until endName.
    pub fn collectTTextUntil(&mut self, end_name: &str) -> String {
        let mut out: String = String::new();
        let mut depth: i64 = 1;
        while self.pos < self.srcLen && depth > 0 {
            let t: Rc<RefCell<XmlTag>> = self.nextTag();
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
                if t.borrow().name == "t" && !t.borrow().isEnd {
                    if t.borrow().selfClose {
                        // empty
                    } else {
                        out = format!("{}{}", out, self.readTextUntilEnd("t"));
                    }
                }
            }
        }
        out
    }
}
