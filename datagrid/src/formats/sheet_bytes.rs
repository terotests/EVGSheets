// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;

use crate::rgr::EVGCodepoint;

/// SheetBytes: little-endian numbers and UTF-16 text out of a byte buffer.
///
/// The binary workbook formats (.xls, .xlsb, and the compound file around the
/// first) are all little-endian records of fixed-width integers, IEEE doubles
/// and UTF-16 strings. Ranger has `buffer_get` and nothing wider, so the rest is
/// built here once.
///
/// 32-bit values are read SIGNED (`i32`). Sector ids and record sizes in these
/// formats are below 2^31 when they are real, and the special ids (end of chain,
/// free sector) are the negative ones, which is easier to test for than numbers
/// near 2^32 on a target whose `int` is 32 bits.
pub struct SheetBytes;

impl SheetBytes {
    /// The byte at `at`, 0 outside the buffer.
    pub fn u8(b: &[u8], at: i64) -> i64 {
        if at < 0 || at >= b.len() as i64 {
            return 0;
        }
        b[at as usize] as i64
    }
    pub fn u16(b: &[u8], at: i64) -> i64 {
        SheetBytes::u8(b, at) + SheetBytes::u8(b, at + 1) * 256
    }
    pub fn i32(b: &[u8], at: i64) -> i64 {
        let lo = SheetBytes::u16(b, at);
        let mut hi = SheetBytes::u16(b, at + 2);
        if hi >= 32768 {
            hi -= 65536;
        }
        hi * 65536 + lo
    }
    /// 2^n for the exponents a double can have, by squaring.
    pub fn pow2(n: i64) -> f64 {
        let mut out = 1.0;
        let mut base = if n < 0 { 0.5 } else { 2.0 };
        let mut e = if n < 0 { -n } else { n };
        while e > 0 {
            if e % 2 == 1 {
                out *= base;
            }
            base *= base;
            e /= 2;
        }
        out
    }
    /// An IEEE 754 double, assembled from its bytes: sign, 11-bit exponent,
    /// 52-bit mantissa. Every step is exact in a double. NaN and infinity come
    /// back as 0, which no spreadsheet cell can hold anyway.
    pub fn f64(b: &[u8], at: i64) -> f64 {
        let b7 = SheetBytes::u8(b, at + 7);
        let b6 = SheetBytes::u8(b, at + 6);
        let exp = (b7 % 128) * 16 + b6 / 16;
        if exp == 2047 {
            return 0.0;
        }
        let mut mant = (b6 % 16) as f64;
        let mut k = 5;
        while k >= 0 {
            mant = mant * 256.0 + SheetBytes::u8(b, at + k) as f64;
            k -= 1;
        }
        let v = if exp == 0 {
            mant * SheetBytes::pow2(-1074)
        } else {
            (1.0 + mant * SheetBytes::pow2(-52)) * SheetBytes::pow2(exp - 1023)
        };
        if b7 >= 128 {
            0.0 - v
        } else {
            v
        }
    }
    /// Excel's RK number: a 30-bit integer or the top 30 bits of a double, and
    /// a flag that says the value was multiplied by 100.
    pub fn rk(b: &[u8], at: i64) -> f64 {
        let b0 = SheetBytes::u8(b, at);
        let v = if b0 & 2 != 0 {
            (SheetBytes::i32(b, at) as f64 / 4.0).floor()
        } else {
            let mut d = vec![0u8; 8];
            d[4] = (b0 & 252) as u8;
            d[5] = SheetBytes::u8(b, at + 1) as u8;
            d[6] = SheetBytes::u8(b, at + 2) as u8;
            d[7] = SheetBytes::u8(b, at + 3) as u8;
            SheetBytes::f64(&d, 0)
        };
        if b0 & 1 != 0 {
            v / 100.0
        } else {
            v
        }
    }
    /// `n` UTF-16LE code units from `at`, or `n` single bytes when `wide` is
    /// false (the "compressed" Latin-1 form .xls writes when it can).
    pub fn utf16(b: &[u8], at: i64, n: i64, wide: bool) -> String {
        let mut out = String::new();
        let mut i = 0;
        let mut pos = at;
        while i < n {
            let mut u = if wide { SheetBytes::u16(b, pos) } else { SheetBytes::u8(b, pos) };
            pos += if wide { 2 } else { 1 };
            if wide && u >= 0xD800 && u < 0xDC00 && i + 1 < n {
                let lo = SheetBytes::u16(b, pos);
                if lo >= 0xDC00 && lo < 0xE000 {
                    pos += 2;
                    i += 1;
                    u = (u - 0xD800) * 1024 + (lo - 0xDC00) + 0x10000;
                }
            }
            out.push_str(&EVGCodepoint::to_str(u));
            i += 1;
        }
        out
    }
    /// Code units collected one at a time: a string continued across records
    /// changes width in the middle, so it cannot be read in one call.
    pub fn unitsToString(units: &[i64]) -> String {
        let mut out = String::new();
        let mut i = 0;
        let n = units.len();
        while i < n {
            let mut u = units[i];
            if u >= 0xD800 && u < 0xDC00 && i + 1 < n {
                let lo = units[i + 1];
                if lo >= 0xDC00 && lo < 0xE000 {
                    u = (u - 0xD800) * 1024 + (lo - 0xDC00) + 0x10000;
                    i += 1;
                }
            }
            out.push_str(&EVGCodepoint::to_str(u));
            i += 1;
        }
        out
    }
    /// A number as a cell's raw text: integers without a decimal point, the
    /// rest as the shortest form that reads back the same.
    pub fn numText(v: f64) -> String {
        let lim = 1e15;
        if v < lim && v > -lim && v == v.floor() {
            return (v as i64).to_string();
        }
        v.to_string()
    }
    pub fn lower(s: &str) -> String {
        s.to_lowercase()
    }
    /// The extension of a file name, lowercased, without the dot.
    pub fn extOf(name: &str) -> String {
        let mut ext = String::new();
        let mut seen = false;
        for ch in name.chars() {
            if ch == '/' || ch == '\\' {
                ext = String::new();
                seen = false;
            } else if ch == '.' {
                ext = String::new();
                seen = true;
            } else if seen {
                ext.push(ch);
            }
        }
        ext.to_lowercase()
    }
    /// The same name with another extension.
    pub fn withExt(name: &str, ext: &str) -> String {
        // the name up to its last dot, and what follows it
        let mut base = String::new();
        let mut tail = String::new();
        let mut seen = false;
        for ch in name.chars() {
            if ch == '.' {
                if seen {
                    base.push('.');
                    base.push_str(&tail);
                }
                tail = String::new();
                seen = true;
            } else if ch == '/' || ch == '\\' {
                if seen {
                    base.push('.');
                    base.push_str(&tail);
                    tail = String::new();
                    seen = false;
                }
                base.push(ch);
            } else if seen {
                tail.push(ch);
            } else {
                base.push(ch);
            }
        }
        if tail.is_empty() {
            return format!("{}.{}", name, ext);
        }
        format!("{}.{}", base, ext)
    }
    /// Built-in number formats 0-49, as [MS-XLS] 2.5.165 and ECMA-376 18.8.30
    /// list them, in the en-US spelling Excel shows.
    pub fn builtinNumFmt(id: i64) -> String {
        let table: Vec<String> = vec!["General".to_string(), "0".to_string(), "0.00".to_string(), "#,##0".to_string(), "#,##0.00".to_string(), "\"$\"#,##0_);(\"$\"#,##0)".to_string(), "\"$\"#,##0_);[Red](\"$\"#,##0)".to_string(), "\"$\"#,##0.00_);(\"$\"#,##0.00)".to_string(), "\"$\"#,##0.00_);[Red](\"$\"#,##0.00)".to_string(), "0%".to_string(), "0.00%".to_string(), "0.00E+00".to_string(), "# ?/?".to_string(), "# ??/??".to_string(), "m/d/yyyy".to_string(), "d-mmm-yy".to_string(), "d-mmm".to_string(), "mmm-yy".to_string(), "h:mm AM/PM".to_string(), "h:mm:ss AM/PM".to_string(), "h:mm".to_string(), "h:mm:ss".to_string(), "m/d/yyyy h:mm".to_string(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), "#,##0_);(#,##0)".to_string(), "#,##0_);[Red](#,##0)".to_string(), "#,##0.00_);(#,##0.00)".to_string(), "#,##0.00_);[Red](#,##0.00)".to_string(), "_(* #,##0_);_(* (#,##0);_(* \"-\"_);_(@_)".to_string(), "_(\"$\"* #,##0_);_(\"$\"* (#,##0);_(\"$\"* \"-\"_);_(@_)".to_string(), "_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)".to_string(), "_(\"$\"* #,##0.00_);_(\"$\"* (#,##0.00);_(\"$\"* \"-\"??_);_(@_)".to_string(), "mm:ss".to_string(), "[h]:mm:ss".to_string(), "mm:ss.0".to_string(), "##0.0E+0".to_string(), "@".to_string()];
        if id < 0 || id >= (table.len() as i64) {
            return "General".to_string();
        }
        let code: String = table[id as usize].clone();
        if code.is_empty() {
            return "General".to_string();
        }
        code
    }
}
