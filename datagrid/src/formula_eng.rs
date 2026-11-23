// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;
use crate::date_serial::DateSerial;
use crate::formula_value::FormulaValue;
use crate::formula_functions::FormulaFunctions;
use crate::formula_more::FormulaMore;
use crate::formula_stats::FormulaStats;

/// FormulaEng - the fourth part of the function library: complex numbers
/// (COMPLEX, IM*), unit conversion (CONVERT), Bessel functions and the bond
/// functions (coupon dates, PRICE, YIELD, DURATION, accrued interest).
/// FormulaMore falls through to `invoke` for the names in `names()`.
#[derive(Clone)]
pub struct FormulaEng {
}

impl FormulaEng {
    pub fn new() -> Self {
        Self {
        }
    }
    pub fn names() -> String {
        " COMPLEX IMABS IMAGINARY IMARGUMENT IMCONJUGATE IMCOS IMCOSH IMCOT IMCSC IMCSCH IMDIV IMEXP IMLN IMLOG10 IMLOG2 IMPOWER IMPRODUCT IMREAL IMSEC IMSECH IMSIN IMSINH IMSQRT IMSUB IMSUM IMTAN CONVERT BESSELI BESSELJ BESSELK BESSELY COUPDAYBS COUPDAYS COUPDAYSNC COUPNCD COUPNUM COUPPCD PRICE YIELD DURATION MDURATION ACCRINT ACCRINTM PRICEMAT YIELDMAT AMORLINC AMORDEGRC ".to_string()
    }
    pub fn isKnown(n: &str) -> bool {
        DgNative::index_of(&FormulaEng::names(), &format!(" {} ", n)) >= 0
    }
    pub fn err(e: &str) -> Rc<RefCell<FormulaValue>> {
        FormulaValue::fromError(e.to_string())
    }
    pub fn numV(x: f64) -> Rc<RefCell<FormulaValue>> {
        FormulaValue::fromNumber(x)
    }
    pub fn invoke(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if !FormulaEng::isKnown(n) {
            return FormulaValue::blank();
        }
        let e: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        if e.borrow().isError() {
            return e;
        }
        if args.is_empty() {
            return FormulaEng::err("#VALUE!");
        }
        if n == "CONVERT" {
            return FormulaEng::convert(args);
        }
        if DgNative::substr(n, 0, 6) == "BESSEL" {
            return FormulaEng::bessel(n, args);
        }
        if n == "COMPLEX" || DgNative::substr(n, 0, 2) == "IM" {
            return FormulaEng::complexFn(n, args);
        }
        FormulaEng::bondFn(n, args)
    }

    // --- complex numbers -------------------------------------------------------------

