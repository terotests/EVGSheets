// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;

use crate::rgr::DgNative;

/// What the fill handle writes for text, as Excel's AutoFill does: month and
/// weekday names go on round the calendar (Jan → Feb, Monday → Tuesday, in
/// the case and length they were written in), and text ending in a number
/// counts on (Item1 → Item2, Q09 → Q10). Numbers and dates are the caller's.
pub struct FillSeries {}

impl FillSeries {
    pub fn lists() -> Vec<String> {
        vec![
            "JANUARY FEBRUARY MARCH APRIL MAY JUNE JULY AUGUST SEPTEMBER OCTOBER NOVEMBER DECEMBER".to_string(),
            "JAN FEB MAR APR MAY JUN JUL AUG SEP OCT NOV DEC".to_string(),
            "SUNDAY MONDAY TUESDAY WEDNESDAY THURSDAY FRIDAY SATURDAY".to_string(),
            "SUN MON TUE WED THU FRI SAT".to_string(),
        ]
    }
    /// The position of `word` in list `li`, -1 when it is not there.
    pub fn indexIn(li: i64, word: &str) -> i64 {
        let up: String = word.trim().to_uppercase();
        let items: Vec<String> = FillSeries::lists()[li as usize].split(" ").map(|s| s.to_string()).collect::<Vec<String>>();
        let mut i: usize = 0;
        while i < items.len() {
            if items[i] == up {
                return i as i64;
            }
            i += 1;
        }
        -1
    }
    /// `name` spelt the way `like` is: ALL CAPS, all small, or Capitalised.
    pub fn caseLike(name: &str, like: &str) -> String {
        let t: String = like.trim().to_string();
        if t == t.to_uppercase() && t != t.to_lowercase() {
            return name.to_uppercase();
        }
        if t == t.to_lowercase() {
            return name.to_lowercase();
        }
        let low: String = name.to_lowercase();
        format!("{}{}", DgNative::substr(&low, 0, 1).to_uppercase(), DgNative::substr(&low, 1, DgNative::str_len(&low)))
    }
    /// The trailing digits of `s`: [prefix length, digit count], or [-1, 0].
    pub fn tailNumber(s: &str) -> Vec<i64> {
        let n: i64 = DgNative::str_len(s);
        let mut i: i64 = n;
        while i > 0 {
            let ch: i64 = DgNative::char_at(s, i - 1);
            if ch < 48 || ch > 57 {
                break;
            }
            i -= 1;
        }
        if i == n || i == 0 {
            return vec![-1, 0];
        }
        vec![i, n - i]
    }
    /// The value the fill puts `at` steps from the first source cell, given
    /// the source cells along the fill direction. "" when they are not a
    /// series this knows.
    pub fn next(vals: &Vec<String>, at: i64) -> String {
        let n: i64 = vals.len() as i64;
        if n == 0 {
            return String::new();
        }
        // Calendar names.
        let mut li: i64 = 0;
        while li < 4 {
            let mut idx: Vec<i64> = Vec::new();
            let mut k: i64 = 0;
            while k < n {
                let p: i64 = FillSeries::indexIn(li, &vals[k as usize].clone());
                if p < 0 {
                    break;
                }
                idx.push(p);
                k += 1;
            }
            if k == n {
                let len: i64 = if li < 2 { 12 } else { 7 };
                let mut step: i64 = 1;
                if n >= 2 {
                    step = idx[1] - idx[0];
                }
                let mut p: i64 = (idx[0] + step * at) % len;
                if p < 0 {
                    p += len;
                }
                let items: Vec<String> = FillSeries::lists()[li as usize].split(" ").map(|s| s.to_string()).collect::<Vec<String>>();
                return FillSeries::caseLike(&items[p as usize].clone(), &vals[0].clone());
            }
            li += 1;
        }
        // Text ending in a number, the same text before it in every cell.
        let t0: Vec<i64> = FillSeries::tailNumber(&vals[0].clone());
        if t0[0] < 0 {
            return String::new();
        }
        let prefix: String = DgNative::substr(&vals[0].clone(), 0, t0[0]);
        let mut nums: Vec<i64> = Vec::new();
        let mut width: i64 = t0[1];
        let mut k: i64 = 0;
        while k < n {
            let v: String = vals[k as usize].clone();
            let tk: Vec<i64> = FillSeries::tailNumber(&v);
            if tk[0] < 0 || DgNative::substr(&v, 0, tk[0]) != prefix {
                return String::new();
            }
            let digits: String = DgNative::substr(&v, tk[0], DgNative::str_len(&v));
            nums.push(digits.parse::<i64>().unwrap_or(0));
            if tk[1] < width {
                width = tk[1];
            }
            k += 1;
        }
        let mut step: i64 = 1;
        if n >= 2 {
            step = nums[1] - nums[0];
        }
        let mut v: i64 = nums[0] + step * at;
        if v < 0 {
            v = 0 - v;
        }
        let mut digits: String = v.to_string();
        // Keep leading zeros: Q09 → Q10, File001 → File002.
        if DgNative::char_at(&vals[0].clone(), t0[0]) == 48 {
            while DgNative::str_len(&digits) < width {
                digits = format!("0{}", digits);
            }
        }
        format!("{}{}", prefix, digits)
    }
}
