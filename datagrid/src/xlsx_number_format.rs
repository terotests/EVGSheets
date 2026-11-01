// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;

/// XlsxNumberFormat - reusable Spreadsheet number-format engine.
///
/// Not SpreadsheetML-specific: raw numeric/string → displayText via a format
/// code (Excel-like). Used by XLSX styles, formula results, and DataGrid paint.
///
///   format code  →  compile (sections)  →  format(value)  →  displayText
///
/// Supported this milestone (practical subset, not every Excel edge case):
///   General, 0, 0.00, #,##0, #,##0.00, 0%, 0.00%, $#,##0.00, €#,##0.00,
///   0.00E+00, yyyy-mm-dd, dd.mm.yyyy, mm/dd/yyyy, hh:mm, hh:mm:ss, @
///   and pos;neg;zero section triples when present.
#[derive(Clone)]
pub struct CompiledNumberFormat {
    pub rawCode: String,
    pub pos: String,
    pub neg: String,
    pub zero: String,
    pub text: String,
    pub sectionCount: i64,
}

impl CompiledNumberFormat {
    pub fn new() -> Self {
        Self {
            rawCode: "General".to_string(),
            pos: "General".to_string(),
            neg: String::new(),
            zero: String::new(),
            text: "@".to_string(),
            sectionCount: 1,
        }
    }
}

#[derive(Clone)]
pub struct XlsxNumberFormat {
}

