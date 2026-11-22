// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;
use crate::date_serial::DateSerial;
use crate::formula_value::FormulaValue;
use crate::formula_functions::FormulaFunctions;

/// FormulaMore - the second half of the pure function library: math and
/// trigonometry, statistics, dates and times, text, information, lookup over
/// arrays and finance. Same contract as FormulaFunctions: FormulaValue in,
/// FormulaValue out, no workbook. FormulaFunctions.isKnown and invoke fall
/// through to this file for the names in `names()`.
///
/// The functions that need a reference rather than its values (ROW, COLUMN,
/// ROWS, COLUMNS, OFFSET, INDIRECT, ISREF, ISFORMULA, FORMULATEXT) and the ones
/// that need a state (RAND, RANDBETWEEN) are FormulaEngine's.
#[derive(Clone)]
pub struct FormulaMore {
}

impl FormulaMore {
    pub fn new() -> Self {
        Self {
        }
    }
    /// Every name answered here, space-separated, with a space at each end so
    /// a lookup is one search for " NAME ".
    pub fn names() -> String {
        " LOG SQRTPI EVEN ODD ISEVEN ISODD FACT FACTDOUBLE COMBIN COMBINA PERMUT GCD LCM QUOTIENT MROUND CEILING.MATH FLOOR.MATH CEILING.PRECISE FLOOR.PRECISE ISO.CEILING SUMSQ SUMX2MY2 SUMX2PY2 SUMXMY2 RADIANS DEGREES SIN COS TAN ASIN ACOS ATAN ATAN2 SINH COSH TANH COT CSC SEC BASE DECIMAL DEC2BIN BIN2DEC DEC2HEX HEX2DEC DEC2OCT OCT2DEC ROMAN ARABIC MULTINOMIAL MAXIFS MINIFS RANK RANK.EQ RANK.AVG MODE MODE.SNGL PERCENTILE PERCENTILE.INC PERCENTILE.EXC QUARTILE QUARTILE.INC QUARTILE.EXC PERCENTRANK PERCENTRANK.INC GEOMEAN HARMEAN AVEDEV DEVSQ AVERAGEA MAXA MINA CORREL PEARSON COVAR COVARIANCE.P COVARIANCE.S SLOPE INTERCEPT RSQ FORECAST FORECAST.LINEAR STEYX STANDARDIZE TRIMMEAN COUNTUNIQUE TIME HOUR MINUTE SECOND TIMEVALUE WEEKNUM ISOWEEKNUM NETWORKDAYS WORKDAY NETWORKDAYS.INTL WORKDAY.INTL DATEDIF DAYS360 YEARFRAC TEXTBEFORE TEXTAFTER TEXTSPLIT VALUETOTEXT NUMBERVALUE FIXED DOLLAR CLEAN UNICHAR UNICODE ISLOGICAL ISNONTEXT ISNA TYPE ERROR.TYPE XMATCH CHOOSECOLS CHOOSEROWS TAKE DROP VSTACK HSTACK SEQUENCE SORTBY TOCOL TOROW WRAPROWS WRAPCOLS EXPAND ADDRESS HYPERLINK IPMT PPMT CUMIPMT CUMPRINC RATE IRR XNPV XIRR MIRR DB DDB SYD EFFECT NOMINAL PDURATION RRI ROW COLUMN ROWS COLUMNS OFFSET INDIRECT ISREF ISFORMULA FORMULATEXT RAND RANDBETWEEN MMULT AGGREGATE MDETERM MINVERSE MUNIT AREAS LET LAMBDA REDUCE MAP SCAN BYROW BYCOL MAKEARRAY ".to_string()
    }
    pub fn isKnown(n: &str) -> bool {
        let all: String = FormulaMore::names();
        let key: String = format!(" {} ", n);
        DgNative::index_of(&all, &key) >= 0
    }

    // --- small helpers ---------------------------------------------------------

    pub fn absD(x: f64) -> f64 {
        if x < 0.0 {
            return 0.0 - x;
        }
        x
    }
    pub fn sqrtD(x: f64) -> f64 {
        if x <= 0.0 {
            return 0.0;
        }
        let mut g: f64 = x;
        if g < 1.0 {
            g = 1.0;
        }
        let mut k: i64 = 0;
        while k < 60 {
            g = (g + x / g) / 2.0;
            k += 1;
        }
        g
    }
    pub fn roundD(x: f64, digits: i64) -> f64 {
        let p: f64 = FormulaFunctions::powTen(digits);
        let mut v: f64 = FormulaMore::absD(x) * p + 0.5;
        v = v.floor() / p;
        if x < 0.0 {
            return 0.0 - v;
        }
        v
    }
    pub fn num(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64, dflt: f64) -> f64 {
        FormulaFunctions::tvmArg(args, i, dflt)
    }
    pub fn int(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64, dflt: i64) -> i64 {
        let v: f64 = FormulaFunctions::tvmArg(args, i, dflt as f64);
        v.floor() as i64
    }
    pub fn txt(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64) -> String {
        if i >= (args.len() as i64) {
            return String::new();
        }
        let s: String = args[i as usize].borrow().single().borrow().asDisplay();
        s
    }
    pub fn has(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64) -> bool {
        if i >= (args.len() as i64) {
            return false;
        }
        let b: bool = args[i as usize].borrow().isBlank();
        !b
    }
    pub fn err(e: &str) -> Rc<RefCell<FormulaValue>> {
        FormulaValue::fromError(e.to_string())
    }
    pub fn numV(x: f64) -> Rc<RefCell<FormulaValue>> {
        FormulaValue::fromNumber(x)
    }
    /// A value as a grid: an array as it is, anything else as one cell.
    pub fn grid(v: &Rc<RefCell<FormulaValue>>) -> Rc<RefCell<FormulaValue>> {
        if v.borrow().isArray() {
            return v.clone();
        }
        let mut one: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        one.push(v.clone());
        FormulaValue::fromArray(1, 1, &one)
    }
    pub fn numbersOf(v: &Rc<RefCell<FormulaValue>>) -> Vec<f64> {
        let mut one: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        one.push(v.clone());
        FormulaFunctions::collectNumbers(&one)
    }
    pub fn flatOf(v: &Rc<RefCell<FormulaValue>>) -> Vec<Rc<RefCell<FormulaValue>>> {
        let mut one: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        one.push(v.clone());
        FormulaFunctions::flatArgs(&one)
    }
    pub fn firstErr(args: &Vec<Rc<RefCell<FormulaValue>>>, need: i64) -> Rc<RefCell<FormulaValue>> {
        if (args.len() as i64) < need {
            return FormulaMore::err("#VALUE!");
        }
        let e: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        e
    }

    // --- elementary functions ----------------------------------------------------

    pub fn PI() -> f64 {
        3.141592653589793
    }
    pub fn sinD(x0: f64) -> f64 {
        let two_pi: f64 = 2.0 * FormulaMore::PI();
        let k: f64 = (x0 / two_pi + 0.5).floor();
        let x: f64 = x0 - k * two_pi;
        let mut term: f64 = x;
        let mut sum: f64 = x;
        let mut n: i64 = 1;
        while n < 30 {
            term = (0.0 - term * x * x) / (((2 * n) * (2 * n + 1)) as f64);
            sum += term;
            n += 1;
        }
        sum
    }
    pub fn cosD(x: f64) -> f64 {
        FormulaMore::sinD(x + FormulaMore::PI() / 2.0)
    }
    pub fn atanD(x: f64) -> f64 {
        if x < 0.0 {
            return 0.0 - FormulaMore::atanD(0.0 - x);
        }
        if x > 1.0 {
            return FormulaMore::PI() / 2.0 - FormulaMore::atanD(1.0 / x);
        }
        if x > 0.2 {
            // atan(x) = 2 atan(x / (1 + sqrt(1 + x^2)))
            return 2.0 * FormulaMore::atanD(x / (1.0 + FormulaMore::sqrtD(1.0 + x * x)));
        }
        let mut term: f64 = x;
        let mut sum: f64 = x;
        let mut n: i64 = 1;
        while n < 40 {
            term = (0.0 - term) * x * x;
            sum += term / ((2 * n + 1) as f64);
            n += 1;
        }
        sum
    }
    pub fn asinD(x: f64) -> f64 {
        if x >= 1.0 {
            return FormulaMore::PI() / 2.0;
        }
        if x <= -1.0 {
            return 0.0 - FormulaMore::PI() / 2.0;
        }
        FormulaMore::atanD(x / FormulaMore::sqrtD(1.0 - x * x))
    }
    pub fn lnD(x: f64) -> f64 {
        FormulaFunctions::lnD(x)
    }
    pub fn expD(x: f64) -> f64 {
        FormulaFunctions::expD(x)
    }
    pub fn powD(b: f64, e: f64) -> f64 {
        if b < 0.0 {
            // A negative base has a real power only for a whole exponent.
            let ei: f64 = e.floor();
            if ei == e {
                let p: f64 = FormulaFunctions::powD(0.0 - b, e);
                let half: f64 = (ei / 2.0).floor();
                if half * 2.0 == ei {
                    return p;
                }
                return 0.0 - p;
            }
            return 0.0;
        }
        if e < 0.0 {
            if b == 0.0 {
                return 0.0;
            }
            return 1.0 / FormulaFunctions::powD(b, 0.0 - e);
        }
        FormulaFunctions::powD(b, e)
    }
    pub fn fact(n: i64) -> f64 {
        let mut out: f64 = 1.0;
        let mut i: i64 = 2;
        while i <= n {
            out *= i as f64;
            i += 1;
        }
        out
    }
    pub fn combin(n: i64, k: i64) -> f64 {
        if k < 0 || k > n {
            return 0.0;
        }
        let mut kk: i64 = k;
        if n - k < kk {
            kk = n - k;
        }
        let mut out: f64 = 1.0;
        let mut i: i64 = 1;
        while i <= kk {
            out = (out * ((n - kk + i) as f64)) / (i as f64);
            i += 1;
        }
        FormulaMore::roundD(out, 0)
    }
    pub fn gcd2(a0: i64, b0: i64) -> i64 {
        let mut a: i64 = a0;
        let mut b: i64 = b0;
        while b != 0 {
            let t: i64 = a % b;
            a = b;
            b = t;
        }
        a
    }

    // --- dispatch ------------------------------------------------------------------

    pub fn invoke(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let nm: String = n.to_string();
        let r0: Rc<RefCell<FormulaValue>> = FormulaMore::matrixFn(&nm, args);
        if !r0.borrow().isBlank() {
            return r0;
        }
        let r1: Rc<RefCell<FormulaValue>> = FormulaMore::mathFn(&nm, args);
        if !r1.borrow().isBlank() {
            return r1;
        }
        let r2: Rc<RefCell<FormulaValue>> = FormulaMore::statFn(&nm, args);
        if !r2.borrow().isBlank() {
            return r2;
        }
        let r3: Rc<RefCell<FormulaValue>> = FormulaMore::dateFn(&nm, args);
        if !r3.borrow().isBlank() {
            return r3;
        }
        let r4: Rc<RefCell<FormulaValue>> = FormulaMore::textFn(&nm, args);
        if !r4.borrow().isBlank() {
            return r4;
        }
        let r5: Rc<RefCell<FormulaValue>> = FormulaMore::arrayFn(&nm, args);
        if !r5.borrow().isBlank() {
            return r5;
        }
        let r6: Rc<RefCell<FormulaValue>> = FormulaMore::financeFn(&nm, args);
        r6
    }

    // --- matrices and AGGREGATE ------------------------------------------------------