    /// A number as Excel writes it inside a complex number: 15 significant
    /// digits, exponent as E.
    pub fn num15(x: f64) -> String {
        if x == 0.0 {
            return "0".to_string();
        }
        let ax: f64 = FormulaStats::absD(x);
        let mut mag: i64 = 0;
        let mut t: f64 = ax;
        while t >= 10.0 {
            t /= 10.0;
            mag += 1;
        }
        while t < 1.0 {
            t *= 10.0;
            mag -= 1;
        }
        let digits: i64 = 14 - mag;
        let mut v: f64 = ax;
        if digits >= 0 && digits < 300 {
            let p: f64 = FormulaFunctions::powTen(digits);
            v = (ax * p + 0.5).floor() / p;
        } else if digits < 0 {
            let p: f64 = FormulaFunctions::powTen(0 - digits);
            v = (ax / p + 0.5).floor() * p;
        }
        if x < 0.0 {
            v = 0.0 - v;
        }
        let s: String = FormulaValue::fromNumber(v).borrow().asRaw();
        s.replace("e-", "E-").replace("e+", "E+")
    }
    pub fn formatComplex(re: f64, im: f64, suffix: &str) -> String {
        if im == 0.0 {
            return FormulaEng::num15(re);
        }
        let mut ims: String = FormulaEng::num15(im);
        if ims == "1" {
            ims = String::new();
        }
        if ims == "-1" {
            ims = "-".to_string();
        }
        if re == 0.0 {
            return format!("{}{}", ims, suffix);
        }
        let mut sign: String = String::new();
        if im > 0.0 {
            sign = "+".to_string();
        }
        format!("{}{}{}{}", FormulaEng::num15(re), sign, ims, suffix)
    }
    /// Parse "a+bi" (or j). Fills out[0] re, out[1] im, out[2] 1 for j;
    /// false when the text is not a complex number.
    pub fn parseComplex(v: &Rc<RefCell<FormulaValue>>, out: &mut Vec<f64>) -> bool {
        out.clear();
        let one: Rc<RefCell<FormulaValue>> = v.borrow().single();
        let k: i64 = one.borrow().kind;
        if k == 1 {
            out.push(one.borrow().num);
            out.push(0.0);
            out.push(0.0);
            return true;
        }
        if k == 0 {
            out.push(0.0);
            out.push(0.0);
            out.push(0.0);
            return true;
        }
        if k != 2 {
            return false;
        }
        let s: String = one.borrow().text.trim().to_string();
        let n: i64 = DgNative::str_len(&s);
        if n == 0 {
            out.push(0.0);
            out.push(0.0);
            out.push(0.0);
            return true;
        }
        let last: i64 = DgNative::char_at(&s, n - 1);
        if last != 105 && last != 106 {
            if let Some(x) = FormulaValue::parsePlainNumber(&s) {
                out.push(x);
                out.push(0.0);
                out.push(0.0);
                return true;
            }
            return false;
        }
        let jflag: f64 = if last == 106 { 1.0 } else { 0.0 };
        let body: String = DgNative::substr(&s, 0, n - 1);
        let bl: i64 = DgNative::str_len(&body);
        // the sign that starts the imaginary part: the last + or - that is
        // not at the start and not part of an exponent
        let mut split: i64 = -1;
        let mut i: i64 = bl - 1;
        while i > 0 {
            let c: i64 = DgNative::char_at(&body, i);
            if c == 43 || c == 45 {
                let prev: i64 = DgNative::char_at(&body, i - 1);
                if prev != 101 && prev != 69 {
                    split = i;
                    break;
                }
            }
            i -= 1;
        }
        let mut re_s: String = String::new();
        let mut im_s: String = body.clone();
        if split > 0 {
            re_s = DgNative::substr(&body, 0, split);
            im_s = DgNative::substr(&body, split, bl);
        }
        let mut re: f64 = 0.0;
        if !re_s.is_empty() {
            if let Some(x) = FormulaValue::parsePlainNumber(&re_s) {
                re = x;
            } else {
                return false;
            }
        }
        let mut im: f64 = 0.0;
        if im_s.is_empty() || im_s == "+" {
            im = 1.0;
        } else if im_s == "-" {
            im = -1.0;
        } else {
            let mut t: String = im_s.clone();
            if DgNative::char_at(&t, 0) == 43 {
                t = DgNative::substr(&t, 1, DgNative::str_len(&t));
            }
            if let Some(x) = FormulaValue::parsePlainNumber(&t) {
                im = x;
            } else {
                return false;
            }
        }
        out.push(re);
        out.push(im);
        out.push(jflag);
        true
    }
    pub fn atan2(y: f64, x: f64) -> f64 {
        DgNative::atan2(y, x)
    }
    pub fn cplx(re: f64, im: f64, j: bool) -> Rc<RefCell<FormulaValue>> {
        let suffix: &str = if j { "j" } else { "i" };
        FormulaValue::fromString(FormulaEng::formatComplex(re, im, suffix))
    }
    pub fn cdiv(a: &Vec<f64>, b: &Vec<f64>, out: &mut Vec<f64>) -> bool {
        let d: f64 = b[0] * b[0] + b[1] * b[1];
        if d == 0.0 {
            return false;
        }
        out.clear();
        out.push((a[0] * b[0] + a[1] * b[1]) / d);
        out.push((a[1] * b[0] - a[0] * b[1]) / d);
        true
    }
    pub fn csin(z: &Vec<f64>, out: &mut Vec<f64>) {
        out.clear();
        out.push(DgNative::sin(z[0]) * FormulaStats::cosh(z[1]));
        out.push(DgNative::cos(z[0]) * FormulaStats::sinh(z[1]));
    }
    pub fn ccos(z: &Vec<f64>, out: &mut Vec<f64>) {
        out.clear();
        out.push(DgNative::cos(z[0]) * FormulaStats::cosh(z[1]));
        out.push(0.0 - DgNative::sin(z[0]) * FormulaStats::sinh(z[1]));
    }
    pub fn complexFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if n == "COMPLEX" {
            let re: f64 = FormulaMore::num(args, 0, 0.0);
            let im: f64 = FormulaMore::num(args, 1, 0.0);
            let mut suffix: String = "i".to_string();
            if FormulaMore::has(args, 2) {
                suffix = FormulaMore::txt(args, 2);
            }
            if suffix != "i" && suffix != "j" {
                return FormulaEng::err("#VALUE!");
            }
            return FormulaValue::fromString(FormulaEng::formatComplex(re, im, &suffix));
        }
        if n == "IMSUM" || n == "IMPRODUCT" {
            let mut acc: Vec<f64> = Vec::new();
            acc.push(if n == "IMSUM" { 0.0 } else { 1.0 });
            acc.push(0.0);
            let mut use_j: bool = false;
            let mut z: Vec<f64> = Vec::new();
            for a in args.iter() {
                for x in FormulaMore::flatOf(a).iter() {
                    if x.borrow().isBlank() && a.borrow().isArray() {
                        continue;
                    }
                    if !FormulaEng::parseComplex(x, &mut z) {
                        return FormulaEng::err("#NUM!");
                    }
                    if z[2] > 0.5 {
                        use_j = true;
                    }
                    if n == "IMSUM" {
                        acc[0] = acc[0] + z[0];
                        acc[1] = acc[1] + z[1];
                    } else {
                        let r: f64 = acc[0] * z[0] - acc[1] * z[1];
                        let i: f64 = acc[0] * z[1] + acc[1] * z[0];
                        acc[0] = r;
                        acc[1] = i;
                    }
                }
            }
            return FormulaEng::cplx(acc[0], acc[1], use_j);
        }
        let mut z: Vec<f64> = Vec::new();
        if !FormulaEng::parseComplex(&args[0].clone(), &mut z) {
            return FormulaEng::err("#NUM!");
        }
        let j: bool = z[2] > 0.5;
        if n == "IMREAL" {
            return FormulaEng::numV(z[0]);
        }
        if n == "IMAGINARY" {
            return FormulaEng::numV(z[1]);
        }
        let r: f64 = FormulaStats::sqrtD(z[0] * z[0] + z[1] * z[1]);
        let th: f64 = FormulaEng::atan2(z[1], z[0]);
        if n == "IMABS" {
            return FormulaEng::numV(r);
        }
        if n == "IMARGUMENT" {
            if r == 0.0 {
                return FormulaEng::err("#DIV/0!");
            }
            return FormulaEng::numV(th);
        }
        if n == "IMCONJUGATE" {
            return FormulaEng::cplx(z[0], 0.0 - z[1], j);
        }
        if n == "IMSUB" || n == "IMDIV" {
            let mut w: Vec<f64> = Vec::new();
            if (args.len() as i64) < 2 || !FormulaEng::parseComplex(&args[1].clone(), &mut w) {
                return FormulaEng::err("#NUM!");
            }
            let wj: bool = j || w[2] > 0.5;
            if n == "IMSUB" {
                return FormulaEng::cplx(z[0] - w[0], z[1] - w[1], wj);
            }
            let mut q: Vec<f64> = Vec::new();
            if !FormulaEng::cdiv(&z, &w, &mut q) {
                return FormulaEng::err("#NUM!");
            }
            return FormulaEng::cplx(q[0], q[1], wj);
        }
        if n == "IMEXP" {
            let m: f64 = FormulaStats::exp(z[0]);
            return FormulaEng::cplx(m * DgNative::cos(z[1]), m * DgNative::sin(z[1]), j);
        }
        if n == "IMLN" || n == "IMLOG10" || n == "IMLOG2" {
            if r == 0.0 {
                return FormulaEng::err("#NUM!");
            }
            let mut lr: f64 = FormulaStats::ln(r);
            let mut li: f64 = th;
            if n == "IMLOG10" {
                lr /= 2.302585092994046;
                li /= 2.302585092994046;
            }
            if n == "IMLOG2" {
                lr /= 0.6931471805599453;
                li /= 0.6931471805599453;
            }
            return FormulaEng::cplx(lr, li, j);
        }
        if n == "IMPOWER" || n == "IMSQRT" {
            let mut p: f64 = 0.5;
            if n == "IMPOWER" {
                if (args.len() as i64) < 2 {
                    return FormulaEng::err("#VALUE!");
                }
                p = FormulaMore::num(args, 1, 0.0);
            }
            if r == 0.0 {
                if p <= 0.0 {
                    return FormulaEng::err("#NUM!");
                }
                return FormulaEng::cplx(0.0, 0.0, j);
            }
            let m: f64 = FormulaStats::pow(r, p);
            return FormulaEng::cplx(m * DgNative::cos(p * th), m * DgNative::sin(p * th), j);
        }
        let mut s: Vec<f64> = Vec::new();
        let mut c: Vec<f64> = Vec::new();
        FormulaEng::csin(&z, &mut s);
        FormulaEng::ccos(&z, &mut c);
        if n == "IMSIN" {
            return FormulaEng::cplx(s[0], s[1], j);
        }
        if n == "IMCOS" {
            return FormulaEng::cplx(c[0], c[1], j);
        }
        if n == "IMSINH" || n == "IMCOSH" || n == "IMCSCH" || n == "IMSECH" {
            // sinh(a+bi) = sinh a cos b + i cosh a sin b
            let sh: Vec<f64> = vec![FormulaStats::sinh(z[0]) * DgNative::cos(z[1]), FormulaStats::cosh(z[0]) * DgNative::sin(z[1])];
            let ch: Vec<f64> = vec![FormulaStats::cosh(z[0]) * DgNative::cos(z[1]), FormulaStats::sinh(z[0]) * DgNative::sin(z[1])];
            if n == "IMSINH" {
                return FormulaEng::cplx(sh[0], sh[1], j);
            }
            if n == "IMCOSH" {
                return FormulaEng::cplx(ch[0], ch[1], j);
            }
            let one: Vec<f64> = vec![1.0, 0.0];
            let mut q: Vec<f64> = Vec::new();
            let den: Vec<f64> = if n == "IMCSCH" { sh.clone() } else { ch.clone() };
            if !FormulaEng::cdiv(&one, &den, &mut q) {
                return FormulaEng::err("#NUM!");
            }
            return FormulaEng::cplx(q[0], q[1], j);
        }
        let one: Vec<f64> = vec![1.0, 0.0];
        let mut q: Vec<f64> = Vec::new();
        let mut ok: bool = true;
        if n == "IMTAN" {
            ok = FormulaEng::cdiv(&s, &c, &mut q);
        }
        if n == "IMCOT" {
            ok = FormulaEng::cdiv(&c, &s, &mut q);
        }
        if n == "IMSEC" {
            ok = FormulaEng::cdiv(&one, &c, &mut q);
        }
        if n == "IMCSC" {
            ok = FormulaEng::cdiv(&one, &s, &mut q);
        }
        if !ok || q.is_empty() {
            return FormulaEng::err("#NUM!");
        }
        FormulaEng::cplx(q[0], q[1], j)
    }

    // --- CONVERT ---------------------------------------------------------------------

    /// The unit table: "name|group|factor|prefix", factor to the group's base
    /// unit. prefix: 0 none, 1 metric, 2 metric and binary.
    pub fn units() -> Vec<String> {
        let ly: f64 = 9460730472580800.0;
        let pica: f64 = 0.0254 / 72.0;
        let mut u: Vec<String> = Vec::new();
        let rows: Vec<&str> = vec![
            "g|mass|1|1", "sg|mass|14593.9029372064|0", "lbm|mass|453.59237|0", "u|mass|1.66053906660e-24|1", "ozm|mass|28.349523125|0", "grain|mass|0.06479891|0",
            "cwt|mass|45359.237|0", "shweight|mass|45359.237|0", "uk_cwt|mass|50802.34544|0", "lcwt|mass|50802.34544|0", "hweight|mass|50802.34544|0", "stone|mass|6350.29318|0",
            "ton|mass|907184.74|0", "uk_ton|mass|1016046.9088|0", "LTON|mass|1016046.9088|0", "brton|mass|1016046.9088|0",
            "m|dist|1|1", "mi|dist|1609.344|0", "Nmi|dist|1852|0", "in|dist|0.0254|0", "ft|dist|0.3048|0", "yd|dist|0.9144|0", "ang|dist|1e-10|1", "ell|dist|1.143|0",
            "parsec|dist|30856775812815500|1", "pc|dist|30856775812815500|1", "pica|dist|0.00423333333333333|0", "survey_mi|dist|1609.34721869444|0",
            "yr|time|31557600|0", "day|time|86400|0", "d|time|86400|0", "hr|time|3600|0", "mn|time|60|0", "min|time|60|0", "sec|time|1|1", "s|time|1|1",
            "Pa|press|1|1", "p|press|1|1", "atm|press|101325|1", "at|press|101325|1", "mmHg|press|133.322387415|1", "psi|press|6894.75729316836|0", "Torr|press|133.322368421053|0",
            "N|force|1|1", "dyn|force|1e-5|1", "dy|force|1e-5|1", "lbf|force|4.4482216152605|0", "pond|force|0.00980665|1",
            "J|energy|1|1", "e|energy|1e-7|1", "c|energy|4.184|1", "cal|energy|4.1868|1", "eV|energy|1.602176634e-19|1", "ev|energy|1.602176634e-19|1",
            "HPh|energy|2684519.53769617|0", "hh|energy|2684519.53769617|0", "Wh|energy|3600|1", "wh|energy|3600|1", "flb|energy|1.3558179483314|0", "BTU|energy|1055.05585262|0", "btu|energy|1055.05585262|0",
            "HP|power|745.69987158227|0", "h|power|745.69987158227|0", "PS|power|735.49875|0", "W|power|1|1", "w|power|1|1",
            "T|mag|1|1", "ga|mag|1e-4|1",
            "tsp|vol|4.92892159375e-6|0", "tspm|vol|5e-6|0", "tbs|vol|1.478676478125e-5|0", "oz|vol|2.95735295625e-5|0", "cup|vol|2.365882365e-4|0", "pt|vol|4.73176473e-4|0", "us_pt|vol|4.73176473e-4|0",
            "uk_pt|vol|5.6826125e-4|0", "qt|vol|9.46352946e-4|0", "uk_qt|vol|1.1365225e-3|0", "gal|vol|3.785411784e-3|0", "uk_gal|vol|4.54609e-3|0", "l|vol|1e-3|1", "L|vol|1e-3|1", "lt|vol|1e-3|1",
            "barrel|vol|0.158987294928|0", "bushel|vol|0.03523907016688|0", "ft3|vol|0.028316846592|0", "in3|vol|1.6387064e-5|0", "yd3|vol|0.764554857984|0", "GRT|vol|2.8316846592|0", "regton|vol|2.8316846592|0", "MTON|vol|1.13267386368|0",
            "uk_acre|area|4046.8564224|0", "us_acre|area|4046.87260987425|0", "ar|area|100|1", "ft2|area|0.09290304|0", "ha|area|10000|0", "in2|area|0.00064516|0", "Morgen|area|2500|0", "yd2|area|0.83612736|0",
            "bit|info|1|2", "byte|info|8|2",
            "admkn|speed|0.514773333333333|0", "kn|speed|0.514444444444444|0", "m/h|speed|0.000277777777777778|1", "m/hr|speed|0.000277777777777778|1", "m/s|speed|1|1", "m/sec|speed|1|1", "mph|speed|0.44704|0"];
        for r in rows.iter() {
            u.push(r.to_string());
        }
        // lengths that also come squared and cubed
        let bases: Vec<&str> = vec!["m", "ang", "ly", "Pica", "Picapt", "mi", "Nmi"];
        let lens: Vec<f64> = vec![1.0, 1e-10, ly, pica, pica, 1609.344, 1852.0];
        let mut i: i64 = 0;
        while i < (bases.len() as i64) {
            let b: &str = bases[i as usize];
            let f: f64 = lens[i as usize];
            let pre: &str = if b == "m" || b == "ang" || b == "ly" { "1" } else { "0" };
            if b != "m" && b != "ang" && b != "mi" && b != "Nmi" {
                u.push(format!("{}|dist|{}|{}", b, FormulaValue::fromNumber(f).borrow().asRaw(), pre));
            }
            if b == "ly" {
                u.push(format!("ly|dist|{}|1", FormulaValue::fromNumber(f).borrow().asRaw()));
            }
            u.push(format!("{}2|area|{}|{}", b, FormulaValue::fromNumber(f * f).borrow().asRaw(), pre));
            u.push(format!("{}3|vol|{}|{}", b, FormulaValue::fromNumber(f * f * f).borrow().asRaw(), pre));
            i += 1;
        }
        u
    }
    /// out: [factor, power] for a unit name, group in `group`; false if unknown.
    pub fn findUnit(name: &str, group: &mut Vec<String>, out: &mut Vec<f64>) -> bool {
        let table: Vec<String> = FormulaEng::units();
        // exact name first
        for row in table.iter() {
            let p: Vec<String> = row.split("|").map(|s| s.to_string()).collect::<Vec<String>>();
            if p[0] == name {
                group.clear();
                group.push(p[1].clone());
                out.clear();
                out.push(FormulaValue::parsePlainNumber(&p[2]).unwrap_or(1.0));
                return true;
            }
        }
        // prefix + unit
        let pres: Vec<&str> = vec!["Y", "Z", "E", "P", "T", "G", "M", "k", "h", "da", "e", "d", "c", "m", "u", "n", "p", "f", "a", "z", "y", "Yi", "Zi", "Ei", "Pi", "Ti", "Gi", "Mi", "ki"];
        let facs: Vec<f64> = vec![1e24, 1e21, 1e18, 1e15, 1e12, 1e9, 1e6, 1e3, 1e2, 1e1, 1e1, 1e-1, 1e-2, 1e-3, 1e-6, 1e-9, 1e-12, 1e-15, 1e-18, 1e-21, 1e-24,
            1208925819614629174706176.0, 1180591620717411303424.0, 1152921504606846976.0, 1125899906842624.0, 1099511627776.0, 1073741824.0, 1048576.0, 1024.0];
        let mut k: i64 = 0;
        while k < (pres.len() as i64) {
            let pre: &str = pres[k as usize];
            let pl: i64 = DgNative::str_len(pre);
            if DgNative::str_len(name) > pl && DgNative::substr(name, 0, pl) == pre {
                let rest: String = DgNative::substr(name, pl, DgNative::str_len(name));
                for row in table.iter() {
                    let p: Vec<String> = row.split("|").map(|s| s.to_string()).collect::<Vec<String>>();
                    if p[0] == rest && p[3] != "0" {
                        let binary: bool = k >= 21;
                        if binary && p[3] != "2" {
                            continue;
                        }
                        let mut f: f64 = facs[k as usize];
                        let last: i64 = DgNative::char_at(&rest, DgNative::str_len(&rest) - 1);
                        if p[1] == "area" && last == 50 {
                            f = f * f;
                        }
                        if p[1] == "vol" && last == 51 {
                            f = f * f * f;
                        }
                        group.clear();
                        group.push(p[1].clone());
                        out.clear();
                        out.push(FormulaValue::parsePlainNumber(&p[2]).unwrap_or(1.0) * f);
                        return true;
                    }
                }
            }
            k += 1;
        }
        false
    }
    /// Temperature in kelvin from a unit, and back.
    pub fn tempKind(name: &str) -> i64 {
        if name == "C" || name == "cel" {
            return 1;
        }
        if name == "F" || name == "fah" {
            return 2;
        }
        if name == "K" || name == "kel" {
            return 3;
        }
        if name == "Rank" {
            return 4;
        }
        if name == "Reau" {
            return 5;
        }
        0
    }
    pub fn convert(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if (args.len() as i64) < 3 {
            return FormulaEng::err("#VALUE!");
        }
        let x: f64 = FormulaMore::num(args, 0, 0.0);
        let from: String = FormulaMore::txt(args, 1);
        let to: String = FormulaMore::txt(args, 2);
        let tf: i64 = FormulaEng::tempKind(&from);
        let tt: i64 = FormulaEng::tempKind(&to);
        if tf > 0 || tt > 0 {
            // a prefixed kelvin (mK) is also a temperature
            let mut kf: f64 = 1.0;
            let mut kt: f64 = 1.0;
            let mut a: i64 = tf;
            let mut b: i64 = tt;
            let mut g: Vec<String> = Vec::new();
            let mut o: Vec<f64> = Vec::new();
            if a == 0 && DgNative::str_len(&from) > 1 && FormulaEng::tempKind(&DgNative::substr(&from, DgNative::str_len(&from) - 1, DgNative::str_len(&from))) == 3 {
                if FormulaEng::findUnit(&format!("{}m", DgNative::substr(&from, 0, DgNative::str_len(&from) - 1)), &mut g, &mut o) {
                    a = 3;
                    kf = o[0];
                }
            }
            if b == 0 && DgNative::str_len(&to) > 1 && FormulaEng::tempKind(&DgNative::substr(&to, DgNative::str_len(&to) - 1, DgNative::str_len(&to))) == 3 {
                if FormulaEng::findUnit(&format!("{}m", DgNative::substr(&to, 0, DgNative::str_len(&to) - 1)), &mut g, &mut o) {
                    b = 3;
                    kt = o[0];
                }
            }
            if a == 0 || b == 0 {
                return FormulaEng::err("#N/A");
            }
            let mut kel: f64 = x * kf;
            if a == 1 {
                kel = x + 273.15;
            }
            if a == 2 {
                kel = (x - 32.0) * 5.0 / 9.0 + 273.15;
            }
            if a == 4 {
                kel = x * 5.0 / 9.0;
            }
            if a == 5 {
                kel = x * 1.25 + 273.15;
            }
            let mut out: f64 = kel / kt;
            if b == 1 {
                out = kel - 273.15;
            }
            if b == 2 {
                out = (kel - 273.15) * 9.0 / 5.0 + 32.0;
            }
            if b == 4 {
                out = kel * 9.0 / 5.0;
            }
            if b == 5 {
                out = (kel - 273.15) * 0.8;
            }
            return FormulaEng::numV(out);
        }
        let mut g1: Vec<String> = Vec::new();
        let mut f1: Vec<f64> = Vec::new();
        let mut g2: Vec<String> = Vec::new();
        let mut f2: Vec<f64> = Vec::new();
        if !FormulaEng::findUnit(&from, &mut g1, &mut f1) || !FormulaEng::findUnit(&to, &mut g2, &mut f2) {
            return FormulaEng::err("#N/A");
        }
        if g1[0] != g2[0] {
            return FormulaEng::err("#N/A");
        }
        FormulaEng::numV(x * f1[0] / f2[0])
    }

    // --- Bessel ----------------------------------------------------------------------

    pub fn besselJ0(x: f64) -> f64 {
        let ax: f64 = FormulaStats::absD(x);
        if ax < 8.0 {
            let y: f64 = x * x;
            let a1: f64 = 57568490574.0 + y * (-13362590354.0 + y * (651619640.7 + y * (-11214424.18 + y * (77392.33017 + y * (-184.9052456)))));
            let a2: f64 = 57568490411.0 + y * (1029532985.0 + y * (9494680.718 + y * (59272.64853 + y * (267.8532712 + y * 1.0))));
            return a1 / a2;
        }
        let z: f64 = 8.0 / ax;
        let y: f64 = z * z;
        let xx: f64 = ax - 0.785398164;
        let a1: f64 = 1.0 + y * (-0.1098628627e-2 + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
        let a2: f64 = -0.1562499995e-1 + y * (0.1430488765e-3 + y * (-0.6911147651e-5 + y * (0.7621095161e-6 - y * 0.934935152e-7)));
        FormulaStats::sqrtD(0.636619772 / ax) * (DgNative::cos(xx) * a1 - z * DgNative::sin(xx) * a2)
    }
    pub fn besselJ1(x: f64) -> f64 {
        let ax: f64 = FormulaStats::absD(x);
        if ax < 8.0 {
            let y: f64 = x * x;
            let a1: f64 = x * (72362614232.0 + y * (-7895059235.0 + y * (242396853.1 + y * (-2972611.439 + y * (15704.48260 + y * (-30.16036606))))));
            let a2: f64 = 144725228442.0 + y * (2300535178.0 + y * (18583304.74 + y * (99447.43394 + y * (376.9991397 + y * 1.0))));
            return a1 / a2;
        }
        let z: f64 = 8.0 / ax;
        let y: f64 = z * z;
        let xx: f64 = ax - 2.356194491;
        let a1: f64 = 1.0 + y * (0.183105e-2 + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * (-0.240337019e-6))));
        let a2: f64 = 0.04687499995 + y * (-0.2002690873e-3 + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
        let mut ans: f64 = FormulaStats::sqrtD(0.636619772 / ax) * (DgNative::cos(xx) * a1 - z * DgNative::sin(xx) * a2);
        if x < 0.0 {
            ans = 0.0 - ans;
        }
        ans
    }
    pub fn besselY0(x: f64) -> f64 {
        if x < 8.0 {
            let y: f64 = x * x;
            let a1: f64 = -2957821389.0 + y * (7062834065.0 + y * (-512359803.6 + y * (10879881.29 + y * (-86327.92757 + y * 228.4622733))));
            let a2: f64 = 40076544269.0 + y * (745249964.8 + y * (7189466.438 + y * (47447.26470 + y * (226.1030244 + y * 1.0))));
            return a1 / a2 + 0.636619772 * FormulaEng::besselJ0(x) * FormulaStats::ln(x);
        }
        let z: f64 = 8.0 / x;
        let y: f64 = z * z;
        let xx: f64 = x - 0.785398164;
        let a1: f64 = 1.0 + y * (-0.1098628627e-2 + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
        let a2: f64 = -0.1562499995e-1 + y * (0.1430488765e-3 + y * (-0.6911147651e-5 + y * (0.7621095161e-6 + y * (-0.934945152e-7))));
        FormulaStats::sqrtD(0.636619772 / x) * (DgNative::sin(xx) * a1 + z * DgNative::cos(xx) * a2)
    }
    pub fn besselY1(x: f64) -> f64 {
        if x < 8.0 {
            let y: f64 = x * x;
            let a1: f64 = x * (-0.4900604943e13 + y * (0.1275274390e13 + y * (-0.5153438139e11 + y * (0.7349264551e9 + y * (-0.4237922726e7 + y * 0.8511937935e4)))));
            let a2: f64 = 0.2499580570e14 + y * (0.4244419664e12 + y * (0.3733650367e10 + y * (0.2245904002e8 + y * (0.1020426050e6 + y * (0.3549632885e3 + y)))));
            return a1 / a2 + 0.636619772 * (FormulaEng::besselJ1(x) * FormulaStats::ln(x) - 1.0 / x);
        }
        let z: f64 = 8.0 / x;
        let y: f64 = z * z;
        let xx: f64 = x - 2.356194491;
        let a1: f64 = 1.0 + y * (0.183105e-2 + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * (-0.240337019e-6))));
        let a2: f64 = 0.04687499995 + y * (-0.2002690873e-3 + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
        FormulaStats::sqrtD(0.636619772 / x) * (DgNative::sin(xx) * a1 + z * DgNative::cos(xx) * a2)
    }
    pub fn besselK0(x: f64) -> f64 {
        if x <= 2.0 {
            let y: f64 = x * x / 4.0;
            return (0.0 - FormulaStats::ln(x / 2.0) * FormulaEng::besselIn(0, x)) + (-0.57721566 + y * (0.42278420 + y * (0.23069756 + y * (0.3488590e-1 + y * (0.262698e-2 + y * (0.10750e-3 + y * 0.74e-5))))));
        }
        let y: f64 = 2.0 / x;
        FormulaStats::exp(0.0 - x) / FormulaStats::sqrtD(x) * (1.25331414 + y * (-0.7832358e-1 + y * (0.2189568e-1 + y * (-0.1062446e-1 + y * (0.587872e-2 + y * (-0.251540e-2 + y * 0.53208e-3))))))
    }
    pub fn besselK1(x: f64) -> f64 {
        if x <= 2.0 {
            let y: f64 = x * x / 4.0;
            return (FormulaStats::ln(x / 2.0) * FormulaEng::besselIn(1, x)) + (1.0 / x) * (1.0 + y * (0.15443144 + y * (-0.67278579 + y * (-0.18156897 + y * (-0.1919402e-1 + y * (-0.110404e-2 + y * (-0.4686e-4)))))));
        }
        let y: f64 = 2.0 / x;
        FormulaStats::exp(0.0 - x) / FormulaStats::sqrtD(x) * (1.25331414 + y * (0.23498619 + y * (-0.3655620e-1 + y * (0.1504268e-1 + y * (-0.780353e-2 + y * (0.325614e-2 + y * (-0.68245e-3)))))))
    }
    /// I_n(x) and J_n(x) by their power series (sign = -1 for J).
    pub fn seriesIJ(n: i64, x: f64, sign: f64) -> f64 {
        let h: f64 = x / 2.0;
        let mut term: f64 = 1.0;
        let mut k: i64 = 1;
        while k <= n {
            term = term * h / (k as f64);
            k += 1;
        }
        let mut sum: f64 = term;
        let h2: f64 = h * h;
        let mut m: i64 = 1;
        while m < 500 {
            term = term * sign * h2 / ((m as f64) * ((m + n) as f64));
            sum += term;
            if FormulaStats::absD(term) < 1.0e-17 * FormulaStats::absD(sum) {
                break;
            }
            m += 1;
        }
        sum
    }
    pub fn besselIn(n: i64, x: f64) -> f64 {
        FormulaEng::seriesIJ(n, x, 1.0)
    }
    pub fn bessel(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if (args.len() as i64) < 2 {
            return FormulaEng::err("#VALUE!");
        }
        let x: f64 = FormulaMore::num(args, 0, 0.0);
        let ord: i64 = FormulaMore::num(args, 1, 0.0).floor() as i64;
        if ord < 0 {
            return FormulaEng::err("#NUM!");
        }
        if n == "BESSELI" {
            return FormulaEng::numV(FormulaEng::besselIn(ord, x));
        }
        if n == "BESSELJ" {
            if FormulaStats::absD(x) < 12.0 + (ord as f64) {
                return FormulaEng::numV(FormulaEng::seriesIJ(ord, x, -1.0));
            }
            // large x: upward recurrence from J0, J1 is stable for n < x
            let mut a: f64 = FormulaEng::besselJ0(x);
            if ord == 0 {
                return FormulaEng::numV(a);
            }
            let mut b: f64 = FormulaEng::besselJ1(x);
            let mut k: i64 = 1;
            while k < ord {
                let c: f64 = 2.0 * (k as f64) / x * b - a;
                a = b;
                b = c;
                k += 1;
            }
            return FormulaEng::numV(b);
        }
        if x <= 0.0 {
            return FormulaEng::err("#NUM!");
        }
        let mut a: f64 = 0.0;
        let mut b: f64 = 0.0;
        if n == "BESSELY" {
            a = FormulaEng::besselY0(x);
            b = FormulaEng::besselY1(x);
        } else {
            a = FormulaEng::besselK0(x);
            b = FormulaEng::besselK1(x);
        }
        if ord == 0 {
            return FormulaEng::numV(a);
        }
        let mut k: i64 = 1;
        while k < ord {
            let mut c: f64 = 2.0 * (k as f64) / x * b - a;
            if n == "BESSELK" {
                c = 2.0 * (k as f64) / x * b + a;
            }
            a = b;
            b = c;
            k += 1;
        }
        FormulaEng::numV(b)
    }

    // --- bonds -----------------------------------------------------------------------

    pub fn isMonthEnd(d: f64) -> bool {
        let p: Vec<i64> = DateSerial::toYMD(d);
        p[2] == DateSerial::daysInMonth(p[0], p[1])
    }
    /// The coupon date `k` periods of 12/freq months before maturity (k may
    /// be negative), keeping month ends at month ends.
    pub fn couponDate(mat: f64, freq: i64, k: i64) -> f64 {
        let months: i64 = 0 - k * (12 / freq);
        if FormulaEng::isMonthEnd(mat) {
            return DateSerial::endOfMonth(mat, months);
        }
        DateSerial::addMonths(mat, months)
    }
    /// out[0] previous coupon date, out[1] next, out[2] number of coupons left.
    pub fn coupons(settle: f64, mat: f64, freq: i64, out: &mut Vec<f64>) {
        let mut k: i64 = 0;
        while FormulaEng::couponDate(mat, freq, k + 1) > settle {
            k += 1;
        }
        out.clear();
        out.push(FormulaEng::couponDate(mat, freq, k + 1));
        out.push(FormulaEng::couponDate(mat, freq, k));
        out.push((k + 1) as f64);
    }
    pub fn dayCount(a: f64, b: f64, basis: i64) -> f64 {
        if basis == 0 || basis == 4 {
            let pa: Vec<i64> = DateSerial::toYMD(a);
            let pb: Vec<i64> = DateSerial::toYMD(b);
            return FormulaMore::days360(&pa, &pb, basis == 4) as f64;
        }
        b - a
    }
    /// The length of the coupon period [pcd, ncd] in days, by basis.
    pub fn periodDays(pcd: f64, ncd: f64, freq: i64, basis: i64) -> f64 {
        if basis == 1 {
            return ncd - pcd;
        }
        if basis == 3 {
            return 365.0 / (freq as f64);
        }
        360.0 / (freq as f64)
    }
    pub fn yearDays(basis: i64, d: f64) -> f64 {
        if basis == 3 {
            return 365.0;
        }
        if basis == 1 {
            let y: i64 = DateSerial::toYMD(d)[0];
            if DateSerial::isLeap(y) {
                return 366.0;
            }
            return 365.0;
        }
        360.0
    }
    pub fn bondPrice(settle: f64, mat: f64, rate: f64, yld: f64, red: f64, freq: i64, basis: i64) -> f64 {
        let mut c: Vec<f64> = Vec::new();
        FormulaEng::coupons(settle, mat, freq, &mut c);
        let f: f64 = freq as f64;
        let e: f64 = FormulaEng::periodDays(c[0], c[1], freq, basis);
        let a: f64 = FormulaEng::dayCount(c[0], settle, basis);
        let mut dsc: f64 = c[1] - settle;
        if basis == 0 || basis == 4 {
            dsc = e - a;
        }
        let nn: f64 = c[2];
        let cpn: f64 = 100.0 * rate / f;
        if nn <= 1.0 {
            return (red + cpn) / (1.0 + dsc / e * yld / f) - cpn * a / e;
        }
        let base: f64 = 1.0 + yld / f;
        let mut p: f64 = red / FormulaStats::pow(base, nn - 1.0 + dsc / e);
        let mut k: f64 = 1.0;
        while k <= nn {
            p += cpn / FormulaStats::pow(base, k - 1.0 + dsc / e);
            k += 1.0;
        }
        p - cpn * a / e
    }
    pub fn bondArgs(args: &Vec<Rc<RefCell<FormulaValue>>>, freq_at: i64, freq: &mut Vec<i64>) -> bool {
        let f: i64 = FormulaMore::num(args, freq_at, 0.0).floor() as i64;
        let b: i64 = FormulaMore::num(args, freq_at + 1, 0.0).floor() as i64;
        freq.clear();
        freq.push(f);
        freq.push(b);
        (f == 1 || f == 2 || f == 4) && b >= 0 && b <= 4
    }
    pub fn bondFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let na: i64 = args.len() as i64;
        let settle: f64 = FormulaMore::num(args, 0, 0.0).floor();
        let mat: f64 = FormulaMore::num(args, 1, 0.0).floor();
        if DgNative::substr(n, 0, 4) == "COUP" {
            if na < 3 {
                return FormulaEng::err("#VALUE!");
            }
            let mut fb: Vec<i64> = Vec::new();
            if !FormulaEng::bondArgs(args, 2, &mut fb) || settle >= mat {
                return FormulaEng::err("#NUM!");
            }
            let mut c: Vec<f64> = Vec::new();
            FormulaEng::coupons(settle, mat, fb[0], &mut c);
            if n == "COUPPCD" {
                return FormulaEng::numV(c[0]);
            }
            if n == "COUPNCD" {
                return FormulaEng::numV(c[1]);
            }
            if n == "COUPNUM" {
                return FormulaEng::numV(c[2]);
            }
            let e: f64 = FormulaEng::periodDays(c[0], c[1], fb[0], fb[1]);
            if n == "COUPDAYS" {
                return FormulaEng::numV(e);
            }
            let a: f64 = FormulaEng::dayCount(c[0], settle, fb[1]);
            if n == "COUPDAYBS" {
                return FormulaEng::numV(a);
            }
            if fb[1] == 0 || fb[1] == 4 {
                return FormulaEng::numV(e - a);
            }
            return FormulaEng::numV(c[1] - settle);
        }
        if n == "PRICE" || n == "YIELD" || n == "DURATION" || n == "MDURATION" {
            let mut fb: Vec<i64> = Vec::new();
            let mut freq_at: i64 = 5;
            if n == "DURATION" || n == "MDURATION" {
                freq_at = 4;
            }
            if na < freq_at + 1 {
                return FormulaEng::err("#VALUE!");
            }
            if !FormulaEng::bondArgs(args, freq_at, &mut fb) || settle >= mat {
                return FormulaEng::err("#NUM!");
            }
            let rate: f64 = FormulaMore::num(args, 2, 0.0);
            let third: f64 = FormulaMore::num(args, 3, 0.0);
            if rate < 0.0 || third < 0.0 {
                return FormulaEng::err("#NUM!");
            }
            let freq: i64 = fb[0];
            let basis: i64 = fb[1];
            if n == "PRICE" {
                let red: f64 = FormulaMore::num(args, 4, 0.0);
                if red <= 0.0 {
                    return FormulaEng::err("#NUM!");
                }
                return FormulaEng::numV(FormulaEng::bondPrice(settle, mat, rate, third, red, freq, basis));
            }
            if n == "YIELD" {
                let red: f64 = FormulaMore::num(args, 4, 0.0);
                if red <= 0.0 || third <= 0.0 {
                    return FormulaEng::err("#NUM!");
                }
                let mut c: Vec<f64> = Vec::new();
                FormulaEng::coupons(settle, mat, freq, &mut c);
                let f: f64 = freq as f64;
                if c[2] <= 1.0 {
                    let e: f64 = FormulaEng::periodDays(c[0], c[1], freq, basis);
                    let a: f64 = FormulaEng::dayCount(c[0], settle, basis);
                    let dsr: f64 = e - a;
                    let par: f64 = third / 100.0 + a / e * rate / f;
                    return FormulaEng::numV(((red / 100.0 + rate / f) - par) / par * f * e / dsr);
                }
                // bisection then Newton polish on PRICE(y) = pr
                let mut lo: f64 = -0.99 * f;
                let mut hi: f64 = 1.0;
                while FormulaEng::bondPrice(settle, mat, rate, hi, red, freq, basis) > third && hi < 1.0e6 {
                    hi *= 2.0;
                }
                let mut k: i64 = 0;
                while k < 200 {
                    let mid: f64 = (lo + hi) / 2.0;
                    if FormulaEng::bondPrice(settle, mat, rate, mid, red, freq, basis) > third {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                    if hi - lo < 1.0e-15 {
                        break;
                    }
                    k += 1;
                }
                return FormulaEng::numV((lo + hi) / 2.0);
            }
            // DURATION(settlement, maturity, coupon, yld, frequency, [basis])
            let yld: f64 = third;
            let mut c: Vec<f64> = Vec::new();
            FormulaEng::coupons(settle, mat, freq, &mut c);
            let f: f64 = freq as f64;
            let e: f64 = FormulaEng::periodDays(c[0], c[1], freq, basis);
            let a: f64 = FormulaEng::dayCount(c[0], settle, basis);
            let mut dsc: f64 = c[1] - settle;
            if basis == 0 || basis == 4 {
                dsc = e - a;
            }
            let base: f64 = 1.0 + yld / f;
            let cpn: f64 = 100.0 * rate / f;
            let mut pv: f64 = 0.0;
            let mut tw: f64 = 0.0;
            let mut k: f64 = 1.0;
            while k <= c[2] {
                let t: f64 = k - 1.0 + dsc / e;
                let mut cf: f64 = cpn;
                if k == c[2] {
                    cf += 100.0;
                }
                let v: f64 = cf / FormulaStats::pow(base, t);
                pv += v;
                tw += t * v;
                k += 1.0;
            }
            let d: f64 = tw / pv / f;
            if n == "MDURATION" {
                return FormulaEng::numV(d / base);
            }
            return FormulaEng::numV(d);
        }
        if n == "ACCRINTM" {
            // ACCRINTM(issue, settlement, rate, [par], [basis])
            let rate: f64 = FormulaMore::num(args, 2, 0.0);
            let par: f64 = FormulaMore::num(args, 3, 1000.0);
            let basis: i64 = FormulaMore::num(args, 4, 0.0).floor() as i64;
            if rate <= 0.0 || par <= 0.0 || FormulaMore::num(args, 1, 0.0).floor() <= settle || basis < 0 || basis > 4 {
                return FormulaEng::err("#NUM!");
            }
            let yf: Rc<RefCell<FormulaValue>> = FormulaStats::yearFrac(args, 4);
            if yf.borrow().isError() {
                return yf;
            }
            return FormulaEng::numV(par * rate * yf.borrow().num);
        }
        if n == "ACCRINT" {
            // ACCRINT(issue, first_interest, settlement, rate, par, frequency, [basis], [calc_method])
            if na < 6 {
                return FormulaEng::err("#VALUE!");
            }
            let issue: f64 = settle;
            let first: f64 = mat;
            let st: f64 = FormulaMore::num(args, 2, 0.0).floor();
            let rate: f64 = FormulaMore::num(args, 3, 0.0);
            let par: f64 = FormulaMore::num(args, 4, 1000.0);
            let mut fb: Vec<i64> = Vec::new();
            if !FormulaEng::bondArgs(args, 5, &mut fb) || rate <= 0.0 || par <= 0.0 || issue >= st {
                return FormulaEng::err("#NUM!");
            }
            let freq: i64 = fb[0];
            let basis: i64 = fb[1];
            let mut from: f64 = issue;
            if FormulaMore::has(args, 7) && !args[7].borrow().single().borrow().isTrue() && st > first {
                from = first;
            }
            // quasi-coupon periods anchored at first_interest
            let mut k: i64 = 0;
            while FormulaEng::couponDate(first, freq, k) > from {
                k += 1;
            }
            let mut total: f64 = 0.0;
            let mut j: i64 = k;
            while j > -2000 {
                let p0: f64 = FormulaEng::couponDate(first, freq, j);
                let p1: f64 = FormulaEng::couponDate(first, freq, j - 1);
                if p0 >= st {
                    break;
                }
                let mut a: f64 = p0;
                if from > a {
                    a = from;
                }
                let mut b: f64 = p1;
                if st < b {
                    b = st;
                }
                let len: f64 = FormulaEng::periodDays(p0, p1, freq, basis);
                total += FormulaEng::dayCount(a, b, basis) / len;
                j -= 1;
            }
            return FormulaEng::numV(par * rate / (freq as f64) * total);
        }
        if n == "PRICEMAT" || n == "YIELDMAT" {
            // (settlement, maturity, issue, rate, yld|pr, [basis])
            if na < 5 {
                return FormulaEng::err("#VALUE!");
            }
            let issue: f64 = FormulaMore::num(args, 2, 0.0).floor();
            let rate: f64 = FormulaMore::num(args, 3, 0.0);
            let v: f64 = FormulaMore::num(args, 4, 0.0);
            let basis: i64 = FormulaMore::num(args, 5, 0.0).floor() as i64;
            if settle >= mat || rate < 0.0 || v < 0.0 || basis < 0 || basis > 4 {
                return FormulaEng::err("#NUM!");
            }
            let b: f64 = FormulaEng::yearDays(basis, settle);
            let dim: f64 = FormulaEng::dayCount(issue, mat, basis) / b;
            let dsm: f64 = FormulaEng::dayCount(settle, mat, basis) / b;
            let a: f64 = FormulaEng::dayCount(issue, settle, basis) / b;
            if n == "PRICEMAT" {
                return FormulaEng::numV((100.0 + dim * rate * 100.0) / (1.0 + dsm * v) - a * rate * 100.0);
            }
            let p: f64 = v / 100.0 + a * rate;
            return FormulaEng::numV(((1.0 + dim * rate) - p) / p / dsm);
        }
        if n == "AMORLINC" || n == "AMORDEGRC" {
            // (cost, date_purchased, first_period, salvage, period, rate, [basis])
            if na < 6 {
                return FormulaEng::err("#VALUE!");
            }
            let cost: f64 = FormulaMore::num(args, 0, 0.0);
            let bought: f64 = FormulaMore::num(args, 1, 0.0).floor();
            let first: f64 = FormulaMore::num(args, 2, 0.0).floor();
            let salvage: f64 = FormulaMore::num(args, 3, 0.0);
            let period: i64 = FormulaMore::num(args, 4, 0.0).floor() as i64;
            let rate: f64 = FormulaMore::num(args, 5, 0.0);
            let basis: i64 = FormulaMore::num(args, 6, 0.0).floor() as i64;
            if cost < 0.0 || salvage < 0.0 || salvage > cost || period < 0 || rate <= 0.0 || basis == 2 || basis < 0 || basis > 4 || first < bought {
                return FormulaEng::err("#NUM!");
            }
            let mut yfa: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            yfa.push(FormulaValue::fromNumber(bought));
            yfa.push(FormulaValue::fromNumber(first));
            yfa.push(FormulaValue::fromNumber(basis as f64));
            let yf: f64 = FormulaFunctions::invoke("YEARFRAC", &yfa).borrow().numberValue();
            if n == "AMORLINC" {
                let full: f64 = cost * rate;
                let first_dep: f64 = full * yf;
                let left: f64 = cost - salvage;
                if period == 0 {
                    return FormulaEng::numV(if first_dep < left { first_dep } else { left });
                }
                let mut done: f64 = first_dep;
                let mut p: i64 = 1;
                while p < period {
                    done += full;
                    p += 1;
                }
                if done >= left {
                    return FormulaEng::numV(0.0);
                }
                let rest: f64 = left - done;
                return FormulaEng::numV(if full < rest { full } else { rest });
            }
            // AMORDEGRC: French declining balance with the life coefficient
            let life: f64 = 1.0 / rate;
            let mut coef: f64 = 1.0;
            if life >= 3.0 && life <= 4.0 {
                coef = 1.5;
            } else if life >= 5.0 && life <= 6.0 {
                coef = 2.0;
            } else if life > 6.0 {
                coef = 2.5;
            }
            if life < 3.0 || (life > 4.0 && life < 5.0) {
                return FormulaEng::err("#NUM!");
            }
            let r: f64 = rate * coef;
            let mut dep: f64 = FormulaEng::roundHalf(yf * r * cost);
            let mut book: f64 = cost - dep;
            if period == 0 {
                return FormulaEng::numV(dep);
            }
            let periods: f64 = (1.0 / rate).floor();
            let mut p: i64 = 1;
            while p <= period {
                let left: f64 = periods - (p as f64);
                if left < 0.0 {
                    return FormulaEng::numV(0.0);
                }
                if left == 1.0 {
                    dep = FormulaEng::roundHalf(book * 0.5);
                } else if left == 0.0 {
                    dep = book - salvage;
                    if dep < 0.0 {
                        dep = 0.0;
                    }
                } else {
                    dep = FormulaEng::roundHalf(r * book);
                }
                book -= dep;
                p += 1;
            }
            return FormulaEng::numV(dep);
        }
        FormulaValue::blank()
    }
    pub fn roundHalf(x: f64) -> f64 {
        (x + 0.5).floor()
    }
}
