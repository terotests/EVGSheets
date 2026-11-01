// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// FormulaValue - typed spreadsheet values + Excel-ish coercion.
///
/// Shared by FormulaFunctions (pure library) and FormulaEngine (workbook eval).
/// kind: 0 blank | 1 number | 2 string | 3 bool | 4 error | 5 array
///
/// Arithmetic uses coerceNumber() → blank/"" /bool → 0/1, non-numeric → #VALUE!.
/// Equality prefers numeric compare when both sides coerce; else toText().
#[derive(Clone)]
pub struct FormulaValue {
    pub kind: i64,
    pub num: f64,
    pub text: String,
    pub flag: bool,
    pub err: String,
    /// --- arrays ---------------------------------------------------------------
    /// A range is a RECTANGLE, and some functions need to know that: VLOOKUP
    /// reads down a column and across a row, MATCH returns a position, INDEX
    /// picks one out. Flattening a range into a list of arguments — which is all
    /// the engine could do before — throws that shape away.
    ///
    /// So a range evaluates to one value of kind 5 carrying its own dimensions.
    /// Functions that only ever wanted numbers keep working because the helpers
    /// that walk arguments expand arrays on the way past.
    pub rows: i64,
    pub cols: i64,
    pub items: Vec<Rc<RefCell<FormulaValue>>>,
}

