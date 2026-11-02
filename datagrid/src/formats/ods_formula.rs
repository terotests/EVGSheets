// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;

use crate::rgr::DgNative;
/// OdsFormula: OpenFormula (ODF 1.2 part 2) to Excel formula text and back.
///
/// The two languages share their functions and operators. They differ in how a
/// reference is spelled and in the argument separator:
///
///   OpenFormula                     Excel
///   of:=SUM([.A1:.B3];2)            SUM(A1:B3,2)
///   [$Sheet2.$C$4]                  Sheet2!$C$4
///   ['My Sheet'.A1:.B2]             'My Sheet'!A1:B2
///   [.A1]~[.C1]   (union)           A1,C1
///
/// Both directions walk the text once, copy string literals untouched and
/// rewrite only references and separators. Functions LibreOffice names under a
/// namespace (COM.MICROSOFT.…, _xlfn.…) lose the prefix on the way in.
#[derive(Clone)]
pub struct OdsFormula {
}

impl OdsFormula {
    pub fn new() -> Self {
        Self {
        }
    }
    /// --- in: OpenFormula → Excel ----------------------------------------------
    pub fn toExcel(raw: &str) -> String {
        let mut s: String = raw.to_string();
        if DgNative::index_of(&s, "of:") == 0 {
            s = DgNative::substr(&s, 3, DgNative::str_len(&s));
        }
        if DgNative::index_of(&s, "msoxl:") == 0 {
            s = DgNative::substr(&s, 6, DgNative::str_len(&s));
        }
        if !s.is_empty() {
            if DgNative::char_at(&s, 0) == 61 {
                s = DgNative::substr(&s, 1, DgNative::str_len(&s));
            }
        }
        let mut out: Vec<String> = Vec::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        let mut depth: i64 = 0;
        while i < n {
            let ch: i64 = DgNative::char_at(&s, i);
            if ch == 34 {
                let j: i64 = OdsFormula::stringEnd(&s, i);
                out.push(DgNative::substr(&s, i, j));
                i = j;
                continue;
            }
            if ch == 91 {
                // [ … ] — a reference; quotes inside may hold a "]".
                let mut k: i64 = i + 1;
                let mut in_q: bool = false;
                while k < n {
                    let c2: i64 = DgNative::char_at(&s, k);
                    if c2 == 39 {
                        in_q = !in_q;
                    }
                    if c2 == 93 && !in_q {
                        break;
                    }
                    k += 1;
                }
                out.push(OdsFormula::refToExcel(&DgNative::substr(&s, i + 1, k)));
                i = k + 1;
                continue;
            }
            if ch == 40 {
                depth += 1;
            }
            if ch == 41 {
                depth -= 1;
            }
            if ch == 59 {
                out.push(",".to_string().to_string());
                i += 1;
                continue;
            }
            if ch == 126 {
                out.push(",".to_string().to_string());
                i += 1;
                continue;
            }
            if ch == 33 {
                out.push(" ".to_string().to_string());
                i += 1;
                continue;
            }
            // A namespaced function name: drop the namespace.
            if OdsFormula::isNameStart(ch) {
                let mut e: i64 = i;
                while e < n && OdsFormula::isNameChar(DgNative::char_at(&s, e)) {
                    e += 1;
                }
                let mut word: String = DgNative::substr(&s, i, e);
                if e < n && DgNative::char_at(&s, e) == 40 {
                    word = OdsFormula::functionIn(&word);
                }
                out.push(word.clone());
                i = e;
                continue;
            }
            out.push(DgNative::substr(&s, i, i + 1));
            i += 1;
        }
        out.join("")
    }
    pub fn functionIn(name: &str) -> String {
        let up: String = name.to_uppercase();
        if DgNative::index_of(&up, "COM.MICROSOFT.") == 0 {
            return DgNative::substr(&name, 14, DgNative::str_len(&name));
        }
        if DgNative::index_of(&up, "_XLFN.") == 0 {
            return DgNative::substr(&name, 6, DgNative::str_len(&name));
        }
        name.to_string()
    }
    pub fn isNameStart(ch: i64) -> bool {
        ((ch >= 65 && ch <= 90) || (ch >= 97 && ch <= 122)) || ch == 95
    }
    pub fn isNameChar(ch: i64) -> bool {
        ((OdsFormula::isNameStart(ch) || (ch >= 48 && ch <= 57)) || ch == 46) || ch == 36
    }
    /// Index just past the string literal starting at `i` ("" is a quote).
    pub fn stringEnd(s: &str, i: i64) -> i64 {
        let n: i64 = DgNative::str_len(&s);
        let mut k: i64 = i + 1;
        while k < n {
            if DgNative::char_at(&s, k) == 34 {
                if k + 1 < n && DgNative::char_at(&s, k + 1) == 34 {
                    k += 2;
                    continue;
                }
                return k + 1;
            }
            k += 1;
        }
        n
    }
    /// "$Sheet1.A1:.B2" → "Sheet1!A1:B2"
    pub fn refToExcel(body: &str) -> String {
        let parts: Vec<String> = OdsFormula::splitOutsideQuotes(body, 58);
        let mut sheets: Vec<String> = Vec::new();
        let mut cells: Vec<String> = Vec::new();
        let mut i: i64 = 0;
        while i < (parts.len() as i64) {
            let p: String = parts[i as usize].clone();
            let dot: i64 = OdsFormula::lastDotOutsideQuotes(&p);
            let mut sheet: String = String::new();
            let mut cell: String = p.clone();
            if dot >= 0 {
                sheet = DgNative::substr(&p, 0, dot);
                cell = DgNative::substr(&p, dot + 1, DgNative::str_len(&p));
            }
            if !sheet.is_empty() {
                if DgNative::char_at(&sheet, 0) == 36 {
                    sheet = DgNative::substr(&sheet, 1, DgNative::str_len(&sheet));
                }
            }
            sheets.push(sheet.clone());
            cells.push(cell.clone());
            i += 1;
        }
        if cells.is_empty() {
            return "#REF!".to_string();
        }
        let first: String = sheets[0].clone();
        let mut prefix: String = String::new();
        if !first.is_empty() {
            prefix = format!("{}!", first);
            if (sheets.len() as i64) > 1 {
                let last: String = sheets[((sheets.len() as i64) - 1) as usize].clone();
                if !last.is_empty() && last != first {
                    prefix = format!("{}:{}!", first, last);
                }
            }
        }
        format!("{}{}", prefix, cells.join(":"))
    }
    pub fn splitOutsideQuotes(s: &str, sep: i64) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut in_q: bool = false;
        let mut start: i64 = 0;
        let mut i: i64 = 0;
        while i < DgNative::str_len(&s) {
            let ch: i64 = DgNative::char_at(&s, i);
            if ch == 39 {
                in_q = !in_q;
            }
            if ch == sep && !in_q {
                out.push(DgNative::substr(&s, start, i));
                start = i + 1;
            }
            i += 1;
        }
        out.push(DgNative::substr(&s, start, DgNative::str_len(&s)));
        out
    }
    pub fn lastDotOutsideQuotes(s: &str) -> i64 {
        let mut in_q: bool = false;
        let mut at: i64 = -1;
        let mut i: i64 = 0;
        while i < DgNative::str_len(&s) {
            let ch: i64 = DgNative::char_at(&s, i);
            if ch == 39 {
                in_q = !in_q;
            }
            if ch == 46 && !in_q {
                at = i;
            }
            i += 1;
        }
        at
    }
    /// --- out: Excel → OpenFormula ---------------------------------------------
    pub fn isCellRef(s: &str) -> bool {
        // $?LETTERS$?DIGITS, or a whole column ($?LETTERS) or row ($?DIGITS)
        // when `part` allows — callers check the forms they accept.
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        if i < n && DgNative::char_at(&s, i) == 36 {
            i += 1;
        }
        let ls: i64 = i;
        while i < n && (DgNative::char_at(&s, i) >= 65 && DgNative::char_at(&s, i) <= 90 || DgNative::char_at(&s, i) >= 97 && DgNative::char_at(&s, i) <= 122) {
            i += 1;
        }
        if i - ls < 1 || i - ls > 3 {
            return false;
        }
        if i < n && DgNative::char_at(&s, i) == 36 {
            i += 1;
        }
        let ds: i64 = i;
        while (i < n && DgNative::char_at(&s, i) >= 48) && DgNative::char_at(&s, i) <= 57 {
            i += 1;
        }
        i == n && i - ds >= 1
    }
    pub fn isColRef(s: &str) -> bool {
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        if i < n && DgNative::char_at(&s, i) == 36 {
            i += 1;
        }
        let ls: i64 = i;
        while i < n && (DgNative::char_at(&s, i) >= 65 && DgNative::char_at(&s, i) <= 90 || DgNative::char_at(&s, i) >= 97 && DgNative::char_at(&s, i) <= 122) {
            i += 1;
        }
        (i == n && i - ls >= 1) && i - ls <= 3
    }
    pub fn isRowRef(s: &str) -> bool {
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        if i < n && DgNative::char_at(&s, i) == 36 {
            i += 1;
        }
        let ds: i64 = i;
        while (i < n && DgNative::char_at(&s, i) >= 48) && DgNative::char_at(&s, i) <= 57 {
            i += 1;
        }
        i == n && i - ds >= 1
    }
    /// One reference, already split into an optional sheet and one or two
    /// anchors, in OpenFormula's brackets.
    pub fn refOut(sheet: &str, a: &str, b: &str) -> String {
        let mut pre: String = ".".to_string();
        if !sheet.is_empty() {
            pre = format!("${}.", sheet);
        }
        let mut a2: String = a.to_string();
        let mut b2: String = b.to_string();
        // Whole columns and rows have no short form in ODF 1.2; they are
        // written as the full extent.
        if (!b.is_empty() && OdsFormula::isColRef(a)) && OdsFormula::isColRef(b) {
            a2 = format!("{}1", a);
            b2 = format!("{}1048576", b);
        }
        if (!b.is_empty() && OdsFormula::isRowRef(a)) && OdsFormula::isRowRef(b) {
            a2 = format!("A{}", a);
            b2 = format!("XFD{}", b);
        }
        if b2.is_empty() {
            return format!("[{}{}]", pre, a2);
        }
        format!("[{}{}:.{}]", pre, a2, b2)
    }
    pub fn toOds(formula: &str) -> String {
        let s: String = formula.to_string();
        let mut out: Vec<String> = Vec::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        while i < n {
            let ch: i64 = DgNative::char_at(&s, i);
            if ch == 34 {
                let j: i64 = OdsFormula::stringEnd(&s, i);
                out.push(DgNative::substr(&s, i, j));
                i = j;
                continue;
            }
            if ch == 44 {
                out.push(";".to_string().to_string());
                i += 1;
                continue;
            }
            // 'Quoted sheet'!ref
            if ch == 39 {
                let mut k: i64 = i + 1;
                while k < n {
                    if DgNative::char_at(&s, k) == 39 {
                        if k + 1 < n && DgNative::char_at(&s, k + 1) == 39 {
                            k += 2;
                            continue;
                        }
                        break;
                    }
                    k += 1;
                }
                let sheet_q: String = DgNative::substr(&s, i, k + 1);
                if k + 1 < n && DgNative::char_at(&s, k + 1) == 33 {
                    let mut used: Vec<i64> = vec![0];
                    let r#ref: String = OdsFormula::refAt(&s, k + 2, &sheet_q, &mut used);
                    if DgNative::str_len(&r#ref) > 0 {
                        out.push(r#ref.clone());
                        i = used[0];
                        continue;
                    }
                }
                out.push(sheet_q.clone());
                i = k + 1;
                continue;
            }
            if (OdsFormula::isNameStart(ch) || ch == 36) || (ch >= 48 && ch <= 57) {
                // A word: a function, a name, a reference, a sheet prefix, a
                // number.
                let mut e: i64 = i;
                while e < n && (OdsFormula::isNameChar(DgNative::char_at(&s, e)) || DgNative::char_at(&s, e) == 58) {
                    e += 1;
                }
                let word: String = DgNative::substr(&s, i, e);
                if e < n && DgNative::char_at(&s, e) == 33 {
                    let mut used2: Vec<i64> = vec![0];
                    let ref2: String = OdsFormula::refAt(&s, e + 1, &word, &mut used2);
                    if !ref2.is_empty() {
                        out.push(ref2.clone());
                        i = used2[0];
                        continue;
                    }
                }
                if e < n && DgNative::char_at(&s, e) == 40 {
                    out.push(word.clone());
                    i = e;
                    continue;
                }
                let r: String = OdsFormula::plainRef(&word, "");
                if !r.is_empty() {
                    out.push(r.clone());
                } else {
                    out.push(word.clone());
                }
                i = e;
                continue;
            }
            out.push(DgNative::substr(&s, i, i + 1));
            i += 1;
        }
        format!("of:={}", out.join(""))
    }
    /// "A1", "A1:B2", "A:A", "1:3" (with `sheet` when it had one) as an ODF
    /// reference, or "" when the word is not a reference.
    pub fn plainRef(word: &str, sheet: &str) -> String {
        let colon: i64 = DgNative::index_of(&word, ":");
        if colon < 0 {
            if OdsFormula::isCellRef(word) {
                return OdsFormula::refOut(sheet, word, "");
            }
            return String::new();
        }
        let a: String = DgNative::substr(&word, 0, colon);
        let b: String = DgNative::substr(&word, colon + 1, DgNative::str_len(&word));
        if DgNative::index_of(&b, ":") >= 0 {
            return String::new();
        }
        if OdsFormula::isCellRef(&a) && OdsFormula::isCellRef(&b) {
            return OdsFormula::refOut(sheet, &a, &b);
        }
        if OdsFormula::isColRef(&a) && OdsFormula::isColRef(&b) {
            return OdsFormula::refOut(sheet, &a, &b);
        }
        if OdsFormula::isRowRef(&a) && OdsFormula::isRowRef(&b) {
            return OdsFormula::refOut(sheet, &a, &b);
        }
        String::new()
    }
    /// The reference after "Sheet!" at `at`; `used` gets the index after it.
    pub fn refAt(s: &str, at: i64, sheet: &str, used: &mut Vec<i64>) -> String {
        let mut e: i64 = at;
        let n: i64 = DgNative::str_len(&s);
        while e < n && (OdsFormula::isNameChar(DgNative::char_at(&s, e)) || DgNative::char_at(&s, e) == 58) {
            e += 1;
        }
        let word: String = DgNative::substr(&s, at, e);
        let r: String = OdsFormula::plainRef(&word, sheet);
        used[0] = e;
        r
    }
}
