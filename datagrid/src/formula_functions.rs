// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;
use crate::date_serial::DateSerial;
use crate::formula_value::FormulaValue;
use crate::xlsx_number_format::XlsxNumberFormat;

/// FormulaFunctions - pure, unit-testable spreadsheet function library.
///
/// No workbook / sheet / dependency graph here — only FormulaValue in → out.
/// FormulaEngine evaluates AST children into args[], then:
///
///   FormulaFunctions.invoke(name args)
///
/// Add new Excel-ish functions in this file + FormulaFunctionsTest.rgr.
#[derive(Clone)]
pub struct FormulaFunctions {
}

impl FormulaFunctions {
    pub fn new() -> Self {
        Self {
        }
    }
    /// UPPER / LOWER, for the letters a case change is defined for.
    ///
    /// Anything else is copied with `substring`, not rebuilt with
    /// `(strfromcode ch)`. That pair looks like an identity and is one in
    /// JavaScript, where a string is code units; in C++ a string is BYTES, so
    /// `charAt` hands back 0xC3 and `strfromcode` — whose job is a code POINT —
    /// encodes 195 as two bytes of UTF-8. Every accented letter would come out
    /// encoded twice.
    pub fn upper(s: &str) -> String {
        let mut out: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        while i < n {
            let ch: i64 = DgNative::char_at(&s, i);
            if ch >= 97 && ch <= 122 {
                out = format!("{}{}", out, char::from_u32((ch - 32) as u32).unwrap_or('\0'));
            } else {
                out = format!("{}{}", out, DgNative::substr(&s, i, i + 1));
            }
            i += 1;
        }
        out
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
    /// First error in args, or blank if none.
    pub fn firstError(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let mut i: i64 = 0;
        let n: i64 = args.len() as i64;
        while i < n {
            let v: Rc<RefCell<FormulaValue>> = args[i as usize].clone();
            let e: bool = v.borrow().isError();
            if e {
                // A range carrying one #DIV/0! is an error argument; the error
                // that comes back is the one from inside it, not the range.
                return v.borrow().errorInside();
            }
            i += 1;
        }
        FormulaValue::blank()
    }
    pub fn isKnown(name: &str) -> bool {
        let n: String = FormulaFunctions::upper(name);
        if n == "SUM" {
            return true;
        }
        if n == "AVERAGE" {
            return true;
        }
        if n == "MIN" {
            return true;
        }
        if n == "MAX" {
            return true;
        }
        if n == "COUNT" {
            return true;
        }
        if n == "COUNTA" {
            return true;
        }
        if n == "PRODUCT" {
            return true;
        }
        if n == "IF" {
            return true;
        }
        if n == "AND" {
            return true;
        }
        if n == "OR" {
            return true;
        }
        if n == "NOT" {
            return true;
        }
        if n == "ABS" {
            return true;
        }
        if n == "ROUND" {
            return true;
        }
        if n == "INT" {
            return true;
        }
        if n == "MOD" {
            return true;
        }
        if n == "POWER" {
            return true;
        }
        if n == "SQRT" {
            return true;
        }
        if n == "SIGN" {
            return true;
        }
        if n == "PI" {
            return true;
        }
        if n == "EXP" {
            return true;
        }
        if n == "LN" {
            return true;
        }
        if n == "LOG10" {
            return true;
        }
        if n == "FLOOR" {
            return true;
        }
        if n == "CEILING" {
            return true;
        }
        if n == "LEN" {
            return true;
        }
        if n == "LEFT" {
            return true;
        }
        if n == "RIGHT" {
            return true;
        }
        if n == "MID" {
            return true;
        }
        if n == "UPPER" {
            return true;
        }
        if n == "LOWER" {
            return true;
        }
        if n == "TRIM" {
            return true;
        }
        if n == "CONCAT" {
            return true;
        }
        if n == "CONCATENATE" {
            return true;
        }
        if n == "VALUE" {
            return true;
        }
        if n == "TRUE" {
            return true;
        }
        if n == "FALSE" {
            return true;
        }
        if n == "IFERROR" {
            return true;
        }
        if n == "IFNA" {
            return true;
        }
        if n == "ISBLANK" {
            return true;
        }
        if n == "ISNUMBER" {
            return true;
        }
        if n == "ISTEXT" {
            return true;
        }
        if n == "ISERROR" {
            return true;
        }
        if n == "ISERR" {
            return true;
        }
        if n == "N" {
            return true;
        }
        if n == "T" {
            return true;
        }
        if n == "DATE" {
            return true;
        }
        if n == "YEAR" {
            return true;
        }
        if n == "MONTH" {
            return true;
        }
        if n == "DAY" {
            return true;
        }
        if n == "WEEKDAY" {
            return true;
        }
        if n == "EDATE" {
            return true;
        }
        if n == "EOMONTH" {
            return true;
        }
        if n == "DAYS" {
            return true;
        }
        if n == "DATEVALUE" {
            return true;
        }
        if n == "TEXT" {
            return true;
        }
        if n == "ROUNDUP" {
            return true;
        }
        if n == "ROUNDDOWN" {
            return true;
        }
        if n == "TRUNC" {
            return true;
        }
        if n == "MEDIAN" {
            return true;
        }
        if n == "COUNTBLANK" {
            return true;
        }
        if n == "COUNTIF" {
            return true;
        }
        if n == "SUMIF" {
            return true;
        }
        if n == "AVERAGEIF" {
            return true;
        }
        if n == "COUNTIFS" {
            return true;
        }
        if n == "SUMIFS" {
            return true;
        }
        if n == "AVERAGEIFS" {
            return true;
        }
        if n == "CHOOSE" {
            return true;
        }
        if n == "SWITCH" {
            return true;
        }
        if n == "TEXTJOIN" {
            return true;
        }
        if n == "REPLACE" {
            return true;
        }
        if n == "XLOOKUP" {
            return true;
        }
        if n == "LOOKUP" {
            return true;
        }
        if n == "FILTER" {
            return true;
        }
        if n == "UNIQUE" {
            return true;
        }
        if n == "SORT" {
            return true;
        }
        if n == "NA" {
            return true;
        }
        if n == "XOR" {
            return true;
        }
        if n == "VLOOKUP" {
            return true;
        }
        if n == "HLOOKUP" {
            return true;
        }
        if n == "INDEX" {
            return true;
        }
        if n == "MATCH" {
            return true;
        }
        if n == "SUMPRODUCT" {
            return true;
        }
        if n == "LARGE" {
            return true;
        }
        if n == "SMALL" {
            return true;
        }
        if n == "SUBSTITUTE" {
            return true;
        }
        if n == "FIND" {
            return true;
        }
        if n == "SEARCH" {
            return true;
        }
        if n == "REPT" {
            return true;
        }
        if n == "PROPER" {
            return true;
        }
        if n == "EXACT" {
            return true;
        }
        if n == "CHAR" {
            return true;
        }
        if n == "CODE" {
            return true;
        }
        if n == "TRANSPOSE" {
            return true;
        }
        if n == "TODAY" {
            return true;
        }
        if n == "NOW" {
            return true;
        }
        if n == "IFS" {
            return true;
        }
        if n == "SUBTOTAL" {
            return true;
        }
        if n == "STDEV" {
            return true;
        }
        if n == "STDEV.S" {
            return true;
        }
        if n == "STDEVP" {
            return true;
        }
        if n == "STDEV.P" {
            return true;
        }
        if n == "VAR" {
            return true;
        }
        if n == "VAR.S" {
            return true;
        }
        if n == "VARP" {
            return true;
        }
        if n == "VAR.P" {
            return true;
        }
        if ((((n == "PMT" || n == "PV") || n == "FV") || n == "NPER") || n == "NPV") || n == "SLN" {
            return true;
        }
        false
    }
    /// Every function name this library answers, space-separated — for docs,
    /// hosts, and "what do we actually have?" checks.
    pub fn catalog() -> String {
        "SUM AVERAGE MIN MAX COUNT COUNTA PRODUCT IF IFS SWITCH AND OR NOT XOR ABS ROUND INT MOD POWER SQRT SIGN PI EXP LN LOG10 FLOOR CEILING LEN LEFT RIGHT MID UPPER LOWER TRIM CONCAT CONCATENATE TEXTJOIN REPLACE VALUE TRUE FALSE IFERROR IFNA ISBLANK ISNUMBER ISTEXT ISERROR ISERR N T NA DATE YEAR MONTH DAY WEEKDAY EDATE EOMONTH DAYS DATEVALUE TEXT ROUNDUP ROUNDDOWN TRUNC MEDIAN COUNTBLANK COUNTIF SUMIF AVERAGEIF COUNTIFS SUMIFS AVERAGEIFS CHOOSE VLOOKUP HLOOKUP XLOOKUP LOOKUP INDEX MATCH FILTER UNIQUE SORT SUMPRODUCT LARGE SMALL SUBSTITUTE FIND SEARCH REPT PROPER EXACT CHAR CODE TRANSPOSE TODAY NOW SUBTOTAL STDEV STDEV.S STDEVP STDEV.P VAR VAR.S VARP VAR.P PMT PV FV NPER NPV SLN".to_string()
    }
    /// Dispatch by uppercase function name. (`call` is reserved in Ranger.)
    pub fn invoke(name: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let n: String = FormulaFunctions::upper(name);
        if n == "SUBTOTAL" {
            return FormulaFunctions::fnSubtotal(args);
        }
        if (((n == "PMT" || n == "PV") || n == "FV") || n == "NPER") || n == "SLN" {
            return FormulaFunctions::fnTvm(&n, args);
        }
        if n == "NPV" {
            return FormulaFunctions::fnNpv(args);
        }
        if n == "STDEV" || n == "STDEV.S" {
            return FormulaFunctions::fnSpread(args, true, true);
        }
        if n == "STDEVP" || n == "STDEV.P" {
            return FormulaFunctions::fnSpread(args, false, true);
        }
        if n == "VAR" || n == "VAR.S" {
            return FormulaFunctions::fnSpread(args, true, false);
        }
        if n == "VARP" || n == "VAR.P" {
            return FormulaFunctions::fnSpread(args, false, false);
        }
        if n == "SUM" {
            return FormulaFunctions::fnSum(args);
        }
        if n == "AVERAGE" {
            return FormulaFunctions::fnAverage(args);
        }
        if n == "MIN" {
            return FormulaFunctions::fnMin(args);
        }
        if n == "MAX" {
            return FormulaFunctions::fnMax(args);
        }
        if n == "COUNT" {
            return FormulaFunctions::fnCount(args);
        }
        if n == "COUNTA" {
            return FormulaFunctions::fnCountA(args);
        }
        if n == "PRODUCT" {
            return FormulaFunctions::fnProduct(args);
        }
        if n == "IF" {
            return FormulaFunctions::fnIf(args);
        }
        if n == "IFS" {
            return FormulaFunctions::fnIfs(args);
        }
        if n == "SWITCH" {
            return FormulaFunctions::fnSwitch(args);
        }
        if n == "AND" {
            return FormulaFunctions::fnAnd(args);
        }
        if n == "OR" {
            return FormulaFunctions::fnOr(args);
        }
        if n == "XOR" {
            return FormulaFunctions::fnXor(args);
        }
        if n == "NOT" {
            return FormulaFunctions::fnNot(args);
        }
        if n == "ABS" {
            return FormulaFunctions::fnAbs(args);
        }
        if n == "ROUND" {
            return FormulaFunctions::fnRound(args);
        }
        if n == "INT" {
            return FormulaFunctions::fnInt(args);
        }
        if n == "MOD" {
            return FormulaFunctions::fnMod(args);
        }
        if n == "POWER" {
            return FormulaFunctions::fnPower(args);
        }
        if n == "SQRT" {
            return FormulaFunctions::fnSqrt(args);
        }
        if n == "SIGN" {
            return FormulaFunctions::fnSign(args);
        }
        if n == "PI" {
            return FormulaValue::fromNumber(3.141592653589793);
        }
        if n == "EXP" {
            return FormulaFunctions::fnExp(args);
        }
        if n == "LN" {
            return FormulaFunctions::fnLn(args);
        }
        if n == "LOG10" {
            return FormulaFunctions::fnLog10(args);
        }
        if n == "FLOOR" {
            return FormulaFunctions::fnFloor(args);
        }
        if n == "CEILING" {
            return FormulaFunctions::fnCeiling(args);
        }
        if n == "LEN" {
            return FormulaFunctions::fnLen(args);
        }
        if n == "LEFT" {
            return FormulaFunctions::fnLeft(args);
        }
        if n == "RIGHT" {
            return FormulaFunctions::fnRight(args);
        }
        if n == "MID" {
            return FormulaFunctions::fnMid(args);
        }
        if n == "UPPER" {
            return FormulaFunctions::fnUpper(args);
        }
        if n == "LOWER" {
            return FormulaFunctions::fnLower(args);
        }
        if n == "TRIM" {
            return FormulaFunctions::fnTrim(args);
        }
        if n == "CONCAT" {
            return FormulaFunctions::fnConcat(args);
        }
        if n == "CONCATENATE" {
            return FormulaFunctions::fnConcat(args);
        }
        if n == "TEXTJOIN" {
            return FormulaFunctions::fnTextjoin(args);
        }
        if n == "REPLACE" {
            return FormulaFunctions::fnReplace(args);
        }
        if n == "VALUE" {
            return FormulaFunctions::fnValue(args);
        }
        if n == "TRUE" {
            return FormulaValue::fromBool(true);
        }
        if n == "FALSE" {
            return FormulaValue::fromBool(false);
        }
        if n == "IFERROR" {
            return FormulaFunctions::fnIfError(args);
        }
        if n == "IFNA" {
            return FormulaFunctions::fnIfNa(args);
        }
        if n == "ISBLANK" {
            return FormulaFunctions::fnIsBlank(args);
        }
        if n == "ISNUMBER" {
            return FormulaFunctions::fnIsNumber(args);
        }
        if n == "ISTEXT" {
            return FormulaFunctions::fnIsText(args);
        }
        if n == "ISERROR" {
            return FormulaFunctions::fnIsError(args);
        }
        if n == "ISERR" {
            return FormulaFunctions::fnIsErr(args);
        }
        if n == "N" {
            return FormulaFunctions::fnN(args);
        }
        if n == "T" {
            return FormulaFunctions::fnT(args);
        }
        if n == "NA" {
            return FormulaFunctions::fnNa(args);
        }
        if n == "DATE" {
            return FormulaFunctions::fnDate(args);
        }
        if n == "YEAR" {
            return FormulaFunctions::fnYear(args);
        }
        if n == "MONTH" {
            return FormulaFunctions::fnMonth(args);
        }
        if n == "DAY" {
            return FormulaFunctions::fnDay(args);
        }
        if n == "WEEKDAY" {
            return FormulaFunctions::fnWeekday(args);
        }
        if n == "EDATE" {
            return FormulaFunctions::fnEdate(args);
        }
        if n == "EOMONTH" {
            return FormulaFunctions::fnEomonth(args);
        }
        if n == "DAYS" {
            return FormulaFunctions::fnDays(args);
        }
        if n == "DATEVALUE" {
            return FormulaFunctions::fnDatevalue(args);
        }
        if n == "TEXT" {
            return FormulaFunctions::fnText(args);
        }
        if n == "ROUNDUP" {
            return FormulaFunctions::fnRoundup(args);
        }
        if n == "ROUNDDOWN" {
            return FormulaFunctions::fnRounddown(args);
        }
        if n == "TRUNC" {
            return FormulaFunctions::fnTrunc(args);
        }
        if n == "MEDIAN" {
            return FormulaFunctions::fnMedian(args);
        }
        if n == "COUNTBLANK" {
            return FormulaFunctions::fnCountblank(args);
        }
        if n == "COUNTIF" {
            return FormulaFunctions::fnCountif(args);
        }
        if n == "SUMIF" {
            return FormulaFunctions::fnSumif(args);
        }
        if n == "AVERAGEIF" {
            return FormulaFunctions::fnAverageif(args);
        }
        if n == "COUNTIFS" {
            return FormulaFunctions::fnCountifs(args);
        }
        if n == "SUMIFS" {
            return FormulaFunctions::fnSumifs(args);
        }
        if n == "AVERAGEIFS" {
            return FormulaFunctions::fnAverageifs(args);
        }
        if n == "CHOOSE" {
            return FormulaFunctions::fnChoose(args);
        }
        if n == "VLOOKUP" {
            return FormulaFunctions::fnVlookup(args);
        }
        if n == "HLOOKUP" {
            return FormulaFunctions::fnHlookup(args);
        }
        if n == "XLOOKUP" {
            return FormulaFunctions::fnXlookup(args);
        }
        if n == "LOOKUP" {
            return FormulaFunctions::fnLookup(args);
        }
        if n == "INDEX" {
            return FormulaFunctions::fnIndex(args);
        }
        if n == "MATCH" {
            return FormulaFunctions::fnMatch(args);
        }
        if n == "FILTER" {
            return FormulaFunctions::fnFilter(args);
        }
        if n == "UNIQUE" {
            return FormulaFunctions::fnUnique(args);
        }
        if n == "SORT" {
            return FormulaFunctions::fnSort(args);
        }
        if n == "SUMPRODUCT" {
            return FormulaFunctions::fnSumproduct(args);
        }
        if n == "LARGE" {
            return FormulaFunctions::fnLarge(args);
        }
        if n == "SMALL" {
            return FormulaFunctions::fnSmall(args);
        }
        if n == "SUBSTITUTE" {
            return FormulaFunctions::fnSubstitute(args);
        }
        if n == "FIND" {
            return FormulaFunctions::fnFind(args);
        }
        if n == "SEARCH" {
            return FormulaFunctions::fnSearch(args);
        }
        if n == "REPT" {
            return FormulaFunctions::fnRept(args);
        }
        if n == "PROPER" {
            return FormulaFunctions::fnProper(args);
        }
        if n == "EXACT" {
            return FormulaFunctions::fnExact(args);
        }
        if n == "CHAR" {
            return FormulaFunctions::fnChar(args);
        }
        if n == "CODE" {
            return FormulaFunctions::fnCode(args);
        }
        if n == "TRANSPOSE" {
            return FormulaFunctions::fnTranspose(args);
        }
        FormulaValue::fromError("#NAME?".to_string())
    }
    /// --- helpers: collect numeric scalars from args (flattens nothing; engine expands ranges) ---
    /// Every argument, with ranges opened out. A function that only ever wanted
    /// a list of values gets one whether it was handed cells, a range, or both.
    pub fn flatArgs(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Vec<Rc<RefCell<FormulaValue>>> {
        let mut out: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut i: i64 = 0;
        while i < (args.len() as i64) {
            let v: Rc<RefCell<FormulaValue>> = args[i as usize].clone();
            if v.borrow().isArray() {
                let mut k: i64 = 0;
                while k < (v.borrow().items.len() as i64) {
                    out.push(v.borrow().items[k as usize].clone());
                    k += 1;
                }
            } else {
                out.push(v.clone());
            }
            i += 1;
        }
        out
    }
    /// The numbers among the arguments, arrays expanded; text and blanks skipped.
    pub fn collectNumbers(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Vec<f64> {
        let mut vals: Vec<f64> = Vec::new();
        for v in FormulaFunctions::flatArgs(args).iter() {
            if v.borrow().isNumeric() {
                vals.push(v.borrow().numberValue());
            }
        }
        vals
    }
    pub fn needArity(args: &Vec<Rc<RefCell<FormulaValue>>>, min_n: i64) -> Rc<RefCell<FormulaValue>> {
        if (args.len() as i64) < min_n {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        FormulaValue::blank()
    }
    /// --- aggregates ---
    /// Variance and standard deviation of the numbers in args: `sample` divides
    /// by n - 1 (VAR.S, STDEV.S), otherwise by n (VAR.P, STDEV.P).
    pub fn fnSpread(args: &Vec<Rc<RefCell<FormulaValue>>>, sample: bool, root: bool) -> Rc<RefCell<FormulaValue>> {
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let vals: Vec<f64> = FormulaFunctions::collectNumbers(args);
        let n: i64 = vals.len() as i64;
        let mut need: i64 = 1;
        if sample {
            need = 2;
        }
        if n < need {
            return FormulaValue::fromError("#DIV/0!".to_string());
        }
        let mut mean: f64 = 0.0;
        let mut i: i64 = 0;
        while i < n {
            mean += vals[i as usize];
            i += 1;
        }
        mean /= n as f64;
        let mut ss: f64 = 0.0;
        i = 0;
        while i < n {
            let d: f64 = vals[i as usize] - mean;
            ss += d * d;
            i += 1;
        }
        let mut div: f64 = n as f64;
        if sample {
            div = (n - 1) as f64;
        }
        let v: f64 = ss / div;
        if root {
            return FormulaValue::fromNumber(v.sqrt());
        }
        FormulaValue::fromNumber(v)
    }
    /// SUBTOTAL(function_num, ref1, …): 1–11 (and 101–111, which also skip
    /// hidden rows; rows are not known here, so they count the same) pick
    /// AVERAGE, COUNT, COUNTA, MAX, MIN, PRODUCT, STDEV, STDEVP, SUM, VAR, VARP.
    /// The time-value-of-money family, on Excel's sign convention (money paid
    /// out is negative): rate, nper, pmt, pv, fv and type, each solved from the
    /// others by  pv*(1+r)^n + pmt*(1+r*type)*((1+r)^n - 1)/r + fv = 0.
    pub fn lnD(x0: f64) -> f64 {
        let mut x: f64 = x0;
        let mut k: i64 = 0;
        while x >= 2.718281828459045 {
            x /= 2.718281828459045;
            k += 1;
        }
        while x < 1.0 {
            x *= 2.718281828459045;
            k -= 1;
        }
        let y: f64 = (x - 1.0) / (x + 1.0);
        let y2: f64 = y * y;
        let mut term: f64 = y;
        let mut sum: f64 = y;
        let mut n: i64 = 1;
        while n < 40 {
            term *= y2;
            sum += term / ((2 * n + 1) as f64);
            n += 1;
        }
        2.0 * sum + (k as f64)
    }
    pub fn expD(x0: f64) -> f64 {
        // e^x = (e^(x/2^m))^(2^m), the small power by its series.
        let mut x: f64 = x0;
        let mut m: i64 = 0;
        while x > 0.5 || x < -0.5 {
            x /= 2.0;
            m += 1;
        }
        let mut term: f64 = 1.0;
        let mut sum: f64 = 1.0;
        let mut n: i64 = 1;
        while n < 25 {
            term = (term * x) / (n as f64);
            sum += term;
            n += 1;
        }
        while m > 0 {
            sum *= sum;
            m -= 1;
        }
        sum
    }
    pub fn powD(b: f64, e: f64) -> f64 {
        let ei: i64 = e.floor() as i64;
        if ((ei as f64) == e && ei >= 0) && ei < 100000 {
            let mut out: f64 = 1.0;
            let mut i: i64 = 0;
            while i < ei {
                out *= b;
                i += 1;
            }
            return out;
        }
        if b <= 0.0 {
            return 0.0;
        }
        FormulaFunctions::expD(e * FormulaFunctions::lnD(b))
    }
    pub fn tvmArg(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64, dflt: f64) -> f64 {
        if i >= (args.len() as i64) {
            return dflt;
        }
        let v: Rc<RefCell<FormulaValue>> = args[i as usize].clone();
        if v.borrow().isBlank() {
            return dflt;
        }
        let out = v.borrow().asNumber();
        out
    }
    pub fn fnTvm(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let mut need: i64 = 3;
        if n == "SLN" {
            need = 3;
        }
        if (args.len() as i64) < need {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let mut ai: i64 = 0;
        while ai < (args.len() as i64) {
            let e: Rc<RefCell<FormulaValue>> = args[ai as usize].clone();
            if e.borrow().isError() {
                return e;
            }
            ai += 1;
        }
        if n == "SLN" {
            let life: f64 = FormulaFunctions::tvmArg(args, 2, 0.0);
            if life == 0.0 {
                return FormulaValue::fromError("#DIV/0!".to_string());
            }
            return FormulaValue::fromNumber((FormulaFunctions::tvmArg(
                args,
                0,
                0.0
            ) - FormulaFunctions::tvmArg(args, 1, 0.0)) / life).clone();
        }
        let r: f64 = FormulaFunctions::tvmArg(args, 0, 0.0);
        if n == "NPER" {
            let pmt_n: f64 = FormulaFunctions::tvmArg(args, 1, 0.0);
            let pv_n: f64 = FormulaFunctions::tvmArg(args, 2, 0.0);
            let fv_n: f64 = FormulaFunctions::tvmArg(args, 3, 0.0);
            let t_n: f64 = FormulaFunctions::tvmArg(args, 4, 0.0);
            if r == 0.0 {
                if pmt_n == 0.0 {
                    return FormulaValue::fromError("#NUM!".to_string());
                }
                return FormulaValue::fromNumber((0.0 - (pv_n + fv_n)) / pmt_n);
            }
            let k: f64 = (pmt_n * (1.0 + r * t_n)) / r;
            let num: f64 = k - fv_n;
            let den: f64 = k + pv_n;
            if den == 0.0 || num / den <= 0.0 {
                return FormulaValue::fromError("#NUM!".to_string());
            }
            return FormulaValue::fromNumber(FormulaFunctions::lnD(num / den) / FormulaFunctions::lnD(1.0 + r));
        }
        let nper: f64 = FormulaFunctions::tvmArg(args, 1, 0.0);
        let x: f64 = FormulaFunctions::tvmArg(args, 2, 0.0);
        let y: f64 = FormulaFunctions::tvmArg(args, 3, 0.0);
        let t: f64 = FormulaFunctions::tvmArg(args, 4, 0.0);
        let g: f64 = FormulaFunctions::powD(1.0 + r, nper);
        let mut annuity: f64 = nper;
        if r != 0.0 {
            annuity = ((1.0 + r * t) * (g - 1.0)) / r;
        }
        if n == "PMT" {
            // PMT(rate, nper, pv, [fv], [type])
            if annuity == 0.0 {
                return FormulaValue::fromError("#NUM!".to_string());
            }
            return FormulaValue::fromNumber((0.0 - (x * g + y)) / annuity);
        }
        if n == "PV" {
            // PV(rate, nper, pmt, [fv], [type])
            if g == 0.0 {
                return FormulaValue::fromError("#NUM!".to_string());
            }
            return FormulaValue::fromNumber((0.0 - (x * annuity + y)) / g);
        }
        // FV(rate, nper, pmt, [pv], [type])
        FormulaValue::fromNumber(0.0 - (y * g + x * annuity))
    }
    /// NPV(rate, value1, …): the values discounted from the end of period 1.
    pub fn fnNpv(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if (args.len() as i64) < 2 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let first: Rc<RefCell<FormulaValue>> = args[0].clone();
        if first.borrow().isError() {
            return first;
        }
        let r: f64 = first.borrow().asNumber();
        let mut rest: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut i: i64 = 1;
        while i < (args.len() as i64) {
            rest.push(args[i as usize].clone());
            i += 1;
        }
        let vals: Vec<f64> = FormulaFunctions::collectNumbers(&rest);
        let mut total: f64 = 0.0;
        let mut k: i64 = 0;
        while k < (vals.len() as i64) {
            total += vals[k as usize] / FormulaFunctions::powD(1.0 + r, (k + 1) as f64);
            k += 1;
        }
        FormulaValue::fromNumber(total)
    }
    pub fn fnSubtotal(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if (args.len() as i64) < 2 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let f0: Rc<RefCell<FormulaValue>> = args[0].clone();
        if f0.borrow().isError() {
            return f0;
        }
        let mut code: i64 = (f0.borrow().asNumber()).floor() as i64;
        if code > 100 {
            code -= 100;
        }
        let mut rest: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut i: i64 = 1;
        while i < (args.len() as i64) {
            rest.push(args[i as usize].clone());
            i += 1;
        }
        if code == 1 {
            return FormulaFunctions::fnAverage(&rest);
        }
        if code == 2 {
            return FormulaFunctions::fnCount(&rest);
        }
        if code == 3 {
            return FormulaFunctions::fnCountA(&rest);
        }
        if code == 4 {
            return FormulaFunctions::fnMax(&rest);
        }
        if code == 5 {
            return FormulaFunctions::fnMin(&rest);
        }
        if code == 6 {
            return FormulaFunctions::fnProduct(&rest);
        }
        if code == 7 {
            return FormulaFunctions::fnSpread(&rest, true, true);
        }
        if code == 8 {
            return FormulaFunctions::fnSpread(&rest, false, true);
        }
        if code == 9 {
            return FormulaFunctions::fnSum(&rest);
        }
        if code == 10 {
            return FormulaFunctions::fnSpread(&rest, true, false);
        }
        if code == 11 {
            return FormulaFunctions::fnSpread(&rest, false, false);
        }
        FormulaValue::fromError("#VALUE!".to_string())
    }
    pub fn fnSum(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let vals: Vec<f64> = FormulaFunctions::collectNumbers(args);
        let mut sum: f64 = 0.0;
        let mut i: i64 = 0;
        let n: i64 = vals.len() as i64;
        while i < n {
            sum += vals[i as usize];
            i += 1;
        }
        FormulaValue::fromNumber(sum)
    }
    pub fn fnAverage(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let vals: Vec<f64> = FormulaFunctions::collectNumbers(args);
        let n: i64 = vals.len() as i64;
        if n == 0 {
            return FormulaValue::fromError("#DIV/0!".to_string());
        }
        let mut sum: f64 = 0.0;
        let mut i: i64 = 0;
        while i < n {
            sum += vals[i as usize];
            i += 1;
        }
        FormulaValue::fromNumber(sum / (n as f64))
    }
    pub fn fnMin(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let vals: Vec<f64> = FormulaFunctions::collectNumbers(args);
        if vals.is_empty() {
            return FormulaValue::fromNumber(0.0);
        }
        let mut mn: f64 = vals[0];
        let mut i: i64 = 1;
        let n: i64 = vals.len() as i64;
        while i < n {
            let x: f64 = vals[i as usize];
            if x < mn {
                mn = x;
            }
            i += 1;
        }
        FormulaValue::fromNumber(mn)
    }
    pub fn fnMax(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let vals: Vec<f64> = FormulaFunctions::collectNumbers(args);
        if vals.is_empty() {
            return FormulaValue::fromNumber(0.0);
        }
        let mut mx: f64 = vals[0];
        let mut i: i64 = 1;
        let n: i64 = vals.len() as i64;
        while i < n {
            let x: f64 = vals[i as usize];
            if x > mx {
                mx = x;
            }
            i += 1;
        }
        FormulaValue::fromNumber(mx)
    }
    pub fn fnCount(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let vals: Vec<f64> = FormulaFunctions::collectNumbers(args);
        FormulaValue::fromNumber((vals.len() as i64) as f64)
    }
    pub fn fnCountA(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(args);
        let mut c: i64 = 0;
        let mut i: i64 = 0;
        let n: i64 = flat.len() as i64;
        while i < n {
            let v: Rc<RefCell<FormulaValue>> = flat[i as usize].clone();
            let b: bool = v.borrow().isBlank();
            if !b {
                c += 1;
            }
            i += 1;
        }
        FormulaValue::fromNumber(c as f64)
    }
    pub fn fnProduct(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let vals: Vec<f64> = FormulaFunctions::collectNumbers(args);
        if vals.is_empty() {
            return FormulaValue::fromNumber(0.0);
        }
        let mut p: f64 = 1.0;
        let mut i: i64 = 0;
        let n: i64 = vals.len() as i64;
        while i < n {
            p *= vals[i as usize];
            i += 1;
        }
        FormulaValue::fromNumber(p)
    }
    /// --- logical ---
    pub fn fnIf(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let cond: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ce: bool = cond.borrow().isError();
        if ce {
            return cond;
        }
        let ct: bool = cond.borrow().isTrue();
        if ct {
            return args[1].clone();
        }
        if (args.len() as i64) >= 3 {
            return args[2].clone();
        }
        FormulaValue::fromBool(false)
    }
    /// IFS(cond1, val1, cond2, val2, …) — first true condition wins; else #N/A.
    pub fn fnIfs(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let n: i64 = args.len() as i64;
        if n < 2 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if n - (n / 2) * 2 != 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let mut i: i64 = 0;
        while i < n {
            let cond: Rc<RefCell<FormulaValue>> = args[i as usize].clone();
            let ce: bool = cond.borrow().isError();
            if ce {
                return cond;
            }
            if cond.borrow().isTrue() {
                return args[(i + 1) as usize].clone();
            }
            i += 2;
        }
        FormulaValue::fromError("#N/A".to_string())
    }
    /// SWITCH(expr, v1, r1, …, [default]) — first equal value wins.
    pub fn fnSwitch(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 3);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let expr: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = expr.borrow().isError();
        if ee {
            return expr;
        }
        let mut i: i64 = 1;
        let n: i64 = args.len() as i64;
        while i + 1 < n {
            let cand: Rc<RefCell<FormulaValue>> = args[i as usize].clone();
            let ce: bool = cand.borrow().isError();
            if ce {
                return cand;
            }
            if FormulaFunctions::valuesEqualLoose(&expr, &cand) {
                return args[(i + 1) as usize].clone();
            }
            i += 2;
        }
        if i < n {
            return args[i as usize].clone();
        }
        FormulaValue::fromError("#N/A".to_string())
    }
    pub fn fnAnd(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let mut i: i64 = 0;
        let n: i64 = args.len() as i64;
        if n == 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        while i < n {
            let v: Rc<RefCell<FormulaValue>> = args[i as usize].clone();
            let vt: bool = v.borrow().isTrue();
            if !vt {
                return FormulaValue::fromBool(false);
            }
            i += 1;
        }
        FormulaValue::fromBool(true)
    }
    pub fn fnOr(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let mut i: i64 = 0;
        let n: i64 = args.len() as i64;
        if n == 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        while i < n {
            let v: Rc<RefCell<FormulaValue>> = args[i as usize].clone();
            let vt: bool = v.borrow().isTrue();
            if vt {
                return FormulaValue::fromBool(true);
            }
            i += 1;
        }
        FormulaValue::fromBool(false)
    }
    /// XOR — true when an odd number of arguments are true (Excel).
    pub fn fnXor(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let n: i64 = args.len() as i64;
        if n == 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let mut hits: i64 = 0;
        let mut i: i64 = 0;
        while i < n {
            if args[i as usize].borrow().isTrue() {
                hits += 1;
            }
            i += 1;
        }
        let odd: i64 = hits - (hits / 2) * 2;
        FormulaValue::fromBool(odd != 0)
    }
    pub fn fnNot(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let vt: bool = v.borrow().isTrue();
        if vt {
            return FormulaValue::fromBool(false);
        }
        FormulaValue::fromBool(true)
    }
    pub fn fnIfError(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return args[1].clone();
        }
        v
    }
    pub fn fnIfNa(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        if v.borrow().kind == 4 {
            if v.borrow().err == "#N/A" {
                return args[1].clone();
            }
        }
        v
    }
    pub fn fnNa(_args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaValue::fromError("#N/A".to_string())
    }
    /// --- math ---
    pub fn fnAbs(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let mut x: f64 = v.borrow().asNumber();
        if x < 0.0 {
            x = 0.0 - x;
        }
        FormulaValue::fromNumber(x)
    }
    pub fn fnRound(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let mut digits: i64 = 0;
        if (args.len() as i64) >= 2 {
            let d: Rc<RefCell<FormulaValue>> = args[1].clone();
            digits = (d.borrow().asNumber()).floor() as i64;
        }
        if digits < 0 {
            digits = 0;
        }
        let mut scale: f64 = 1.0;
        let mut i: i64 = 0;
        while i < digits {
            scale *= 10.0;
            i += 1;
        }
        let xx: f64 = v.borrow().asNumber();
        let rr: f64 = (((xx * scale + 0.5).floor() as i64) as f64) / scale;
        FormulaValue::fromNumber(rr)
    }
    pub fn fnInt(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let x: f64 = v.borrow().asNumber();
        // Excel INT = floor toward -inf (Ranger `floor` matches).
        FormulaValue::fromNumber((x.floor() as i64) as f64)
    }
    pub fn fnMod(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let a: Rc<RefCell<FormulaValue>> = args[0].clone();
        let b: Rc<RefCell<FormulaValue>> = args[1].clone();
        let ea: bool = a.borrow().isError();
        if ea {
            return a;
        }
        let eb: bool = b.borrow().isError();
        if eb {
            return b;
        }
        let y: f64 = b.borrow().asNumber();
        if y == 0.0 {
            return FormulaValue::fromError("#DIV/0!".to_string());
        }
        let x: f64 = a.borrow().asNumber();
        let q: i64 = (x / y).floor() as i64;
        let r: f64 = x - (q as f64) * y;
        FormulaValue::fromNumber(r)
    }
    pub fn fnPower(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let a: Rc<RefCell<FormulaValue>> = args[0].clone();
        let b: Rc<RefCell<FormulaValue>> = args[1].clone();
        let ea: bool = a.borrow().isError();
        if ea {
            return a;
        }
        let eb: bool = b.borrow().isError();
        if eb {
            return b;
        }
        let base: f64 = a.borrow().asNumber();
        let exp: i64 = (b.borrow().asNumber()).floor() as i64;
        if exp < 0 {
            return FormulaValue::fromError("#NUM!".to_string());
        }
        let mut acc: f64 = 1.0;
        let mut i: i64 = 0;
        while i < exp {
            acc *= base;
            i += 1;
        }
        FormulaValue::fromNumber(acc)
    }
    pub fn fnSqrt(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let x: f64 = v.borrow().asNumber();
        if x < 0.0 {
            return FormulaValue::fromError("#NUM!".to_string());
        }
        // Newton iterations
        if x == 0.0 {
            return FormulaValue::fromNumber(0.0);
        }
        let mut g: f64 = x / 2.0;
        if g < 1.0 {
            g = 1.0;
        }
        let mut k: i64 = 0;
        while k < 20 {
            g = (g + x / g) / 2.0;
            k += 1;
        }
        FormulaValue::fromNumber(g)
    }
    pub fn fnSign(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let x: f64 = v.borrow().asNumber();
        if x < 0.0 {
            return FormulaValue::fromNumber(-1.0);
        }
        if x > 0.0 {
            return FormulaValue::fromNumber(1.0);
        }
        FormulaValue::fromNumber(0.0)
    }
    pub fn fnExp(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        // series e^x for small |x|; otherwise #NUM! for extreme
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let x: f64 = v.borrow().asNumber();
        if x > 20.0 {
            return FormulaValue::fromError("#NUM!".to_string());
        }
        if x < -20.0 {
            return FormulaValue::fromNumber(0.0);
        }
        let mut term: f64 = 1.0;
        let mut sum: f64 = 1.0;
        let mut i: i64 = 1;
        while i < 40 {
            term = (term * x) / (i as f64);
            sum += term;
            i += 1;
        }
        FormulaValue::fromNumber(sum)
    }
    pub fn fnLn(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let mut x: f64 = v.borrow().asNumber();
        if x <= 0.0 {
            return FormulaValue::fromError("#NUM!".to_string());
        }
        // artanh series on (x-1)/(x+1) for x near 1; reduce via *2 factors
        let mut k: i64 = 0;
        while x >= 2.718281828 {
            x /= 2.718281828;
            k += 1;
        }
        while x < 1.0 {
            x *= 2.718281828;
            k -= 1;
        }
        let y: f64 = (x - 1.0) / (x + 1.0);
        let y2: f64 = y * y;
        let mut term: f64 = y;
        let mut sum: f64 = y;
        let mut n: i64 = 1;
        while n < 40 {
            term *= y2;
            sum += term / ((2 * n + 1) as f64);
            n += 1;
        }
        FormulaValue::fromNumber(2.0 * sum + (k as f64))
    }
    pub fn fnLog10(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ln: Rc<RefCell<FormulaValue>> = FormulaFunctions::fnLn(args);
        let ee: bool = ln.borrow().isError();
        if ee {
            return ln;
        }
        let out = FormulaValue::fromNumber(ln.borrow().num / 2.302585092994046);
        out
    }
    pub fn fnFloor(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::fnInt(args)
    }
    pub fn fnCeiling(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let x: f64 = v.borrow().asNumber();
        let t: i64 = x.floor() as i64;
        if (t as f64) == x {
            return FormulaValue::fromNumber(t as f64);
        }
        if x > 0.0 {
            return FormulaValue::fromNumber((t + 1) as f64);
        }
        FormulaValue::fromNumber(t as f64)
    }
    /// --- text ---
    pub fn fnLen(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let out = FormulaValue::fromNumber(DgNative::str_len(&v.borrow().asDisplay()) as f64);
        out
    }
    pub fn fnLeft(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let mut n: i64 = 1;
        if (args.len() as i64) >= 2 {
            let n_arg: Rc<RefCell<FormulaValue>> = args[1].clone();
            n = (n_arg.borrow().asNumber()).floor() as i64;
        }
        if n < 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let s: String = v.borrow().asDisplay();
        let L: i64 = DgNative::str_len(&s);
        if n > L {
            n = L;
        }
        FormulaValue::fromString(DgNative::substr(&s, 0, n))
    }
    pub fn fnRight(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let mut n: i64 = 1;
        if (args.len() as i64) >= 2 {
            let n_arg: Rc<RefCell<FormulaValue>> = args[1].clone();
            n = (n_arg.borrow().asNumber()).floor() as i64;
        }
        if n < 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let s: String = v.borrow().asDisplay();
        let L: i64 = DgNative::str_len(&s);
        if n > L {
            n = L;
        }
        FormulaValue::fromString(DgNative::substr(&s, L - n, L))
    }
    pub fn fnMid(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 3);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let start_arg: Rc<RefCell<FormulaValue>> = args[1].clone();
        let n_arg: Rc<RefCell<FormulaValue>> = args[2].clone();
        let start: i64 = (start_arg.borrow().asNumber()).floor() as i64;
        let n: i64 = (n_arg.borrow().asNumber()).floor() as i64;
        if start < 1 || n < 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let s: String = v.borrow().asDisplay();
        let L: i64 = DgNative::str_len(&s);
        let i0: i64 = start - 1;
        if i0 >= L {
            return FormulaValue::fromString(String::new());
        }
        let mut i1: i64 = i0 + n;
        if i1 > L {
            i1 = L;
        }
        FormulaValue::fromString(DgNative::substr(&s, i0, i1))
    }
    pub fn fnUpper(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let out = FormulaValue::fromString(FormulaFunctions::upper(&v.borrow().asDisplay()));
        out
    }
    pub fn fnLower(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let out = FormulaValue::fromString(FormulaFunctions::lower(&v.borrow().asDisplay()));
        out
    }
    pub fn fnTrim(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let s: String = v.borrow().asDisplay();
        let n: i64 = DgNative::str_len(&s);
        let mut a: i64 = 0;
        while a < n {
            let ch: i64 = DgNative::char_at(&s, a);
            if ch == 32 || ch == 9 {
                a += 1;
            } else {
                break;
            }
        }
        let mut b: i64 = n;
        while b > a {
            let ch2: i64 = DgNative::char_at(&s, b - 1);
            if ch2 == 32 || ch2 == 9 {
                b -= 1;
            } else {
                break;
            }
        }
        FormulaValue::fromString(DgNative::substr(&s, a, b))
    }
    pub fn fnConcat(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(args);
        let mut out: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = flat.len() as i64;
        while i < n {
            let v: Rc<RefCell<FormulaValue>> = flat[i as usize].clone();
            let ee: bool = v.borrow().isError();
            if ee {
                return v;
            }
            out = format!("{}{}", out, v.borrow().asDisplay());
            i += 1;
        }
        FormulaValue::fromString(out.clone())
    }
    /// TEXTJOIN(delimiter, ignore_empty, text1, …)
    pub fn fnTextjoin(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 3);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let delim: String = args[0].borrow().toText();
        let ignore_empty: bool = args[1].borrow().isTrue();
        let mut rest: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut i: i64 = 2;
        while i < (args.len() as i64) {
            rest.push(args[i as usize].clone());
            i += 1;
        }
        let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&rest);
        let mut out: String = String::new();
        let mut first: bool = true;
        i = 0;
        while i < (flat.len() as i64) {
            let v: Rc<RefCell<FormulaValue>> = flat[i as usize].clone();
            let ee: bool = v.borrow().isError();
            if ee {
                return v;
            }
            let piece: String = v.borrow().asDisplay();
            let mut skip: bool = false;
            if ignore_empty {
                if piece.is_empty() {
                    skip = true;
                }
                if v.borrow().isBlank() {
                    skip = true;
                }
            }
            if !skip {
                if first {
                    first = false;
                } else {
                    out = format!("{}{}", out, delim);
                }
                out = format!("{}{}", out, piece);
            }
            i += 1;
        }
        FormulaValue::fromString(out.clone())
    }
    /// REPLACE(old_text, start_num, num_chars, new_text) — 1-based start.
    pub fn fnReplace(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 4);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v0: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v0.borrow().isError();
        if ee {
            return v0;
        }
        let s: String = v0.borrow().asDisplay();
        let start: i64 = (args[1].borrow().asNumber()).floor() as i64;
        let num: i64 = (args[2].borrow().asNumber()).floor() as i64;
        let neu: String = args[3].borrow().asDisplay();
        if start < 1 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if num < 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let n: i64 = DgNative::str_len(&s);
        let mut left_end: i64 = start - 1;
        if left_end > n {
            left_end = n;
        }
        let mut right_start: i64 = left_end + num;
        if right_start > n {
            right_start = n;
        }
        let left: String = DgNative::substr(&s, 0, left_end);
        let right: String = DgNative::substr(&s, right_start, n);
        FormulaValue::fromString(format!("{}{}{}", left, neu, right))
    }
    /// Dates. A spreadsheet date is a day count with a format, so every one of
    /// these is arithmetic — see DateSerial for the epoch and its 1900 quirk.
    pub fn fnDate(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 3);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let y: i64 = (args[0].borrow().asNumber()).floor() as i64;
        let m: i64 = (args[1].borrow().asNumber()).floor() as i64;
        let d: i64 = (args[2].borrow().asNumber()).floor() as i64;
        // Excel rolls months and days that run past their range, so DATE(2024
        // 13 1) is January 2025 rather than an error.
        let total: i64 = y * 12 + (m - 1);
        let ny: i64 = total / 12;
        let nm: i64 = (total - ny * 12) + 1;
        let base: f64 = DateSerial::fromYMD(ny, nm, 1);
        FormulaValue::fromNumber(base + ((d - 1) as f64))
    }
    pub fn dateParts(v: &Rc<RefCell<FormulaValue>>) -> Vec<i64> {
        let out: Vec<i64> = DateSerial::toYMD(v.borrow().asNumber());
        out
    }
    pub fn fnYear(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ve: bool = v.borrow().isError();
        if ve {
            return v;
        }
        let parts: Vec<i64> = FormulaFunctions::dateParts(&v);
        FormulaValue::fromNumber((parts[0]) as f64)
    }
    pub fn fnMonth(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ve: bool = v.borrow().isError();
        if ve {
            return v;
        }
        let parts: Vec<i64> = FormulaFunctions::dateParts(&v);
        FormulaValue::fromNumber((parts[1]) as f64)
    }
    pub fn fnDay(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ve: bool = v.borrow().isError();
        if ve {
            return v;
        }
        let parts: Vec<i64> = FormulaFunctions::dateParts(&v);
        FormulaValue::fromNumber((parts[2]) as f64)
    }
    /// WEEKDAY(serial, [type]). Type 1 (the default) counts Sunday as 1, type 2
    /// counts Monday as 1, type 3 counts Monday as 0.
    pub fn fnWeekday(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ve: bool = v.borrow().isError();
        if ve {
            return v;
        }
        let mut w: i64 = DateSerial::weekday(v.borrow().asNumber());
        let mut mode: i64 = 1;
        if (args.len() as i64) >= 2 {
            mode = (args[1].borrow().asNumber()).floor() as i64;
        }
        if mode == 2 {
            w -= 1;
            if w < 1 {
                w = 7;
            }
        }
        if mode == 3 {
            w -= 2;
            if w < 0 {
                w += 7;
            }
        }
        FormulaValue::fromNumber(w as f64)
    }
    pub fn fnEdate(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let base: f64 = args[0].borrow().asNumber();
        let months: i64 = (args[1].borrow().asNumber()).floor() as i64;
        FormulaValue::fromNumber(DateSerial::addMonths(base, months))
    }
    pub fn fnEomonth(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let base: f64 = args[0].borrow().asNumber();
        let months: i64 = (args[1].borrow().asNumber()).floor() as i64;
        FormulaValue::fromNumber(DateSerial::endOfMonth(base, months))
    }
    pub fn fnDays(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let end_d: f64 = args[0].borrow().asNumber();
        let start_d: f64 = args[1].borrow().asNumber();
        FormulaValue::fromNumber(end_d - start_d)
    }
    pub fn fnDatevalue(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ve: bool = v.borrow().isError();
        if ve {
            return v;
        }
        let text: String = v.borrow().toText();
        if DateSerial::isDate(&text) {
            return FormulaValue::fromNumber(DateSerial::parseDate(&text));
        }
        FormulaValue::fromError("#VALUE!".to_string())
    }
    /// TEXT(value, format) — the number formatter, reachable from a formula.
    pub fn fnText(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let raw: String = args[0].borrow().asRaw();
        let pat: String = args[1].borrow().toText();
        FormulaValue::fromString(XlsxNumberFormat::format(&raw, &pat))
    }
    /// More arithmetic
    pub fn powTen(digits: i64) -> f64 {
        let mut scale: f64 = 1.0;
        let mut i: i64 = 0;
        let mut n: i64 = digits;
        if n < 0 {
            n = 0 - n;
        }
        while i < n {
            scale *= 10.0;
            i += 1;
        }
        if digits < 0 {
            return 1.0 / scale;
        }
        scale
    }
    pub fn roundAway(args: &Vec<Rc<RefCell<FormulaValue>>>, up: bool) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ve: bool = v.borrow().isError();
        if ve {
            return v;
        }
        let mut digits: i64 = 0;
        if (args.len() as i64) >= 2 {
            digits = (args[1].borrow().asNumber()).floor() as i64;
        }
        let scale: f64 = FormulaFunctions::powTen(digits);
        let mut x: f64 = v.borrow().asNumber() * scale;
        let neg: bool = x < 0.0;
        if neg {
            x = 0.0 - x;
        }
        let mut whole: f64 = (x.floor() as i64) as f64;
        if up {
            if x > whole {
                whole += 1.0;
            }
        }
        if neg {
            whole = 0.0 - whole;
        }
        FormulaValue::fromNumber(whole / scale)
    }
    pub fn fnRoundup(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::roundAway(args, true)
    }
    pub fn fnRounddown(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::roundAway(args, false)
    }
    pub fn fnTrunc(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::roundAway(args, false)
    }
    pub fn sortDoubles(mut vals: Vec<f64>) -> Vec<f64> {
        let n: i64 = vals.len() as i64;
        let mut i: i64 = 1;
        while i < n {
            let key: f64 = vals[i as usize];
            let mut j: i64 = i - 1;
            while j >= 0 {
                let cur: f64 = vals[j as usize];
                if cur > key {
                    vals[(j + 1) as usize] = cur;
                    j -= 1;
                } else {
                    j = -1;
                }
            }
            vals[(j + 1) as usize] = key;
            i += 1;
        }
        vals
    }
    pub fn fnMedian(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let mut vals: Vec<f64> = FormulaFunctions::collectNumbers(args);
        let n: i64 = vals.len() as i64;
        if n == 0 {
            return FormulaValue::fromError("#NUM!".to_string());
        }
        vals = FormulaFunctions::sortDoubles(vals);
        let mid: i64 = n / 2;
        if n % 2 == 1 {
            return FormulaValue::fromNumber(vals[mid as usize]);
        }
        FormulaValue::fromNumber((vals[(mid - 1) as usize] + vals[mid as usize]) / 2.0)
    }
    pub fn nthOrdered(args: &Vec<Rc<RefCell<FormulaValue>>>, from_top: bool) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let head: Vec<Rc<RefCell<FormulaValue>>> = vec![args[0].clone()];
        let mut vals: Vec<f64> = FormulaFunctions::collectNumbers(&head);
        let k: i64 = (args[1].borrow().asNumber()).floor() as i64;
        let n: i64 = vals.len() as i64;
        if k < 1 {
            return FormulaValue::fromError("#NUM!".to_string());
        }
        if k > n {
            return FormulaValue::fromError("#NUM!".to_string());
        }
        vals = FormulaFunctions::sortDoubles(vals);
        if from_top {
            return FormulaValue::fromNumber(vals[(n - k) as usize]);
        }
        FormulaValue::fromNumber(vals[(k - 1) as usize])
    }
    pub fn fnLarge(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::nthOrdered(args, true)
    }
    pub fn fnSmall(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::nthOrdered(args, false)
    }
    pub fn fnCountblank(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(args);
        let mut c: i64 = 0;
        let mut i: i64 = 0;
        while i < (flat.len() as i64) {
            let v: Rc<RefCell<FormulaValue>> = flat[i as usize].clone();
            if v.borrow().isBlank() {
                c += 1;
            }
            i += 1;
        }
        FormulaValue::fromNumber(c as f64)
    }
    /// Conditions. A criterion is Excel's little language: a bare value means
    /// equals, and a leading >, <, >=, <=, <> is the comparison it looks like.
    /// Text order, one code point at a time. Ranger's `<` is for numbers, and a
    /// criterion like ">M" has to mean something for text too.
    pub fn compareText(a: &str, b: &str) -> i64 {
        let an: i64 = DgNative::str_len(&a);
        let bn: i64 = DgNative::str_len(&b);
        let mut n: i64 = an;
        if bn < n {
            n = bn;
        }
        let mut i: i64 = 0;
        while i < n {
            let ca: i64 = DgNative::char_at(&a, i);
            let cb: i64 = DgNative::char_at(&b, i);
            if ca < cb {
                return -1;
            }
            if ca > cb {
                return 1;
            }
            i += 1;
        }
        if an < bn {
            return -1;
        }
        if an > bn {
            return 1;
        }
        0
    }
    pub fn criterionMatches(v: &Rc<RefCell<FormulaValue>>, criterion: &Rc<RefCell<FormulaValue>>) -> bool {
        let c: String = criterion.borrow().toText();
        let mut op: &str = "=";
        let mut rest: String = c.clone();
        if DgNative::str_len(&c) >= 2 {
            let two: String = DgNative::substr(&c, 0, 2);
            if two == ">=" {
                op = ">=";
                rest = DgNative::substr(&c, 2, DgNative::str_len(&c));
            }
            if two == "<=" {
                op = "<=";
                rest = DgNative::substr(&c, 2, DgNative::str_len(&c));
            }
            if two == "<>" {
                op = "<>";
                rest = DgNative::substr(&c, 2, DgNative::str_len(&c));
            }
        }
        if op == "=" {
            if DgNative::str_len(&c) >= 1 {
                let one: String = DgNative::substr(&c, 0, 1);
                if one == ">" {
                    op = ">";
                    rest = DgNative::substr(&c, 1, DgNative::str_len(&c));
                }
                if one == "<" {
                    op = "<";
                    rest = DgNative::substr(&c, 1, DgNative::str_len(&c));
                }
                if one == "=" {
                    op = "=";
                    rest = DgNative::substr(&c, 1, DgNative::str_len(&c));
                }
            }
        }
        let mut target: Rc<RefCell<FormulaValue>> = FormulaValue::fromString(rest.clone());
        let num: Option<f64> = rest.parse::<f64>().ok();
        let mut numeric: bool = false;
        if num.is_some() {
            if XlsxNumberFormat::wholeNumber(&rest) {
                target = FormulaValue::fromNumber(num.unwrap());
                numeric = true;
            }
        }
        let mut cmp: i64 = 0;
        if numeric {
            let a: f64 = v.borrow().asNumber();
            let b: f64 = target.borrow().asNumber();
            if !v.borrow().isNumeric() {
                // Text never satisfies a numeric comparison other than <>.
                if op == "<>" {
                    return true;
                }
                return false;
            }
            if a < b {
                cmp = -1;
            }
            if a > b {
                cmp = 1;
            }
        } else {
            cmp = FormulaFunctions::compareText(&FormulaFunctions::upper(&v.borrow().toText()), &FormulaFunctions::upper(&rest));
        }
        if op == "=" {
            return cmp == 0;
        }
        if op == "<>" {
            return cmp != 0;
        }
        if op == ">" {
            return cmp > 0;
        }
        if op == "<" {
            return cmp < 0;
        }
        if op == ">=" {
            return cmp >= 0;
        }
        if op == "<=" {
            return cmp <= 0;
        }
        false
    }
    pub fn fnCountif(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let mut head: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        head.push(args[0].clone());
        let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&head);
        let crit: Rc<RefCell<FormulaValue>> = args[1].clone();
        let mut c: i64 = 0;
        let mut i: i64 = 0;
        while i < (flat.len() as i64) {
            if FormulaFunctions::criterionMatches(&flat[i as usize].clone(), &crit) {
                c += 1;
            }
            i += 1;
        }
        FormulaValue::fromNumber(c as f64)
    }
    /// SUMIF(range, criterion, [sumRange]) — the third argument is summed in
    /// step with the first, which is what makes "total the sales in the north"
    /// one formula rather than two columns.
    pub fn conditionalTotal(args: &Vec<Rc<RefCell<FormulaValue>>>, want_average: bool) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let mut head: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        head.push(args[0].clone());
        let tested: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&head);
        let mut summed: Vec<Rc<RefCell<FormulaValue>>> = tested.clone();
        if (args.len() as i64) >= 3 {
            let mut third: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            third.push(args[2].clone());
            summed = FormulaFunctions::flatArgs(&third);
        }
        let crit: Rc<RefCell<FormulaValue>> = args[1].clone();
        let mut total: f64 = 0.0;
        let mut hits: i64 = 0;
        let mut i: i64 = 0;
        while i < (tested.len() as i64) {
            if FormulaFunctions::criterionMatches(&tested[i as usize].clone(), &crit) {
                if i < (summed.len() as i64) {
                    let sv: Rc<RefCell<FormulaValue>> = summed[i as usize].clone();
                    if sv.borrow().isNumeric() {
                        total += sv.borrow().numberValue();
                        hits += 1;
                    }
                }
            }
            i += 1;
        }
        if want_average {
            if hits == 0 {
                return FormulaValue::fromError("#DIV/0!".to_string());
            }
            return FormulaValue::fromNumber(total / (hits as f64));
        }
        FormulaValue::fromNumber(total)
    }
    pub fn fnSumif(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::conditionalTotal(args, false)
    }
    pub fn fnAverageif(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::conditionalTotal(args, true)
    }
    /// COUNTIFS / SUMIFS / AVERAGEIFS — AND across criterion pairs, stepped by
    /// index. SUMIFS(sumRange, critRange1, crit1, …); COUNTIFS has only pairs;
    /// AVERAGEIFS mirrors SUMIFS but divides by hits.
    pub fn multiConditional(args: &Vec<Rc<RefCell<FormulaValue>>>, mode: i64) -> Rc<RefCell<FormulaValue>> {
        // mode: 0 = COUNTIFS, 1 = SUMIFS, 2 = AVERAGEIFS
        let mut min_args: i64 = 2;
        if mode > 0 {
            min_args = 3;
        }
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, min_args);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let mut pair_start: i64 = 0;
        let mut values: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        if mode > 0 {
            let mut head: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            head.push(args[0].clone());
            values = FormulaFunctions::flatArgs(&head);
            pair_start = 1;
        }
        let pair_count: i64 = ((args.len() as i64) - pair_start) / 2;
        if pair_count < 1 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if ((args.len() as i64) - pair_start) - pair_count * 2 != 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let mut crit_flats: Vec<Vec<Rc<RefCell<FormulaValue>>>> = Vec::new();
        let mut crits: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut p: i64 = 0;
        let mut n_rows: i64 = -1;
        while p < pair_count {
            let ri: i64 = pair_start + p * 2;
            let mut rh: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            rh.push(args[ri as usize].clone());
            let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&rh);
            if n_rows < 0 {
                n_rows = flat.len() as i64;
            } else {
                if (flat.len() as i64) != n_rows {
                    return FormulaValue::fromError("#VALUE!".to_string());
                }
            }
            crit_flats.push(flat.clone());
            crits.push(args[(ri + 1) as usize].clone());
            p += 1;
        }
        if mode > 0 {
            if (values.len() as i64) != n_rows {
                return FormulaValue::fromError("#VALUE!".to_string());
            }
        }
        let mut total: f64 = 0.0;
        let mut hits: i64 = 0;
        let mut i: i64 = 0;
        while i < n_rows {
            let mut ok: bool = true;
            let mut c: i64 = 0;
            while c < pair_count {
                let crow: Vec<Rc<RefCell<FormulaValue>>> = crit_flats[c as usize].clone();
                if !FormulaFunctions::criterionMatches(&crow[i as usize].clone(), &crits[c as usize].clone()) {
                    ok = false;
                    break;
                }
                c += 1;
            }
            if ok {
                if mode == 0 {
                    hits += 1;
                } else {
                    let sv: Rc<RefCell<FormulaValue>> = values[i as usize].clone();
                    if sv.borrow().isNumeric() {
                        total += sv.borrow().numberValue();
                        hits += 1;
                    }
                }
            }
            i += 1;
        }
        if mode == 0 {
            return FormulaValue::fromNumber(hits as f64);
        }
        if mode == 2 {
            if hits == 0 {
                return FormulaValue::fromError("#DIV/0!".to_string());
            }
            return FormulaValue::fromNumber(total / (hits as f64));
        }
        FormulaValue::fromNumber(total)
    }
    pub fn fnCountifs(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::multiConditional(args, 0)
    }
    pub fn fnSumifs(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::multiConditional(args, 1)
    }
    pub fn fnAverageifs(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::multiConditional(args, 2)
    }
    /// CHOOSE(index, value1, value2, …) — 1-based pick from the trailing args.
    pub fn fnChoose(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let idx: i64 = (args[0].borrow().asNumber()).floor() as i64;
        if idx < 1 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if idx >= (args.len() as i64) {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        args[idx as usize].clone()
    }
    /// Lookups. These are why a range had to keep its shape.
    pub fn valuesEqualLoose(a: &Rc<RefCell<FormulaValue>>, b: &Rc<RefCell<FormulaValue>>) -> bool {
        let a_ok: bool = a.borrow().isNumeric();
        let b_ok: bool = b.borrow().isNumeric();
        if a_ok && b_ok {
            return a.borrow().numberValue() == b.borrow().numberValue();
        }
        let out = FormulaFunctions::upper(&a.borrow().toText()) == FormulaFunctions::upper(&b.borrow().toText());
        out
    }
    /// Exact match only (Excel's range_lookup = FALSE / 0). The optional 4th
    /// argument is accepted and ignored: approximate / sorted search is not
    /// implemented — same stance as MATCH — so a miss stays #N/A rather than a
    /// quietly wrong neighbour row.
    pub fn fnVlookup(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 3);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let needle: Rc<RefCell<FormulaValue>> = args[0].clone();
        let table: Rc<RefCell<FormulaValue>> = args[1].clone();
        let col_idx: i64 = (args[2].borrow().asNumber()).floor() as i64;
        if !table.borrow().isArray() {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if col_idx < 1 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if col_idx > table.borrow().cols {
            return FormulaValue::fromError("#REF!".to_string());
        }
        let mut r: i64 = 0;
        while r < table.borrow().rows {
            if FormulaFunctions::valuesEqualLoose(&table.borrow().at(r, 0), &needle) {
                return table.borrow().at(r, col_idx - 1);
            }
            r += 1;
        }
        FormulaValue::fromError("#N/A".to_string())
    }
    pub fn fnHlookup(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 3);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let needle: Rc<RefCell<FormulaValue>> = args[0].clone();
        let table: Rc<RefCell<FormulaValue>> = args[1].clone();
        let row_idx: i64 = (args[2].borrow().asNumber()).floor() as i64;
        if !table.borrow().isArray() {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if row_idx < 1 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if row_idx > table.borrow().rows {
            return FormulaValue::fromError("#REF!".to_string());
        }
        let mut c: i64 = 0;
        while c < table.borrow().cols {
            if FormulaFunctions::valuesEqualLoose(&table.borrow().at(0, c), &needle) {
                return table.borrow().at(row_idx - 1, c);
            }
            c += 1;
        }
        FormulaValue::fromError("#N/A".to_string())
    }
    /// XLOOKUP(lookup, lookup_array, return_array, [if_not_found]) — exact match.
    pub fn fnXlookup(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 3);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let needle: Rc<RefCell<FormulaValue>> = args[0].clone();
        let mut look_head: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        look_head.push(args[1].clone());
        let look: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&look_head);
        let ret_arg: Rc<RefCell<FormulaValue>> = args[2].clone();
        let mut i: i64 = 0;
        while i < (look.len() as i64) {
            if FormulaFunctions::valuesEqualLoose(&look[i as usize].clone(), &needle) {
                if ret_arg.borrow().isArray() {
                    if ret_arg.borrow().rows == (look.len() as i64) {
                        if ret_arg.borrow().cols > 1 {
                            let mut row_items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
                            let mut c: i64 = 0;
                            while c < ret_arg.borrow().cols {
                                row_items.push(ret_arg.borrow().at(i, c));
                                c += 1;
                            }
                            return FormulaValue::fromArray(1, ret_arg.borrow().cols.clone(), &row_items);
                        }
                        return ret_arg.borrow().at(i, 0);
                    }
                }
                let mut ret_head: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
                ret_head.push(ret_arg.clone());
                let ret_flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&ret_head);
                if i < (ret_flat.len() as i64) {
                    return ret_flat[i as usize].clone();
                }
                return FormulaValue::fromError("#N/A".to_string());
            }
            i += 1;
        }
        if (args.len() as i64) >= 4 {
            return args[3].clone();
        }
        FormulaValue::fromError("#N/A".to_string())
    }
    /// LOOKUP(lookup_value, lookup_vector, [result_vector]) — approximate when
    /// both sides are numbers (largest <= needle); otherwise exact.
    pub fn fnLookup(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let needle: Rc<RefCell<FormulaValue>> = args[0].clone();
        let mut look_head: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        look_head.push(args[1].clone());
        let look: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&look_head);
        let mut ret_flat: Vec<Rc<RefCell<FormulaValue>>> = look.clone();
        if (args.len() as i64) >= 3 {
            let mut ret_head: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            ret_head.push(args[2].clone());
            ret_flat = FormulaFunctions::flatArgs(&ret_head);
        }
        let mut exact: i64 = -1;
        let mut best: i64 = -1;
        let needle_num: bool = needle.borrow().isNumeric();
        let needle_n: f64 = needle.borrow().numberValue();
        let mut i: i64 = 0;
        while i < (look.len() as i64) {
            let cell: Rc<RefCell<FormulaValue>> = look[i as usize].clone();
            if FormulaFunctions::valuesEqualLoose(&cell, &needle) {
                exact = i;
            }
            if needle_num {
                if cell.borrow().isNumeric() {
                    if cell.borrow().numberValue() <= needle_n {
                        best = i;
                    }
                }
            }
            i += 1;
        }
        let mut pick: i64 = exact;
        if pick < 0 {
            pick = best;
        }
        if pick < 0 {
            return FormulaValue::fromError("#N/A".to_string());
        }
        if pick >= (ret_flat.len() as i64) {
            return FormulaValue::fromError("#N/A".to_string());
        }
        ret_flat[pick as usize].clone()
    }
    /// FILTER(array, include, [if_empty]) — keeps rows where include is true.
    pub fn fnFilter(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let src: Rc<RefCell<FormulaValue>> = args[0].clone();
        let mut rows: i64 = 1;
        let mut cols: i64 = 1;
        if src.borrow().isArray() {
            rows = src.borrow().rows;
            cols = src.borrow().cols;
        }
        let mut inc_head: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        inc_head.push(args[1].clone());
        let inc: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&inc_head);
        if (inc.len() as i64) != rows {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let mut out: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut kept: i64 = 0;
        let mut r: i64 = 0;
        while r < rows {
            if inc[r as usize].borrow().isTrue() {
                let mut c: i64 = 0;
                while c < cols {
                    if src.borrow().isArray() {
                        out.push(src.borrow().at(r, c));
                    } else {
                        out.push(src.clone());
                    }
                    c += 1;
                }
                kept += 1;
            }
            r += 1;
        }
        if kept == 0 {
            if (args.len() as i64) >= 3 {
                return args[2].clone();
            }
            return FormulaValue::fromError("#CALC!".to_string());
        }
        FormulaValue::fromArray(kept, cols, &out)
    }
    /// UNIQUE(array) — distinct values in order, spilled as a column.
    pub fn fnUnique(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let mut head: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        head.push(args[0].clone());
        let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&head);
        let mut out: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut i: i64 = 0;
        while i < (flat.len() as i64) {
            let v: Rc<RefCell<FormulaValue>> = flat[i as usize].clone();
            let mut seen: bool = false;
            let mut j: i64 = 0;
            while j < (out.len() as i64) {
                if FormulaFunctions::valuesEqualLoose(&out[j as usize].clone(), &v) {
                    seen = true;
                    break;
                }
                j += 1;
            }
            if !seen {
                out.push(v.clone());
            }
            i += 1;
        }
        FormulaValue::fromArray(out.len() as i64, 1, &out)
    }
    /// SORT(array, [sort_index], [sort_order]) — sort rows by a column.
    /// sort_order: 1 ascending (default), -1 descending.
    pub fn fnSort(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let src: Rc<RefCell<FormulaValue>> = args[0].clone();
        if !src.borrow().isArray() {
            return src;
        }
        let rows: i64 = src.borrow().rows;
        let cols: i64 = src.borrow().cols;
        let mut sort_idx: i64 = 1;
        if (args.len() as i64) >= 2 {
            sort_idx = (args[1].borrow().asNumber()).floor() as i64;
        }
        let mut ascending: bool = true;
        if (args.len() as i64) >= 3 {
            if ((args[2].borrow().asNumber()).floor() as i64) < 0 {
                ascending = false;
            }
        }
        if sort_idx < 1 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if sort_idx > cols {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let mut order: Vec<i64> = Vec::new();
        let mut r: i64 = 0;
        while r < rows {
            order.push(r);
            r += 1;
        }
        let mut a: i64 = 0;
        while a < rows {
            let mut b: i64 = a + 1;
            while b < rows {
                let ra: i64 = order[a as usize];
                let rb: i64 = order[b as usize];
                let va: Rc<RefCell<FormulaValue>> = src.borrow().at(ra, sort_idx - 1);
                let vb: Rc<RefCell<FormulaValue>> = src.borrow().at(rb, sort_idx - 1);
                let mut cmp: i64 = 0;
                if va.borrow().isNumeric() && vb.borrow().isNumeric() {
                    if va.borrow().numberValue() < vb.borrow().numberValue() {
                        cmp = -1;
                    }
                    if va.borrow().numberValue() > vb.borrow().numberValue() {
                        cmp = 1;
                    }
                } else {
                    cmp = FormulaFunctions::compareText(&FormulaFunctions::upper(&va.borrow().toText()), &FormulaFunctions::upper(&vb.borrow().toText()));
                }
                let mut swap: bool = false;
                if ascending {
                    if cmp > 0 {
                        swap = true;
                    }
                } else {
                    if cmp < 0 {
                        swap = true;
                    }
                }
                if swap {
                    order[a as usize] = rb;
                    order[b as usize] = ra;
                }
                b += 1;
            }
            a += 1;
        }
        let mut out: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        r = 0;
        while r < rows {
            let rr: i64 = order[r as usize];
            let mut c: i64 = 0;
            while c < cols {
                out.push(src.borrow().at(rr, c));
                c += 1;
            }
            r += 1;
        }
        FormulaValue::fromArray(rows, cols, &out)
    }
    /// INDEX(range, row, [col]). A one-row or one-column range takes a single
    /// index, which is how it is almost always written.
    pub fn fnIndex(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let table: Rc<RefCell<FormulaValue>> = args[0].clone();
        if !table.borrow().isArray() {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        let a: i64 = (args[1].borrow().asNumber()).floor() as i64;
        if (args.len() as i64) >= 3 {
            let b: i64 = (args[2].borrow().asNumber()).floor() as i64;
            if a < 1 {
                return FormulaValue::fromError("#VALUE!".to_string());
            }
            if b < 1 {
                return FormulaValue::fromError("#VALUE!".to_string());
            }
            if a > table.borrow().rows {
                return FormulaValue::fromError("#REF!".to_string());
            }
            if b > table.borrow().cols {
                return FormulaValue::fromError("#REF!".to_string());
            }
            return table.borrow().at(a - 1, b - 1);
        }
        if a < 1 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if table.borrow().rows == 1 {
            if a > table.borrow().cols {
                return FormulaValue::fromError("#REF!".to_string());
            }
            return table.borrow().at(0, a - 1);
        }
        if table.borrow().cols == 1 {
            if a > table.borrow().rows {
                return FormulaValue::fromError("#REF!".to_string());
            }
            return table.borrow().at(a - 1, 0);
        }
        FormulaValue::fromError("#REF!".to_string())
    }
    /// MATCH(needle, range, [type]). Only exact match (type 0) is implemented;
    /// the sorted-range types are a different search and are refused rather than
    /// quietly answered with the wrong row.
    pub fn fnMatch(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let needle: Rc<RefCell<FormulaValue>> = args[0].clone();
        let mut head: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        head.push(args[1].clone());
        let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&head);
        let mut mode: i64 = 0;
        if (args.len() as i64) >= 3 {
            mode = (args[2].borrow().asNumber()).floor() as i64;
        }
        if mode != 0 {
            return FormulaValue::fromError("#N/A".to_string());
        }
        let mut i: i64 = 0;
        while i < (flat.len() as i64) {
            if FormulaFunctions::valuesEqualLoose(&flat[i as usize].clone(), &needle) {
                return FormulaValue::fromNumber((i + 1) as f64);
            }
            i += 1;
        }
        FormulaValue::fromError("#N/A".to_string())
    }
    pub fn fnSumproduct(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        // unused:  let mut lists: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut total: f64 = 0.0;
        let mut first: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        first.push(args[0].clone());
        let base: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&first);
        let mut i: i64 = 0;
        while i < (base.len() as i64) {
            let mut product: f64 = base[i as usize].borrow().asNumber();
            let mut a: i64 = 1;
            while a < (args.len() as i64) {
                let mut other: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
                other.push(args[a as usize].clone());
                let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(&other);
                if i < (flat.len() as i64) {
                    product *= flat[i as usize].borrow().asNumber();
                } else {
                    product = 0.0;
                }
                a += 1;
            }
            total += product;
            i += 1;
        }
        FormulaValue::fromNumber(total)
    }
    pub fn fnTranspose(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let src: Rc<RefCell<FormulaValue>> = args[0].clone();
        if !src.borrow().isArray() {
            return src;
        }
        let mut out: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut c: i64 = 0;
        while c < src.borrow().cols {
            let mut r: i64 = 0;
            while r < src.borrow().rows {
                out.push(src.borrow().at(r, c));
                r += 1;
            }
            c += 1;
        }
        let (rows, cols) = (src.borrow().rows, src.borrow().cols);
        FormulaValue::fromArray(cols, rows, &out)
    }
    /// More text
    pub fn findFrom(hay: &str, needle: &str, from: i64) -> i64 {
        let n: i64 = DgNative::str_len(&needle);
        let m: i64 = DgNative::str_len(&hay);
        if n == 0 {
            return from;
        }
        let mut i: i64 = from;
        while i + n <= m {
            if DgNative::substr(&hay, i, i + n) == needle {
                return i;
            }
            i += 1;
        }
        -1
    }
    pub fn fnSubstitute(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 3);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let hay: String = args[0].borrow().toText();
        let needle: String = args[1].borrow().toText();
        let repl: String = args[2].borrow().toText();
        let mut which: i64 = 0;
        if (args.len() as i64) >= 4 {
            which = (args[3].borrow().asNumber()).floor() as i64;
        }
        if needle.is_empty() {
            return FormulaValue::fromString(hay.clone());
        }
        let mut out: String = String::new();
        let mut rest: String = hay.clone();
        let mut seen: i64 = 0;
        let mut guard: i64 = 0;
        while guard < 4096 {
            let at: i64 = FormulaFunctions::findFrom(&rest, &needle, 0);
            if at < 0 {
                return FormulaValue::fromString(format!("{}{}", out, rest));
            }
            seen += 1;
            let mut replace: bool = true;
            if which > 0 {
                replace = seen == which;
            }
            out = format!("{}{}", out, DgNative::substr(&rest, 0, at));
            if replace {
                out = format!("{}{}", out, repl);
            } else {
                out = format!("{}{}", out, needle);
            }
            rest = DgNative::substr(&rest, at + DgNative::str_len(&needle), DgNative::str_len(&rest));
            guard += 1;
        }
        FormulaValue::fromString(format!("{}{}", out, rest))
    }
    /// FIND is case sensitive, SEARCH is not — the only difference between them.
    pub fn findIn(args: &Vec<Rc<RefCell<FormulaValue>>>, case_sensitive: bool) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let mut needle: String = args[0].borrow().toText();
        let mut hay: String = args[1].borrow().toText();
        if !case_sensitive {
            needle = FormulaFunctions::upper(&needle);
            hay = FormulaFunctions::upper(&hay);
        }
        let mut from: i64 = 0;
        if (args.len() as i64) >= 3 {
            from = ((args[2].borrow().asNumber()).floor() as i64) - 1;
            if from < 0 {
                from = 0;
            }
        }
        let at: i64 = FormulaFunctions::findFrom(&hay, &needle, from);
        if at < 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        FormulaValue::fromNumber((at + 1) as f64)
    }
    pub fn fnFind(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::findIn(args, true)
    }
    pub fn fnSearch(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        FormulaFunctions::findIn(args, false)
    }
    pub fn fnRept(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let unit: String = args[0].borrow().toText();
        let mut times: i64 = (args[1].borrow().asNumber()).floor() as i64;
        if times < 0 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        if times > 4096 {
            times = 4096;
        }
        let mut out: String = String::new();
        let mut i: i64 = 0;
        while i < times {
            out = format!("{}{}", out, unit);
            i += 1;
        }
        FormulaValue::fromString(out.clone())
    }
    pub fn fnProper(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ve: bool = v.borrow().isError();
        if ve {
            return v;
        }
        let src: String = v.borrow().toText();
        let mut out: String = String::new();
        let mut start_word: bool = true;
        let mut i: i64 = 0;
        while i < DgNative::str_len(&src) {
            let mut ch: i64 = DgNative::char_at(&src, i);
            let mut is_letter: bool = false;
            if ch >= 65 && ch <= 90 {
                is_letter = true;
            }
            if ch >= 97 && ch <= 122 {
                is_letter = true;
            }
            if is_letter {
                if start_word {
                    if ch >= 97 && ch <= 122 {
                        ch -= 32;
                    }
                } else {
                    if ch >= 65 && ch <= 90 {
                        ch += 32;
                    }
                }
                start_word = false;
                out = format!("{}{}", out, char::from_u32(ch as u32).unwrap_or('\0'));
            } else {
                start_word = true;
                // Not a letter this knows how to case — copy the byte through
                // rather than rebuild it. See `upper` above for why.
                out = format!("{}{}", out, DgNative::substr(&src, i, i + 1));
            }
            i += 1;
        }
        FormulaValue::fromString(out.clone())
    }
    pub fn fnExact(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 2);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let err: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        let ee: bool = err.borrow().isError();
        if ee {
            return err;
        }
        let out = FormulaValue::fromBool(args[0].borrow().toText() == args[1].borrow().toText());
        out
    }
    pub fn fnChar(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ve: bool = v.borrow().isError();
        if ve {
            return v;
        }
        let code: i64 = (v.borrow().asNumber()).floor() as i64;
        if code < 1 {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        FormulaValue::fromString(char::from_u32(code as u32).unwrap_or('\0').to_string())
    }
    pub fn fnCode(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ve: bool = v.borrow().isError();
        if ve {
            return v;
        }
        let s: String = v.borrow().toText();
        if s.is_empty() {
            return FormulaValue::fromError("#VALUE!".to_string());
        }
        FormulaValue::fromNumber((DgNative::char_at(&s, 0)) as f64)
    }
    pub fn fnValue(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        let ok: bool = v.borrow().isNumeric();
        if ok {
            return FormulaValue::fromNumber(v.borrow().numberValue());
        }
        FormulaValue::fromError("#VALUE!".to_string())
    }
    /// --- info ---
    pub fn fnIsBlank(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let b: bool = v.borrow().isBlank();
        FormulaValue::fromBool(b)
    }
    pub fn fnIsNumber(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let out = FormulaValue::fromBool(v.borrow().kind == 1);
        out
    }
    pub fn fnIsText(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let out = FormulaValue::fromBool(v.borrow().kind == 2);
        out
    }
    pub fn fnIsError(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let e: bool = v.borrow().isError();
        FormulaValue::fromBool(e)
    }
    pub fn fnIsErr(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if !ee {
            return FormulaValue::fromBool(false);
        }
        if v.borrow().err == "#N/A" {
            return FormulaValue::fromBool(false);
        }
        FormulaValue::fromBool(true)
    }
    pub fn fnN(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        if v.borrow().kind == 1 {
            return v;
        }
        if v.borrow().kind == 3 {
            return FormulaValue::fromNumber(v.borrow().asNumber());
        }
        FormulaValue::fromNumber(0.0)
    }
    pub fn fnT(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let ar: Rc<RefCell<FormulaValue>> = FormulaFunctions::needArity(args, 1);
        let ae: bool = ar.borrow().isError();
        if ae {
            return ar;
        }
        let v: Rc<RefCell<FormulaValue>> = args[0].clone();
        let ee: bool = v.borrow().isError();
        if ee {
            return v;
        }
        if v.borrow().kind == 2 {
            return v;
        }
        FormulaValue::fromString(String::new())
    }
}