impl XlsxNumberFormat {
    pub fn new() -> Self {
        Self {
        }
    }
    /// Excel quotes literal text in a format code: "$"#,##0.00 shows a $, not a
    /// quoted $. Also handles \x escapes, _x (a space the width of x), *x (fill)
    /// and [Red] / [$-409] decorations. Elapsed-time brackets [h] [m] [s] stay.
    /// Characters that are part of a numeric pattern rather than literal text.
    pub fn isPatternChar(ch: i64) -> bool {
        if ch == 35 || ch == 48 {
            return true;
        }
        if ch == 46 || ch == 44 {
            return true;
        }
        if ch == 63 || ch == 37 {
            return true;
        }
        if ch == 69 || ch == 101 {
            return true;
        }
        if ch == 43 || ch == 45 {
            return true;
        }
        false
    }
    pub fn unquotePattern(pat: &str) -> String {
        let mut out: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&pat);
        while i < n {
            let ch: i64 = DgNative::char_at(&pat, i);
            if ch == 34 {
                i += 1;
                while i < n {
                    let q: i64 = DgNative::char_at(&pat, i);
                    if q == 34 {
                        i += 1;
                        break;
                    }
                    out = format!("{}{}", out, DgNative::substr(&pat, i, i + 1));
                    i += 1;
                }
            } else {
                if ch == 92 {
                    i += 1;
                    if i < n {
                        out = format!("{}{}", out, DgNative::substr(&pat, i, i + 1));
                        i += 1;
                    }
                } else {
                    if ch == 95 {
                        // _x reserves the width of x; a space is close enough.
                        out = format!("{} ", out);
                        i += 2;
                    } else {
                        if ch == 42 {
                            // *x repeats x to fill the cell; drop it.
                            i += 2;
                        } else {
                            if ch == 91 {
                                let mut close: i64 = i + 1;
                                while close < n {
                                    if DgNative::char_at(&pat, close) == 93 {
                                        break;
                                    }
                                    close += 1;
                                }
                                let inner: String = DgNative::substr(&pat, i + 1, close);
                                // keep elapsed-time codes, drop colour / locale
                                let mut keep: bool = false;
                                if inner == "h" || inner == "hh" {
                                    keep = true;
                                }
                                if inner == "m" || inner == "mm" {
                                    keep = true;
                                }
                                if inner == "s" || inner == "ss" {
                                    keep = true;
                                }
                                if keep {
                                    out = format!("{}[{}]", out, inner);
                                }
                                i = close + 1;
                            } else {
                                out = format!("{}{}", out, DgNative::substr(&pat, i, i + 1));
                                i += 1;
                            }
                        }
                    }
                }
            }
        }
        out
    }
    pub fn compile(code: String) -> Rc<RefCell<CompiledNumberFormat>> {
        let c: Rc<RefCell<CompiledNumberFormat>> = Rc::new(RefCell::new(CompiledNumberFormat::new()));
        if code.is_empty() {
            c.borrow_mut().rawCode = "General".to_string();
            c.borrow_mut().pos = "General".to_string();
            return c;
        }
        c.borrow_mut().rawCode = code.clone();
        // Split on ';' — but not one inside a quoted literal or a [ ] group.
        let mut parts: Vec<String> = Vec::new();
        let mut cur: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&code);
        let mut in_quote: bool = false;
        let mut in_bracket: bool = false;
        while i < n {
            let ch: i64 = DgNative::char_at(&code, i);
            if ch == 34 {
                in_quote = !in_quote;
                cur = format!("{}{}", cur, DgNative::substr(&code, i, i + 1));
            } else {
                if ch == 91 {
                    in_bracket = true;
                    cur = format!("{}{}", cur, DgNative::substr(&code, i, i + 1));
                } else {
                    if ch == 93 {
                        in_bracket = false;
                        cur = format!("{}{}", cur, DgNative::substr(&code, i, i + 1));
                    } else {
                        if (ch == 59 && !in_quote) && !in_bracket {
                            parts.push(cur.clone());
                            cur = String::new();
                        } else {
                            cur = format!("{}{}", cur, DgNative::substr(&code, i, i + 1));
                        }
                    }
                }
            }
            i += 1;
        }
        parts.push(cur);
        // Literals become plain text once the sections are separated.
        let mut unq: Vec<String> = Vec::new();
        let mut ui: i64 = 0;
        while ui < (parts.len() as i64) {
            unq.push(XlsxNumberFormat::unquotePattern(&parts[ui as usize].clone()));
            ui += 1;
        }
        parts.clear();
        let mut uj: i64 = 0;
        while uj < (unq.len() as i64) {
            parts.push(unq[uj as usize].clone());
            uj += 1;
        }
        c.borrow_mut().sectionCount = parts.len() as i64;
        c.borrow_mut().pos = parts[0].clone();
        if c.borrow().sectionCount >= 2 {
            c.borrow_mut().neg = parts[1].clone();
        }
        if c.borrow().sectionCount >= 3 {
            c.borrow_mut().zero = parts[2].clone();
        }
        if c.borrow().sectionCount >= 4 {
            c.borrow_mut().text = parts[3].clone();
        }
        c
    }
    /// Is the WHOLE string a number? Leading and trailing spaces are allowed,
    /// because a spreadsheet's own writers pad; anything else is text.
    pub fn wholeNumber(text: &str) -> bool {
        let n: i64 = DgNative::str_len(&text);
        let mut start: i64 = 0;
        while start < n {
            if DgNative::char_at(&text, start) != 32 {
                break;
            }
            start += 1;
        }
        let mut stop: i64 = n;
        while stop > start {
            if DgNative::char_at(&text, stop - 1) != 32 {
                break;
            }
            stop -= 1;
        }
        if start >= stop {
            return false;
        }
        let mut digits: i64 = 0;
        let mut dots: i64 = 0;
        let mut exps: i64 = 0;
        let mut i: i64 = start;
        while i < stop {
            let ch: i64 = DgNative::char_at(&text, i);
            if ch >= 48 && ch <= 57 {
                digits += 1;
            } else {
                if ch == 46 {
                    dots += 1;
                    if dots > 1 {
                        return false;
                    }
                } else {
                    if ch == 43 || ch == 45 {
                        if i > start {
                            let prev: i64 = DgNative::char_at(&text, i - 1);
                            if prev != 101 && prev != 69 {
                                return false;
                            }
                        }
                    } else {
                        if ch == 101 || ch == 69 {
                            exps += 1;
                            if exps > 1 {
                                return false;
                            }
                            if digits == 0 {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                }
            }
            i += 1;
        }
        digits > 0
    }
    pub fn format(raw: &str, num_fmt: &str) -> String {
        let c: Rc<RefCell<CompiledNumberFormat>> = XlsxNumberFormat::compile(num_fmt.to_string());
        XlsxNumberFormat::formatCompiled(raw, &c)
    }
    pub fn formatCompiled(raw: &str, c: &Rc<RefCell<CompiledNumberFormat>>) -> String {
        if raw.is_empty() {
            return String::new();
        }
        if c.borrow().pos == "@" {
            return raw.to_string();
        }
        if c.borrow().rawCode == "@" {
            return raw.to_string();
        }
        // `to_double` is a FRONT parser: it reads "45 up" as 45 and
        // "2024-01-01" as 2024. A cell holding text that merely starts with
        // digits is not a number, and formatting it as one silently ate the
        // rest of the string on screen. So the whole string has to look like a
        // number before it is treated as one.
        let numeric: bool = XlsxNumberFormat::wholeNumber(raw);
        let d: Option<f64> = raw.parse::<f64>().ok();
        // Nested `if` (not `&& d`): C++ optional doubles are value types, so a
        // truthiness check in an expression becomes `d != NULL` and fails to
        // compile. `!null?` / a bare `if d` emit `.hasValue` instead.
        if numeric {
            if d.is_some() {
                let v: f64 = d.unwrap();
                let mut pat: String = c.borrow().pos.clone();
                if v < 0.0 && !c.borrow().neg.is_empty() {
                    pat = c.borrow().neg.clone();
                    // Excel neg section usually already has '-', or is absolute
                    if DgNative::char_at(&pat, 0) == 45 {
                        // keep sign in pattern; format abs
                        return XlsxNumberFormat::applyPattern(0.0 - v, &pat, true);
                    }
                    return XlsxNumberFormat::applyPattern(0.0 - v, &pat, false);
                }
                if v == 0.0 && !c.borrow().zero.is_empty() {
                    if c.borrow().zero == "-" {
                        return "-".to_string();
                    }
                    // Accounting's zero is a dash: no digit placeholders, and a
                    // ? holds a digit's width as a space.
                    if DgNative::index_of(&c.borrow().zero, "0") < 0 && DgNative::index_of(&c.borrow().zero, "#") < 0 {
                        return (c.borrow().zero.split("?").map(|s| s.to_string()).collect::<Vec<String>>())
                            .join(" ")
                            .clone();
                    }
                    return XlsxNumberFormat::applyPattern(0.0, &c.borrow().zero.clone(), false);
                }
                if v < 0.0 {
                    return XlsxNumberFormat::applyPattern(0.0 - v, &pat, true);
                }
                return XlsxNumberFormat::applyPattern(v, &pat, false);
            }
        }
        // non-numeric
        if !c.borrow().text.is_empty() {
            if c.borrow().text == "@" {
                return raw.to_string();
            }
        }
        if c.borrow().pos == "General" || c.borrow().rawCode == "General" {
            return raw.to_string();
        }
        raw.to_string()
    }
    pub fn applyPattern(v: f64, pat: &str, mut force_minus: bool) -> String {
        if pat.is_empty() {
            return v.to_string();
        }
        if pat == "General" {
            return XlsxNumberFormat::general(v, force_minus);
        }
        if pat == "@" {
            return v.to_string();
        }
        // strip leading '-' from pattern (already handled)
        let mut p: String = pat.to_string();
        if !p.is_empty() {
            if DgNative::char_at(&p, 0) == 45 {
                p = DgNative::substr(&p, 1, DgNative::str_len(&p));
                force_minus = true;
            }
        }
        // date / time detection
        if XlsxNumberFormat::isDatePattern(&p) {
            return XlsxNumberFormat::formatDate(v, &p, force_minus);
        }
        if XlsxNumberFormat::isTimePattern(&p) {
            return XlsxNumberFormat::formatTime(v, &p, force_minus);
        }
        // fraction: "# ?/?", "# ??/??", "?/4"
        if DgNative::index_of(&p, "/") > 0 && DgNative::index_of(&p, "?") >= 0 {
            let fr: String = XlsxNumberFormat::formatFraction(v, &p);
            if force_minus {
                return format!("-{}", fr);
            }
            return fr;
        }
        // percent
        if XlsxNumberFormat::hasPercent(&p) {
            let pct: f64 = v * 100.0;
            let pp: String = XlsxNumberFormat::stripPercent(&p);
            let body: String = XlsxNumberFormat::formatNumberBody(pct, &pp);
            if force_minus {
                return format!("-{}%", body);
            }
            return format!("{}%", body);
        }
        // scientific
        if XlsxNumberFormat::hasScientific(&p) {
            let body2: String = XlsxNumberFormat::formatScientific(v);
            if force_minus {
                return format!("-{}", body2);
            }
            return body2;
        }
        // currency / number
        let mut prefix: String = String::new();
        let mut suffix: String = String::new();
        let mut core: String = p.clone();
        if !core.is_empty() {
            let c0: i64 = DgNative::char_at(&core, 0);
            if c0 == 36 || c0 == 8364 {
                // $ or euro as first code unit — euro may be multi-byte; check string prefix
            }
        }
        if DgNative::str_len(&core) >= 1 {
            if DgNative::substr(&core, 0, 1) == "$" {
                prefix = "$".to_string();
                core = DgNative::substr(&core, 1, DgNative::str_len(&core));
            }
        }
        if DgNative::str_len(&core) >= 3 {
            // UTF-8 euro often arrives as the character itself in source
        }
        // detect € prefix by scanning for currency-ish leading non #0,
        if !core.is_empty() {
            let fc: i64 = DgNative::char_at(&core, 0);
            if (((fc != 35 && fc != 48) && fc != 46) && fc != 44) && fc != 63 {
                // treat leading run of non-pattern chars as prefix ($, €, etc.)
                let mut pi: i64 = 0;
                let pn: i64 = DgNative::str_len(&core);
                while pi < pn {
                    let pc: i64 = DgNative::char_at(&core, pi);
                    if ((pc == 35 || pc == 48) || pc == 46) || pc == 44 {
                        break;
                    }
                    prefix = format!("{}{}", prefix, DgNative::substr(&core, pi, pi + 1));
                    pi += 1;
                }
                core = DgNative::substr(&core, pi, pn);
            }
        }
        // Trailing literal text, e.g. 0" kg" → suffix " kg". `suffix` was
        // declared but never filled, so anything after the number was dropped.
        let sn: i64 = DgNative::str_len(&core);
        let mut se: i64 = sn;
        while se > 0 {
            let sc: i64 = DgNative::char_at(&core, se - 1);
            if XlsxNumberFormat::isPatternChar(sc) {
                break;
            }
            se -= 1;
        }
        if se < sn {
            suffix = DgNative::substr(&core, se, sn);
            core = DgNative::substr(&core, 0, se);
        }
        let body3: String = XlsxNumberFormat::formatNumberBody(v, &core);
        if force_minus {
            return format!("-{}{}{}", prefix, body3, suffix);
        }
        format!("{}{}{}", prefix, body3, suffix)
    }
    pub fn general(v: f64, force_minus: bool) -> String {
        let iv: i64 = v.floor() as i64;
        if (iv as f64) == v || (iv as f64) - v == 0.0 {
            if force_minus {
                return format!("-{}", iv);
            }
            return iv.to_string();
        }
        let s: String = XlsxNumberFormat::fixedN(v, 2);
        if force_minus {
            return format!("-{}", s);
        }
        s
    }
    pub fn hasPercent(p: &str) -> bool {
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&p);
        while i < n {
            if DgNative::char_at(&p, i) == 37 {
                return true;
            }
            i += 1;
        }
        false
    }
    pub fn stripPercent(p: &str) -> String {
        let mut out: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&p);
        while i < n {
            if DgNative::char_at(&p, i) != 37 {
                out = format!("{}{}", out, DgNative::substr(&p, i, i + 1));
            }
            i += 1;
        }
        out
    }
    pub fn hasScientific(p: &str) -> bool {
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&p);
        while i < n {
            let ch: i64 = DgNative::char_at(&p, i);
            if ch == 69 || ch == 101 {
                return true;
            }
            i += 1;
        }
        false
    }
    pub fn isDatePattern(p: &str) -> bool {
        // yyyy / dd / mm with separators — not pure time
        let mut has_y: bool = false;
        let mut has_d: bool = false;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&p);
        while i < n {
            let ch: i64 = DgNative::char_at(&p, i);
            if ch == 121 || ch == 89 {
                has_y = true;
            }
            if ch == 100 || ch == 68 {
                has_d = true;
            }
            i += 1;
        }
        if has_y {
            return true;
        }
        has_d
    }
    pub fn isTimePattern(p: &str) -> bool {
        if XlsxNumberFormat::isDatePattern(p) {
            return false;
        }
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&p);
        while i < n {
            let ch: i64 = DgNative::char_at(&p, i);
            if ((ch == 104 || ch == 72) || ch == 115) || ch == 83 {
                return true;
            }
            i += 1;
        }
        // mm:ss style — look for ':'
        let mut has_colon: bool = false;
        let mut has_m: bool = false;
        i = 0;
        while i < n {
            if DgNative::char_at(&p, i) == 58 {
                has_colon = true;
            }
            if DgNative::char_at(&p, i) == 109 {
                has_m = true;
            }
            i += 1;
        }
        if has_colon && has_m {
            return true;
        }
        false
    }
    /// Excel serial date: days since 1899-12-30 (with Excel's 1900 leap quirk ignored for display).
    pub fn formatDate(serial: f64, pat: &str, _force_minus: bool) -> String {
        XlsxNumberFormat::formatDateTime(serial, pat)
    }
    pub fn formatTime(serial: f64, pat: &str, _force_minus: bool) -> String {
        XlsxNumberFormat::formatDateTime(serial, pat)
    }
    pub fn monthName(m: i64) -> String {
        let names: Vec<String> = vec!["January".to_string(), "February".to_string(), "March".to_string(), "April".to_string(), "May".to_string(), "June".to_string(), "July".to_string(), "August".to_string(), "September".to_string(), "October".to_string(), "November".to_string(), "December".to_string()];
        if m < 1 || m > 12 {
            return String::new();
        }
        names[(m - 1) as usize].clone()
    }
    pub fn dayName(wd: i64) -> String {
        let names: Vec<String> = vec!["Sunday".to_string(), "Monday".to_string(), "Tuesday".to_string(), "Wednesday".to_string(), "Thursday".to_string(), "Friday".to_string(), "Saturday".to_string()];
        names[(wd % 7) as usize].clone()
    }
    /// The run of one letter (case-insensitive) starting at i.
    pub fn runOf(p: &str, i: i64) -> i64 {
        let ch: i64 = DgNative::char_at(&XlsxNumberFormat::lower(p), i);
        let mut j: i64 = i;
        let lp: String = XlsxNumberFormat::lower(p);
        while j < DgNative::str_len(&lp) {
            if DgNative::char_at(&lp, j) != ch {
                break;
            }
            j += 1;
        }
        j - i
    }
    /// Excel's date and time codes, token by token: y, m (month, or minutes
    /// after an hour or before a second), d, h, s, AM/PM and A/P; anything
    /// else is literal.
    pub fn formatDateTime(serial: f64, pat: &str) -> String {
        let mut day: i64 = serial.floor() as i64;
        if day < 1 {
            day = 1;
        }
        let z: i64 = day + 2415019;
        let a: i64 = z + 32044;
        let b: i64 = (4 * a + 3) / 146097;
        let c: i64 = a - (b * 146097 / 4);
        let d: i64 = (4 * c + 3) / 1461;
        let e: i64 = c - (1461 * d / 4);
        let m: i64 = (5 * e + 2) / 153;
        let dd: i64 = (e - ((153 * m + 2) / 5)) + 1;
        let mo: i64 = (m + 3) - 12 * (m / 10);
        let yy: i64 = ((b * 100 + d) - 4800) + (m / 10);
        // Serial 1 (1900-01-01) is a Sunday.
        let wd: i64 = (day + 6) % 7;
        let mut frac: f64 = serial - ((serial.floor() as i64) as f64);
        if frac < 0.0 {
            frac = 0.0 - frac;
        }
        let total_sec: i64 = (frac * 86400.0 + 0.5).floor() as i64;
        let mut hh: i64 = total_sec / 3600;
        let mi: i64 = (total_sec - hh * 3600) / 60;
        let ss: i64 = (total_sec - hh * 3600) - mi * 60;
        if hh >= 24 {
            hh = hh % 24;
        }
        let lp: String = XlsxNumberFormat::lower(pat);
        let ampm: bool = DgNative::index_of(&lp, "am/pm") >= 0 || DgNative::index_of(&lp, "a/p") >= 0;
        let mut out: String = String::new();
        let mut last_was_hour: bool = false;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&pat);
        while i < n {
            let ch: i64 = DgNative::char_at(&lp, i);
            let run: i64 = XlsxNumberFormat::runOf(pat, i);
            if ch == 121 {
                if run >= 3 {
                    out = format!("{}{}", out, yy);
                } else {
                    out = format!("{}{}", out, XlsxNumberFormat::pad2(yy % 100));
                }
                i += run;
                last_was_hour = false;
            } else {
                if ch == 109 {
                    // Minutes when an hour came before, or a second follows.
                    let mut minutes: bool = last_was_hour;
                    let mut k: i64 = i + run;
                    while k < n && (DgNative::char_at(&lp, k) == 58 || DgNative::char_at(&lp, k) == 32) {
                        k += 1;
                    }
                    if k < n && DgNative::char_at(&lp, k) == 115 {
                        minutes = true;
                    }
                    if minutes && run <= 2 {
                        if run == 2 {
                            out = format!("{}{}", out, XlsxNumberFormat::pad2(mi));
                        } else {
                            out = format!("{}{}", out, mi);
                        }
                    } else {
                        if run >= 4 {
                            out = format!("{}{}", out, XlsxNumberFormat::monthName(mo));
                        } else {
                            if run == 3 {
                                out = format!("{}{}", out, DgNative::substr(&XlsxNumberFormat::monthName(mo), 0, 3));
                            } else {
                                if run == 2 {
                                    out = format!("{}{}", out, XlsxNumberFormat::pad2(mo));
                                } else {
                                    out = format!("{}{}", out, mo);
                                }
                            }
                        }
                    }
                    i += run;
                    last_was_hour = false;
                } else {
                    if ch == 100 {
                        if run >= 4 {
                            out = format!("{}{}", out, XlsxNumberFormat::dayName(wd));
                        } else {
                            if run == 3 {
                                out = format!("{}{}", out, DgNative::substr(&XlsxNumberFormat::dayName(wd), 0, 3));
                            } else {
                                if run == 2 {
                                    out = format!("{}{}", out, XlsxNumberFormat::pad2(dd));
                                } else {
                                    out = format!("{}{}", out, dd);
                                }
                            }
                        }
                        i += run;
                        last_was_hour = false;
                    } else {
                        if ch == 104 {
                            let mut hv: i64 = hh;
                            if ampm {
                                hv = hh % 12;
                                if hv == 0 {
                                    hv = 12;
                                }
                            }
                            if run >= 2 {
                                out = format!("{}{}", out, XlsxNumberFormat::pad2(hv));
                            } else {
                                out = format!("{}{}", out, hv);
                            }
                            i += run;
                            last_was_hour = true;
                        } else {
                            if ch == 115 {
                                if run >= 2 {
                                    out = format!("{}{}", out, XlsxNumberFormat::pad2(ss));
                                } else {
                                    out = format!("{}{}", out, ss);
                                }
                                i += run;
                                last_was_hour = false;
                            } else {
                                if DgNative::index_of(&DgNative::substr(&lp, i, n), "am/pm") == 0 {
                                    if hh < 12 {
                                        out = format!("{}AM", out);
                                    } else {
                                        out = format!("{}PM", out);
                                    }
                                    i += 5;
                                } else {
                                    if DgNative::index_of(&DgNative::substr(&lp, i, n), "a/p") == 0 {
                                        if hh < 12 {
                                            out = format!("{}A", out);
                                        } else {
                                            out = format!("{}P", out);
                                        }
                                        i += 3;
                                    } else {
                                        if ch != 91 {
                                            if ch != 93 {
                                                out = format!("{}{}", out, DgNative::substr(&pat, i, i + 1));
                                            }
                                        }
                                        i += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }
    /// A fraction with up to as many denominator digits as the code has ?s
    /// after the slash; "# " in front keeps a whole part.
    pub fn formatFraction(v: f64, pat: &str) -> String {
        let slash: i64 = DgNative::index_of(&pat, "/");
        let after: String = DgNative::substr(&pat, slash + 1, DgNative::str_len(&pat));
        let mut digits: i64 = 0;
        let mut fixed_den: i64 = 0;
        let mut i: i64 = 0;
        while i < DgNative::str_len(&after) {
            let ch: i64 = DgNative::char_at(&after, i);
            if (ch == 63 || ch == 35) || ch == 48 {
                digits += 1;
            }
            if ch >= 49 && ch <= 57 {
                fixed_den = fixed_den * 10 + (ch - 48);
            }
            i += 1;
        }
        let whole: bool = DgNative::index_of(&DgNative::substr(&pat, 0, slash).trim().to_string(), " ") > 0;
        let mut w: i64 = 0;
        let mut f: f64 = v;
        if whole {
            w = v.floor() as i64;
            f = v - (w as f64);
        }
        let mut max_den: i64 = 9;
        if digits >= 2 {
            max_den = 99;
        }
        if digits >= 3 {
            max_den = 999;
        }
        let mut best_n: i64 = 0;
        let mut best_d: i64 = 1;
        let mut best_err: f64 = 10.0;
        let mut den: i64 = 1;
        while den <= max_den {
            if fixed_den == 0 || den == fixed_den {
                let num: i64 = (f * (den as f64) + 0.5).floor() as i64;
                let mut err: f64 = f - (num as f64) / (den as f64);
                if err < 0.0 {
                    err = 0.0 - err;
                }
                if err < best_err - 1e-7 {
                    best_err = err;
                    best_n = num;
                    best_d = den;
                }
            }
            den += 1;
        }
        if best_n == best_d {
            w += 1;
            best_n = 0;
        }
        if best_n == 0 {
            if whole {
                return w.to_string();
            }
            return "0".to_string();
        }
        let frac: String = format!("{}/{}", best_n, best_d);
        if whole && w != 0 {
            return format!("{} {}", w, frac);
        }
        frac
    }
    pub fn formatScientific(v: f64) -> String {
        if v == 0.0 {
            return "0.00E+00".to_string();
        }
        let mut exp: i64 = 0;
        let mut m: f64 = v;
        while m >= 10.0 {
            m /= 10.0;
            exp += 1;
        }
        while m < 1.0 {
            m *= 10.0;
            exp -= 1;
        }
        let mant: String = XlsxNumberFormat::fixedN(m, 2);
        let es: String = XlsxNumberFormat::pad2(exp);
        if exp >= 0 {
            return format!("{}E+{}", mant, es);
        }
        let ae: i64 = 0 - exp;
        format!("{}E-{}", mant, XlsxNumberFormat::pad2(ae))
    }
    pub fn formatNumberBody(v: f64, core: &str) -> String {
        let decimals: i64 = XlsxNumberFormat::countDecimals(core);
        let use_comma: bool = XlsxNumberFormat::hasThousands(core);
        let rounded: String = XlsxNumberFormat::fixedN(v, decimals);
        if use_comma {
            return XlsxNumberFormat::addThousands(&rounded);
        }
        rounded
    }
    pub fn countDecimals(core: &str) -> i64 {
        let mut dot: i64 = -1;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&core);
        while i < n {
            if DgNative::char_at(&core, i) == 46 {
                dot = i;
            }
            i += 1;
        }
        if dot < 0 {
            return 0;
        }
        let mut c: i64 = 0;
        i = dot + 1;
        while i < n {
            let ch: i64 = DgNative::char_at(&core, i);
            if ch == 48 || ch == 35 {
                c += 1;
            }
            i += 1;
        }
        c
    }
    pub fn hasThousands(core: &str) -> bool {
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&core);
        while i < n {
            if DgNative::char_at(&core, i) == 44 {
                return true;
            }
            i += 1;
        }
        false
    }
    pub fn addThousands(num: &str) -> String {
        let mut dot: i64 = -1;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&num);
        while i < n {
            if DgNative::char_at(&num, i) == 46 {
                dot = i;
            }
            i += 1;
        }
        let mut whole: String = num.to_string();
        let mut frac: String = String::new();
        if dot >= 0 {
            whole = DgNative::substr(&num, 0, dot);
            frac = DgNative::substr(&num, dot, n);
        }
        let __len: i64 = DgNative::str_len(&whole);
        let mut out: String = String::new();
        let mut k: i64 = 0;
        while k < __len {
            if k > 0 && (__len - k) - (((__len - k) as f64 / 3.0).floor() as i64) * 3 == 0 {
                out = format!("{},", out);
            }
            out = format!("{}{}", out, DgNative::substr(&whole, k, k + 1));
            k += 1;
        }
        format!("{}{}", out, frac)
    }
    pub fn fixedN(v: f64, decimals: i64) -> String {
        if decimals <= 0 {
            return ((v + 0.5).floor() as i64).to_string();
        }
        let mut scale: f64 = 1.0;
        let mut d: i64 = 0;
        while d < decimals {
            scale *= 10.0;
            d += 1;
        }
        let scaled: i64 = (v * scale + 0.5).floor() as i64;
        let whole: i64 = ((scaled as f64) / scale).floor() as i64;
        let mut frac: i64 = scaled - (((whole as f64) * scale).floor() as i64);
        if frac < 0 {
            frac = 0 - frac;
        }
        let mut fs: String = frac.to_string();
        while DgNative::str_len(&fs) < decimals {
            fs = format!("0{}", fs);
        }
        format!("{}.{}", whole, fs)
    }
    pub fn pad2(mut n: i64) -> String {
        if n < 0 {
            n = 0 - n;
        }
        if n < 10 {
            return format!("0{}", n);
        }
        n.to_string()
    }
    pub fn lower(s: &str) -> String {
        let mut out: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        while i < n {
            let ch: i64 = DgNative::char_at(&s, i);
            if ch >= 65 && ch <= 90 {
                out = format!("{}{}", out, char::from_u32((ch + 32) as u32).unwrap_or('\0'));
            } else {
                out = format!("{}{}", out, DgNative::substr(&s, i, i + 1));
            }
            i += 1;
        }
        out
    }
}
