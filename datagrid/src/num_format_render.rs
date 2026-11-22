// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;

/// NumFormatRender - renders a NUMBER through an Excel number format code the
/// way Excel does, working on the raw code so that quoted text, \x escapes,
/// [$€-407] currency, [Red] colours and [<=100] conditions stay apart from the
/// digit placeholders.
///
/// Covers: sections (pos;neg;zero;text) with conditions, 0 # ? placeholders
/// with literals between them (000-00-0000), thousands separators and comma
/// scaling (#,##0,  0.0,,"M"), percent, scientific with any digit counts and
/// engineering exponents (##0.0E+0), fractions (# ?/?, ?/100, # ??/16) and
/// General inside a section. Date and time codes are left to the date path
/// (`handles` answers false for them).
///
/// Tokens are kept in two parallel vectors: `kinds` and `texts`.
///   1 digit placeholder (0 # ?)   2 decimal point   3 comma   4 percent
///   5 exponent (E+ / E-)          6 slash           7 literal text
///   8 General                     9 @ (the text value)
#[derive(Clone)]
pub struct NumFormatRender {
}

impl NumFormatRender {
    pub fn new() -> Self {
        Self {
        }
    }
    /// Split on ';' outside quotes, escapes and brackets.
    pub fn sections(code: &str) -> Vec<String> {
        let mut parts: Vec<String> = Vec::new();
        let mut cur: String = String::new();
        let n: i64 = DgNative::str_len(code);
        let mut i: i64 = 0;
        let mut in_quote: bool = false;
        let mut in_bracket: bool = false;
        while i < n {
            let ch: i64 = DgNative::char_at(code, i);
            let s: String = DgNative::substr(code, i, i + 1);
            if !in_quote && !in_bracket && ch == 92 && i + 1 < n {
                cur = format!("{}{}{}", cur, s, DgNative::substr(code, i + 1, i + 2));
                i += 2;
                continue;
            }
            if ch == 34 && !in_bracket {
                in_quote = !in_quote;
            }
            if ch == 91 && !in_quote {
                in_bracket = true;
            }
            if ch == 93 && !in_quote {
                in_bracket = false;
            }
            if ch == 59 && !in_quote && !in_bracket {
                parts.push(cur.clone());
                cur = String::new();
            } else {
                cur = format!("{}{}", cur, s);
            }
            i += 1;
        }
        parts.push(cur);
        parts
    }
    pub fn lowerCode(ch: i64) -> i64 {
        if ch >= 65 && ch <= 90 {
            return ch + 32;
        }
        ch
    }
    /// Does the section hold date or time codes (outside quotes)?
    pub fn isDateSection(sec: &str) -> bool {
        let n: i64 = DgNative::str_len(sec);
        let mut i: i64 = 0;
        while i < n {
            let ch: i64 = DgNative::char_at(sec, i);
            if ch == 34 {
                i += 1;
                while i < n && DgNative::char_at(sec, i) != 34 {
                    i += 1;
                }
                i += 1;
                continue;
            }
            if ch == 92 || ch == 95 || ch == 42 {
                i += 2;
                continue;
            }
            if ch == 91 {
                let close: i64 = DgNative::index_of_from(sec, "]", i);
                if close < 0 {
                    return false;
                }
                let inner: String = DgNative::substr(sec, i + 1, close);
                let c0: i64 = NumFormatRender::lowerCode(DgNative::char_at(&inner, 0));
                if c0 == 104 || c0 == 109 || c0 == 115 {
                    return true;
                }
                i = close + 1;
                continue;
            }
            let lc: i64 = NumFormatRender::lowerCode(ch);
            if lc == 121 || lc == 109 || lc == 100 || lc == 104 || lc == 115 {
                return true;
            }
            if lc == 103 {
                // "General" is not a date code.
                let word: String = DgNative::substr(sec, i, i + 7);
                if NumFormatRender::lowerStr(&word) == "general" {
                    i += 7;
                    continue;
                }
                return true;
            }
            if lc == 97 {
                let w: String = NumFormatRender::lowerStr(&DgNative::substr(sec, i, i + 5));
                if w == "am/pm" || DgNative::substr(&w, 0, 3) == "a/p" {
                    return true;
                }
            }
            if lc == 101 {
                // E+ / E- is scientific; a bare e is the year in some locales.
                let nx: i64 = DgNative::char_at(sec, i + 1);
                if nx != 43 && nx != 45 {
                    return true;
                }
                i += 2;
                continue;
            }
            i += 1;
        }
        false
    }
    pub fn lowerStr(s: &str) -> String {
        let mut out: String = String::new();
        let n: i64 = DgNative::str_len(s);
        let mut i: i64 = 0;
        while i < n {
            out = format!("{}{}", out, DgNative::from_code(NumFormatRender::lowerCode(DgNative::char_at(s, i))));
            i += 1;
        }
        out
    }
    /// True when this renderer should draw numbers for `code`: anything but
    /// General alone, text-only and date/time codes.
    pub fn handles(code: &str) -> bool {
        if code.is_empty() || code == "General" || code == "@" {
            return false;
        }
        let secs: Vec<String> = NumFormatRender::sections(code);
        let mut i: i64 = 0;
        while i < (secs.len() as i64) && i < 3 {
            if NumFormatRender::isDateSection(&secs[i as usize].clone()) {
                return false;
            }
            i += 1;
        }
        true
    }
    /// The bracket condition of a section: op 0 none, 1 <, 2 <=, 3 >, 4 >=, 5 =, 6 <>.
    pub fn condOp(sec: &str) -> i64 {
        let n: i64 = DgNative::str_len(sec);
        let mut i: i64 = 0;
        while i < n {
            if DgNative::char_at(sec, i) == 91 {
                let close: i64 = DgNative::index_of_from(sec, "]", i);
                if close < 0 {
                    return 0;
                }
                let inner: String = DgNative::substr(sec, i + 1, close);
                let c0: i64 = DgNative::char_at(&inner, 0);
                let c1: i64 = DgNative::char_at(&inner, 1);
                if c0 == 60 {
                    if c1 == 61 {
                        return 2;
                    }
                    if c1 == 62 {
                        return 6;
                    }
                    return 1;
                }
                if c0 == 62 {
                    if c1 == 61 {
                        return 4;
                    }
                    return 3;
                }
                if c0 == 61 {
                    return 5;
                }
                i = close + 1;
                continue;
            }
            i += 1;
        }
        0
    }
    pub fn condValue(sec: &str) -> f64 {
        let open: i64 = DgNative::index_of(sec, "[");
        let close: i64 = DgNative::index_of_from(sec, "]", open);
        let inner: String = DgNative::substr(sec, open + 1, close);
        let mut k: i64 = 0;
        while k < DgNative::str_len(&inner) {
            let c: i64 = DgNative::char_at(&inner, k);
            if c != 60 && c != 62 && c != 61 {
                break;
            }
            k += 1;
        }
        let num: String = DgNative::substr(&inner, k, DgNative::str_len(&inner));
        let parsed: Option<f64> = num.trim().parse::<f64>().ok();
        if parsed.is_some() {
            return parsed.unwrap();
        }
        0.0
    }
    pub fn condHolds(op: i64, v: f64, c: f64) -> bool {
        if op == 1 {
            return v < c;
        }
        if op == 2 {
            return v <= c;
        }
        if op == 3 {
            return v > c;
        }
        if op == 4 {
            return v >= c;
        }
        if op == 5 {
            return v == c;
        }
        if op == 6 {
            return v != c;
        }
        false
    }
    /// Format a number. Excel picks a section, then draws the number's
    /// magnitude through it; a minus sign is added when the section chosen
    /// for a negative number is the first one.
    pub fn renderNumber(v: f64, code: &str) -> String {
        let secs: Vec<String> = NumFormatRender::sections(code);
        let count: i64 = secs.len() as i64;
        let op1: i64 = NumFormatRender::condOp(&secs[0].clone());
        let mut op2: i64 = 0;
        if count > 1 {
            op2 = NumFormatRender::condOp(&secs[1].clone());
        }
        let mut pick: i64 = 0;
        let mut signed: bool = v < 0.0;
        if op1 > 0 || op2 > 0 {
            // Conditional sections: the first whose test holds; otherwise the
            // next one without a test. The sign stays on the number.
            if op1 > 0 && NumFormatRender::condHolds(op1, v, NumFormatRender::condValue(&secs[0].clone())) {
                pick = 0;
            } else {
                if op2 > 0 && NumFormatRender::condHolds(op2, v, NumFormatRender::condValue(&secs[1].clone())) {
                    pick = 1;
                } else {
                    if op2 > 0 {
                        pick = 2;
                    } else {
                        pick = 1;
                    }
                    if op1 == 0 {
                        pick = 0;
                    }
                }
            }
            if pick >= count {
                return "#".to_string();
            }
            // With conditions the section draws the magnitude; any sign has
            // to be written into the section itself.
            signed = false;
        } else {
            if v < 0.0 && count >= 2 {
                pick = 1;
                signed = false;
            }
            if v == 0.0 && count >= 3 {
                pick = 2;
            }
        }
        let sec: String = secs[pick as usize].clone();
        let mut mag: f64 = v;
        if mag < 0.0 {
            mag = 0.0 - mag;
        }
        let body: String = NumFormatRender::renderSection(mag, &sec);
        if signed {
            // A value that rounds to zero still shows its sign in Excel only
            // when a digit is non-zero; keep it simple and show "-" unless
            // nothing but zeros came out.
            if NumFormatRender::hasNonZeroDigit(&body) {
                return format!("-{}", body);
            }
        }
        body
    }
    pub fn hasNonZeroDigit(s: &str) -> bool {
        let n: i64 = DgNative::str_len(s);
        let mut i: i64 = 0;
        while i < n {
            let c: i64 = DgNative::char_at(s, i);
            if c >= 49 && c <= 57 {
                return true;
            }
            i += 1;
        }
        false
    }
    /// Text through a format: the fourth section, or a lone section holding
    /// @. Otherwise the text as it is. `ok` reports whether the format said
    /// anything about text.
    pub fn renderText(raw: &str, code: &str) -> String {
        let secs: Vec<String> = NumFormatRender::sections(code);
        let mut sec: String = String::new();
        if secs.len() >= 4 {
            sec = secs[3].clone();
        } else {
            if secs.len() == 1 && DgNative::index_of(&secs[0].clone(), "@") >= 0 {
                sec = secs[0].clone();
            } else {
                return raw.to_string();
            }
        }
        let kinds: Rc<RefCell<Vec<i64>>> = Rc::new(RefCell::new(Vec::new()));
        let texts: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        NumFormatRender::tokenize(&sec, &kinds, &texts);
        let mut out: String = String::new();
        let n: i64 = kinds.borrow().len() as i64;
        let mut i: i64 = 0;
        while i < n {
            let k: i64 = kinds.borrow()[i as usize];
            let t: String = texts.borrow()[i as usize].clone();
            if k == 9 {
                out = format!("{}{}", out, raw);
            } else {
                if k == 7 || k == 2 || k == 3 || k == 4 || k == 6 {
                    out = format!("{}{}", out, t);
                }
            }
            i += 1;
        }
        out
    }
    pub fn tokenize(sec: &str, kinds: &Rc<RefCell<Vec<i64>>>, texts: &Rc<RefCell<Vec<String>>>) {
        let n: i64 = DgNative::str_len(sec);
        let mut i: i64 = 0;
        while i < n {
            let ch: i64 = DgNative::char_at(sec, i);
            let s: String = DgNative::substr(sec, i, i + 1);
            if ch == 34 {
                let mut j: i64 = i + 1;
                while j < n && DgNative::char_at(sec, j) != 34 {
                    j += 1;
                }
                kinds.borrow_mut().push(7);
                texts.borrow_mut().push(DgNative::substr(sec, i + 1, j));
                i = j + 1;
                continue;
            }
            if ch == 92 {
                kinds.borrow_mut().push(7);
                texts.borrow_mut().push(DgNative::substr(sec, i + 1, i + 2));
                i += 2;
                continue;
            }
            if ch == 95 {
                kinds.borrow_mut().push(7);
                texts.borrow_mut().push(" ".to_string());
                i += 2;
                continue;
            }
            if ch == 42 {
                // *x fills the cell with x; one x stands in for it.
                kinds.borrow_mut().push(7);
                texts.borrow_mut().push(DgNative::substr(sec, i + 1, i + 2));
                i += 2;
                continue;
            }
            if ch == 91 {
                let mut close: i64 = DgNative::index_of_from(sec, "]", i);
                if close < 0 {
                    close = n - 1;
                }
                let inner: String = DgNative::substr(sec, i + 1, close);
                if DgNative::char_at(&inner, 0) == 36 {
                    // [$€-407] → €
                    let dash: i64 = DgNative::index_of(&inner, "-");
                    let mut sym: String = DgNative::substr(&inner, 1, DgNative::str_len(&inner));
                    if dash > 0 {
                        sym = DgNative::substr(&inner, 1, dash);
                    }
                    kinds.borrow_mut().push(7);
                    texts.borrow_mut().push(sym);
                }
                i = close + 1;
                continue;
            }
            if ch == 48 || ch == 35 || ch == 63 {
                kinds.borrow_mut().push(1);
                texts.borrow_mut().push(s);
                i += 1;
                continue;
            }
            if ch == 46 {
                kinds.borrow_mut().push(2);
                texts.borrow_mut().push(s);
                i += 1;
                continue;
            }
            if ch == 44 {
                kinds.borrow_mut().push(3);
                texts.borrow_mut().push(s);
                i += 1;
                continue;
            }
            if ch == 37 {
                kinds.borrow_mut().push(4);
                texts.borrow_mut().push(s);
                i += 1;
                continue;
            }
            if (ch == 69 || ch == 101) && i + 1 < n {
                let nx: i64 = DgNative::char_at(sec, i + 1);
                if nx == 43 || nx == 45 {
                    kinds.borrow_mut().push(5);
                    texts.borrow_mut().push(DgNative::substr(sec, i, i + 2));
                    i += 2;
                    continue;
                }
            }
            if ch == 47 {
                kinds.borrow_mut().push(6);
                texts.borrow_mut().push(s);
                i += 1;
                continue;
            }
            if ch == 64 {
                kinds.borrow_mut().push(9);
                texts.borrow_mut().push(s);
                i += 1;
                continue;
            }
            if ch == 71 || ch == 103 {
                let word: String = NumFormatRender::lowerStr(&DgNative::substr(sec, i, i + 7));
                if word == "general" {
                    kinds.borrow_mut().push(8);
                    texts.borrow_mut().push("General".to_string());
                    i += 7;
                    continue;
                }
            }
            kinds.borrow_mut().push(7);
            texts.borrow_mut().push(s);
            i += 1;
        }
    }
    pub fn pow10(n: i64) -> f64 {
        let mut p: f64 = 1.0;
        let mut k: i64 = 0;
        if n >= 0 {
            while k < n {
                p *= 10.0;
                k += 1;
            }
            return p;
        }
        while k < 0 - n {
            p /= 10.0;
            k += 1;
        }
        p
    }
    /// The digits of `v` rounded to `decimals` places: [integer, fraction].
    pub fn digits(v: f64, decimals: i64) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let scale: f64 = NumFormatRender::pow10(decimals);
        let scaled: f64 = (v * scale + 0.5 + 1e-9).floor();
        if scaled >= 9.0e15 {
            // Too big for exact integer digits: no fraction.
            let w: f64 = (v + 0.5).floor();
            out.push(format!("{}", w as i64));
            let mut z: String = String::new();
            let mut k: i64 = 0;
            while k < decimals {
                z = format!("{}0", z);
                k += 1;
            }
            out.push(z);
            return out;
        }
        let si: i64 = scaled as i64;
        let mut s: String = si.to_string();
        while DgNative::str_len(&s) <= decimals {
            s = format!("0{}", s);
        }
        let cut: i64 = DgNative::str_len(&s) - decimals;
        out.push(DgNative::substr(&s, 0, cut));
        out.push(DgNative::substr(&s, cut, DgNative::str_len(&s)));
        out
    }
    /// Lay integer digits into the placeholder/literal tokens [a, b): digits fill
    /// from the right, extra digits go in front of the first placeholder, and
    /// leftover placeholders show 0, a space (?) or nothing (#).
    pub fn placeInteger(dig: &str, kinds: &Rc<RefCell<Vec<i64>>>, texts: &Rc<RefCell<Vec<String>>>, a: i64, b: i64, group: bool) -> String {
        let mut out: String = String::new();
        let mut di: i64 = DgNative::str_len(dig);
        let mut first_ph: i64 = -1;
        let mut t: i64 = a;
        while t < b {
            if kinds.borrow()[t as usize] == 1 {
                first_ph = t;
                break;
            }
            t += 1;
        }
        if first_ph < 0 {
            // .00 — the integer digits still show, in front
            let mut lits: String = String::new();
            let mut q: i64 = a;
            while q < b {
                if kinds.borrow()[q as usize] == 7 {
                    lits = format!("{}{}", lits, texts.borrow()[q as usize].clone());
                }
                q += 1;
            }
            return format!("{}{}", dig, lits);
        }
        let mut written: i64 = 0;
        let mut j: i64 = b - 1;
        while j >= a {
            let k: i64 = kinds.borrow()[j as usize];
            let tx: String = texts.borrow()[j as usize].clone();
            if k == 1 {
                let mut piece: String = String::new();
                if di > 0 {
                    piece = DgNative::substr(dig, di - 1, di);
                    di -= 1;
                    if j == first_ph && di > 0 {
                        // the rest of the digits ride on the first placeholder
                        let mut rest: String = DgNative::substr(dig, 0, di);
                        if group {
                            rest = NumFormatRender::groupHead(&rest, written + 1);
                        }
                        piece = format!("{}{}", rest, piece);
                        di = 0;
                    }
                } else {
                    if tx == "0" {
                        piece = "0".to_string();
                    }
                    if tx == "?" {
                        piece = " ".to_string();
                    }
                }
                if group && written > 0 && written % 3 == 0 && piece != "" && piece != " " {
                    piece = format!("{},", piece);
                }
                if piece != "" {
                    written += 1;
                }
                out = format!("{}{}", piece, out);
            } else {
                if k == 7 {
                    out = format!("{}{}", tx, out);
                }
            }
            j -= 1;
        }
        out
    }
    /// Commas for the digits that ride on the first placeholder, given how
    /// many digits are already to their right.
    pub fn groupHead(rest: &str, already: i64) -> String {
        let mut out: String = String::new();
        let n: i64 = DgNative::str_len(rest);
        let mut pos: i64 = already;
        let mut i: i64 = n - 1;
        while i >= 0 {
            let d: String = DgNative::substr(rest, i, i + 1);
            if pos > 0 && pos % 3 == 0 {
                out = format!("{},{}", d, out);
            } else {
                out = format!("{}{}", d, out);
            }
            pos += 1;
            i -= 1;
        }
        out
    }
    /// Fraction digits into the placeholders [a, b), left to right; trailing
    /// zeros under # vanish and under ? become spaces.
    pub fn placeFraction(dig: &str, kinds: &Rc<RefCell<Vec<i64>>>, texts: &Rc<RefCell<Vec<String>>>, a: i64, b: i64) -> String {
        let mut ph: Vec<String> = Vec::new();
        let mut t: i64 = a;
        while t < b {
            if kinds.borrow()[t as usize] == 1 {
                ph.push(texts.borrow()[t as usize].clone());
            }
            t += 1;
        }
        let np: i64 = ph.len() as i64;
        let mut shown: Vec<String> = Vec::new();
        let mut i: i64 = 0;
        while i < np {
            shown.push(DgNative::substr(dig, i, i + 1));
            i += 1;
        }
        let mut k: i64 = np - 1;
        while k >= 0 {
            if shown[k as usize] != "0" {
                break;
            }
            if ph[k as usize] == "#" {
                shown[k as usize] = String::new();
            } else {
                if ph[k as usize] == "?" {
                    shown[k as usize] = " ".to_string();
                } else {
                    break;
                }
            }
            k -= 1;
        }
        let mut out: String = String::new();
        let mut pi: i64 = 0;
        t = a;
        while t < b {
            let kk: i64 = kinds.borrow()[t as usize];
            if kk == 1 {
                out = format!("{}{}", out, shown[pi as usize].clone());
                pi += 1;
            } else {
                if kk == 7 {
                    out = format!("{}{}", out, texts.borrow()[t as usize].clone());
                }
            }
            t += 1;
        }
        out
    }
    pub fn countPh(kinds: &Rc<RefCell<Vec<i64>>>, a: i64, b: i64) -> i64 {
        let mut c: i64 = 0;
        let mut t: i64 = a;
        while t < b {
            if kinds.borrow()[t as usize] == 1 {
                c += 1;
            }
            t += 1;
        }
        c
    }
    pub fn literalsOf(kinds: &Rc<RefCell<Vec<i64>>>, texts: &Rc<RefCell<Vec<String>>>, a: i64, b: i64, v: f64) -> String {
        let mut out: String = String::new();
        let mut t: i64 = a;
        while t < b {
            let k: i64 = kinds.borrow()[t as usize];
            if k == 7 || k == 4 {
                out = format!("{}{}", out, texts.borrow()[t as usize].clone());
            }
            if k == 8 {
                out = format!("{}{}", out, NumFormatRender::general(v));
            }
            t += 1;
        }
        out
    }
    /// General: up to ten significant digits, no trailing zeros.
    pub fn general(v: f64) -> String {
        if v == 0.0 {
            return "0".to_string();
        }
        if v >= 1e11 || v < 1e-9 {
            return NumFormatRender::scientific(v, 1, 5, true);
        }
        let mut int_digits: i64 = 0;
        let mut t: f64 = v;
        while t >= 1.0 {
            t /= 10.0;
            int_digits += 1;
        }
        let mut decimals: i64 = 10 - int_digits;
        if int_digits == 0 {
            decimals = 9;
        }
        if decimals < 0 {
            decimals = 0;
        }
        let d: Vec<String> = NumFormatRender::digits(v, decimals);
        let mut frac: String = d[1].clone();
        while DgNative::str_len(&frac) > 0 && DgNative::char_at(&frac, DgNative::str_len(&frac) - 1) == 48 {
            frac = DgNative::substr(&frac, 0, DgNative::str_len(&frac) - 1);
        }
        if frac.is_empty() {
            return d[0].clone();
        }
        format!("{}.{}", d[0].clone(), frac)
    }
    /// d.dddE+xx with `decimals` places, trailing zeros trimmed when asked.
    pub fn scientific(v: f64, min_exp: i64, decimals: i64, trim: bool) -> String {
        let mut exp: i64 = 0;
        let mut m: f64 = v;
        if m != 0.0 {
            while m >= 10.0 {
                m /= 10.0;
                exp += 1;
            }
            while m < 1.0 {
                m *= 10.0;
                exp -= 1;
            }
        }
        let mut d: Vec<String> = NumFormatRender::digits(m, decimals);
        if d[0] == "10" {
            exp += 1;
            d = NumFormatRender::digits(m / 10.0, decimals);
        }
        let mut frac: String = d[1].clone();
        if trim {
            while DgNative::str_len(&frac) > 0 && DgNative::char_at(&frac, DgNative::str_len(&frac) - 1) == 48 {
                frac = DgNative::substr(&frac, 0, DgNative::str_len(&frac) - 1);
            }
        }
        let mut mant: String = d[0].clone();
        if !frac.is_empty() {
            mant = format!("{}.{}", mant, frac);
        }
        let mut ae: i64 = exp;
        let mut sign: String = "+".to_string();
        if exp < 0 {
            ae = 0 - exp;
            sign = "-".to_string();
        }
        let mut es: String = ae.to_string();
        while DgNative::str_len(&es) < min_exp {
            es = format!("0{}", es);
        }
        format!("{}E{}{}", mant, sign, es)
    }
    /// Draw a non-negative number through one section.
    pub fn renderSection(v0: f64, sec: &str) -> String {
        let kinds: Rc<RefCell<Vec<i64>>> = Rc::new(RefCell::new(Vec::new()));
        let texts: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        NumFormatRender::tokenize(sec, &kinds, &texts);
        let n: i64 = kinds.borrow().len() as i64;
        let mut v: f64 = v0;
        // Where the number lives: first and last placeholder / point.
        let mut first: i64 = -1;
        let mut last: i64 = -1;
        let mut dot: i64 = -1;
        let mut exp_at: i64 = -1;
        let mut slash: i64 = -1;
        let mut i: i64 = 0;
        while i < n {
            let k: i64 = kinds.borrow()[i as usize];
            if k == 4 {
                v *= 100.0;
            }
            if k == 1 || (k == 2 && exp_at < 0 && slash < 0) {
                if first < 0 {
                    first = i;
                }
                last = i;
            }
            if k == 2 && dot < 0 && exp_at < 0 && slash < 0 {
                dot = i;
            }
            if k == 5 && exp_at < 0 {
                exp_at = i;
            }
            if k == 6 && slash < 0 {
                slash = i;
            }
            i += 1;
        }
        if first < 0 {
            // "x"@ is a text format: a number shows as General.
            let mut has_at: bool = false;
            let mut q: i64 = 0;
            while q < n {
                if kinds.borrow()[q as usize] == 9 {
                    has_at = true;
                }
                q += 1;
            }
            if has_at {
                return NumFormatRender::general(v);
            }
            // No digits at all: only literals (and General).
            return NumFormatRender::literalsOf(&kinds, &texts, 0, n, v);
        }
        // Commas: scaling when nothing but commas/points/end follow up to the
        // next placeholder; grouping when between integer placeholders.
        let mut group: bool = false;
        let mut int_end: i64 = last + 1;
        if dot >= 0 {
            int_end = dot;
        } else {
            if exp_at >= 0 {
                int_end = exp_at;
            }
        }
        i = first;
        while i < n {
            if kinds.borrow()[i as usize] == 3 {
                let mut j: i64 = i + 1;
                while j < n && kinds.borrow()[j as usize] == 3 {
                    j += 1;
                }
                let next_is_ph: bool = j < n && kinds.borrow()[j as usize] == 1 && j < int_end;
                if next_is_ph {
                    group = true;
                } else {
                    // a run of commas after the digits divides by 1000 each
                    let mut r: i64 = i;
                    while r < j {
                        v /= 1000.0;
                        r += 1;
                    }
                }
                i = j;
                continue;
            }
            i += 1;
        }
        let prefix: String = NumFormatRender::literalsOf(&kinds, &texts, 0, first, v);
        let suffix_from: i64 = NumFormatRender::lastNumberToken(&kinds, &texts, last, exp_at, slash, n);
        let suffix: String = NumFormatRender::literalsOf(&kinds, &texts, suffix_from + 1, n, v);
        if slash >= 0 {
            return format!("{}{}{}", prefix, NumFormatRender::fraction(v, &kinds, &texts, first, slash, suffix_from), suffix);
        }
        if exp_at >= 0 {
            return format!("{}{}{}", prefix, NumFormatRender::sciSection(v, &kinds, &texts, first, dot, exp_at, suffix_from), suffix);
        }
        let ip_end: i64 = if dot >= 0 { dot } else { last + 1 };
        let decimals: i64 = if dot >= 0 { NumFormatRender::countPh(&kinds, dot + 1, last + 1) } else { 0 };
        let d: Vec<String> = NumFormatRender::digits(v, decimals);
        let mut idig: String = d[0].clone();
        if idig == "0" {
            idig = String::new();
        }
        let mut out: String = NumFormatRender::placeInteger(&idig, &kinds, &texts, first, ip_end, group);
        if dot >= 0 {
            out = format!("{}.{}", out, NumFormatRender::placeFraction(&d[1].clone(), &kinds, &texts, dot + 1, last + 1));
        }
        format!("{}{}{}", prefix, out, suffix)
    }
    /// The index of the last token that belongs to the number (exponent digits
    /// or the denominator included).
    pub fn lastNumberToken(kinds: &Rc<RefCell<Vec<i64>>>, texts: &Rc<RefCell<Vec<String>>>, last: i64, exp_at: i64, slash: i64, n: i64) -> i64 {
        if exp_at < 0 && slash < 0 {
            return last;
        }
        let mut start: i64 = slash;
        if exp_at >= 0 {
            start = exp_at;
        }
        let mut end: i64 = start;
        let mut j: i64 = start + 1;
        let fixed: bool = slash >= 0 && exp_at < 0 && NumFormatRender::isFixedDen(kinds, texts, slash + 1, n);
        while j < n {
            let k: i64 = kinds.borrow()[j as usize];
            if k == 1 {
                if fixed && texts.borrow()[j as usize] != "0" {
                    break;
                }
                end = j;
            } else {
                if fixed && k == 7 && NumFormatRender::isDigitText(&texts.borrow()[j as usize].clone()) {
                    end = j;
                } else {
                    break;
                }
            }
            j += 1;
        }
        end
    }
    pub fn isDigitText(t: &str) -> bool {
        let c: i64 = DgNative::char_at(t, 0);
        DgNative::str_len(t) == 1 && c >= 48 && c <= 57
    }
    /// ?/100, # ??/16: the denominator starts with a literal digit 1-9.
    pub fn isFixedDen(kinds: &Rc<RefCell<Vec<i64>>>, texts: &Rc<RefCell<Vec<String>>>, at: i64, n: i64) -> bool {
        if at >= n {
            return false;
        }
        if kinds.borrow()[at as usize] != 7 {
            return false;
        }
        let t: String = texts.borrow()[at as usize].clone();
        let c: i64 = DgNative::char_at(&t, 0);
        DgNative::str_len(&t) == 1 && c >= 49 && c <= 57
    }
    pub fn sciSection(v: f64, kinds: &Rc<RefCell<Vec<i64>>>, texts: &Rc<RefCell<Vec<String>>>, first: i64, dot: i64, exp_at: i64, end: i64) -> String {
        let ip_end: i64 = if dot >= 0 { dot } else { exp_at };
        let n_int: i64 = NumFormatRender::countPh(kinds, first, ip_end);
        let n_frac: i64 = if dot >= 0 { NumFormatRender::countPh(kinds, dot + 1, exp_at) } else { 0 };
        let n_exp: i64 = NumFormatRender::countPh(kinds, exp_at + 1, end + 1);
        let mut exp: i64 = 0;
        if v != 0.0 {
            let mut m: f64 = v;
            while m >= 10.0 {
                m /= 10.0;
                exp += 1;
            }
            while m < 1.0 {
                m *= 10.0;
                exp -= 1;
            }
            if n_int > 1 {
                // engineering: the exponent is a multiple of the integer width
                let mut q: i64 = exp / n_int;
                if exp < 0 && exp % n_int != 0 {
                    q -= 1;
                }
                exp = q * n_int;
            }
        }
        let mut mant: f64 = v / NumFormatRender::pow10(exp);
        let mut d: Vec<String> = NumFormatRender::digits(mant, n_frac);
        let limit: i64 = if n_int > 1 { n_int } else { 1 };
        if DgNative::str_len(&d[0].clone()) > limit && v != 0.0 {
            exp += limit;
            mant = v / NumFormatRender::pow10(exp);
            d = NumFormatRender::digits(mant, n_frac);
        }
        let mut idig: String = d[0].clone();
        if idig == "0" {
            idig = String::new();
        }
        let mut out: String = NumFormatRender::placeInteger(&idig, kinds, texts, first, ip_end, false);
        if dot >= 0 {
            out = format!("{}.{}", out, NumFormatRender::placeFraction(&d[1].clone(), kinds, texts, dot + 1, exp_at));
        }
        let et: String = texts.borrow()[exp_at as usize].clone();
        let mut ae: i64 = exp;
        let mut sign: String = String::new();
        if exp < 0 {
            ae = 0 - exp;
            sign = "-".to_string();
        } else {
            if DgNative::char_at(&et, 1) == 43 {
                sign = "+".to_string();
            }
        }
        let mut es: String = ae.to_string();
        while DgNative::str_len(&es) < n_exp {
            es = format!("0{}", es);
        }
        format!("{}{}{}{}", out, DgNative::substr(&et, 0, 1), sign, es)
    }
    /// # ?/?, ?/100, # ??/16, 0/0: an optional whole part, then a numerator
    /// over a denominator that is either fixed or the closest fit with as
    /// many digits as it has placeholders.
    pub fn fraction(v: f64, kinds: &Rc<RefCell<Vec<i64>>>, texts: &Rc<RefCell<Vec<String>>>, first: i64, slash: i64, end: i64) -> String {
        // Numerator placeholders: the run right before the slash.
        let mut num_start: i64 = slash;
        while num_start - 1 >= first && kinds.borrow()[(num_start - 1) as usize] == 1 {
            num_start -= 1;
        }
        // A whole part exists when placeholders come before the numerator.
        let has_whole: bool = NumFormatRender::countPh(kinds, first, num_start) > 0;
        // Denominator: placeholders, or fixed digits.
        let mut fixed: String = String::new();
        let mut den_ph: Vec<String> = Vec::new();
        let fixed_mode: bool = NumFormatRender::isFixedDen(kinds, texts, slash + 1, end + 1);
        let mut j: i64 = slash + 1;
        while j <= end {
            let t: String = texts.borrow()[j as usize].clone();
            if fixed_mode {
                fixed = format!("{}{}", fixed, t);
            } else {
                if kinds.borrow()[j as usize] == 1 {
                    den_ph.push(t);
                }
            }
            j += 1;
        }
        // Digit placeholders "0" right after the slash, like 0/0, are
        // placeholders; literal digits 1-9 make a fixed denominator.
        let mut whole: f64 = 0.0;
        let mut f: f64 = v;
        if has_whole {
            whole = v.floor();
            f = v - whole;
        }
        let mut best_n: i64 = 0;
        let mut best_d: i64 = 1;
        if !fixed.is_empty() {
            let dd: i64 = fixed.parse::<i64>().unwrap_or(1);
            best_d = dd;
            best_n = (f * (dd as f64) + 0.5).floor() as i64;
        } else {
            let mut max_d: i64 = 1;
            let mut k2: i64 = 0;
            while k2 < (den_ph.len() as i64) {
                max_d *= 10;
                k2 += 1;
            }
            max_d -= 1;
            if max_d < 1 {
                max_d = 1;
            }
            let mut best_err: f64 = 2.0;
            let mut dd: i64 = 1;
            while dd <= max_d {
                let nn: i64 = (f * (dd as f64) + 0.5).floor() as i64;
                let mut err: f64 = f - (nn as f64) / (dd as f64);
                if err < 0.0 {
                    err = 0.0 - err;
                }
                if err < best_err - 1e-12 {
                    best_err = err;
                    best_n = nn;
                    best_d = dd;
                }
                dd += 1;
            }
        }
        if has_whole && best_n == best_d && fixed.is_empty() {
            whole += 1.0;
            best_n = 0;
        }
        let mut out: String = String::new();
        if has_whole {
            let mut wd: String = format!("{}", whole as i64);
            if whole == 0.0 {
                wd = String::new();
                if best_n == 0 {
                    wd = "0".to_string();
                }
            }
            out = NumFormatRender::placeInteger(&wd, kinds, texts, first, num_start, false);
            if best_n == 0 {
                // no fraction part: spaces keep the width
                let width: i64 = (slash - num_start) + 1 + (end - slash);
                let mut sp: String = String::new();
                let mut q: i64 = 0;
                while q < width {
                    sp = format!("{} ", sp);
                    q += 1;
                }
                return format!("{}{}", out, sp);
            }
        }
        let nd: String = format!("{}", best_n);
        out = format!("{}{}", out, NumFormatRender::placeInteger(&nd, kinds, texts, num_start, slash, false));
        out = format!("{}/", out);
        if !fixed.is_empty() {
            return format!("{}{}", out, fixed);
        }
        let mut ds: String = format!("{}", best_d);
        let mut pad: i64 = (den_ph.len() as i64) - DgNative::str_len(&ds);
        while pad > 0 {
            ds = format!("{} ", ds);
            pad -= 1;
        }
        format!("{}{}", out, ds)
    }
}
