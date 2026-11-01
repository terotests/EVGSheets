// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;

/// DataValidation.rgr — what a cell will accept.
///
/// A validation is a rule attached to a RANGE, not to a cell: "A3:A6 is one of
/// north / south / east / west", "B3:B6 is a whole number between 1 and 100".
/// That is how OOXML writes it and how a spreadsheet thinks about it — you set
/// the rule once and it holds for everything you type into the block afterwards.
///
/// The rule answers one question — `accepts(text)` — and the editor asks it
/// before writing. Everything else here is what it takes to answer that:
/// parsing the list form, coercing the bounds, and knowing which comparison the
/// operator meant.
///
/// A list rule also has a second job: it knows what the choices ARE, so the grid
/// can offer them rather than making you remember them.
#[derive(Clone)]
pub struct DataValidation {
    /// "list" | "whole" | "decimal" | "textLength" — anything else accepts
    /// everything, which is what OOXML's `type="none"` means too.
    pub kind: String,
    /// between | notBetween | equal | notEqual | greaterThan | lessThan |
    /// greaterThanOrEqual | lessThanOrEqual
    pub op: String,
    pub r0: i64,
    pub c0: i64,
    pub r1: i64,
    pub c1: i64,
    pub f1: String,
    pub f2: String,
    pub allowBlank: bool,
    pub errorTitle: String,
    pub errorText: String,
}

