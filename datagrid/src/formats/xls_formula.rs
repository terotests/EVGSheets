// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::formats::sheet_bytes::SheetBytes;
use crate::formats::xls_function_table::XlsFunctionTable;

/// Where 3-D references and external names point: the EXTERNSHEET (XTI) table,
/// the SUPBOOK records it indexes, and the workbook's own sheet and name lists.
#[derive(Clone)]
pub struct XlsLinks {
    pub sheets: Vec<String>,
    pub names: Vec<String>,
    pub bookInternal: Vec<bool>,
    pub bookNames: Vec<Vec<String>>,
    pub xtiBook: Vec<i64>,
    pub xtiFirst: Vec<i64>,
    pub xtiLast: Vec<i64>,
}

impl XlsLinks {
    pub fn new() -> Self {
        Self {
            sheets: Vec::new(),
            names: Vec::new(),
            bookInternal: Vec::new(),
            bookNames: Vec::new(),
            xtiBook: Vec::new(),
            xtiFirst: Vec::new(),
            xtiLast: Vec::new(),
        }
    }
}

/// XlsFormula: a BIFF8 parsed formula (Rgce, [MS-XLS] 2.5.198) back to text.
///
/// A .xls keeps no formula text. It keeps the formula compiled to reverse
/// Polish tokens: operands push, operators and function calls pop. Turning that
/// back into text is a stack of strings. Parentheses are tokens of their own
/// (tParen) wherever the author wrote them, so nothing here has to work out
/// precedence: printing the operators in the order they pop reproduces it.
///
/// What it cannot turn into text — a data table, a reference into another
/// workbook, a token it has never heard of — sets `failure`, and the caller
/// keeps the cell's cached value instead.
#[derive(Clone)]
pub struct XlsFormula {
    pub links: Rc<RefCell<XlsLinks>>,
    pub failure: String,
    pub fnIndex: HashMap<i64,i64>,
    pub fnNames: Vec<String>,
    pub fnArgs: Vec<i64>,
}

