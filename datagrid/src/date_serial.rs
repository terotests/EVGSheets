// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;

use crate::rgr::DgNative;

/// DateSerial.rgr — the calendar, as a spreadsheet counts it.
///
/// A spreadsheet does not have a date type. It has NUMBERS WITH A FORMAT: 45000
/// is a number, and it is the 15th of April 2023 because the cell says to draw
/// it that way. Every date feature — typing one, adding a month to one, asking
/// which weekday it is — is therefore arithmetic on a day count, and this is
/// where that arithmetic lives.
///
/// The epoch is Excel's: day 1 is 1900-01-01, and day 60 is the 29th of February
/// 1900, a date that never happened. Lotus 1-2-3 had the bug, Excel copied it for
/// compatibility, and every file written since encodes it. The offset used here
/// (day 25569 is 1970-01-01) already contains that phantom day, so it is the
/// dates BEFORE it that need correcting — they come out one too high otherwise.
/// Getting this backwards shifts every date before March 1900 by one and nothing
/// else, which is exactly the kind of bug that survives for years; the test
/// checks day 1, day 59 and day 61 against Excel's own answers for that reason.
#[derive(Clone)]
pub struct DateSerial;

impl DateSerial {
    pub fn isLeap(y: i64) -> bool {
        if y % 4 != 0 {
            return false;
        }
        if y % 100 != 0 {
            return true;
        }
        y % 400 == 0
    }
    pub fn daysInMonth(y: i64, m: i64) -> i64 {
        if m == 2 {
            if DateSerial::isLeap(y) {
                return 29;
            }
            return 28;
        }
        if m == 4 {
            return 30;
        }
        if m == 6 {
            return 30;
        }
        if m == 9 {
            return 30;
        }
        if m == 11 {
            return 30;
        }
        31
    }
    /// Days since 1899-12-30, by the civil-from-days algorithm — the same one
    /// the number formatter inverts when it draws a date.
    pub fn daysFromCivil(y: i64, m: i64, d: i64) -> i64 {
        let mut yy: i64 = y;
        if m <= 2 {
            yy -= 1;
        }
        let mut era: i64 = yy / 400;
        if yy < 0 {
            era = (yy - 399) / 400;
        }
        let yoe: i64 = yy - era * 400;
        let mut mp: i64 = m + 9;
        if m > 2 {
            mp = m - 3;
        }
        let doy: i64 = ((153 * mp + 2) / 5) + (d - 1);
        let doe: i64 = ((yoe * 365 + (yoe / 4)) - (yoe / 100)) + doy;
        (era * 146097 + doe) - 719468
    }
    /// A calendar date as a spreadsheet serial.
    pub fn fromYMD(y: i64, m: i64, d: i64) -> f64 {
        // Day 25569 is the Unix epoch, 1970-01-01: the offset between the civil
        // day count and the spreadsheet's. That offset already contains the
        // phantom day, so it is dates BEFORE the phantom that need correcting —
        // they are one lower than the straight-line count says.
        let days: i64 = DateSerial::daysFromCivil(y, m, d);
        let mut serial: i64 = days + 25569;
        if serial < 61 {
            serial -= 1;
        }
        serial as f64
    }
    /// The inverse: [year, month, day].
    pub fn toYMD(serial: f64) -> Vec<i64> {
        let mut s: i64 = serial.floor() as i64;
        if s < 1 {
            s = 1;
        }
        if s < 61 {
            s += 1;
        }
        let days: i64 = s - 25569;
        let z: i64 = days + 719468;
        let mut era: i64 = z / 146097;
        if z < 0 {
            era = (z - 146096) / 146097;
        }
        let doe: i64 = z - era * 146097;
        let yoe: i64 = (((doe - (doe / 1460)) + (doe / 36524)) - (doe / 146096)) / 365;
        let mut y: i64 = yoe + era * 400;
        let doy: i64 = doe - ((365 * yoe + (yoe / 4)) - (yoe / 100));
        let mp: i64 = (5 * doy + 2) / 153;
        let d: i64 = (doy - ((153 * mp + 2) / 5)) + 1;
        let mut m: i64 = mp + 3;
        if mp >= 10 {
            m = mp - 9;
        }
        if m <= 2 {
            y += 1;
        }
        vec![y, m, d]
    }
    /// Day of the week, Excel's WEEKDAY with return type 1: Sunday = 1.
    pub fn weekday(serial: f64) -> i64 {
        let mut s: i64 = serial.floor() as i64;
        if s < 61 {
            s += 1;
        }
        let days: i64 = s - 25569;
        // 1970-01-01 was a Thursday, which is 5 in this numbering.
        let mut w: i64 = (days + 4) % 7;
        if w < 0 {
            w += 7;
        }
        w + 1
    }
    /// Whole months added, clamping the day to the end of the target month —
    /// the 31st of January plus one month is the 28th of February, not the 3rd
    /// of March.
    pub fn addMonths(serial: f64, months: i64) -> f64 {
        let parts = DateSerial::toYMD(serial);
        let y: i64 = parts[0];
        let m: i64 = parts[1];
        let mut d: i64 = parts[2];
        let total: i64 = (y * 12 + (m - 1)) + months;
        let mut ny: i64 = total / 12;
        let mut nm: i64 = (total - ny * 12) + 1;
        if nm < 1 {
            nm += 12;
            ny -= 1;
        }
        let max_d: i64 = DateSerial::daysInMonth(ny, nm);
        if d > max_d {
            d = max_d;
        }
        DateSerial::fromYMD(ny, nm, d)
    }
    pub fn endOfMonth(serial: f64, months: i64) -> f64 {
        let moved: f64 = DateSerial::addMonths(serial, months);
        let parts = DateSerial::toYMD(moved);
        let y: i64 = parts[0];
        let m: i64 = parts[1];
        DateSerial::fromYMD(y, m, DateSerial::daysInMonth(y, m))
    }
    /// --- reading a typed date -------------------------------------------------
    pub fn isDigit(ch: i64) -> bool {
        ch >= 48 && ch <= 57
    }
    /// Three numbers separated by one repeated punctuation mark. Returns false
    /// for anything else, INCLUDING text that merely starts with a number:
    /// "2024 was a good year" is not a date.
    fn splitParts(text: &str) -> Option<(Vec<i64>, i64)> {
        let n: i64 = DgNative::str_len(text);
        let mut nums: Vec<i64> = Vec::new();
        let mut sep: Vec<i64> = Vec::new();
        let mut cur: i64 = 0;
        let mut digits: i64 = 0;
        let mut i: i64 = 0;
        while i < n {
            let ch: i64 = DgNative::char_at(text, i);
            if DateSerial::isDigit(ch) {
                cur = cur * 10 + (ch - 48);
                digits += 1;
                if digits > 4 {
                    return None;
                }
            } else {
                if digits == 0 {
                    return None;
                }
                nums.push(cur);
                sep.push(ch);
                cur = 0;
                digits = 0;
            }
            i += 1;
        }
        if digits == 0 {
            return None;
        }
        nums.push(cur);
        if nums.len() != 3 || sep.len() != 2 || sep[0] != sep[1] {
            return None;
        }
        Some((nums, sep[0]))
    }
    /// Is this text a date, and if so which one? Three orders are accepted, told
    /// apart by the separator and by which part is four digits:
    ///   2024-03-15   ISO, dash
    ///   15.3.2024    day first, dot — how most of Europe writes it
    ///   3/15/2024    month first, slash — how the US writes it
    /// An ambiguous "03/04/2024" is read as the separator says, not guessed at.
    fn parse(text: &str) -> Option<f64> {
        let (nums, s) = DateSerial::splitParts(text)?;
        // year, month, day by the separator: ISO dash, European dot, US slash
        let (mut y, m, d) = match s {
            45 => (nums[0], nums[1], nums[2]),
            46 => (nums[2], nums[1], nums[0]),
            47 => (nums[2], nums[0], nums[1]),
            _ => return None,
        };
        if y < 100 {
            // Two-digit years, the way every spreadsheet resolves them.
            if y < 30 {
                y += 2000;
            } else {
                y += 1900;
            }
        }
        if y < 1900 {
            return None;
        }
        if y > 9999 {
            return None;
        }
        if m < 1 {
            return None;
        }
        if m > 12 {
            return None;
        }
        if d < 1 {
            return None;
        }
        if d > DateSerial::daysInMonth(y, m) {
            return None;
        }
        Some(DateSerial::fromYMD(y, m, d))
    }
    /// The serial of a typed date, 0 when `isDate` says it is not one.
    pub fn parseDate(text: &str) -> f64 {
        DateSerial::parse(text).unwrap_or(0.0)
    }
    /// Whether the text is a date `parseDate` reads.
    pub fn isDate(text: &str) -> bool {
        DateSerial::parse(text).is_some()
    }
    /// The number format a typed date should wear, chosen to match how it was
    /// typed — someone who writes 15.3.2024 does not want to see 2024-03-15.
    pub fn formatForTyped(text: &str) -> String {
        let s: i64 = match DateSerial::splitParts(text) {
            Some((_, s)) => s,
            None => return "yyyy-mm-dd".to_string(),
        };
        if s == 46 {
            return "dd.mm.yyyy".to_string();
        }
        if s == 47 {
            return "mm/dd/yyyy".to_string();
        }
        "yyyy-mm-dd".to_string()
    }
}