    pub fn matrixOf(v: &Rc<RefCell<FormulaValue>>) -> Rc<RefCell<FormulaValue>> {
        if v.borrow().isArray() {
            return v.clone();
        }
        let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        items.push(v.clone());
        FormulaValue::fromArray(1, 1, &items)
    }
    pub fn cellNum(m: &Rc<RefCell<FormulaValue>>, r: i64, c: i64) -> f64 {
        let x: Rc<RefCell<FormulaValue>> = m.borrow().at(r, c);
        let k: i64 = x.borrow().kind;
        if k == 1 {
            return x.borrow().num;
        }
        0.0
    }
    pub fn cellIsNum(m: &Rc<RefCell<FormulaValue>>, r: i64, c: i64) -> bool {
        m.borrow().at(r, c).borrow().kind == 1
    }
    pub fn matrixFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if n == "MMULT" {
            if (args.len() as i64) < 2 {
                return FormulaMore::err("#VALUE!");
            }
            let a: Rc<RefCell<FormulaValue>> = FormulaMore::matrixOf(&args[0].clone());
            let b: Rc<RefCell<FormulaValue>> = FormulaMore::matrixOf(&args[1].clone());
            let ar: i64 = a.borrow().rows;
            let ac: i64 = a.borrow().cols;
            let br: i64 = b.borrow().rows;
            let bc: i64 = b.borrow().cols;
            if ac != br {
                return FormulaMore::err("#VALUE!");
            }
            let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut i: i64 = 0;
            while i < ar {
                let mut j: i64 = 0;
                while j < bc {
                    let mut s: f64 = 0.0;
                    let mut k: i64 = 0;
                    while k < ac {
                        if !FormulaMore::cellIsNum(&a, i, k) || !FormulaMore::cellIsNum(&b, k, j) {
                            return FormulaMore::err("#VALUE!");
                        }
                        let x: f64 = FormulaMore::cellNum(&a, i, k);
                        let y: f64 = FormulaMore::cellNum(&b, k, j);
                        s += x * y;
                        k += 1;
                    }
                    items.push(FormulaValue::fromNumber(s));
                    j += 1;
                }
                i += 1;
            }
            return FormulaValue::fromArray(ar, bc, &items);
        }
        if n == "MUNIT" {
            if (args.len() as i64) < 1 {
                return FormulaMore::err("#VALUE!");
            }
            let size: i64 = args[0].borrow().single().borrow().numberValue() as i64;
            if size < 1 {
                return FormulaMore::err("#VALUE!");
            }
            let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut i: i64 = 0;
            while i < size * size {
                if i / size == i % size {
                    items.push(FormulaValue::fromNumber(1.0));
                } else {
                    items.push(FormulaValue::fromNumber(0.0));
                }
                i += 1;
            }
            return FormulaValue::fromArray(size, size, &items);
        }
        if n == "MDETERM" || n == "MINVERSE" {
            if (args.len() as i64) < 1 {
                return FormulaMore::err("#VALUE!");
            }
            let a: Rc<RefCell<FormulaValue>> = FormulaMore::matrixOf(&args[0].clone());
            let sz: i64 = a.borrow().rows;
            if sz != a.borrow().cols {
                return FormulaMore::err("#VALUE!");
            }
            // Gauss-Jordan on [A | I]
            let w: i64 = sz * 2;
            let mut m: Vec<f64> = Vec::new();
            let mut r: i64 = 0;
            while r < sz {
                let mut c: i64 = 0;
                while c < w {
                    if c < sz {
                        if !FormulaMore::cellIsNum(&a, r, c) {
                            return FormulaMore::err("#VALUE!");
                        }
                        m.push(FormulaMore::cellNum(&a, r, c));
                    } else {
                        if c - sz == r {
                            m.push(1.0);
                        } else {
                            m.push(0.0);
                        }
                    }
                    c += 1;
                }
                r += 1;
            }
            let mut det: f64 = 1.0;
            let mut col: i64 = 0;
            while col < sz {
                let mut piv: i64 = col;
                let mut best: f64 = m[(col * w + col) as usize].abs();
                let mut q: i64 = col + 1;
                while q < sz {
                    let v: f64 = m[(q * w + col) as usize].abs();
                    if v > best {
                        best = v;
                        piv = q;
                    }
                    q += 1;
                }
                if best < 1e-300 {
                    if n == "MDETERM" {
                        return FormulaValue::fromNumber(0.0);
                    }
                    return FormulaMore::err("#NUM!");
                }
                if piv != col {
                    let mut c2: i64 = 0;
                    while c2 < w {
                        let t: f64 = m[(col * w + c2) as usize];
                        m[(col * w + c2) as usize] = m[(piv * w + c2) as usize];
                        m[(piv * w + c2) as usize] = t;
                        c2 += 1;
                    }
                    det = 0.0 - det;
                }
                let pv: f64 = m[(col * w + col) as usize];
                det *= pv;
                let mut c3: i64 = 0;
                while c3 < w {
                    m[(col * w + c3) as usize] = m[(col * w + c3) as usize] / pv;
                    c3 += 1;
                }
                let mut r2: i64 = 0;
                while r2 < sz {
                    if r2 != col {
                        let f: f64 = m[(r2 * w + col) as usize];
                        let mut c4: i64 = 0;
                        while c4 < w {
                            m[(r2 * w + c4) as usize] = m[(r2 * w + c4) as usize] - f * m[(col * w + c4) as usize];
                            c4 += 1;
                        }
                    }
                    r2 += 1;
                }
                col += 1;
            }
            if n == "MDETERM" {
                return FormulaValue::fromNumber(det);
            }
            let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut r3: i64 = 0;
            while r3 < sz {
                let mut c5: i64 = 0;
                while c5 < sz {
                    items.push(FormulaValue::fromNumber(m[(r3 * w + sz + c5) as usize]));
                    c5 += 1;
                }
                r3 += 1;
            }
            return FormulaValue::fromArray(sz, sz, &items);
        }
        if n == "AGGREGATE" {
            // AGGREGATE(function, options, ref1, [k]): options 2, 3, 6 and 7
            // leave out error values; hidden rows are not known here.
            if (args.len() as i64) < 3 {
                return FormulaMore::err("#VALUE!");
            }
            let f: i64 = args[0].borrow().single().borrow().numberValue() as i64;
            let opt: i64 = args[1].borrow().single().borrow().numberValue() as i64;
            let skip_err: bool = opt == 2 || opt == 3 || opt == 6 || opt == 7;
            let names: Vec<&str> = vec!["", "AVERAGE", "COUNT", "COUNTA", "MAX", "MIN", "PRODUCT", "STDEV.S", "STDEV.P", "SUM", "VAR.S", "VAR.P", "MEDIAN", "MODE.SNGL", "LARGE", "SMALL", "PERCENTILE.INC", "QUARTILE.INC", "PERCENTILE.EXC", "QUARTILE.EXC"];
            if f < 1 || f > 19 {
                return FormulaMore::err("#VALUE!");
            }
            let mut vals: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let last: i64 = if f >= 14 { 3 } else { args.len() as i64 };
            let mut ai: i64 = 2;
            while ai < last && ai < (args.len() as i64) {
                for x in FormulaMore::flatOf(&args[ai as usize].clone()).iter() {
                    if x.borrow().isError() {
                        if skip_err {
                            continue;
                        }
                        return x.clone();
                    }
                    vals.push(x.clone());
                }
                ai += 1;
            }
            let count: i64 = vals.len() as i64;
            let arr: Rc<RefCell<FormulaValue>> = FormulaValue::fromArray(count, 1, &vals);
            let mut call: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            call.push(arr);
            if f >= 14 {
                if (args.len() as i64) < 4 {
                    return FormulaMore::err("#VALUE!");
                }
                call.push(args[3].clone());
            }
            return FormulaFunctions::invoke(names[f as usize], &call);
        }
        FormulaValue::blank()
    }

    // --- math and trigonometry -------------------------------------------------------

    pub fn mathFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if n == "SUMSQ" || n == "SUMX2MY2" || n == "SUMX2PY2" || n == "SUMXMY2" {
            let e: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
            if e.borrow().isError() {
                return e;
            }
            if n == "SUMSQ" {
                let mut s: f64 = 0.0;
                for x in FormulaFunctions::collectNumbers(args).iter() {
                    s += *x * *x;
                }
                return FormulaMore::numV(s);
            }
            if (args.len() as i64) < 2 {
                return FormulaMore::err("#VALUE!");
            }
            let a: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[0].clone());
            let b: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[1].clone());
            if a.len() != b.len() {
                return FormulaMore::err("#N/A");
            }
            let mut s2: f64 = 0.0;
            let mut i: i64 = 0;
            while i < (a.len() as i64) {
                let av: Rc<RefCell<FormulaValue>> = a[i as usize].clone();
                let bv: Rc<RefCell<FormulaValue>> = b[i as usize].clone();
                if av.borrow().isNumeric() && bv.borrow().isNumeric() {
                    let x: f64 = av.borrow().numberValue();
                    let y: f64 = bv.borrow().numberValue();
                    if n == "SUMX2MY2" {
                        s2 += x * x - y * y;
                    }
                    if n == "SUMX2PY2" {
                        s2 += x * x + y * y;
                    }
                    if n == "SUMXMY2" {
                        s2 += (x - y) * (x - y);
                    }
                }
                i += 1;
            }
            return FormulaMore::numV(s2);
        }
        if n == "GCD" || n == "LCM" || n == "MULTINOMIAL" {
            let e2: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
            if e2.borrow().isError() {
                return e2;
            }
            let vals: Vec<f64> = FormulaFunctions::collectNumbers(args);
            if vals.is_empty() {
                return FormulaMore::err("#VALUE!");
            }
            if n == "MULTINOMIAL" {
                let mut total: i64 = 0;
                let mut den: f64 = 1.0;
                for v in vals.iter() {
                    let k: i64 = v.floor() as i64;
                    if k < 0 {
                        return FormulaMore::err("#NUM!");
                    }
                    total += k;
                    den *= FormulaMore::fact(k);
                }
                return FormulaMore::numV(FormulaMore::roundD(FormulaMore::fact(total) / den, 0));
            }
            let mut acc: i64 = 0;
            let mut first: bool = true;
            for v in vals.iter() {
                let k: i64 = v.floor() as i64;
                if k < 0 {
                    return FormulaMore::err("#NUM!");
                }
                if first {
                    acc = k;
                    first = false;
                } else {
                    if n == "GCD" {
                        acc = FormulaMore::gcd2(acc, k);
                    } else {
                        if acc == 0 || k == 0 {
                            acc = 0;
                        } else {
                            acc = (acc / FormulaMore::gcd2(acc, k)) * k;
                        }
                    }
                }
            }
            return FormulaMore::numV(acc as f64);
        }
        let mut known: bool = false;
        let ones: String = " LOG SQRTPI EVEN ODD ISEVEN ISODD FACT FACTDOUBLE COMBIN COMBINA PERMUT QUOTIENT MROUND CEILING.MATH FLOOR.MATH CEILING.PRECISE FLOOR.PRECISE ISO.CEILING RADIANS DEGREES SIN COS TAN ASIN ACOS ATAN ATAN2 SINH COSH TANH COT CSC SEC BASE DECIMAL DEC2BIN BIN2DEC DEC2HEX HEX2DEC DEC2OCT OCT2DEC ROMAN ARABIC ".to_string();
        if DgNative::index_of(&ones, &format!(" {} ", n)) >= 0 {
            known = true;
        }
        if !known {
            return FormulaValue::blank();
        }
        let e3: Rc<RefCell<FormulaValue>> = FormulaMore::firstErr(args, 1);
        if e3.borrow().isError() {
            return e3;
        }
        // Text conversions read their first argument as text.
        if n == "BIN2DEC" || n == "HEX2DEC" || n == "OCT2DEC" || n == "DECIMAL" {
            let mut base: i64 = 2;
            if n == "HEX2DEC" {
                base = 16;
            }
            if n == "OCT2DEC" {
                base = 8;
            }
            if n == "DECIMAL" {
                base = FormulaMore::int(args, 1, 10);
            }
            let s: String = FormulaFunctions::upper(&FormulaMore::txt(args, 0));
            let mut v: f64 = 0.0;
            let mut i: i64 = 0;
            let len: i64 = DgNative::str_len(&s);
            while i < len {
                let ch: i64 = DgNative::char_at(&s, i);
                let mut d: i64 = -1;
                if ch >= 48 && ch <= 57 {
                    d = ch - 48;
                }
                if ch >= 65 && ch <= 90 {
                    d = ch - 55;
                }
                if d < 0 || d >= base {
                    return FormulaMore::err("#NUM!");
                }
                v = v * (base as f64) + (d as f64);
                i += 1;
            }
            // Ten digits of two's complement for the Excel conversions.
            if n != "DECIMAL" && len == 10 {
                let top: f64 = FormulaMore::powD(base as f64, 10.0);
                if v >= top / 2.0 {
                    v -= top;
                }
            }
            return FormulaMore::numV(v);
        }
        if n == "ARABIC" {
            let s2: String = FormulaFunctions::upper(&FormulaMore::txt(args, 0));
            let mut total: i64 = 0;
            let mut prev: i64 = 0;
            let mut i2: i64 = DgNative::str_len(&s2) - 1;
            while i2 >= 0 {
                let ch2: i64 = DgNative::char_at(&s2, i2);
                let mut d2: i64 = 0;
                if ch2 == 73 {
                    d2 = 1;
                }
                if ch2 == 86 {
                    d2 = 5;
                }
                if ch2 == 88 {
                    d2 = 10;
                }
                if ch2 == 76 {
                    d2 = 50;
                }
                if ch2 == 67 {
                    d2 = 100;
                }
                if ch2 == 68 {
                    d2 = 500;
                }
                if ch2 == 77 {
                    d2 = 1000;
                }
                if d2 == 0 {
                    return FormulaMore::err("#VALUE!");
                }
                if d2 < prev {
                    total -= d2;
                } else {
                    total += d2;
                    prev = d2;
                }
                i2 -= 1;
            }
            return FormulaMore::numV(total as f64);
        }
        let x: f64 = FormulaMore::num(args, 0, 0.0);
        if n == "LOG" {
            let b: f64 = FormulaMore::num(args, 1, 10.0);
            if x <= 0.0 || b <= 0.0 || b == 1.0 {
                return FormulaMore::err("#NUM!");
            }
            return FormulaMore::numV(FormulaMore::lnD(x) / FormulaMore::lnD(b));
        }
        if n == "SQRTPI" {
            if x < 0.0 {
                return FormulaMore::err("#NUM!");
            }
            return FormulaMore::numV(FormulaMore::sqrtD(x * FormulaMore::PI()));
        }
        if n == "EVEN" || n == "ODD" {
            let a: f64 = FormulaMore::absD(x);
            let mut k: f64 = a.floor();
            if k < a {
                k += 1.0;
            }
            let half: f64 = (k / 2.0).floor();
            let is_even: bool = half * 2.0 == k;
            if n == "EVEN" && !is_even {
                k += 1.0;
            }
            if n == "ODD" {
                if is_even {
                    k += 1.0;
                }
            }
            if x < 0.0 {
                return FormulaMore::numV(0.0 - k);
            }
            return FormulaMore::numV(k);
        }
        if n == "ISEVEN" || n == "ISODD" {
            let k2: f64 = FormulaMore::absD(x).floor();
            let even: bool = (k2 / 2.0).floor() * 2.0 == k2;
            if n == "ISEVEN" {
                return FormulaValue::fromBool(even);
            }
            return FormulaValue::fromBool(!even);
        }
        if n == "FACT" {
            if x < 0.0 {
                return FormulaMore::err("#NUM!");
            }
            return FormulaMore::numV(FormulaMore::fact(x.floor() as i64));
        }
        if n == "FACTDOUBLE" {
            if x < -1.0 {
                return FormulaMore::err("#NUM!");
            }
            let mut out: f64 = 1.0;
            let mut k3: i64 = x.floor() as i64;
            while k3 > 1 {
                out *= k3 as f64;
                k3 -= 2;
            }
            return FormulaMore::numV(out);
        }
        if n == "COMBIN" || n == "COMBINA" || n == "PERMUT" {
            let nn: i64 = x.floor() as i64;
            let kk: i64 = FormulaMore::int(args, 1, 0);
            if nn < 0 || kk < 0 {
                return FormulaMore::err("#NUM!");
            }
            if n == "COMBINA" {
                return FormulaMore::numV(FormulaMore::combin(nn + kk - 1, kk));
            }
            if kk > nn {
                return FormulaMore::err("#NUM!");
            }
            if n == "COMBIN" {
                return FormulaMore::numV(FormulaMore::combin(nn, kk));
            }
            let mut p: f64 = 1.0;
            let mut i3: i64 = 0;
            while i3 < kk {
                p *= (nn - i3) as f64;
                i3 += 1;
            }
            return FormulaMore::numV(p);
        }
        if n == "QUOTIENT" {
            let d: f64 = FormulaMore::num(args, 1, 0.0);
            if d == 0.0 {
                return FormulaMore::err("#DIV/0!");
            }
            let q: f64 = x / d;
            if q < 0.0 {
                return FormulaMore::numV(0.0 - (0.0 - q).floor());
            }
            return FormulaMore::numV(q.floor());
        }
        if n == "MROUND" {
            let m: f64 = FormulaMore::num(args, 1, 0.0);
            if m == 0.0 {
                return FormulaMore::numV(0.0);
            }
            if (x > 0.0 && m < 0.0) || (x < 0.0 && m > 0.0) {
                return FormulaMore::err("#NUM!");
            }
            let q2: f64 = FormulaMore::roundD(x / m, 0);
            return FormulaMore::numV(FormulaMore::roundD(q2 * m, 10));
        }
        if n == "CEILING.MATH" || n == "FLOOR.MATH" || n == "CEILING.PRECISE" || n == "FLOOR.PRECISE" || n == "ISO.CEILING" {
            let sig: f64 = FormulaMore::absD(FormulaMore::num(args, 1, 1.0));
            if sig == 0.0 {
                return FormulaMore::numV(0.0);
            }
            let mode: f64 = FormulaMore::num(args, 2, 0.0);
            let mut up: bool = n == "CEILING.MATH" || n == "CEILING.PRECISE" || n == "ISO.CEILING";
            // A non-zero mode turns negative numbers the other way: CEILING.MATH
            // away from zero, FLOOR.MATH toward it.
            if x < 0.0 && mode != 0.0 && (n == "CEILING.MATH" || n == "FLOOR.MATH") {
                up = !up;
            }
            let q3: f64 = x / sig;
            let mut k4: f64 = q3.floor();
            if up && k4 < q3 {
                k4 += 1.0;
            }
            return FormulaMore::numV(FormulaMore::roundD(k4 * sig, 10));
        }
        if n == "RADIANS" {
            return FormulaMore::numV((x * FormulaMore::PI()) / 180.0);
        }
        if n == "DEGREES" {
            return FormulaMore::numV((x * 180.0) / FormulaMore::PI());
        }
        if n == "SIN" {
            return FormulaMore::numV(FormulaMore::sinD(x));
        }
        if n == "COS" {
            return FormulaMore::numV(FormulaMore::cosD(x));
        }
        if n == "TAN" || n == "COT" {
            let c2: f64 = FormulaMore::cosD(x);
            let s3: f64 = FormulaMore::sinD(x);
            if n == "TAN" {
                if c2 == 0.0 {
                    return FormulaMore::err("#DIV/0!");
                }
                return FormulaMore::numV(s3 / c2);
            }
            if s3 == 0.0 {
                return FormulaMore::err("#DIV/0!");
            }
            return FormulaMore::numV(c2 / s3);
        }
        if n == "CSC" {
            let s4: f64 = FormulaMore::sinD(x);
            if s4 == 0.0 {
                return FormulaMore::err("#DIV/0!");
            }
            return FormulaMore::numV(1.0 / s4);
        }
        if n == "SEC" {
            let c3: f64 = FormulaMore::cosD(x);
            if c3 == 0.0 {
                return FormulaMore::err("#DIV/0!");
            }
            return FormulaMore::numV(1.0 / c3);
        }
        if n == "ASIN" || n == "ACOS" {
            if x < -1.0 || x > 1.0 {
                return FormulaMore::err("#NUM!");
            }
            if n == "ASIN" {
                return FormulaMore::numV(FormulaMore::asinD(x));
            }
            return FormulaMore::numV(FormulaMore::PI() / 2.0 - FormulaMore::asinD(x));
        }
        if n == "ATAN" {
            return FormulaMore::numV(FormulaMore::atanD(x));
        }
        if n == "ATAN2" {
            // ATAN2(x, y): Excel puts x first.
            let y: f64 = FormulaMore::num(args, 1, 0.0);
            if x == 0.0 && y == 0.0 {
                return FormulaMore::err("#DIV/0!");
            }
            if x > 0.0 {
                return FormulaMore::numV(FormulaMore::atanD(y / x));
            }
            if x < 0.0 {
                if y >= 0.0 {
                    return FormulaMore::numV(FormulaMore::atanD(y / x) + FormulaMore::PI());
                }
                return FormulaMore::numV(FormulaMore::atanD(y / x) - FormulaMore::PI());
            }
            if y > 0.0 {
                return FormulaMore::numV(FormulaMore::PI() / 2.0);
            }
            return FormulaMore::numV(0.0 - FormulaMore::PI() / 2.0);
        }
        if n == "SINH" || n == "COSH" || n == "TANH" {
            let ep: f64 = FormulaMore::expD(x);
            let em: f64 = 1.0 / ep;
            if n == "SINH" {
                return FormulaMore::numV((ep - em) / 2.0);
            }
            if n == "COSH" {
                return FormulaMore::numV((ep + em) / 2.0);
            }
            return FormulaMore::numV((ep - em) / (ep + em));
        }
        if n == "BASE" || n == "DEC2BIN" || n == "DEC2HEX" || n == "DEC2OCT" {
            let mut base2: i64 = 2;
            let mut places: i64 = FormulaMore::int(args, 1, 0);
            if n == "DEC2HEX" {
                base2 = 16;
            }
            if n == "DEC2OCT" {
                base2 = 8;
            }
            if n == "BASE" {
                base2 = FormulaMore::int(args, 1, 10);
                places = FormulaMore::int(args, 2, 0);
                if base2 < 2 || base2 > 36 || x < 0.0 {
                    return FormulaMore::err("#NUM!");
                }
            }
            let mut v2: f64 = x.floor();
            if v2 < 0.0 {
                if n == "BASE" {
                    return FormulaMore::err("#NUM!");
                }
                v2 += FormulaMore::powD(base2 as f64, 10.0);
                places = 0;
            }
            let digits: String = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_string();
            let mut out2: String = String::new();
            if v2 == 0.0 {
                out2 = "0".to_string();
            }
            while v2 > 0.0 {
                let q4: f64 = (v2 / (base2 as f64)).floor();
                let d3: i64 = (v2 - q4 * (base2 as f64)) as i64;
                out2 = format!("{}{}", DgNative::substr(&digits, d3, d3 + 1), out2);
                v2 = q4;
            }
            while DgNative::str_len(&out2) < places {
                out2 = format!("0{}", out2);
            }
            return FormulaValue::fromString(out2);
        }
        if n == "ROMAN" {
            let mut v3: i64 = x.floor() as i64;
            if v3 < 0 || v3 > 3999 {
                return FormulaMore::err("#VALUE!");
            }
            let vals: Vec<i64> = vec![1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1];
            let syms: Vec<String> = vec!["M".to_string(), "CM".to_string(), "D".to_string(), "CD".to_string(), "C".to_string(), "XC".to_string(), "L".to_string(), "XL".to_string(), "X".to_string(), "IX".to_string(), "V".to_string(), "IV".to_string(), "I".to_string()];
            let mut out3: String = String::new();
            let mut i4: i64 = 0;
            while i4 < 13 {
                while v3 >= vals[i4 as usize] {
                    out3 = format!("{}{}", out3, syms[i4 as usize].clone());
                    v3 -= vals[i4 as usize];
                }
                i4 += 1;
            }
            return FormulaValue::fromString(out3);
        }
        FormulaValue::blank()
    }

    // --- statistics -------------------------------------------------------------------

    pub fn percentileOf(vals: &Vec<f64>, p: f64, exclusive: bool) -> f64 {
        let n: i64 = vals.len() as i64;
        let mut h: f64 = p * ((n - 1) as f64);
        if exclusive {
            h = p * ((n + 1) as f64) - 1.0;
        }
        if h <= 0.0 {
            return vals[0];
        }
        if h >= ((n - 1) as f64) {
            return vals[(n - 1) as usize];
        }
        let lo: i64 = h.floor() as i64;
        let frac: f64 = h - (lo as f64);
        vals[lo as usize] + frac * (vals[(lo + 1) as usize] - vals[lo as usize])
    }
    /// Pairs of numbers from two equal-length arguments, for CORREL and its kin.
    pub fn pairs(args: &Vec<Rc<RefCell<FormulaValue>>>, xs: &mut Vec<f64>, ys: &mut Vec<f64>, y_first: bool) -> bool {
        let a: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[0].clone());
        let b: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[1].clone());
        if a.len() != b.len() {
            return false;
        }
        let mut i: i64 = 0;
        while i < (a.len() as i64) {
            let av: Rc<RefCell<FormulaValue>> = a[i as usize].clone();
            let bv: Rc<RefCell<FormulaValue>> = b[i as usize].clone();
            if av.borrow().isNumeric() && bv.borrow().isNumeric() {
                if y_first {
                    ys.push(av.borrow().numberValue());
                    xs.push(bv.borrow().numberValue());
                } else {
                    xs.push(av.borrow().numberValue());
                    ys.push(bv.borrow().numberValue());
                }
            }
            i += 1;
        }
        true
    }
    pub fn meanOf(v: &Vec<f64>) -> f64 {
        let mut s: f64 = 0.0;
        for x in v.iter() {
            s += *x;
        }
        s / (v.len() as f64)
    }
    pub fn statFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if n == "MAXIFS" || n == "MINIFS" {
            if (args.len() as i64) < 3 || ((args.len() as i64) - 1) % 2 != 0 {
                return FormulaMore::err("#VALUE!");
            }
            let values: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[0].clone());
            let mut flats: Vec<Vec<Rc<RefCell<FormulaValue>>>> = Vec::new();
            let mut p: i64 = 1;
            while p < (args.len() as i64) {
                let f: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[p as usize].clone());
                if f.len() != values.len() {
                    return FormulaMore::err("#VALUE!");
                }
                flats.push(f.clone());
                p += 2;
            }
            let mut found: bool = false;
            let mut best: f64 = 0.0;
            let mut i: i64 = 0;
            while i < (values.len() as i64) {
                let mut ok: bool = true;
                let mut c: i64 = 0;
                while c < (flats.len() as i64) {
                    let row: Vec<Rc<RefCell<FormulaValue>>> = flats[c as usize].clone();
                    let crit: Rc<RefCell<FormulaValue>> = args[(2 + c * 2) as usize].clone();
                    if !FormulaFunctions::criterionMatches(&row[i as usize].clone(), &crit) {
                        ok = false;
                        break;
                    }
                    c += 1;
                }
                let v: Rc<RefCell<FormulaValue>> = values[i as usize].clone();
                if ok && v.borrow().isNumeric() {
                    let x: f64 = v.borrow().numberValue();
                    if !found || (n == "MAXIFS" && x > best) || (n == "MINIFS" && x < best) {
                        best = x;
                        found = true;
                    }
                }
                i += 1;
            }
            return FormulaMore::numV(best);
        }
        let all: String = " RANK RANK.EQ RANK.AVG MODE MODE.SNGL PERCENTILE PERCENTILE.INC PERCENTILE.EXC QUARTILE QUARTILE.INC QUARTILE.EXC PERCENTRANK PERCENTRANK.INC GEOMEAN HARMEAN AVEDEV DEVSQ AVERAGEA MAXA MINA CORREL PEARSON COVAR COVARIANCE.P COVARIANCE.S SLOPE INTERCEPT RSQ FORECAST FORECAST.LINEAR STEYX STANDARDIZE TRIMMEAN COUNTUNIQUE ".to_string();
        if DgNative::index_of(&all, &format!(" {} ", n)) < 0 {
            return FormulaValue::blank();
        }
        let e: Rc<RefCell<FormulaValue>> = FormulaMore::firstErr(args, 1);
        if e.borrow().isError() {
            return e;
        }
        if n == "STANDARDIZE" {
            let sd: f64 = FormulaMore::num(args, 2, 0.0);
            if sd <= 0.0 {
                return FormulaMore::err("#NUM!");
            }
            return FormulaMore::numV((FormulaMore::num(args, 0, 0.0) - FormulaMore::num(args, 1, 0.0)) / sd);
        }
        if n == "RANK" || n == "RANK.EQ" || n == "RANK.AVG" {
            if (args.len() as i64) < 2 {
                return FormulaMore::err("#VALUE!");
            }
            let x: f64 = FormulaMore::num(args, 0, 0.0);
            let asc: bool = FormulaMore::num(args, 2, 0.0) != 0.0;
            let vals: Vec<f64> = FormulaMore::numbersOf(&args[1].clone());
            let mut better: i64 = 0;
            let mut same: i64 = 0;
            for v in vals.iter() {
                if *v == x {
                    same += 1;
                } else {
                    if (asc && *v < x) || (!asc && *v > x) {
                        better += 1;
                    }
                }
            }
            if same == 0 {
                return FormulaMore::err("#N/A");
            }
            if n == "RANK.AVG" {
                return FormulaMore::numV((better as f64) + ((same + 1) as f64) / 2.0);
            }
            return FormulaMore::numV((better + 1) as f64);
        }
        if n == "AVERAGEA" || n == "MAXA" || n == "MINA" {
            let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(args);
            let mut s: f64 = 0.0;
            let mut cnt: i64 = 0;
            let mut hi: f64 = 0.0;
            let mut lo: f64 = 0.0;
            for v in flat.iter() {
                if v.borrow().isBlank() {
                    continue;
                }
                let mut x2: f64 = 0.0;
                if v.borrow().isNumeric() {
                    x2 = v.borrow().numberValue();
                } else {
                    if v.borrow().kind == 3 && v.borrow().flag {
                        x2 = 1.0;
                    }
                }
                if cnt == 0 || x2 > hi {
                    hi = x2;
                }
                if cnt == 0 || x2 < lo {
                    lo = x2;
                }
                s += x2;
                cnt += 1;
            }
            if n == "MAXA" {
                return FormulaMore::numV(hi);
            }
            if n == "MINA" {
                return FormulaMore::numV(lo);
            }
            if cnt == 0 {
                return FormulaMore::err("#DIV/0!");
            }
            return FormulaMore::numV(s / (cnt as f64));
        }
        if n == "COUNTUNIQUE" {
            let flat2: Vec<Rc<RefCell<FormulaValue>>> = FormulaFunctions::flatArgs(args);
            let mut seen: Vec<String> = Vec::new();
            for v in flat2.iter() {
                if v.borrow().isBlank() {
                    continue;
                }
                let k: String = v.borrow().asRaw();
                if !seen.contains(&k) {
                    seen.push(k.clone());
                }
            }
            return FormulaMore::numV(seen.len() as f64);
        }
        let pair_fns: String = " CORREL PEARSON COVAR COVARIANCE.P COVARIANCE.S SLOPE INTERCEPT RSQ FORECAST FORECAST.LINEAR STEYX ".to_string();
        if DgNative::index_of(&pair_fns, &format!(" {} ", n)) >= 0 {
            let mut xs: Vec<f64> = Vec::new();
            let mut ys: Vec<f64> = Vec::new();
            let mut base: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut px: f64 = 0.0;
            if n == "FORECAST" || n == "FORECAST.LINEAR" {
                if (args.len() as i64) < 3 {
                    return FormulaMore::err("#VALUE!");
                }
                px = FormulaMore::num(args, 0, 0.0);
                base.push(args[1].clone());
                base.push(args[2].clone());
            } else {
                if (args.len() as i64) < 2 {
                    return FormulaMore::err("#VALUE!");
                }
                base.push(args[0].clone());
                base.push(args[1].clone());
            }
            // SLOPE, INTERCEPT, RSQ, FORECAST and STEYX take the y values first.
            let y_first: bool = n == "SLOPE" || n == "INTERCEPT" || n == "RSQ" || n == "FORECAST" || n == "FORECAST.LINEAR" || n == "STEYX";
            if !FormulaMore::pairs(&base, &mut xs, &mut ys, y_first) {
                return FormulaMore::err("#N/A");
            }
            let cnt2: i64 = xs.len() as i64;
            if cnt2 < 1 {
                return FormulaMore::err("#DIV/0!");
            }
            let mx: f64 = FormulaMore::meanOf(&xs);
            let my: f64 = FormulaMore::meanOf(&ys);
            let mut sxy: f64 = 0.0;
            let mut sxx: f64 = 0.0;
            let mut syy: f64 = 0.0;
            let mut i2: i64 = 0;
            while i2 < cnt2 {
                let dx: f64 = xs[i2 as usize] - mx;
                let dy: f64 = ys[i2 as usize] - my;
                sxy += dx * dy;
                sxx += dx * dx;
                syy += dy * dy;
                i2 += 1;
            }
            if n == "COVAR" || n == "COVARIANCE.P" {
                return FormulaMore::numV(sxy / (cnt2 as f64));
            }
            if n == "COVARIANCE.S" {
                if cnt2 < 2 {
                    return FormulaMore::err("#DIV/0!");
                }
                return FormulaMore::numV(sxy / ((cnt2 - 1) as f64));
            }
            if n == "CORREL" || n == "PEARSON" || n == "RSQ" {
                if sxx == 0.0 || syy == 0.0 {
                    return FormulaMore::err("#DIV/0!");
                }
                let r: f64 = sxy / FormulaMore::sqrtD(sxx * syy);
                if n == "RSQ" {
                    return FormulaMore::numV(r * r);
                }
                return FormulaMore::numV(r);
            }
            if sxx == 0.0 {
                return FormulaMore::err("#DIV/0!");
            }
            let slope: f64 = sxy / sxx;
            let icept: f64 = my - slope * mx;
            if n == "SLOPE" {
                return FormulaMore::numV(slope);
            }
            if n == "INTERCEPT" {
                return FormulaMore::numV(icept);
            }
            if n == "STEYX" {
                if cnt2 < 3 {
                    return FormulaMore::err("#DIV/0!");
                }
                return FormulaMore::numV(FormulaMore::sqrtD((syy - (sxy * sxy) / sxx) / ((cnt2 - 2) as f64)));
            }
            return FormulaMore::numV(icept + slope * px);
        }
        // The rest read one list of numbers (and sometimes a parameter after it).
        let mut list: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        list.push(args[0].clone());
        let one_list: bool = n == "PERCENTILE" || n == "PERCENTILE.INC" || n == "PERCENTILE.EXC" || n == "QUARTILE" || n == "QUARTILE.INC" || n == "QUARTILE.EXC" || n == "PERCENTRANK" || n == "PERCENTRANK.INC" || n == "TRIMMEAN";
        let mut vals2: Vec<f64> = Vec::new();
        if one_list {
            vals2 = FormulaFunctions::collectNumbers(&list);
        } else {
            vals2 = FormulaFunctions::collectNumbers(args);
        }
        if vals2.is_empty() {
            if n == "MODE" || n == "MODE.SNGL" {
                return FormulaMore::err("#N/A");
            }
            return FormulaMore::err("#NUM!");
        }
        let sorted: Vec<f64> = FormulaFunctions::sortDoubles(vals2.clone());
        let cnt3: i64 = sorted.len() as i64;
        if n == "MODE" || n == "MODE.SNGL" {
            let mut best_v: f64 = 0.0;
            let mut best_n: i64 = 1;
            let mut i3: i64 = 0;
            while i3 < (vals2.len() as i64) {
                let mut c3: i64 = 0;
                for v in vals2.iter() {
                    if *v == vals2[i3 as usize] {
                        c3 += 1;
                    }
                }
                if c3 > best_n {
                    best_n = c3;
                    best_v = vals2[i3 as usize];
                }
                i3 += 1;
            }
            if best_n < 2 {
                return FormulaMore::err("#N/A");
            }
            return FormulaMore::numV(best_v);
        }
        if n == "PERCENTILE" || n == "PERCENTILE.INC" || n == "PERCENTILE.EXC" {
            let p2: f64 = FormulaMore::num(args, 1, 0.0);
            if p2 < 0.0 || p2 > 1.0 {
                return FormulaMore::err("#NUM!");
            }
            if n == "PERCENTILE.EXC" {
                let h: f64 = p2 * ((cnt3 + 1) as f64);
                if h < 1.0 || h > (cnt3 as f64) {
                    return FormulaMore::err("#NUM!");
                }
            }
            return FormulaMore::numV(FormulaMore::percentileOf(&sorted, p2, n == "PERCENTILE.EXC"));
        }
        if n == "QUARTILE" || n == "QUARTILE.INC" || n == "QUARTILE.EXC" {
            let q: i64 = FormulaMore::int(args, 1, 0);
            if q < 0 || q > 4 {
                return FormulaMore::err("#NUM!");
            }
            if n == "QUARTILE.EXC" && (q == 0 || q == 4) {
                return FormulaMore::err("#NUM!");
            }
            return FormulaMore::numV(FormulaMore::percentileOf(&sorted, (q as f64) / 4.0, n == "QUARTILE.EXC"));
        }
        if n == "PERCENTRANK" || n == "PERCENTRANK.INC" {
            let x3: f64 = FormulaMore::num(args, 1, 0.0);
            let digits: i64 = FormulaMore::int(args, 2, 3);
            if x3 < sorted[0] || x3 > sorted[(cnt3 - 1) as usize] {
                return FormulaMore::err("#N/A");
            }
            if cnt3 == 1 {
                return FormulaMore::numV(1.0);
            }
            let mut i4: i64 = 0;
            while i4 < cnt3 - 1 && sorted[(i4 + 1) as usize] <= x3 {
                i4 += 1;
            }
            let mut rank: f64 = i4 as f64;
            if sorted[i4 as usize] < x3 {
                rank += (x3 - sorted[i4 as usize]) / (sorted[(i4 + 1) as usize] - sorted[i4 as usize]);
            } else {
                // the first of equal values
                let mut j: i64 = i4;
                while j > 0 && sorted[(j - 1) as usize] == x3 {
                    j -= 1;
                }
                rank = j as f64;
            }
            let pr: f64 = rank / ((cnt3 - 1) as f64);
            let p10: f64 = FormulaFunctions::powTen(digits);
            return FormulaMore::numV((pr * p10).floor() / p10);
        }
        if n == "TRIMMEAN" {
            let pct: f64 = FormulaMore::num(args, 1, 0.0);
            if pct < 0.0 || pct >= 1.0 {
                return FormulaMore::err("#NUM!");
            }
            let mut cut: i64 = ((cnt3 as f64) * pct / 2.0).floor() as i64;
            if cut * 2 >= cnt3 {
                cut = 0;
            }
            let mut s2: f64 = 0.0;
            let mut i5: i64 = cut;
            while i5 < cnt3 - cut {
                s2 += sorted[i5 as usize];
                i5 += 1;
            }
            return FormulaMore::numV(s2 / ((cnt3 - 2 * cut) as f64));
        }
        let mean: f64 = FormulaMore::meanOf(&vals2);
        if n == "GEOMEAN" {
            let mut sl: f64 = 0.0;
            for v in vals2.iter() {
                if *v <= 0.0 {
                    return FormulaMore::err("#NUM!");
                }
                sl += FormulaMore::lnD(*v);
            }
            return FormulaMore::numV(FormulaMore::expD(sl / (cnt3 as f64)));
        }
        if n == "HARMEAN" {
            let mut si: f64 = 0.0;
            for v in vals2.iter() {
                if *v <= 0.0 {
                    return FormulaMore::err("#NUM!");
                }
                si += 1.0 / *v;
            }
            return FormulaMore::numV((cnt3 as f64) / si);
        }
        let mut dev: f64 = 0.0;
        let mut sq: f64 = 0.0;
        for v in vals2.iter() {
            dev += FormulaMore::absD(*v - mean);
            sq += (*v - mean) * (*v - mean);
        }
        if n == "AVEDEV" {
            return FormulaMore::numV(dev / (cnt3 as f64));
        }
        FormulaMore::numV(sq)
    }

    // --- dates and times -----------------------------------------------------------------

    pub fn serialOf(v: &Rc<RefCell<FormulaValue>>) -> f64 {
        let s: Rc<RefCell<FormulaValue>> = v.borrow().single();
        if s.borrow().kind == 2 {
            let t: String = s.borrow().text.clone();
            let d: f64 = DateSerial::parseDate(&t);
            if d > 0.0 {
                return d;
            }
            return FormulaMore::timeOfText(&t);
        }
        let out: f64 = s.borrow().asNumber();
        out
    }
    /// "13:45", "1:45 PM", "13:45:30": the fraction of a day, or -1.
    pub fn timeOfText(t0: &str) -> f64 {
        let t: String = FormulaFunctions::upper(t0);
        let mut parts: Vec<f64> = Vec::new();
        let mut cur: f64 = 0.0;
        let mut digits: i64 = 0;
        let mut pm: i64 = 0;
        let mut i: i64 = 0;
        let len: i64 = DgNative::str_len(&t);
        while i < len {
            let ch: i64 = DgNative::char_at(&t, i);
            if ch >= 48 && ch <= 57 {
                cur = cur * 10.0 + ((ch - 48) as f64);
                digits += 1;
            } else {
                if ch == 58 {
                    parts.push(cur);
                    cur = 0.0;
                    digits = 0;
                } else {
                    if ch == 65 {
                        pm = 1;
                    }
                    if ch == 80 {
                        pm = 2;
                    }
                }
            }
            i += 1;
        }
        if digits > 0 {
            parts.push(cur);
        }
        if parts.len() < 2 {
            return -1.0;
        }
        let mut h: f64 = parts[0];
        if pm == 2 && h < 12.0 {
            h += 12.0;
        }
        if pm == 1 && h == 12.0 {
            h = 0.0;
        }
        let mut secs: f64 = h * 3600.0 + parts[1] * 60.0;
        if parts.len() > 2 {
            secs += parts[2];
        }
        secs / 86400.0
    }
    pub fn secondsOfDay(serial: f64) -> i64 {
        let frac: f64 = serial - serial.floor();
        let s: i64 = (frac * 86400.0 + 0.5).floor() as i64;
        s % 86400
    }
    pub fn isWeekend(serial: f64, mask: &str) -> bool {
        // mask: seven characters Monday..Sunday, "1" = weekend.
        let w: i64 = DateSerial::weekday(serial);
        // weekday: 1 = Sunday … 7 = Saturday; mask index: 0 = Monday.
        let mut idx: i64 = w - 2;
        if idx < 0 {
            idx = 6;
        }
        DgNative::char_at(mask, idx) == 49
    }
    pub fn weekendMask(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64) -> String {
        if !FormulaMore::has(args, i) {
            return "0000011".to_string();
        }
        let v: Rc<RefCell<FormulaValue>> = args[i as usize].borrow().single();
        if v.borrow().kind == 2 {
            let t: String = v.borrow().text.clone();
            if DgNative::str_len(&t) == 7 {
                return t;
            }
            return "0000011".to_string();
        }
        let code: i64 = v.borrow().asNumber().floor() as i64;
        let masks: Vec<String> = vec!["0000011".to_string(), "1000001".to_string(), "1100000".to_string(), "0110000".to_string(), "0011000".to_string(), "0001100".to_string(), "0000110".to_string()];
        if code >= 1 && code <= 7 {
            return masks[(code - 1) as usize].clone();
        }
        let singles: Vec<String> = vec!["1000000".to_string(), "0100000".to_string(), "0010000".to_string(), "0001000".to_string(), "0000100".to_string(), "0000010".to_string(), "0000001".to_string()];
        if code >= 11 && code <= 17 {
            return singles[(code - 11) as usize].clone();
        }
        "0000011".to_string()
    }
    pub fn holidaysOf(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64) -> Vec<f64> {
        let mut out: Vec<f64> = Vec::new();
        if i >= (args.len() as i64) {
            return out;
        }
        for v in FormulaMore::flatOf(&args[i as usize].clone()).iter() {
            if !v.borrow().isBlank() {
                out.push(FormulaMore::serialOf(v).floor());
            }
        }
        out
    }
    pub fn dateFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let all: String = " TIME HOUR MINUTE SECOND TIMEVALUE WEEKNUM ISOWEEKNUM NETWORKDAYS WORKDAY NETWORKDAYS.INTL WORKDAY.INTL DATEDIF DAYS360 YEARFRAC ".to_string();
        if DgNative::index_of(&all, &format!(" {} ", n)) < 0 {
            return FormulaValue::blank();
        }
        let e: Rc<RefCell<FormulaValue>> = FormulaMore::firstErr(args, 1);
        if e.borrow().isError() {
            return e;
        }
        if n == "TIME" {
            if (args.len() as i64) < 3 {
                return FormulaMore::err("#VALUE!");
            }
            let secs: f64 = FormulaMore::num(args, 0, 0.0).floor() * 3600.0 + FormulaMore::num(args, 1, 0.0).floor() * 60.0 + FormulaMore::num(args, 2, 0.0).floor();
            if secs < 0.0 {
                return FormulaMore::err("#NUM!");
            }
            let day: f64 = secs / 86400.0;
            return FormulaMore::numV(day - day.floor());
        }
        if n == "TIMEVALUE" {
            let t: f64 = FormulaMore::timeOfText(&FormulaMore::txt(args, 0));
            if t < 0.0 {
                return FormulaMore::err("#VALUE!");
            }
            return FormulaMore::numV(t - t.floor());
        }
        let d: f64 = FormulaMore::serialOf(&args[0].clone());
        if n == "HOUR" || n == "MINUTE" || n == "SECOND" {
            if d < 0.0 {
                return FormulaMore::err("#NUM!");
            }
            let s: i64 = FormulaMore::secondsOfDay(d);
            if n == "HOUR" {
                return FormulaMore::numV((s / 3600) as f64);
            }
            if n == "MINUTE" {
                return FormulaMore::numV(((s / 60) % 60) as f64);
            }
            return FormulaMore::numV((s % 60) as f64);
        }
        let ymd: Vec<i64> = DateSerial::toYMD(d);
        if n == "WEEKNUM" {
            let kind: i64 = FormulaMore::int(args, 1, 1);
            if kind == 21 {
                return FormulaMore::isoWeek(d);
            }
            let jan1: f64 = DateSerial::fromYMD(ymd[0], 1, 1);
            // first day of the week: 1/17 Sunday, 2/11 Monday, 12 Tuesday … 16 Saturday
            let mut start: i64 = 1;
            if kind == 2 || kind == 11 {
                start = 2;
            }
            if kind >= 12 && kind <= 16 {
                start = kind - 9;
            }
            let mut off: i64 = DateSerial::weekday(jan1) - start;
            if off < 0 {
                off += 7;
            }
            let doy: i64 = (d.floor() - jan1) as i64;
            return FormulaMore::numV(((doy + off) / 7 + 1) as f64);
        }
        if n == "ISOWEEKNUM" {
            return FormulaMore::isoWeek(d);
        }
        if n == "NETWORKDAYS" || n == "NETWORKDAYS.INTL" {
            if (args.len() as i64) < 2 {
                return FormulaMore::err("#VALUE!");
            }
            let mut a: f64 = d.floor();
            let mut b: f64 = FormulaMore::serialOf(&args[1].clone()).floor();
            let mut sign: f64 = 1.0;
            if b < a {
                let t2: f64 = a;
                a = b;
                b = t2;
                sign = -1.0;
            }
            let mut mask: String = "0000011".to_string();
            let mut hi: i64 = 2;
            if n == "NETWORKDAYS.INTL" {
                mask = FormulaMore::weekendMask(args, 2);
                hi = 3;
            }
            let hols: Vec<f64> = FormulaMore::holidaysOf(args, hi);
            let mut cnt: i64 = 0;
            let mut x: f64 = a;
            while x <= b {
                if !FormulaMore::isWeekend(x, &mask) && !hols.contains(&x) {
                    cnt += 1;
                }
                x += 1.0;
            }
            return FormulaMore::numV((cnt as f64) * sign);
        }
        if n == "WORKDAY" || n == "WORKDAY.INTL" {
            if (args.len() as i64) < 2 {
                return FormulaMore::err("#VALUE!");
            }
            let mut days: i64 = FormulaMore::int(args, 1, 0);
            let mut mask2: String = "0000011".to_string();
            let mut hi2: i64 = 2;
            if n == "WORKDAY.INTL" {
                mask2 = FormulaMore::weekendMask(args, 2);
                hi2 = 3;
                if mask2 == "1111111" {
                    return FormulaMore::err("#VALUE!");
                }
            }
            let hols2: Vec<f64> = FormulaMore::holidaysOf(args, hi2);
            let mut step: f64 = 1.0;
            if days < 0 {
                step = -1.0;
                days = 0 - days;
            }
            let mut x2: f64 = d.floor();
            while days > 0 {
                x2 += step;
                if !FormulaMore::isWeekend(x2, &mask2) && !hols2.contains(&x2) {
                    days -= 1;
                }
            }
            return FormulaMore::numV(x2);
        }
        if (args.len() as i64) < 2 {
            return FormulaMore::err("#VALUE!");
        }
        let d2: f64 = FormulaMore::serialOf(&args[1].clone());
        let e2: Vec<i64> = DateSerial::toYMD(d2);
        if n == "DATEDIF" {
            if d2 < d {
                return FormulaMore::err("#NUM!");
            }
            let unit: String = FormulaFunctions::upper(&FormulaMore::txt(args, 2));
            let mut months: i64 = (e2[0] - ymd[0]) * 12 + (e2[1] - ymd[1]);
            if e2[2] < ymd[2] {
                months -= 1;
            }
            if unit == "Y" {
                return FormulaMore::numV((months / 12) as f64);
            }
            if unit == "M" {
                return FormulaMore::numV(months as f64);
            }
            if unit == "D" {
                return FormulaMore::numV((d2.floor() - d.floor()).clone());
            }
            if unit == "YM" {
                return FormulaMore::numV((months % 12) as f64);
            }
            if unit == "MD" {
                let mut md: i64 = e2[2] - ymd[2];
                if md < 0 {
                    let mut pm: i64 = e2[1] - 1;
                    let mut py: i64 = e2[0];
                    if pm < 1 {
                        pm = 12;
                        py -= 1;
                    }
                    md += DateSerial::daysInMonth(py, pm);
                }
                return FormulaMore::numV(md as f64);
            }
            if unit == "YD" {
                let mut anniv: f64 = DateSerial::fromYMD(e2[0], ymd[1], 1) + ((ymd[2] - 1) as f64);
                if anniv > d2.floor() {
                    anniv = DateSerial::fromYMD(e2[0] - 1, ymd[1], 1) + ((ymd[2] - 1) as f64);
                }
                return FormulaMore::numV(d2.floor() - anniv);
            }
            return FormulaMore::err("#NUM!");
        }
        if n == "DAYS360" {
            let european: bool = FormulaMore::has(args, 2) && args[2].borrow().single().borrow().isTrue();
            return FormulaMore::numV(FormulaMore::days360(&ymd, &e2, european) as f64);
        }
        // YEARFRAC(start, end, [basis])
        let basis: i64 = FormulaMore::int(args, 2, 0);
        let mut s0: f64 = d.floor();
        let mut s1: f64 = d2.floor();
        let mut a0: Vec<i64> = ymd.clone();
        let mut a1: Vec<i64> = e2.clone();
        if s1 < s0 {
            let t3: f64 = s0;
            s0 = s1;
            s1 = t3;
            let t4: Vec<i64> = a0.clone();
            a0 = a1.clone();
            a1 = t4.clone();
        }
        if basis == 0 {
            return FormulaMore::numV((FormulaMore::days360(&a0, &a1, false) as f64) / 360.0);
        }
        if basis == 4 {
            return FormulaMore::numV((FormulaMore::days360(&a0, &a1, true) as f64) / 360.0);
        }
        if basis == 2 {
            return FormulaMore::numV((s1 - s0) / 360.0);
        }
        if basis == 3 {
            return FormulaMore::numV((s1 - s0) / 365.0);
        }
        if basis == 1 {
            // Actual/actual: the average length of the years the span touches.
            let mut total: f64 = 0.0;
            let mut y: i64 = a0[0];
            while y <= a1[0] {
                if DateSerial::isLeap(y) {
                    total += 366.0;
                } else {
                    total += 365.0;
                }
                y += 1;
            }
            let years: f64 = ((a1[0] - a0[0]) + 1) as f64;
            if a0[0] != a1[0] {
                return FormulaMore::numV((s1 - s0) / (total / years));
            }
            return FormulaMore::numV((s1 - s0) / total);
        }
        FormulaMore::err("#NUM!")
    }
    pub fn days360(a: &Vec<i64>, b: &Vec<i64>, european: bool) -> i64 {
        let mut d1: i64 = a[2];
        let mut d2: i64 = b[2];
        if european {
            if d1 == 31 {
                d1 = 30;
            }
            if d2 == 31 {
                d2 = 30;
            }
        } else {
            let last_feb: bool = a[1] == 2 && d1 == DateSerial::daysInMonth(a[0], 2);
            if last_feb {
                d1 = 30;
            }
            if d1 == 31 {
                d1 = 30;
            }
            if d2 == 31 && d1 >= 30 {
                d2 = 30;
            }
        }
        (b[0] - a[0]) * 360 + (b[1] - a[1]) * 30 + (d2 - d1)
    }
    pub fn isoWeek(d: f64) -> Rc<RefCell<FormulaValue>> {
        let s: f64 = d.floor();
        // Thursday of this week decides the year.
        let mut wd: i64 = DateSerial::weekday(s) - 1;
        if wd == 0 {
            wd = 7;
        }
        let thu: f64 = s + ((4 - wd) as f64);
        let ty: Vec<i64> = DateSerial::toYMD(thu);
        let jan1: f64 = DateSerial::fromYMD(ty[0], 1, 1);
        FormulaMore::numV((((thu - jan1) as i64) / 7 + 1) as f64)
    }

    // --- text and information ----------------------------------------------------------

    pub fn nthIndex(hay: &str, needle: &str, nth: i64, ci: bool) -> i64 {
        let mut h: String = hay.to_string();
        let mut nd: String = needle.to_string();
        if ci {
            h = FormulaFunctions::upper(hay);
            nd = FormulaFunctions::upper(needle);
        }
        let nl: i64 = DgNative::str_len(&nd);
        if nth > 0 {
            let mut at: i64 = -1;
            let mut k: i64 = 0;
            let mut from: i64 = 0;
            while k < nth {
                at = DgNative::index_of_from(&h, &nd, from);
                if at < 0 {
                    return -1;
                }
                from = at + nl;
                k += 1;
            }
            return at;
        }
        // From the end.
        let mut positions: Vec<i64> = Vec::new();
        let mut from2: i64 = 0;
        loop {
            let at2: i64 = DgNative::index_of_from(&h, &nd, from2);
            if at2 < 0 {
                break;
            }
            positions.push(at2);
            from2 = at2 + nl;
        }
        let want: i64 = (positions.len() as i64) + nth;
        if want < 0 {
            return -1;
        }
        positions[want as usize]
    }
    pub fn groupThousands(digits: &str) -> String {
        let len: i64 = DgNative::str_len(digits);
        let mut out: String = String::new();
        let mut i: i64 = 0;
        while i < len {
            if i > 0 && (len - i) % 3 == 0 {
                out = format!("{},", out);
            }
            out = format!("{}{}", out, DgNative::substr(digits, i, i + 1));
            i += 1;
        }
        out
    }
    pub fn fixedText(x: f64, dec: i64, commas: bool) -> String {
        let r: f64 = FormulaMore::roundD(x, dec);
        let a: f64 = FormulaMore::absD(r);
        let whole: f64 = a.floor();
        let mut ws: String = format!("{}", whole as i64);
        if commas {
            ws = FormulaMore::groupThousands(&ws);
        }
        let mut out: String = ws.clone();
        if dec > 0 {
            let p: f64 = FormulaFunctions::powTen(dec);
            let frac: i64 = ((a - whole) * p + 0.5).floor() as i64;
            let mut fs: String = format!("{}", frac);
            while DgNative::str_len(&fs) < dec {
                fs = format!("0{}", fs);
            }
            out = format!("{}.{}", out, fs);
        }
        if r < 0.0 {
            return format!("-{}", out);
        }
        out
    }
    pub fn textFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if n == "ISLOGICAL" || n == "ISNONTEXT" || n == "ISNA" || n == "TYPE" || n == "ERROR.TYPE" {
            if (args.len() as i64) < 1 {
                return FormulaMore::err("#VALUE!");
            }
            let v: Rc<RefCell<FormulaValue>> = args[0].clone();
            let k: i64 = v.borrow().kind;
            if n == "ISLOGICAL" {
                return FormulaValue::fromBool(k == 3);
            }
            if n == "ISNONTEXT" {
                return FormulaValue::fromBool(k != 2);
            }
            if n == "ISNA" {
                return FormulaValue::fromBool(k == 4 && v.borrow().err == "#N/A");
            }
            if n == "TYPE" {
                if k == 5 {
                    return FormulaMore::numV(64.0);
                }
                if k == 4 {
                    return FormulaMore::numV(16.0);
                }
                if k == 3 {
                    return FormulaMore::numV(4.0);
                }
                if k == 2 {
                    return FormulaMore::numV(2.0);
                }
                return FormulaMore::numV(1.0);
            }
            if k != 4 {
                return FormulaMore::err("#N/A");
            }
            let codes: Vec<String> = vec!["#NULL!".to_string(), "#DIV/0!".to_string(), "#VALUE!".to_string(), "#REF!".to_string(), "#NAME?".to_string(), "#NUM!".to_string(), "#N/A".to_string(), "#GETTING_DATA".to_string(), "#SPILL!".to_string(), "#CALC!".to_string()];
            let mut i: i64 = 0;
            while i < (codes.len() as i64) {
                if codes[i as usize] == v.borrow().err {
                    return FormulaMore::numV((i + 1) as f64);
                }
                i += 1;
            }
            return FormulaMore::err("#N/A");
        }
        let all: String = " TEXTBEFORE TEXTAFTER TEXTSPLIT VALUETOTEXT NUMBERVALUE FIXED DOLLAR CLEAN UNICHAR UNICODE ADDRESS HYPERLINK ".to_string();
        if DgNative::index_of(&all, &format!(" {} ", n)) < 0 {
            return FormulaValue::blank();
        }
        let e: Rc<RefCell<FormulaValue>> = FormulaMore::firstErr(args, 1);
        if e.borrow().isError() {
            return e;
        }
        let s: String = FormulaMore::txt(args, 0);
        if n == "HYPERLINK" {
            if FormulaMore::has(args, 1) {
                return FormulaValue::fromString(FormulaMore::txt(args, 1));
            }
            return FormulaValue::fromString(s);
        }
        if n == "TEXTBEFORE" || n == "TEXTAFTER" {
            let delim: String = FormulaMore::txt(args, 1);
            let nth: i64 = FormulaMore::int(args, 2, 1);
            let ci: bool = FormulaMore::num(args, 3, 0.0) != 0.0;
            if nth == 0 {
                return FormulaMore::err("#VALUE!");
            }
            let at: i64 = FormulaMore::nthIndex(&s, &delim, nth, ci);
            if at < 0 {
                if FormulaMore::has(args, 5) {
                    return args[5].clone();
                }
                return FormulaMore::err("#N/A");
            }
            if n == "TEXTBEFORE" {
                return FormulaValue::fromString(DgNative::substr(&s, 0, at));
            }
            return FormulaValue::fromString(DgNative::substr(&s, at + DgNative::str_len(&delim), DgNative::str_len(&s)));
        }
        if n == "TEXTSPLIT" {
            let cd: String = FormulaMore::txt(args, 1);
            let rd: String = FormulaMore::txt(args, 2);
            let ignore_empty: bool = FormulaMore::num(args, 3, 0.0) != 0.0;
            let mut rows: Vec<String> = Vec::new();
            if DgNative::str_len(&rd) > 0 {
                rows = FormulaMore::splitOn(&s, &rd, ignore_empty);
            } else {
                rows.push(s.clone());
            }
            let mut cells: Vec<Vec<String>> = Vec::new();
            let mut width: i64 = 1;
            for r in rows.iter() {
                let mut parts: Vec<String> = Vec::new();
                if DgNative::str_len(&cd) > 0 {
                    parts = FormulaMore::splitOn(r, &cd, ignore_empty);
                } else {
                    parts.push(r.clone());
                }
                if (parts.len() as i64) > width {
                    width = parts.len() as i64;
                }
                cells.push(parts.clone());
            }
            let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            for row in cells.iter() {
                let mut c: i64 = 0;
                while c < width {
                    if c < (row.len() as i64) {
                        items.push(FormulaValue::fromString(row[c as usize].clone()));
                    } else {
                        items.push(FormulaMore::err("#N/A"));
                    }
                    c += 1;
                }
            }
            return FormulaValue::fromArray(cells.len() as i64, width, &items);
        }
        if n == "VALUETOTEXT" {
            let strict: bool = FormulaMore::num(args, 1, 0.0) != 0.0;
            let v2: Rc<RefCell<FormulaValue>> = args[0].borrow().single();
            if strict && v2.borrow().kind == 2 {
                return FormulaValue::fromString(format!("\"{}\"", s));
            }
            return FormulaValue::fromString(s);
        }
        if n == "NUMBERVALUE" {
            let mut dec: String = ".".to_string();
            let mut grp: String = ",".to_string();
            if FormulaMore::has(args, 1) {
                dec = DgNative::substr(&FormulaMore::txt(args, 1), 0, 1);
            }
            if FormulaMore::has(args, 2) {
                grp = DgNative::substr(&FormulaMore::txt(args, 2), 0, 1);
            }
            let mut clean: String = String::new();
            let mut pct: i64 = 0;
            let mut i2: i64 = 0;
            while i2 < DgNative::str_len(&s) {
                let ch: String = DgNative::substr(&s, i2, i2 + 1);
                if ch == dec {
                    clean = format!("{}.", clean);
                } else {
                    if ch == "%" {
                        pct += 1;
                    } else {
                        if ch != grp && ch != " " {
                            clean = format!("{}{}", clean, ch);
                        }
                    }
                }
                i2 += 1;
            }
            if DgNative::str_len(&clean) == 0 {
                return FormulaMore::numV(0.0);
            }
            let parsed: Rc<RefCell<FormulaValue>> = FormulaValue::fromString(clean.clone());
            let num: Rc<RefCell<FormulaValue>> = parsed.borrow().coerceNumber();
            if num.borrow().isError() {
                return FormulaMore::err("#VALUE!");
            }
            let mut out: f64 = num.borrow().num;
            while pct > 0 {
                out /= 100.0;
                pct -= 1;
            }
            return FormulaMore::numV(out);
        }
        if n == "FIXED" || n == "DOLLAR" {
            let x: f64 = FormulaMore::num(args, 0, 0.0);
            let dec2: i64 = FormulaMore::int(args, 1, 2);
            let mut d: i64 = dec2;
            if d < 0 {
                d = 0;
            }
            if n == "DOLLAR" {
                let body: String = FormulaMore::fixedText(FormulaMore::absD(FormulaMore::roundD(x, dec2)), d, true);
                if x < 0.0 {
                    return FormulaValue::fromString(format!("(${})", body));
                }
                return FormulaValue::fromString(format!("${}", body));
            }
            let no_commas: bool = FormulaMore::num(args, 2, 0.0) != 0.0;
            return FormulaValue::fromString(FormulaMore::fixedText(FormulaMore::roundD(x, dec2), d, !no_commas));
        }
        if n == "CLEAN" {
            let mut out2: String = String::new();
            let mut i3: i64 = 0;
            while i3 < DgNative::str_len(&s) {
                let code: i64 = DgNative::char_at(&s, i3);
                if code >= 32 {
                    out2 = format!("{}{}", out2, DgNative::substr(&s, i3, i3 + 1));
                }
                i3 += 1;
            }
            return FormulaValue::fromString(out2);
        }
        if n == "UNICHAR" {
            let code2: i64 = FormulaMore::int(args, 0, 0);
            if code2 < 1 {
                return FormulaMore::err("#VALUE!");
            }
            return FormulaValue::fromString(DgNative::from_code(code2));
        }
        if n == "UNICODE" {
            if DgNative::str_len(&s) == 0 {
                return FormulaMore::err("#VALUE!");
            }
            return FormulaMore::numV(DgNative::char_at(&s, 0) as f64);
        }
        // ADDRESS(row, col, [abs], [a1], [sheet])
        let row: i64 = FormulaMore::int(args, 0, 0);
        let col: i64 = FormulaMore::int(args, 1, 0);
        if row < 1 || col < 1 {
            return FormulaMore::err("#VALUE!");
        }
        let absn: i64 = FormulaMore::int(args, 2, 1);
        let a1: bool = !FormulaMore::has(args, 3) || args[3].borrow().single().borrow().isTrue();
        let mut out3: String = String::new();
        if a1 {
            let mut letters: String = String::new();
            let mut c2: i64 = col;
            while c2 > 0 {
                let rem: i64 = (c2 - 1) % 26;
                letters = format!("{}{}", DgNative::from_code(65 + rem), letters);
                c2 = (c2 - 1) / 26;
            }
            let mut cp: String = String::new();
            let mut rp: String = String::new();
            if absn == 1 || absn == 3 {
                cp = "$".to_string();
            }
            if absn == 1 || absn == 2 {
                rp = "$".to_string();
            }
            out3 = format!("{}{}{}{}", cp, letters, rp, row);
        } else {
            let mut rs: String = format!("R{}", row);
            let mut cs: String = format!("C{}", col);
            if absn == 3 || absn == 4 {
                rs = format!("R[{}]", row);
            }
            if absn == 2 || absn == 4 {
                cs = format!("C[{}]", col);
            }
            out3 = format!("{}{}", rs, cs);
        }
        if FormulaMore::has(args, 4) {
            let sheet: String = FormulaMore::txt(args, 4);
            out3 = format!("{}{}", crate::formula_engine::FormulaParser::formatSheetPrefix(&sheet), out3);
        }
        FormulaValue::fromString(out3)
    }
    pub fn splitOn(s: &str, d: &str, ignore_empty: bool) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let dl: i64 = DgNative::str_len(d);
        let mut from: i64 = 0;
        loop {
            let at: i64 = DgNative::index_of_from(s, d, from);
            let mut piece: String = String::new();
            if at < 0 {
                piece = DgNative::substr(s, from, DgNative::str_len(s));
            } else {
                piece = DgNative::substr(s, from, at);
            }
            if !(ignore_empty && DgNative::str_len(&piece) == 0) {
                out.push(piece.clone());
            }
            if at < 0 {
                break;
            }
            from = at + dl;
        }
        out
    }

    // --- arrays ----------------------------------------------------------------------------

    pub fn arrayFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let all: String = " XMATCH CHOOSECOLS CHOOSEROWS TAKE DROP VSTACK HSTACK SEQUENCE SORTBY TOCOL TOROW WRAPROWS WRAPCOLS EXPAND ".to_string();
        if DgNative::index_of(&all, &format!(" {} ", n)) < 0 {
            return FormulaValue::blank();
        }
        if (args.len() as i64) < 1 {
            return FormulaMore::err("#VALUE!");
        }
        if n == "SEQUENCE" {
            let rows: i64 = FormulaMore::int(args, 0, 1);
            let cols: i64 = FormulaMore::int(args, 1, 1);
            let start: f64 = FormulaMore::num(args, 2, 1.0);
            let step: f64 = FormulaMore::num(args, 3, 1.0);
            if rows < 1 || cols < 1 {
                return FormulaMore::err("#CALC!");
            }
            let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut i: i64 = 0;
            while i < rows * cols {
                items.push(FormulaMore::numV(start + (i as f64) * step));
                i += 1;
            }
            return FormulaValue::fromArray(rows, cols, &items);
        }
        if n == "VSTACK" || n == "HSTACK" {
            let mut rows2: i64 = 0;
            let mut cols2: i64 = 0;
            for a in args.iter() {
                let g: Rc<RefCell<FormulaValue>> = FormulaMore::grid(a);
                if n == "VSTACK" {
                    rows2 += g.borrow().rows;
                    if g.borrow().cols > cols2 {
                        cols2 = g.borrow().cols;
                    }
                } else {
                    cols2 += g.borrow().cols;
                    if g.borrow().rows > rows2 {
                        rows2 = g.borrow().rows;
                    }
                }
            }
            let mut items2: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut r: i64 = 0;
            while r < rows2 {
                let mut c: i64 = 0;
                while c < cols2 {
                    items2.push(FormulaMore::err("#N/A"));
                    c += 1;
                }
                r += 1;
            }
            let mut off: i64 = 0;
            for a in args.iter() {
                let g2: Rc<RefCell<FormulaValue>> = FormulaMore::grid(a);
                let gr: i64 = g2.borrow().rows;
                let gc: i64 = g2.borrow().cols;
                let mut r2: i64 = 0;
                while r2 < gr {
                    let mut c2: i64 = 0;
                    while c2 < gc {
                        let mut at: i64 = (off + r2) * cols2 + c2;
                        if n == "HSTACK" {
                            at = r2 * cols2 + off + c2;
                        }
                        items2[at as usize] = g2.borrow().at(r2, c2);
                        c2 += 1;
                    }
                    r2 += 1;
                }
                if n == "VSTACK" {
                    off += gr;
                } else {
                    off += gc;
                }
            }
            return FormulaValue::fromArray(rows2, cols2, &items2);
        }
        let e: Rc<RefCell<FormulaValue>> = args[0].clone();
        if n != "XMATCH" && e.borrow().isError() && !e.borrow().isArray() {
            return e;
        }
        if n == "XMATCH" {
            if (args.len() as i64) < 2 {
                return FormulaMore::err("#VALUE!");
            }
            let key: Rc<RefCell<FormulaValue>> = args[0].borrow().single();
            let list: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[1].clone());
            let mode: i64 = FormulaMore::int(args, 2, 0);
            let search: i64 = FormulaMore::int(args, 3, 1);
            let cnt: i64 = list.len() as i64;
            let mut best: i64 = -1;
            let mut i2: i64 = 0;
            while i2 < cnt {
                let mut idx: i64 = i2;
                if search < 0 {
                    idx = cnt - 1 - i2;
                }
                let v: Rc<RefCell<FormulaValue>> = list[idx as usize].clone();
                if mode == 2 {
                    let pat: String = FormulaFunctions::upper(&key.borrow().asDisplay());
                    if FormulaMore::wildMatch(&FormulaFunctions::upper(&v.borrow().asDisplay()), &pat) {
                        return FormulaMore::numV((idx + 1) as f64);
                    }
                } else {
                    let cmp: i64 = FormulaValue::excelCompare(&v, &key);
                    if cmp == 0 && !v.borrow().isBlank() {
                        return FormulaMore::numV((idx + 1) as f64);
                    }
                    if !v.borrow().isBlank() {
                        if mode == -1 && cmp < 0 {
                            if best < 0 || FormulaValue::excelCompare(&list[best as usize].clone(), &v) < 0 {
                                best = idx;
                            }
                        }
                        if mode == 1 && cmp > 0 {
                            if best < 0 || FormulaValue::excelCompare(&list[best as usize].clone(), &v) > 0 {
                                best = idx;
                            }
                        }
                    }
                }
                i2 += 1;
            }
            if best >= 0 {
                return FormulaMore::numV((best + 1) as f64);
            }
            return FormulaMore::err("#N/A");
        }
        let g3: Rc<RefCell<FormulaValue>> = FormulaMore::grid(&args[0].clone());
        let rows3: i64 = g3.borrow().rows;
        let cols3: i64 = g3.borrow().cols;
        if n == "TOCOL" || n == "TOROW" {
            let ignore: i64 = FormulaMore::int(args, 1, 0);
            let by_col: bool = FormulaMore::num(args, 2, 0.0) != 0.0;
            let mut items3: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut outer: i64 = 0;
            let mut outer_n: i64 = rows3;
            let mut inner_n: i64 = cols3;
            if by_col {
                outer_n = cols3;
                inner_n = rows3;
            }
            while outer < outer_n {
                let mut inner: i64 = 0;
                while inner < inner_n {
                    let mut v2: Rc<RefCell<FormulaValue>> = g3.borrow().at(outer, inner);
                    if by_col {
                        v2 = g3.borrow().at(inner, outer);
                    }
                    let skip: bool = ((ignore == 1 || ignore == 3) && v2.borrow().isBlank()) || ((ignore == 2 || ignore == 3) && v2.borrow().kind == 4);
                    if !skip {
                        items3.push(v2.clone());
                    }
                    inner += 1;
                }
                outer += 1;
            }
            let k: i64 = items3.len() as i64;
            if k == 0 {
                return FormulaMore::err("#CALC!");
            }
            if n == "TOCOL" {
                return FormulaValue::fromArray(k, 1, &items3);
            }
            return FormulaValue::fromArray(1, k, &items3);
        }
        if n == "WRAPROWS" || n == "WRAPCOLS" {
            let flat: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[0].clone());
            let w: i64 = FormulaMore::int(args, 1, 0);
            if w < 1 {
                return FormulaMore::err("#NUM!");
            }
            let total: i64 = flat.len() as i64;
            let other: i64 = (total + w - 1) / w;
            let mut items4: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut pad: Rc<RefCell<FormulaValue>> = FormulaMore::err("#N/A");
            if FormulaMore::has(args, 2) {
                pad = args[2].borrow().single();
            }
            let mut r3: i64 = 0;
            let mut rcount: i64 = other;
            let mut ccount: i64 = w;
            if n == "WRAPCOLS" {
                rcount = w;
                ccount = other;
            }
            while r3 < rcount {
                let mut c3: i64 = 0;
                while c3 < ccount {
                    let mut idx2: i64 = r3 * w + c3;
                    if n == "WRAPCOLS" {
                        idx2 = c3 * w + r3;
                    }
                    if idx2 < total {
                        items4.push(flat[idx2 as usize].clone());
                    } else {
                        items4.push(pad.clone());
                    }
                    c3 += 1;
                }
                r3 += 1;
            }
            return FormulaValue::fromArray(rcount, ccount, &items4);
        }
        if n == "CHOOSECOLS" || n == "CHOOSEROWS" {
            let mut picks: Vec<i64> = Vec::new();
            let limit: i64 = if n == "CHOOSECOLS" { cols3 } else { rows3 };
            let mut a: i64 = 1;
            while a < (args.len() as i64) {
                for x in FormulaMore::numbersOf(&args[a as usize].clone()).iter() {
                    let mut k2: i64 = x.floor() as i64;
                    if k2 < 0 {
                        k2 = limit + k2 + 1;
                    }
                    if k2 < 1 || k2 > limit {
                        return FormulaMore::err("#VALUE!");
                    }
                    picks.push(k2 - 1);
                }
                a += 1;
            }
            if picks.is_empty() {
                return FormulaMore::err("#VALUE!");
            }
            let mut items5: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            if n == "CHOOSECOLS" {
                let mut r4: i64 = 0;
                while r4 < rows3 {
                    for p in picks.iter() {
                        items5.push(g3.borrow().at(r4, *p));
                    }
                    r4 += 1;
                }
                return FormulaValue::fromArray(rows3, picks.len() as i64, &items5);
            }
            for p in picks.iter() {
                let mut c4: i64 = 0;
                while c4 < cols3 {
                    items5.push(g3.borrow().at(*p, c4));
                    c4 += 1;
                }
            }
            return FormulaValue::fromArray(picks.len() as i64, cols3, &items5);
        }
        if n == "TAKE" || n == "DROP" || n == "EXPAND" {
            let mut r0: i64 = 0;
            let mut r1: i64 = rows3;
            let mut c0: i64 = 0;
            let mut c1: i64 = cols3;
            let rn: i64 = FormulaMore::int(args, 1, if n == "DROP" { 0 } else { rows3 });
            let cn: i64 = FormulaMore::int(args, 2, if n == "DROP" { 0 } else { cols3 });
            if n == "EXPAND" {
                if rn < rows3 || cn < cols3 {
                    return FormulaMore::err("#VALUE!");
                }
                let mut pad2: Rc<RefCell<FormulaValue>> = FormulaMore::err("#N/A");
                if FormulaMore::has(args, 3) {
                    pad2 = args[3].borrow().single();
                }
                let mut items6: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
                let mut r5: i64 = 0;
                while r5 < rn {
                    let mut c5: i64 = 0;
                    while c5 < cn {
                        if r5 < rows3 && c5 < cols3 {
                            items6.push(g3.borrow().at(r5, c5));
                        } else {
                            items6.push(pad2.clone());
                        }
                        c5 += 1;
                    }
                    r5 += 1;
                }
                return FormulaValue::fromArray(rn, cn, &items6);
            }
            if n == "TAKE" {
                if rn >= 0 {
                    if rn < rows3 {
                        r1 = rn;
                    }
                } else {
                    if 0 - rn < rows3 {
                        r0 = rows3 + rn;
                    }
                }
                if cn >= 0 {
                    if cn < cols3 {
                        c1 = cn;
                    }
                } else {
                    if 0 - cn < cols3 {
                        c0 = cols3 + cn;
                    }
                }
            } else {
                if rn >= 0 {
                    r0 = rn;
                } else {
                    r1 = rows3 + rn;
                }
                if cn >= 0 {
                    c0 = cn;
                } else {
                    c1 = cols3 + cn;
                }
            }
            if r1 <= r0 || c1 <= c0 {
                return FormulaMore::err("#CALC!");
            }
            let mut items7: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut r6: i64 = r0;
            while r6 < r1 {
                let mut c6: i64 = c0;
                while c6 < c1 {
                    items7.push(g3.borrow().at(r6, c6));
                    c6 += 1;
                }
                r6 += 1;
            }
            return FormulaValue::fromArray(r1 - r0, c1 - c0, &items7);
        }
        // SORTBY(array, by1, [order1], by2, [order2] …): rows, stable.
        let mut keys: Vec<Vec<Rc<RefCell<FormulaValue>>>> = Vec::new();
        let mut orders: Vec<i64> = Vec::new();
        let mut a2: i64 = 1;
        while a2 < (args.len() as i64) {
            let by: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[a2 as usize].clone());
            if (by.len() as i64) != rows3 {
                return FormulaMore::err("#VALUE!");
            }
            keys.push(by.clone());
            let mut ord: i64 = 1;
            if a2 + 1 < (args.len() as i64) {
                ord = FormulaMore::int(args, a2 + 1, 1);
            }
            orders.push(ord);
            a2 += 2;
        }
        let mut order: Vec<i64> = Vec::new();
        let mut i3: i64 = 0;
        while i3 < rows3 {
            order.push(i3);
            i3 += 1;
        }
        let mut i4: i64 = 1;
        while i4 < rows3 {
            let cur: i64 = order[i4 as usize];
            let mut j: i64 = i4 - 1;
            while j >= 0 {
                let other2: i64 = order[j as usize];
                let mut cmp2: i64 = 0;
                let mut k3: i64 = 0;
                while k3 < (keys.len() as i64) && cmp2 == 0 {
                    let col: Vec<Rc<RefCell<FormulaValue>>> = keys[k3 as usize].clone();
                    cmp2 = col[other2 as usize].borrow().compare(&col[cur as usize].clone());
                    if orders[k3 as usize] < 0 {
                        cmp2 = 0 - cmp2;
                    }
                    k3 += 1;
                }
                if cmp2 > 0 {
                    order[(j + 1) as usize] = other2;
                    j -= 1;
                } else {
                    break;
                }
            }
            order[(j + 1) as usize] = cur;
            i4 += 1;
        }
        let mut items8: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        for r7 in order.iter() {
            let mut c7: i64 = 0;
            while c7 < cols3 {
                items8.push(g3.borrow().at(*r7, c7));
                c7 += 1;
            }
        }
        FormulaValue::fromArray(rows3, cols3, &items8)
    }
    /// `*` any run, `?` one character, `~` escapes: both sides upper-cased.
    pub fn wildMatch(s: &str, p: &str) -> bool {
        let sl: i64 = DgNative::str_len(s);
        let pl: i64 = DgNative::str_len(p);
        let mut si: i64 = 0;
        let mut pi: i64 = 0;
        let mut star: i64 = -1;
        let mut mark: i64 = 0;
        while si < sl {
            if pi < pl && DgNative::char_at(p, pi) == 126 && pi + 1 < pl && DgNative::char_at(p, pi + 1) == DgNative::char_at(s, si) {
                pi += 2;
                si += 1;
            } else {
                if pi < pl && (DgNative::char_at(p, pi) == 63 || (DgNative::char_at(p, pi) == DgNative::char_at(s, si) && DgNative::char_at(p, pi) != 42)) {
                    pi += 1;
                    si += 1;
                } else {
                    if pi < pl && DgNative::char_at(p, pi) == 42 {
                        star = pi;
                        mark = si;
                        pi += 1;
                    } else {
                        if star >= 0 {
                            pi = star + 1;
                            mark += 1;
                            si = mark;
                        } else {
                            return false;
                        }
                    }
                }
            }
        }
        while pi < pl && DgNative::char_at(p, pi) == 42 {
            pi += 1;
        }
        pi == pl
    }

    // --- finance --------------------------------------------------------------------------

    pub fn pmtOf(r: f64, nper: f64, pv: f64, fv: f64, t: f64) -> f64 {
        if r == 0.0 {
            return (0.0 - (pv + fv)) / nper;
        }
        let g: f64 = FormulaMore::powD(1.0 + r, nper);
        (0.0 - (pv * g + fv)) / (((1.0 + r * t) * (g - 1.0)) / r)
    }
    pub fn fvOf(r: f64, nper: f64, pmt: f64, pv: f64, t: f64) -> f64 {
        if r == 0.0 {
            return 0.0 - (pv + pmt * nper);
        }
        let g: f64 = FormulaMore::powD(1.0 + r, nper);
        0.0 - (pv * g + (pmt * (1.0 + r * t) * (g - 1.0)) / r)
    }
    pub fn ipmtOf(r: f64, per: f64, nper: f64, pv: f64, fv: f64, t: f64) -> f64 {
        let pmt: f64 = FormulaMore::pmtOf(r, nper, pv, fv, t);
        if per == 1.0 && t == 1.0 {
            return 0.0;
        }
        let mut before: f64 = FormulaMore::fvOf(r, per - 1.0, pmt, pv, t);
        if t == 1.0 {
            before = FormulaMore::fvOf(r, per - 2.0, pmt, pv, t) - pmt;
        }
        before * r
    }
    pub fn npvAt(r: f64, flows: &Vec<f64>) -> f64 {
        let mut s: f64 = 0.0;
        let mut i: i64 = 0;
        while i < (flows.len() as i64) {
            s += flows[i as usize] / FormulaMore::powD(1.0 + r, (i + 1) as f64);
            i += 1;
        }
        s
    }
    pub fn xnpvAt(r: f64, flows: &Vec<f64>, dates: &Vec<f64>) -> f64 {
        let mut s: f64 = 0.0;
        let mut i: i64 = 0;
        while i < (flows.len() as i64) {
            s += flows[i as usize] / FormulaMore::powD(1.0 + r, (dates[i as usize] - dates[0]) / 365.0);
            i += 1;
        }
        s
    }
    pub fn financeFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let all: String = " IPMT PPMT CUMIPMT CUMPRINC RATE IRR XNPV XIRR MIRR DB DDB SYD EFFECT NOMINAL PDURATION RRI ".to_string();
        if DgNative::index_of(&all, &format!(" {} ", n)) < 0 {
            return FormulaMore::err("#NAME?");
        }
        let e: Rc<RefCell<FormulaValue>> = FormulaMore::firstErr(args, 1);
        if e.borrow().isError() {
            return e;
        }
        if n == "IRR" || n == "MIRR" {
            let mut list: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            list.push(args[0].clone());
            let flows: Vec<f64> = FormulaFunctions::collectNumbers(&list);
            if flows.len() < 2 {
                return FormulaMore::err("#NUM!");
            }
            if n == "MIRR" {
                let fr: f64 = FormulaMore::num(args, 1, 0.0);
                let rr: f64 = FormulaMore::num(args, 2, 0.0);
                let cnt: i64 = flows.len() as i64;
                let mut pos: f64 = 0.0;
                let mut neg: f64 = 0.0;
                let mut i: i64 = 0;
                while i < cnt {
                    let f: f64 = flows[i as usize];
                    if f > 0.0 {
                        pos += f * FormulaMore::powD(1.0 + rr, ((cnt - 1) - i) as f64);
                    } else {
                        neg += f / FormulaMore::powD(1.0 + fr, i as f64);
                    }
                    i += 1;
                }
                if neg == 0.0 || pos == 0.0 {
                    return FormulaMore::err("#DIV/0!");
                }
                return FormulaMore::numV(FormulaMore::powD((0.0 - pos) / neg, 1.0 / ((cnt - 1) as f64)) - 1.0);
            }
            // NPV of the flows from period 0.
            let mut r: f64 = FormulaMore::num(args, 1, 0.1);
            let mut it: i64 = 0;
            while it < 100 {
                let mut f0: f64 = 0.0;
                let mut f1: f64 = 0.0;
                let mut i2: i64 = 0;
                while i2 < (flows.len() as i64) {
                    let g: f64 = FormulaMore::powD(1.0 + r, i2 as f64);
                    f0 += flows[i2 as usize] / g;
                    f1 -= ((i2 as f64) * flows[i2 as usize]) / (g * (1.0 + r));
                    i2 += 1;
                }
                if f1 == 0.0 {
                    return FormulaMore::err("#NUM!");
                }
                let nr: f64 = r - f0 / f1;
                if FormulaMore::absD(nr - r) < 0.0000001 {
                    return FormulaMore::numV(nr);
                }
                r = nr;
                if r <= -1.0 {
                    r = -0.99;
                }
                it += 1;
            }
            return FormulaMore::err("#NUM!");
        }
        if n == "XNPV" || n == "XIRR" {
            let mut fi: i64 = 0;
            let mut di: i64 = 1;
            if n == "XNPV" {
                fi = 1;
                di = 2;
            }
            if (args.len() as i64) <= di {
                return FormulaMore::err("#VALUE!");
            }
            let flows2: Vec<f64> = FormulaMore::numbersOf(&args[fi as usize].clone());
            let dates: Vec<f64> = FormulaMore::numbersOf(&args[di as usize].clone());
            if flows2.len() != dates.len() || flows2.is_empty() {
                return FormulaMore::err("#NUM!");
            }
            if n == "XNPV" {
                let rate: f64 = FormulaMore::num(args, 0, 0.0);
                return FormulaMore::numV(FormulaMore::xnpvAt(rate, &flows2, &dates));
            }
            let mut r2: f64 = FormulaMore::num(args, 2, 0.1);
            let mut it2: i64 = 0;
            while it2 < 100 {
                let f0b: f64 = FormulaMore::xnpvAt(r2, &flows2, &dates);
                let h: f64 = 0.000001;
                let f1b: f64 = (FormulaMore::xnpvAt(r2 + h, &flows2, &dates) - f0b) / h;
                if f1b == 0.0 {
                    return FormulaMore::err("#NUM!");
                }
                let nr2: f64 = r2 - f0b / f1b;
                if FormulaMore::absD(nr2 - r2) < 0.0000001 {
                    return FormulaMore::numV(nr2);
                }
                r2 = nr2;
                if r2 <= -1.0 {
                    r2 = -0.99;
                }
                it2 += 1;
            }
            return FormulaMore::err("#NUM!");
        }
        if n == "EFFECT" {
            let nom: f64 = FormulaMore::num(args, 0, 0.0);
            let np: f64 = FormulaMore::num(args, 1, 0.0).floor();
            if nom <= 0.0 || np < 1.0 {
                return FormulaMore::err("#NUM!");
            }
            return FormulaMore::numV(FormulaMore::powD(1.0 + nom / np, np) - 1.0);
        }
        if n == "NOMINAL" {
            let eff: f64 = FormulaMore::num(args, 0, 0.0);
            let np2: f64 = FormulaMore::num(args, 1, 0.0).floor();
            if eff <= 0.0 || np2 < 1.0 {
                return FormulaMore::err("#NUM!");
            }
            return FormulaMore::numV(np2 * (FormulaMore::powD(1.0 + eff, 1.0 / np2) - 1.0));
        }
        if n == "PDURATION" {
            let rate2: f64 = FormulaMore::num(args, 0, 0.0);
            let pv: f64 = FormulaMore::num(args, 1, 0.0);
            let fv: f64 = FormulaMore::num(args, 2, 0.0);
            if rate2 <= 0.0 || pv <= 0.0 || fv <= 0.0 {
                return FormulaMore::err("#NUM!");
            }
            return FormulaMore::numV((FormulaMore::lnD(fv) - FormulaMore::lnD(pv)) / FormulaMore::lnD(1.0 + rate2));
        }
        if n == "RRI" {
            let np3: f64 = FormulaMore::num(args, 0, 0.0);
            let pv2: f64 = FormulaMore::num(args, 1, 0.0);
            let fv2: f64 = FormulaMore::num(args, 2, 0.0);
            if np3 <= 0.0 || pv2 == 0.0 {
                return FormulaMore::err("#NUM!");
            }
            return FormulaMore::numV(FormulaMore::powD(fv2 / pv2, 1.0 / np3) - 1.0);
        }
        if n == "SYD" {
            let cost: f64 = FormulaMore::num(args, 0, 0.0);
            let salv: f64 = FormulaMore::num(args, 1, 0.0);
            let life: f64 = FormulaMore::num(args, 2, 0.0);
            let per: f64 = FormulaMore::num(args, 3, 0.0);
            if life <= 0.0 || per < 1.0 || per > life {
                return FormulaMore::err("#NUM!");
            }
            return FormulaMore::numV(((cost - salv) * (life - per + 1.0) * 2.0) / (life * (life + 1.0)));
        }
        if n == "DDB" || n == "DB" {
            let cost2: f64 = FormulaMore::num(args, 0, 0.0);
            let salv2: f64 = FormulaMore::num(args, 1, 0.0);
            let life2: f64 = FormulaMore::num(args, 2, 0.0);
            let per2: i64 = FormulaMore::int(args, 3, 0);
            if life2 <= 0.0 || per2 < 1 || cost2 < 0.0 {
                return FormulaMore::err("#NUM!");
            }
            if n == "DDB" {
                let factor: f64 = FormulaMore::num(args, 4, 2.0);
                let mut book: f64 = cost2;
                let mut dep: f64 = 0.0;
                let mut p: i64 = 1;
                while p <= per2 {
                    dep = book * factor / life2;
                    if book - dep < salv2 {
                        dep = book - salv2;
                    }
                    if dep < 0.0 {
                        dep = 0.0;
                    }
                    book -= dep;
                    p += 1;
                }
                return FormulaMore::numV(dep);
            }
            let month: f64 = FormulaMore::num(args, 4, 12.0);
            if cost2 == 0.0 {
                return FormulaMore::numV(0.0);
            }
            let rate3: f64 = FormulaMore::roundD(1.0 - FormulaMore::powD(salv2 / cost2, 1.0 / life2), 3);
            let mut total: f64 = 0.0;
            let mut dep2: f64 = 0.0;
            let mut p2: i64 = 1;
            while p2 <= per2 {
                if p2 == 1 {
                    dep2 = cost2 * rate3 * month / 12.0;
                } else {
                    if (p2 as f64) == life2 + 1.0 {
                        dep2 = (cost2 - total) * rate3 * (12.0 - month) / 12.0;
                    } else {
                        dep2 = (cost2 - total) * rate3;
                    }
                }
                total += dep2;
                p2 += 1;
            }
            return FormulaMore::numV(dep2);
        }
        if n == "RATE" {
            let nper: f64 = FormulaMore::num(args, 0, 0.0);
            let pmt: f64 = FormulaMore::num(args, 1, 0.0);
            let pv3: f64 = FormulaMore::num(args, 2, 0.0);
            let fv3: f64 = FormulaMore::num(args, 3, 0.0);
            let t: f64 = FormulaMore::num(args, 4, 0.0);
            let mut r3: f64 = FormulaMore::num(args, 5, 0.1);
            let mut it3: i64 = 0;
            while it3 < 100 {
                let f: f64 = FormulaMore::fvOf(r3, nper, pmt, pv3, t) + fv3;
                let h2: f64 = 0.0000001;
                let fd: f64 = (FormulaMore::fvOf(r3 + h2, nper, pmt, pv3, t) + fv3 - f) / h2;
                if fd == 0.0 {
                    return FormulaMore::err("#NUM!");
                }
                let nr3: f64 = r3 - f / fd;
                if FormulaMore::absD(nr3 - r3) < 0.0000001 {
                    return FormulaMore::numV(nr3);
                }
                r3 = nr3;
                it3 += 1;
            }
            return FormulaMore::err("#NUM!");
        }
        // IPMT / PPMT (rate, per, nper, pv, [fv], [type]); CUMIPMT / CUMPRINC
        // (rate, nper, pv, start, end, type).
        let rate4: f64 = FormulaMore::num(args, 0, 0.0);
        if n == "CUMIPMT" || n == "CUMPRINC" {
            if (args.len() as i64) < 6 {
                return FormulaMore::err("#VALUE!");
            }
            let nper2: f64 = FormulaMore::num(args, 1, 0.0);
            let pv4: f64 = FormulaMore::num(args, 2, 0.0);
            let st: i64 = FormulaMore::int(args, 3, 0);
            let en: i64 = FormulaMore::int(args, 4, 0);
            let t2: f64 = FormulaMore::num(args, 5, 0.0);
            if st < 1 || en < st || rate4 <= 0.0 || pv4 <= 0.0 {
                return FormulaMore::err("#NUM!");
            }
            let pmt2: f64 = FormulaMore::pmtOf(rate4, nper2, pv4, 0.0, t2);
            let mut s: f64 = 0.0;
            let mut p3: i64 = st;
            while p3 <= en {
                let ip: f64 = FormulaMore::ipmtOf(rate4, p3 as f64, nper2, pv4, 0.0, t2);
                if n == "CUMIPMT" {
                    s += ip;
                } else {
                    s += pmt2 - ip;
                }
                p3 += 1;
            }
            return FormulaMore::numV(s);
        }
        if (args.len() as i64) < 4 {
            return FormulaMore::err("#VALUE!");
        }
        let per3: f64 = FormulaMore::num(args, 1, 0.0);
        let nper3: f64 = FormulaMore::num(args, 2, 0.0);
        let pv5: f64 = FormulaMore::num(args, 3, 0.0);
        let fv5: f64 = FormulaMore::num(args, 4, 0.0);
        let t3: f64 = FormulaMore::num(args, 5, 0.0);
        if per3 < 1.0 || per3 > nper3 {
            return FormulaMore::err("#NUM!");
        }
        let ip2: f64 = FormulaMore::ipmtOf(rate4, per3, nper3, pv5, fv5, t3);
        if n == "IPMT" {
            return FormulaMore::numV(ip2);
        }
        FormulaMore::numV(FormulaMore::pmtOf(rate4, nper3, pv5, fv5, t3) - ip2)
    }
}