impl XlsFormula {
    pub fn new() -> Self {
        let mut me = Self {
            links: Rc::new(RefCell::new(XlsLinks::new())),
            failure: String::new(),
            fnIndex: HashMap::new(),
            fnNames: Vec::new(),
            fnArgs: Vec::new(),
        };
        let ids: Vec<i64> = XlsFunctionTable::ids();
        me.fnNames = XlsFunctionTable::names().iter().map(|s| s.to_string()).collect();
        me.fnArgs = XlsFunctionTable::argCounts();
        let mut i: i64 = 0;
        while i < (ids.len() as i64) {
            me.fnIndex.insert(ids[i as usize], i);
            i += 1;
        }
        me
    }
    pub fn colName(col: i64) -> String {
        let mut out: String = String::new();
        let mut c: i64 = col + 1;
        while c > 0 {
            let m: i64 = (c - 1) % 26;
            out = format!("{}{}", char::from_u32((65 + m) as u32).unwrap_or('\0'), out);
            c = (c - 1) / 26;
        }
        out
    }
    pub fn errorText(code: i64) -> String {
        match code {
            0 => "#NULL!",
            7 => "#DIV/0!",
            15 => "#VALUE!",
            23 => "#REF!",
            29 => "#NAME?",
            36 => "#NUM!",
            43 => "#GETTING_DATA",
            _ => "#N/A",
        }
        .to_string()
    }
    /// A sheet name as a formula writes it: quoted unless it is a plain word.
    pub fn sheetRef(name: &str) -> String {
        let mut plain = !name.is_empty();
        let mut first = true;
        for ch in name.chars() {
            let ok = ch.is_ascii_alphabetic() || ch == '_' || ch == '.' || (ch as i64) > 127 || (!first && ch.is_ascii_digit());
            if !ok {
                plain = false;
            }
            first = false;
        }
        if plain {
            return name.to_string();
        }
        format!("'{}'", name.replace("'", "''"))
    }
    /// "$A$1" → "$A", and → "$1": the halves of an anchor.
    pub fn colPart(a: &str) -> String {
        let mut out = String::new();
        let mut first = true;
        for ch in a.chars() {
            if !(first && ch == '$') && (ch == '$' || ch.is_ascii_digit()) {
                return out;
            }
            out.push(ch);
            first = false;
        }
        out
    }
    pub fn rowPart(a: &str) -> String {
        let col = XlsFormula::colPart(a);
        match a.strip_prefix(&*col) {
            Some(rest) => rest.to_string(),
            None => String::new(),
        }
    }
    pub fn fnName(raw: &str) -> String {
        let mut n = raw.to_string();
        if let Some(rest) = n.strip_prefix("_xlfn.") {
            n = rest.to_string();
        }
        if let Some(rest) = n.strip_prefix("_xlws.") {
            n = rest.to_string();
        }
        n.to_uppercase()
    }
    pub fn quoteText(s: &str) -> String {
        format!("\"{}\"", s.replace("\"", "\"\""))
    }
    /// One cell anchor. `rel` reads the row and column as offsets from the
    /// base cell (tRefN / tAreaN in a shared formula).
    pub fn anchor(&self, row: i64, col_field: i64, base_row: i64, base_col: i64, rel: bool) -> String {
        let col: i64 = col_field % 16384;
        let col_rel: bool = (col_field & 16384) != 0;
        let row_rel: bool = (col_field & 32768) != 0;
        let mut r: i64 = row;
        let mut c: i64 = col % 256;
        if rel {
            if row_rel {
                if r >= 32768 {
                    r -= 65536;
                }
                r = (r + base_row) % 65536;
                if r < 0 {
                    r += 65536;
                }
            }
            if col_rel {
                if c >= 128 {
                    c -= 256;
                }
                c = (c + base_col) % 256;
                if c < 0 {
                    c += 256;
                }
            }
        }
        let mut out: String = String::new();
        if !col_rel {
            out = "$".to_string();
        }
        out = format!("{}{}", out, XlsFormula::colName(c));
        if !row_rel {
            out = format!("{}$", out);
        }
        format!("{}{}", out, r + 1)
    }
    pub fn area(&self, r0: i64, r1: i64, c0: i64, c1: i64, base_row: i64, base_col: i64, rel: bool) -> String {
        // Whole columns and whole rows are written the short way.
        if r0 == 0 && r1 == 65535 {
            let a: String = self.anchor(0, c0, base_row, base_col, rel);
            let b: String = self.anchor(0, c1, base_row, base_col, rel);
            return format!("{}:{}", XlsFormula::colPart(&a), XlsFormula::colPart(&b));
        }
        if c0 % 256 == 0 && c1 % 256 == 255 {
            let a2: String = self.anchor(r0, c0, base_row, base_col, rel);
            let b2: String = self.anchor(r1, c1, base_row, base_col, rel);
            return format!("{}:{}", XlsFormula::rowPart(&a2), XlsFormula::rowPart(&b2));
        }
        format!("{}:{}", self.anchor(r0,
            c0,
            base_row,
            base_col,
            rel
        ), self.anchor(r1, c1, base_row, base_col, rel))
    }
    /// The sheet prefix of a 3-D reference, or "#REF" when it points nowhere
    /// this reader can follow.
    pub fn sheetPrefix(&mut self, ixti: i64) -> String {
        if ixti < 0 || ixti >= (self.links.borrow().xtiBook.len() as i64) {
            self.failure = "a reference to a missing sheet link".to_string();
            return String::new();
        }
        let book: i64 = self.links.borrow().xtiBook[ixti as usize];
        if book < 0 || book >= (self.links.borrow().bookInternal.len() as i64) {
            self.failure = "a reference to a missing workbook link".to_string();
            return String::new();
        }
        if !(self.links.borrow().bookInternal[book as usize]) {
            self.failure = "references to other workbooks".to_string();
            return String::new();
        }
        let first: i64 = self.links.borrow().xtiFirst[ixti as usize];
        let last: i64 = self.links.borrow().xtiLast[ixti as usize];
        let n: i64 = self.links.borrow().sheets.len() as i64;
        if ((first < 0 || first >= n) || last < 0) || last >= n {
            return "#REF".to_string();
        }
        if first == last {
            return XlsFormula::sheetRef(&self.links.borrow().sheets[first as usize].clone());
        }
        format!("{}:{}", XlsFormula::sheetRef(&self.links.borrow().sheets[first as usize].clone()), XlsFormula::sheetRef(&self.links.borrow().sheets[last as usize].clone()))
    }
    pub fn externName(&mut self, ixti: i64, index: i64) -> String {
        if ixti < 0 || ixti >= (self.links.borrow().xtiBook.len() as i64) {
            self.failure = "a reference to a missing name link".to_string();
            return String::new();
        }
        let book: i64 = self.links.borrow().xtiBook[ixti as usize];
        if book < 0 || book >= (self.links.borrow().bookNames.len() as i64) {
            self.failure = "a reference to a missing workbook link".to_string();
            return String::new();
        }
        let names: Vec<String> = self.links.borrow()
            .bookNames[book as usize].clone();
        if index < 1 || index > (names.len() as i64) {
            self.failure = "a reference to a missing name".to_string();
            return String::new();
        }
        names[(index - 1) as usize].clone()
    }
    /// Pop `n` operands, oldest first.
    pub fn popN(&mut self, stack: &mut Vec<String>, n: i64) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let __len: i64 = stack.len() as i64;
        if n > __len {
            self.failure = "a malformed formula (an argument is missing)".to_string();
            return out;
        }
        let mut i: i64 = __len - n;
        while i < __len {
            out.push(stack[i as usize].clone());
            i += 1;
        }
        let mut k: i64 = 0;
        while k < n {
            stack.pop();
            k += 1;
        }
        out
    }
    pub fn pop1(&mut self, stack: &mut Vec<String>) -> String {
        let __len: i64 = stack.len() as i64;
        if __len == 0 {
            self.failure = "a malformed formula (an operand is missing)".to_string();
            return String::new();
        }
        let v: String = stack[(__len - 1) as usize].clone();
        stack.pop();
        v
    }
    /// The formula text without its leading "=", or "" with `failure` set.
    /// `shared` reads tRefN / tAreaN against (baseRow, baseCol).
    pub fn decode(&mut self, rgce: &[u8], rgcb: &[u8], base_row: i64, base_col: i64) -> String {
        self.failure = String::new();
        let mut stack: Vec<String> = Vec::new();
        let mut t: i64 = 0;
        let mut x: i64 = 0;
        let n: i64 = rgce.len() as i64;
        while t < n && self.failure.is_empty() {
            let ptg: i64 = rgce[t as usize] as i64;
            t += 1;
            let mut id: i64 = ptg;
            if (ptg & 96) != 0 {
                id = 32 | (ptg & 31);
            }
            if id >= 3 && id <= 17 {
                let ops: Vec<String> = vec!["+".to_string(), "-".to_string(), "*".to_string(), "/".to_string(), "^".to_string(), "&".to_string(), "<".to_string(), "<=".to_string(), "=".to_string(), ">=".to_string(), ">".to_string(), "<>".to_string(), " ".to_string(), ",".to_string(), ":".to_string()];
                let b: String = self.pop1(&mut stack);
                let a: String = self.pop1(&mut stack);
                stack.push(format!("{}{}{}", a, ops[(id - 3) as usize].clone(), b));
                continue;
            }
            if id == 18 {
                let v = self.pop1(&mut stack);
                stack.push(format!("+{}", v));
                continue;
            }
            if id == 19 {
                let v = self.pop1(&mut stack);
                stack.push(format!("-{}", v));
                continue;
            }
            if id == 20 {
                let v = self.pop1(&mut stack);
                stack.push(format!("{}%", v));
                continue;
            }
            if id == 21 {
                let v = self.pop1(&mut stack);
                stack.push(format!("({})", v));
                continue;
            }
            if id == 22 {
                stack.push(String::new().to_string());
                continue;
            }
            if id == 23 {
                let cch: i64 = SheetBytes::u8(rgce, t);
                let wide: bool = ((SheetBytes::u8(rgce, t + 1)) & 1) != 0;
                t += 2;
                stack.push(XlsFormula::quoteText(&SheetBytes::utf16(rgce, t, cch, wide)));
                if wide {
                    t += cch * 2;
                } else {
                    t += cch;
                }
                continue;
            }
            if id == 25 {
                // tAttr: a jump, a space, a choose table — or SUM with one
                // argument, which is the only kind that changes the text.
                let kind: i64 = SheetBytes::u8(rgce, t);
                t += 1;
                if (kind & 4) != 0 {
                    let cnt: i64 = SheetBytes::u16(rgce, t) + 1;
                    t = (t + 2) + cnt * 2;
                } else {
                    t += 2;
                }
                if (kind & 16) != 0 {
                    let v = self.pop1(&mut stack);
                    stack.push(format!("SUM({})", v));
                }
                continue;
            }
            if id == 28 {
                stack.push(XlsFormula::errorText(SheetBytes::u8(rgce, t)));
                t += 1;
                continue;
            }
            if id == 29 {
                if SheetBytes::u8(rgce, t) != 0 {
                    stack.push("TRUE".to_string().to_string());
                } else {
                    stack.push("FALSE".to_string().to_string());
                }
                t += 1;
                continue;
            }
            if id == 30 {
                stack.push(SheetBytes::u16(rgce, t).to_string());
                t += 2;
                continue;
            }
            if id == 31 {
                stack.push(SheetBytes::numText(SheetBytes::f64(rgce, t)));
                t += 8;
                continue;
            }
            if id == 32 {
                t += 7;
                let mut arr: Vec<i64> = vec![0];
                arr[0] = x;
                stack.push(self.arrayConstant(rgcb, &mut arr));
                x = arr[0];
                continue;
            }
            if id == 33 {
                let iftab: i64 = SheetBytes::u16(rgce, t);
                t += 2;
                if !self.fnIndex.contains_key(&iftab) {
                    self.failure = "an unknown function".to_string();
                    continue;
                }
                let fi: i64 = self.fnIndex.get(&iftab).cloned().unwrap();
                let argc: i64 = self.fnArgs[fi as usize];
                if argc < 0 {
                    self.failure = "an unknown function argument count".to_string();
                    continue;
                }
                let args: Vec<String> = self.popN(&mut stack, argc);
                stack.push(format!("{}({})", self.fnNames[fi as usize].clone(), args.join(",")));
                continue;
            }
            if id == 34 {
                let argn: i64 = SheetBytes::u8(rgce, t);
                let tab: i64 = SheetBytes::u16(rgce, t + 1);
                t += 3;
                if tab >= 32768 {
                    self.failure = "macro sheet commands".to_string();
                    continue;
                }
                let vargs: Vec<String> = self.popN(&mut stack, argn);
                if tab == 255 {
                    // A newer function: its name is the first operand.
                    if vargs.is_empty() {
                        self.failure = "a malformed function call".to_string();
                        continue;
                    }
                    let fname: String = XlsFormula::fnName(&vargs[0].clone());
                    let mut rest: Vec<String> = Vec::new();
                    let mut vi: i64 = 1;
                    while vi < (vargs.len() as i64) {
                        rest.push(vargs[vi as usize].clone());
                        vi += 1;
                    }
                    stack.push(format!("{}({})", fname, rest.join(",")));
                    continue;
                }
                if !self.fnIndex.contains_key(&tab) {
                    self.failure = "an unknown function".to_string();
                    continue;
                }
                let vfi: i64 = self.fnIndex.get(&tab).cloned().unwrap();
                stack.push(format!("{}({})", self.fnNames[vfi as usize].clone(), vargs.join(",")));
                continue;
            }
            if id == 35 {
                let ni: i64 = SheetBytes::i32(rgce, t);
                t += 4;
                if ni < 1 || ni > (self.links.borrow().names.len() as i64) {
                    self.failure = "a reference to a missing defined name".to_string();
                    continue;
                }
                stack.push(self.links.borrow().names[(ni - 1) as usize].clone());
                continue;
            }
            if id == 36 || id == 44 {
                let rr: i64 = SheetBytes::u16(rgce, t);
                let cc: i64 = SheetBytes::u16(rgce, t + 2);
                t += 4;
                stack.push(self.anchor(rr, cc, base_row, base_col, id == 44));
                continue;
            }
            if id == 37 || id == 45 {
                let ar0: i64 = SheetBytes::u16(rgce, t);
                let ar1: i64 = SheetBytes::u16(rgce, t + 2);
                let ac0: i64 = SheetBytes::u16(rgce, t + 4);
                let ac1: i64 = SheetBytes::u16(rgce, t + 6);
                t += 8;
                stack.push(self.area(ar0, ar1, ac0, ac1, base_row, base_col, id == 45));
                continue;
            }
            if id == 38 {
                // tMemArea: the area is in the operand that follows; its
                // rectangles ride in the extra data and say nothing new.
                t += 6;
                let cnt2: i64 = SheetBytes::u16(rgcb, x);
                x = (x + 2) + cnt2 * 8;
                continue;
            }
            if id == 39 || id == 40 {
                t += 6;
                continue;
            }
            if id == 41 {
                t += 2;
                continue;
            }
            if id == 42 {
                t += 4;
                stack.push("#REF!".to_string().to_string());
                continue;
            }
            if id == 43 {
                t += 8;
                stack.push("#REF!".to_string().to_string());
                continue;
            }
            if id == 57 {
                let xi: i64 = SheetBytes::u16(rgce, t);
                let xn: i64 = SheetBytes::i32(rgce, t + 2);
                t += 6;
                stack.push(self.externName(xi, xn));
                continue;
            }
            if id == 58 {
                let x3: i64 = SheetBytes::u16(rgce, t);
                let r3: i64 = SheetBytes::u16(rgce, t + 2);
                let c3: i64 = SheetBytes::u16(rgce, t + 4);
                t += 6;
                let pre: String = self.sheetPrefix(x3);
                if pre == "#REF" {
                    stack.push("#REF!".to_string().to_string());
                } else {
                    stack.push(format!("{}!{}", pre, self.anchor(r3, c3, 0, 0, false)));
                }
                continue;
            }
            if id == 59 {
                let x4: i64 = SheetBytes::u16(rgce, t);
                let q0: i64 = SheetBytes::u16(rgce, t + 2);
                let q1: i64 = SheetBytes::u16(rgce, t + 4);
                let k0: i64 = SheetBytes::u16(rgce, t + 6);
                let k1: i64 = SheetBytes::u16(rgce, t + 8);
                t += 10;
                let pre2: String = self.sheetPrefix(x4);
                if pre2 == "#REF" {
                    stack.push("#REF!".to_string().to_string());
                } else {
                    stack.push(format!("{}!{}", pre2, self.area(q0,
                        q1,
                        k0,
                        k1,
                        0,
                        0,
                        false
                    )));
                }
                continue;
            }
            if id == 60 {
                t += 6;
                stack.push("#REF!".to_string().to_string());
                continue;
            }
            if id == 61 {
                t += 10;
                stack.push("#REF!".to_string().to_string());
                continue;
            }
            if id == 1 {
                self.failure = "a nested shared formula".to_string();
                continue;
            }
            if id == 2 {
                self.failure = "what-if data tables".to_string();
                continue;
            }
            self.failure = "an unknown formula token".to_string();
        }
        if !self.failure.is_empty() {
            return String::new();
        }
        if t > n {
            self.failure = "a truncated formula".to_string();
            return String::new();
        }
        if (stack.len() as i64) != 1 {
            self.failure = "a malformed formula".to_string();
            return String::new();
        }
        stack[0].clone()
    }
    /// {1,2;3,4} out of the extra data. `at` is a one-element array so the
    /// position can come back to the caller.
    pub fn arrayConstant(&self, b: &[u8], at: &mut Vec<i64>) -> String {
        let mut p: i64 = at[0];
        let cols: i64 = SheetBytes::u8(b, p) + 1;
        let rows: i64 = SheetBytes::u16(b, p + 1) + 1;
        p += 3;
        let mut rows_out: Vec<String> = Vec::new();
        let mut r: i64 = 0;
        while r < rows {
            let mut cells: Vec<String> = Vec::new();
            let mut c: i64 = 0;
            while c < cols {
                let kind: i64 = SheetBytes::u8(b, p);
                p += 1;
                if kind == 1 {
                    cells.push(SheetBytes::numText(SheetBytes::f64(b, p)));
                    p += 8;
                } else {
                    if kind == 2 {
                        let cch: i64 = SheetBytes::u16(b, p);
                        let wide: bool = ((SheetBytes::u8(b, p + 2)) & 1) != 0;
                        p += 3;
                        cells.push(XlsFormula::quoteText(&SheetBytes::utf16(b, p, cch, wide)));
                        if wide {
                            p += cch * 2;
                        } else {
                            p += cch;
                        }
                    } else {
                        if kind == 4 {
                            if SheetBytes::u8(b, p) != 0 {
                                cells.push("TRUE".to_string().to_string());
                            } else {
                                cells.push("FALSE".to_string().to_string());
                            }
                        } else {
                            if kind == 16 {
                                cells.push(XlsFormula::errorText(SheetBytes::u8(b, p)));
                            } else {
                                cells.push(String::new().to_string());
                            }
                        }
                        p += 8;
                    }
                }
                c += 1;
            }
            rows_out.push(cells.join(","));
            r += 1;
        }
        at[0] = p;
        format!("{}{}{}", "{".to_string(), rows_out.join(";"), "}".to_string())
    }
}