impl FormulaValue {
    pub fn new() -> Self {
        Self {
            kind: 0,
            num: 0.0,
            text: String::new(),
            flag: false,
            err: String::new(),
            rows: 0,
            cols: 0,
            items: Vec::new(),
        }
    }
    pub fn blank() -> Rc<RefCell<FormulaValue>> {
        Rc::new(RefCell::new(FormulaValue::new()))
    }
    pub fn fromNumber(v: f64) -> Rc<RefCell<FormulaValue>> {
        let o: Rc<RefCell<FormulaValue>> = Rc::new(RefCell::new(FormulaValue::new()));
        o.borrow_mut().kind = 1;
        o.borrow_mut().num = v;
        o
    }
    pub fn fromString(s: String) -> Rc<RefCell<FormulaValue>> {
        let o: Rc<RefCell<FormulaValue>> = Rc::new(RefCell::new(FormulaValue::new()));
        o.borrow_mut().kind = 2;
        o.borrow_mut().text = s.clone();
        o
    }
    pub fn fromBool(b: bool) -> Rc<RefCell<FormulaValue>> {
        let o: Rc<RefCell<FormulaValue>> = Rc::new(RefCell::new(FormulaValue::new()));
        o.borrow_mut().kind = 3;
        o.borrow_mut().flag = b;
        if b {
            o.borrow_mut().num = 1.0;
            o.borrow_mut().text = "TRUE".to_string();
        } else {
            o.borrow_mut().num = 0.0;
            o.borrow_mut().text = "FALSE".to_string();
        }
        o
    }
    pub fn fromArray(rows: i64, cols: i64, items: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let o: Rc<RefCell<FormulaValue>> = Rc::new(RefCell::new(FormulaValue::new()));
        o.borrow_mut().kind = 5;
        o.borrow_mut().rows = rows;
        o.borrow_mut().cols = cols;
        let mut i: i64 = 0;
        while i < (items.len() as i64) {
            o.borrow_mut().items.push(items[i as usize].clone());
            i += 1;
        }
        o
    }
    pub fn fromError(e: String) -> Rc<RefCell<FormulaValue>> {
        let o: Rc<RefCell<FormulaValue>> = Rc::new(RefCell::new(FormulaValue::new()));
        o.borrow_mut().kind = 4;
        o.borrow_mut().err = e.clone();
        o.borrow_mut().text = e.clone();
        o
    }
    /// Excel-ish equality: numeric when both coerce; else text.
    pub fn valuesEqual(l: &Rc<RefCell<FormulaValue>>, r: &Rc<RefCell<FormulaValue>>) -> bool {
        if let Some(a) = l.borrow().coercedNumber() {
            if let Some(b) = r.borrow().coercedNumber() {
                return a == b;
            }
        }
        let ls: String = l.borrow().toText();
        let rs: String = r.borrow().toText();
        ls == rs
    }
    pub fn isArray(&self) -> bool {
        self.kind == 5
    }
    pub fn count(&self) -> i64 {
        self.items.len() as i64
    }
    /// Row-major, and out of range is blank rather than an error: a jagged read
    /// is a gap in the data, not a broken formula.
    pub fn at(&self, r: i64, c: i64) -> Rc<RefCell<FormulaValue>> {
        if self.kind != 5 {
            if r == 0 {
                if c == 0 {
                    return Rc::new(RefCell::new(self.clone()));
                }
            }
            return FormulaValue::blank();
        }
        if r < 0 {
            return FormulaValue::blank();
        }
        if c < 0 {
            return FormulaValue::blank();
        }
        if r >= self.rows {
            return FormulaValue::blank();
        }
        if c >= self.cols {
            return FormulaValue::blank();
        }
        let at: i64 = r * self.cols + c;
        if at >= (self.items.len() as i64) {
            return FormulaValue::blank();
        }
        self.items[at as usize].clone()
    }
    /// The one value an array stands for when a single value is wanted. Excel
    /// calls this implicit intersection; the top-left is what it comes to for a
    /// formula that is not array-entered.
    pub fn single(&self) -> Rc<RefCell<FormulaValue>> {
        if self.kind != 5 {
            return Rc::new(RefCell::new(self.clone()));
        }
        if self.items.is_empty() {
            return FormulaValue::blank();
        }
        self.items[0].clone()
    }
    pub fn isError(&self) -> bool {
        if self.kind == 5 {
            let mut i: i64 = 0;
            while i < (self.items.len() as i64) {
                let it: Rc<RefCell<FormulaValue>> = self.items[i as usize].clone();
                if it.borrow().kind == 4 {
                    return true;
                }
                i += 1;
            }
            return false;
        }
        self.kind == 4
    }
    /// The first error inside an array, or a blank when there is none. What a
    /// function returns when its input carries one.
    pub fn errorInside(&self) -> Rc<RefCell<FormulaValue>> {
        if self.kind == 4 {
            return Rc::new(RefCell::new(self.clone()));
        }
        if self.kind == 5 {
            let mut i: i64 = 0;
            while i < (self.items.len() as i64) {
                let it: Rc<RefCell<FormulaValue>> = self.items[i as usize].clone();
                if it.borrow().kind == 4 {
                    return it;
                }
                i += 1;
            }
        }
        FormulaValue::blank()
    }
    pub fn isBlank(&self) -> bool {
        self.kind == 0
    }
    pub fn isTrue(&self) -> bool {
        if self.kind == 3 {
            return self.flag;
        }
        if self.kind == 1 {
            return self.num != 0.0;
        }
        if self.kind == 2 {
            if self.text.is_empty() {
                return false;
            }
            return true;
        }
        false
    }
    /// Soft convert used by aggregates (COUNT etc.): numbers + numeric text only.
    /// Blank / "" / bool are NOT numbers for COUNT (Excel-ish).
    pub fn asNumber(&self) -> f64 {
        if let Some(n) = self.number() {
            return n;
        }
        if self.kind == 3 {
            if self.flag {
                return 1.0;
            }
            return 0.0;
        }
        0.0
    }
    /// The number a value is, without coercion: a number, or text that reads
    /// as one. The top-left of an array.
    fn number(&self) -> Option<f64> {
        match self.kind {
            5 => self.single().borrow().number(),
            1 => Some(self.num),
            2 if !self.text.is_empty() => self.text.parse::<f64>().ok(),
            _ => None,
        }
    }
    /// Whether `numberValue` has a number to give.
    pub fn isNumeric(&self) -> bool {
        self.number().is_some()
    }
    /// The number of `isNumeric`, 0 when there is none.
    pub fn numberValue(&self) -> f64 {
        self.number().unwrap_or(0.0)
    }
    /// Arithmetic coerce: blank / "" / bool → 0/1; non-numeric string → #VALUE!.
    fn coercedNumber(&self) -> Option<f64> {
        if self.kind == 5 {
            return self.single().borrow().coercedNumber();
        }
        if self.isError() {
            return None;
        }
        match self.kind {
            0 => Some(0.0),
            1 => Some(self.num),
            3 => Some(if self.flag { 1.0 } else { 0.0 }),
            2 if self.text.is_empty() => Some(0.0),
            2 => self.text.parse::<f64>().ok(),
            _ => None,
        }
    }
    pub fn coerceNumber(&self) -> Rc<RefCell<FormulaValue>> {
        let ee: bool = self.isError();
        if ee {
            return Rc::new(RefCell::new(self.clone()));
        }
        match self.coercedNumber() {
            Some(n) => FormulaValue::fromNumber(n),
            None => FormulaValue::fromError("#VALUE!".to_string()),
        }
    }
    pub fn toText(&self) -> String {
        if self.kind == 5 {
            let one: Rc<RefCell<FormulaValue>> = self.single();
            return one.borrow().toText();
        }
        if self.kind == 0 {
            return String::new();
        }
        if self.kind == 4 {
            return self.err.clone();
        }
        if self.kind == 3 {
            return self.text.clone();
        }
        if self.kind == 2 {
            return self.text.clone();
        }
        self.asRaw()
    }
    pub fn toBoolean(&self) -> bool {
        let t: bool = self.isTrue();
        t
    }
    pub fn compare(&self, other: &Rc<RefCell<FormulaValue>>) -> i64 {
        let a: Rc<RefCell<FormulaValue>> = self.coerceNumber();
        let b: Rc<RefCell<FormulaValue>> = other.borrow().coerceNumber();
        let ae: bool = a.borrow().isError();
        if ae {
            return 0;
        }
        let be: bool = b.borrow().isError();
        if be {
            return 0;
        }
        if a.borrow().num < b.borrow().num {
            return -1;
        }
        if a.borrow().num > b.borrow().num {
            return 1;
        }
        0
    }
    /// Raw string for sheet storage (formatter applies numFmt on display).
    pub fn asRaw(&self) -> String {
        if self.kind == 0 {
            return String::new();
        }
        if self.kind == 4 {
            return self.err.clone();
        }
        if self.kind == 3 {
            return self.text.clone();
        }
        if self.kind == 2 {
            return self.text.clone();
        }
        let iv: i64 = self.num.floor() as i64;
        if (iv as f64) == self.num {
            return iv.to_string();
        }
        self.num.to_string()
    }
    pub fn asDisplay(&self) -> String {
        self.asRaw()
    }
}