impl DataValidation {
    pub fn new() -> Self {
        Self {
            kind: String::new(),
            op: "between".to_string(),
            r0: 0,
            c0: 0,
            r1: 0,
            c1: 0,
            f1: String::new(),
            f2: String::new(),
            allowBlank: true,
            errorTitle: String::new(),
            errorText: String::new(),
        }
    }
    pub fn trimWs(s: &str) -> String {
        let n: i64 = DgNative::str_len(s);
        let mut a: i64 = 0;
        while a < n {
            if DgNative::char_at(&s, a) != 32 {
                break;
            }
            a += 1;
        }
        let mut b: i64 = n;
        while b > a {
            if DgNative::char_at(&s, b - 1) != 32 {
                break;
            }
            b -= 1;
        }
        DgNative::substr(&s, a, b)
    }
    /// The number a text reads as, if any.
    pub fn numberOf(text: &str) -> Option<f64> {
        text.parse::<f64>().ok()
    }
    pub fn copy(&self) -> Rc<RefCell<DataValidation>> {
        let d: Rc<RefCell<DataValidation>> = Rc::new(RefCell::new(DataValidation::new()));
        d.borrow_mut().kind = self.kind.clone();
        d.borrow_mut().op = self.op.clone();
        d.borrow_mut().r0 = self.r0;
        d.borrow_mut().c0 = self.c0;
        d.borrow_mut().r1 = self.r1;
        d.borrow_mut().c1 = self.c1;
        d.borrow_mut().f1 = self.f1.clone();
        d.borrow_mut().f2 = self.f2.clone();
        d.borrow_mut().allowBlank = self.allowBlank;
        d.borrow_mut().errorTitle = self.errorTitle.clone();
        d.borrow_mut().errorText = self.errorText.clone();
        d
    }
    pub fn covers(&self, row: i64, col: i64) -> bool {
        if row < self.r0 {
            return false;
        }
        if row > self.r1 {
            return false;
        }
        if col < self.c0 {
            return false;
        }
        col <= self.c1
    }
    pub fn isList(&self) -> bool {
        self.kind == "list"
    }
    /// A rule whose verdict is a function somebody wrote, named by `f1`. It is
    /// answered by a `CellRuleHost`, not here: this object is read out of an
    /// .xlsx and written back into one, and `accepts` below has to be
    /// answerable from the file alone. So a script rule falls through to the
    /// "anything else accepts everything" case — which is also exactly right
    /// for a sheet opened somewhere the script is not.
    pub fn isScript(&self) -> bool {
        self.kind == "script"
    }
    pub fn scriptName(&self) -> String {
        self.f1.clone()
    }
    /// The choices of a list rule. OOXML writes them as one quoted, comma
    /// separated string — `"north,south,east,west"` — and the quotes are part of
    /// the encoding rather than part of the first and last value.
    pub fn listValues(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let src: String = self.f1.clone();
        let n: i64 = DgNative::str_len(&src);
        if n == 0 {
            return out;
        }
        let mut start: i64 = 0;
        let mut stop: i64 = n;
        if DgNative::char_at(&src, 0) == 34 {
            start = 1;
        }
        if stop > start {
            if DgNative::char_at(&src, stop - 1) == 34 {
                stop -= 1;
            }
        }
        let mut cur: String = String::new();
        let mut i: i64 = start;
        while i < stop {
            let ch: i64 = DgNative::char_at(&src, i);
            if ch == 44 {
                out.push(DataValidation::trimWs(&cur));
                cur = String::new();
            } else {
                cur = format!("{}{}", cur, DgNative::substr(&src, i, i + 1));
            }
            i += 1;
        }
        out.push(DataValidation::trimWs(&cur));
        out
    }
    /// Would this cell take that text?
    pub fn accepts(&self, text: &str) -> bool {
        if text.is_empty() {
            return self.allowBlank;
        }
        if self.kind == "list" {
            let vals: Vec<String> = self.listValues();
            let mut i: i64 = 0;
            while i < (vals.len() as i64) {
                if vals[i as usize].clone() == text {
                    return true;
                }
                i += 1;
            }
            return false;
        }
        let mut numeric: bool = false;
        if self.kind == "whole" {
            numeric = true;
        }
        if self.kind == "decimal" {
            numeric = true;
        }
        if numeric {
            let v: f64 = match DataValidation::numberOf(text) {
                Some(v) => v,
                None => return false,
            };
            if self.kind == "whole" {
                let rounded: f64 = (v.floor() as i64) as f64;
                if rounded != v {
                    return false;
                }
            }
            return self.compare(v);
        }
        if self.kind == "textLength" {
            return self.compare(DgNative::str_len(text) as f64);
        }
        true
    }
    pub fn compare(&self, v: f64) -> bool {
        let a: f64 = match DataValidation::numberOf(&self.f1) {
            Some(a) => a,
            None => return true,
        };
        let b: Option<f64> = DataValidation::numberOf(&self.f2);
        match self.op.as_str() {
            "between" => match b {
                Some(b) => v >= a && v <= b,
                None => v >= a,
            },
            "notBetween" => match b {
                Some(b) => v < a || v > b,
                None => v < a,
            },
            "equal" => v == a,
            "notEqual" => v != a,
            "greaterThan" => v > a,
            "lessThan" => v < a,
            "greaterThanOrEqual" => v >= a,
            "lessThanOrEqual" => v <= a,
            _ => true,
        }
    }
    /// What to tell someone whose entry was refused. The file's own message wins
    /// — it is the one whoever built the sheet wanted them to read.
    pub fn message(&self) -> String {
        if !self.errorText.is_empty() {
            return self.errorText.clone();
        }
        if self.kind == "list" {
            return "not one of the allowed values".to_string();
        }
        if self.kind == "whole" {
            return format!("not a whole number {} {} {}", self.op, self.f1, self.f2);
        }
        if self.kind == "decimal" {
            return format!("not a number {} {} {}", self.op, self.f1, self.f2);
        }
        if self.kind == "textLength" {
            return format!("text length must be {} {} {}", self.op, self.f1, self.f2);
        }
        "not allowed here".to_string()
    }
    pub fn describe(&self) -> String {
        if self.kind == "list" {
            return format!("list: {}", self.f1);
        }
        format!("{} {} {} {}", self.kind, self.op, self.f1, self.f2)
    }
}
