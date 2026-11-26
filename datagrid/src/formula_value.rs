// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::date_serial::DateSerial;
use crate::rgr::DgNative;

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
    /// Excel's order for the comparison operators: numbers < text < TRUE/FALSE;
    /// an empty cell is 0, "" or FALSE, whichever the other side is; text
    /// compares without regard to case; numbers that agree to 15 significant
    /// digits are equal (0.1+0.2=0.3 is TRUE). -1, 0 or 1.
    pub fn excelCompare(l0: &Rc<RefCell<FormulaValue>>, r0: &Rc<RefCell<FormulaValue>>) -> i64 {
        let l: Rc<RefCell<FormulaValue>> = if l0.borrow().isArray() { l0.borrow().single() } else { l0.clone() };
        let r: Rc<RefCell<FormulaValue>> = if r0.borrow().isArray() { r0.borrow().single() } else { r0.clone() };
        let mut lk: i64 = l.borrow().kind;
        let mut rk: i64 = r.borrow().kind;
        if lk == 0 && rk == 0 {
            return 0;
        }
        if lk == 0 {
            lk = rk;
        }
        if rk == 0 {
            rk = lk;
        }
        let rank = |k: i64| -> i64 {
            if k == 1 { 0 } else if k == 2 { 1 } else { 2 }
        };
        if rank(lk) != rank(rk) {
            return if rank(lk) < rank(rk) { -1 } else { 1 };
        }
        if lk == 1 {
            let a: f64 = if l.borrow().kind == 0 { 0.0 } else { l.borrow().num };
            let b: f64 = if r.borrow().kind == 0 { 0.0 } else { r.borrow().num };
            if a == b {
                return 0;
            }
            let big: f64 = if (if a < 0.0 { 0.0 - a } else { a }) > (if b < 0.0 { 0.0 - b } else { b }) { if a < 0.0 { 0.0 - a } else { a } } else { if b < 0.0 { 0.0 - b } else { b } };
            let d: f64 = if a > b { a - b } else { b - a };
            if d <= big * 1e-15 {
                return 0;
            }
            return if a < b { -1 } else { 1 };
        }
        if lk == 2 {
            let a: String = if l.borrow().kind == 0 { String::new() } else { l.borrow().text.to_uppercase() };
            let b: String = if r.borrow().kind == 0 { String::new() } else { r.borrow().text.to_uppercase() };
            let an: i64 = DgNative::str_len(&a);
            let bn: i64 = DgNative::str_len(&b);
            let mut i: i64 = 0;
            while i < an && i < bn {
                let ca: i64 = DgNative::char_at(&a, i);
                let cb: i64 = DgNative::char_at(&b, i);
                if ca != cb {
                    return if ca < cb { -1 } else { 1 };
                }
                i += 1;
            }
            if an == bn {
                return 0;
            }
            return if an < bn { -1 } else { 1 };
        }
        let a: bool = if l.borrow().kind == 0 { false } else { l.borrow().flag };
        let b: bool = if r.borrow().kind == 0 { false } else { r.borrow().flag };
        if a == b {
            return 0;
        }
        if b { -1 } else { 1 }
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
    /// Text as Excel reads it into a number, for arithmetic, VALUE and a typed
    /// cell: "1,234", "12%", "$5.50", "(42)", "1e3", "1 1/2", "18:30",
    /// "6:30 PM", "2024-01-15", "2024-01-15 18:30". None for anything else.
    pub fn parseNumberText(text: &str) -> Option<f64> {
        let s: String = text.trim().to_string();
        if s.is_empty() {
            return None;
        }
        if let Some(v) = FormulaValue::parsePlainNumber(&s) {
            return Some(v);
        }
        if let Some(v) = FormulaValue::parseTime(&s) {
            return Some(v);
        }
        if DateSerial::isDate(&s) {
            return Some(DateSerial::parseDate(&s));
        }
        // A date followed by a time.
        let sp: i64 = DgNative::index_of(&s, " ");
        if sp > 0 {
            let d: String = DgNative::substr(&s, 0, sp);
            let rest: String = DgNative::substr(&s, sp + 1, DgNative::str_len(&s));
            if DateSerial::isDate(&d) {
                if let Some(tm) = FormulaValue::parseTime(rest.trim()) {
                    return Some(DateSerial::parseDate(&d) + tm);
                }
            }
        }
        None
    }
    /// Sign or parentheses, a currency mark, digits grouped by commas, a
    /// decimal point, an exponent, a trailing % — or a whole number and a
    /// fraction ("1 1/2", "3/4" is a date in Excel and stays one here).
    pub fn parsePlainNumber(s0: &str) -> Option<f64> {
        let mut s: String = s0.to_string();
        let mut neg: bool = false;
        let mut pct: bool = false;
        let n0: i64 = DgNative::str_len(&s);
        if n0 >= 2 && DgNative::char_at(&s, 0) == 40 && DgNative::char_at(&s, n0 - 1) == 41 {
            neg = true;
            s = DgNative::substr(&s, 1, n0 - 1).trim().to_string();
        }
        if s.is_empty() {
            return None;
        }
        let c0: i64 = DgNative::char_at(&s, 0);
        if c0 == 45 || c0 == 43 {
            if c0 == 45 {
                neg = !neg;
            }
            s = DgNative::substr(&s, 1, DgNative::str_len(&s)).trim().to_string();
        }
        if s.is_empty() {
            return None;
        }
        let c1: i64 = DgNative::char_at(&s, 0);
        if c1 == 36 || c1 == 8364 || c1 == 163 {
            s = DgNative::substr(&s, 1, DgNative::str_len(&s)).trim().to_string();
            if !s.is_empty() && DgNative::char_at(&s, 0) == 45 {
                neg = !neg;
                s = DgNative::substr(&s, 1, DgNative::str_len(&s)).to_string();
            }
        }
        let ln: i64 = DgNative::str_len(&s);
        if ln > 0 && DgNative::char_at(&s, ln - 1) == 37 {
            pct = true;
            s = DgNative::substr(&s, 0, ln - 1).trim().to_string();
        }
        if s.is_empty() {
            return None;
        }
        // "1 1/2": a whole number, a space, a fraction.
        let sp: i64 = DgNative::index_of(&s, " ");
        let sl: i64 = DgNative::index_of(&s, "/");
        if sp > 0 && sl > sp && !pct {
            let whole: String = DgNative::substr(&s, 0, sp);
            let num: String = DgNative::substr(&s, sp + 1, sl).trim().to_string();
            let den: String = DgNative::substr(&s, sl + 1, DgNative::str_len(&s)).trim().to_string();
            if FormulaValue::allDigits(&whole) && FormulaValue::allDigits(&num) && FormulaValue::allDigits(&den) {
                let w: f64 = whole.parse::<f64>().unwrap_or(0.0);
                let a: f64 = num.parse::<f64>().unwrap_or(0.0);
                let b: f64 = den.parse::<f64>().unwrap_or(0.0);
                if b > 0.0 && a < b {
                    let v: f64 = w + a / b;
                    return Some(if neg { 0.0 - v } else { v });
                }
            }
            return None;
        }
        // Thousands separators: groups of three after the first comma, before
        // any decimal point.
        if DgNative::index_of(&s, ",") >= 0 {
            let dot: i64 = DgNative::index_of(&s, ".");
            let int_part: String = if dot >= 0 { DgNative::substr(&s, 0, dot) } else { s.clone() };
            let groups: Vec<String> = int_part.split(",").map(|g| g.to_string()).collect::<Vec<String>>();
            let mut gi: usize = 0;
            while gi < groups.len() {
                let g: String = groups[gi].clone();
                if !FormulaValue::allDigits(&g) {
                    return None;
                }
                if gi == 0 && (DgNative::str_len(&g) < 1 || DgNative::str_len(&g) > 3) {
                    return None;
                }
                if gi > 0 && DgNative::str_len(&g) != 3 {
                    return None;
                }
                gi += 1;
            }
            s = s.replace(",", "");
        }
        if !FormulaValue::numberShape(&s) {
            return None;
        }
        let mut v: f64 = s.parse::<f64>().ok()?;
        if pct {
            v = v / 100.0;
        }
        if neg {
            v = 0.0 - v;
        }
        Some(v)
    }
    pub fn allDigits(s: &str) -> bool {
        let n: i64 = DgNative::str_len(s);
        if n == 0 {
            return false;
        }
        let mut i: i64 = 0;
        while i < n {
            let ch: i64 = DgNative::char_at(s, i);
            if ch < 48 || ch > 57 {
                return false;
            }
            i += 1;
        }
        true
    }
    /// digits, one optional point, an optional exponent with digits: nothing else.
    pub fn numberShape(s: &str) -> bool {
        let n: i64 = DgNative::str_len(s);
        let mut i: i64 = 0;
        let mut digits: i64 = 0;
        let mut dots: i64 = 0;
        while i < n {
            let ch: i64 = DgNative::char_at(s, i);
            if ch >= 48 && ch <= 57 {
                digits += 1;
            } else if ch == 46 {
                dots += 1;
                if dots > 1 {
                    return false;
                }
            } else if (ch == 69 || ch == 101) && digits > 0 {
                let mut j: i64 = i + 1;
                if j < n && (DgNative::char_at(s, j) == 43 || DgNative::char_at(s, j) == 45) {
                    j += 1;
                }
                if j >= n {
                    return false;
                }
                return FormulaValue::allDigits(&DgNative::substr(s, j, n));
            } else {
                return false;
            }
            i += 1;
        }
        digits > 0
    }
    /// "18:30", "18:30:15", "6:30 PM", "6 PM" as a fraction of a day.
    pub fn parseTime(s0: &str) -> Option<f64> {
        let mut s: String = s0.trim().to_uppercase();
        let mut ampm: i64 = 0;
        if s.ends_with("AM") || s.ends_with("PM") {
            ampm = if s.ends_with("PM") { 2 } else { 1 };
            s = DgNative::substr(&s, 0, DgNative::str_len(&s) - 2).trim().to_string();
        } else if s.ends_with(" A") || s.ends_with(" P") {
            ampm = if s.ends_with(" P") { 2 } else { 1 };
            s = DgNative::substr(&s, 0, DgNative::str_len(&s) - 2).trim().to_string();
        }
        let parts: Vec<String> = s.split(":").map(|p| p.to_string()).collect::<Vec<String>>();
        if parts.len() > 3 || (parts.len() < 2 && ampm == 0) {
            return None;
        }
        let mut nums: Vec<f64> = Vec::new();
        let mut i: usize = 0;
        while i < parts.len() {
            let p: String = parts[i].clone();
            let ok: bool = if i == 2 { FormulaValue::numberShape(&p) && DgNative::index_of(&p, "e") < 0 && DgNative::index_of(&p, "E") < 0 } else { FormulaValue::allDigits(&p) };
            if !ok {
                return None;
            }
            nums.push(p.parse::<f64>().unwrap_or(0.0));
            i += 1;
        }
        let mut h: f64 = nums[0];
        let m: f64 = if nums.len() > 1 { nums[1] } else { 0.0 };
        let sec: f64 = if nums.len() > 2 { nums[2] } else { 0.0 };
        if m >= 60.0 || sec >= 60.0 {
            return None;
        }
        if ampm != 0 {
            if h < 1.0 || h > 12.0 {
                return None;
            }
            if h == 12.0 {
                h = 0.0;
            }
            if ampm == 2 {
                h += 12.0;
            }
        }
        Some((h * 3600.0 + m * 60.0 + sec) / 86400.0)
    }
    /// The number format Excel gives a cell typed as `text` (which
    /// `parseNumberText` reads): "12%" wears 0%, "$5.50" a currency, "1,234"
    /// #,##0, "18:30" h:mm, "1 1/2" a fraction. "" when a plain number needs none.
    pub fn typedFormat(text: &str) -> String {
        let s: String = text.trim().to_string();
        let dot: i64 = DgNative::index_of(&s, ".");
        let decimals: bool = dot >= 0 && DgNative::index_of(&s, ":") < 0;
        if FormulaValue::parsePlainNumber(&s).is_some() {
            if s.ends_with("%") {
                return if decimals { "0.00%".to_string() } else { "0%".to_string() };
            }
            if DgNative::index_of(&s, "$") >= 0 {
                return if decimals { "\"$\"#,##0.00".to_string() } else { "\"$\"#,##0".to_string() };
            }
            if DgNative::index_of(&s, "€") >= 0 {
                return if decimals { "#,##0.00\\ \"€\"".to_string() } else { "#,##0\\ \"€\"".to_string() };
            }
            if DgNative::index_of(&s, ",") >= 0 {
                return if decimals { "#,##0.00".to_string() } else { "#,##0".to_string() };
            }
            if DgNative::index_of(&s, "/") >= 0 {
                return "# ?/?".to_string();
            }
            let up: String = s.to_uppercase();
            if DgNative::index_of(&up, "E") > 0 {
                return "0.00E+00".to_string();
            }
            return String::new();
        }
        if FormulaValue::parseTime(&s).is_some() {
            let up: String = s.to_uppercase();
            // a Vec and its len, not split(..).count(): Ranger's Go writer
            // turns count() into a loop with an index it never uses, which Go
            // refuses to compile (MFilesAPI's desktop/web build)
            let colons: Vec<String> = s.split(":").map(|p| p.to_string()).collect::<Vec<String>>();
            let secs: bool = colons.len() > 2;
            if up.ends_with("M") || up.ends_with(" A") || up.ends_with(" P") {
                return if secs { "h:mm:ss AM/PM".to_string() } else { "h:mm AM/PM".to_string() };
            }
            return if secs { "h:mm:ss".to_string() } else { "h:mm".to_string() };
        }
        String::new()
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
            2 => FormulaValue::parseNumberText(&self.text),
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
