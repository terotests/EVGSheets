// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;
use crate::formula_value::FormulaValue;
use crate::formula_functions::FormulaFunctions;
use crate::formula_more::FormulaMore;

/// FormulaStats - the third part of the function library: probability
/// distributions and statistical tests, regression arrays, database
/// functions, hyperbolic and bit functions, and a few finance and text
/// functions. Same contract as FormulaMore, which falls through to `invoke`
/// for the names in `names()`.
///
/// The distributions rest on four numerical pieces: the log-gamma function
/// (Lanczos), the regularized incomplete gamma and beta functions (series and
/// continued fractions), and an inverse found by bisection on the CDF.
#[derive(Clone)]
pub struct FormulaStats {
}

impl FormulaStats {
    pub fn new() -> Self {
        Self {
        }
    }
    pub fn names() -> String {
        " NORM.DIST NORMDIST NORM.S.DIST NORMSDIST NORM.INV NORMINV NORM.S.INV NORMSINV LOGNORM.DIST LOGNORMDIST LOGNORM.INV LOGINV PHI GAUSS EXPON.DIST EXPONDIST POISSON.DIST POISSON BINOM.DIST BINOMDIST BINOM.INV CRITBINOM BINOM.DIST.RANGE NEGBINOM.DIST NEGBINOMDIST HYPGEOM.DIST HYPGEOMDIST WEIBULL.DIST WEIBULL GAMMA GAMMALN GAMMALN.PRECISE GAMMA.DIST GAMMADIST GAMMA.INV GAMMAINV BETA.DIST BETADIST BETA.INV BETAINV CHISQ.DIST CHISQ.DIST.RT CHIDIST CHISQ.INV CHISQ.INV.RT CHIINV T.DIST T.DIST.2T T.DIST.RT TDIST T.INV T.INV.2T TINV F.DIST F.DIST.RT FDIST F.INV F.INV.RT FINV CONFIDENCE CONFIDENCE.NORM CONFIDENCE.T ERF ERF.PRECISE ERFC ERFC.PRECISE FISHER FISHERINV CHISQ.TEST CHITEST T.TEST TTEST F.TEST FTEST Z.TEST ZTEST KURT SKEW SKEW.P STDEVA STDEVPA VARA VARPA MODE.MULT PERCENTRANK.EXC FREQUENCY PROB PERMUTATIONA SERIESSUM TREND GROWTH LINEST LOGEST ACOSH ASINH ATANH ACOT ACOTH COTH CSCH SECH BITAND BITOR BITXOR BITLSHIFT BITRSHIFT BIN2HEX BIN2OCT HEX2BIN HEX2OCT OCT2BIN OCT2HEX DELTA GESTEP DSUM DAVERAGE DCOUNT DCOUNTA DMAX DMIN DGET DPRODUCT DSTDEV DSTDEVP DVAR DVARP FVSCHEDULE ISPMT DOLLARDE DOLLARFR TBILLEQ TBILLPRICE TBILLYIELD DISC INTRATE RECEIVED PRICEDISC YIELDDISC VDB LEFTB RIGHTB MIDB LENB FINDB SEARCHB REPLACEB ASC DBCS JIS PHONETIC ARRAYTOTEXT PERCENTOF ENCODEURL ECMA.CEILING USDOLLAR ".to_string()
    }
    pub fn isKnown(n: &str) -> bool {
        let all: String = FormulaStats::names();
        DgNative::index_of(&all, &format!(" {} ", n)) >= 0
    }

    // --- numerical core ------------------------------------------------------------

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
        // Scale into [1, 4) by powers of four, then Newton from a fair start.
        let mut v: f64 = x;
        let mut scale: f64 = 1.0;
        while v >= 4.0 {
            v /= 4.0;
            scale *= 2.0;
        }
        while v < 1.0 {
            v *= 4.0;
            scale /= 2.0;
        }
        let mut g: f64 = (v + 1.0) / 2.0;
        let mut k: i64 = 0;
        while k < 8 {
            g = (g + v / g) / 2.0;
            k += 1;
        }
        g * scale
    }
    pub fn ln(x: f64) -> f64 {
        if x <= 0.0 {
            return -1.0e308;
        }
        // Scale by powers of two first so the series in lnD starts near 1.
        let mut v: f64 = x;
        let mut k: i64 = 0;
        while v > 1.0e8 {
            v /= 1.0e8;
            k += 1;
        }
        while v < 1.0e-8 {
            v *= 1.0e8;
            k -= 1;
        }
        FormulaFunctions::lnD(v) + (k as f64) * 18.420680743952367
    }
    pub fn exp(x: f64) -> f64 {
        if x < -745.0 {
            return 0.0;
        }
        if x > 709.0 {
            return 1.0e308;
        }
        // e^x = 2^k e^r with |r| <= ln2/2; ln2 split in two so r keeps its bits.
        let k: f64 = (x / 0.6931471805599453 + 0.5).floor();
        let r: f64 = (x - k * 0.6931471803691238) - k * 1.9082149292705877e-10;
        let mut term: f64 = 1.0;
        let mut sum: f64 = 1.0;
        let mut n: i64 = 1;
        while n < 22 {
            term = (term * r) / (n as f64);
            sum += term;
            n += 1;
        }
        let mut i: i64 = 0;
        let ki: i64 = k as i64;
        while i < ki {
            sum *= 2.0;
            i += 1;
        }
        while i > ki {
            sum /= 2.0;
            i -= 1;
        }
        sum
    }
    pub fn pow(b: f64, e: f64) -> f64 {
        if b == 0.0 {
            if e == 0.0 {
                return 1.0;
            }
            return 0.0;
        }
        if b < 0.0 {
            return FormulaMore::powD(b, e);
        }
        let ei: f64 = e.floor();
        if ei == e && FormulaStats::absD(e) < 64.0 {
            let mut out: f64 = 1.0;
            let mut i: i64 = 0;
            let n: i64 = FormulaStats::absD(e) as i64;
            while i < n {
                out *= b;
                i += 1;
            }
            if e < 0.0 {
                return 1.0 / out;
            }
            return out;
        }
        FormulaStats::exp(e * FormulaStats::ln(b))
    }
    /// ln(Gamma(x)) for x > 0 (Lanczos, g = 7).
    pub fn lnGamma(x: f64) -> f64 {
        if x < 0.5 {
            let s: f64 = FormulaMore::sinD(FormulaMore::PI() * x);
            return FormulaStats::ln(FormulaMore::PI() / FormulaStats::absD(s)) - FormulaStats::lnGamma(1.0 - x);
        }
        let c: Vec<f64> = vec![0.99999999999980993, 676.5203681218851, -1259.1392167224028, 771.32342877765313, -176.61502916214059, 12.507343278686905, -0.13857109526572012, 9.9843695780195716e-6, 1.5056327351493116e-7];
        let xx: f64 = x - 1.0;
        let mut a: f64 = c[0];
        let t: f64 = xx + 7.5;
        let mut i: i64 = 1;
        while i < 9 {
            a += c[i as usize] / (xx + (i as f64));
            i += 1;
        }
        0.9189385332046727 + (xx + 0.5) * FormulaStats::ln(t) - t + FormulaStats::ln(a)
    }
    pub fn gammaFn(x: f64) -> f64 {
        if x > 0.0 {
            let xi: f64 = x.floor();
            if xi == x && x < 30.0 {
                return FormulaMore::fact((x as i64) - 1);
            }
            return FormulaStats::exp(FormulaStats::lnGamma(x));
        }
        // reflection: Gamma(x) = pi / (sin(pi x) Gamma(1 - x))
        let s: f64 = FormulaMore::sinD(FormulaMore::PI() * x);
        FormulaMore::PI() / (s * FormulaStats::gammaFn(1.0 - x))
    }
    /// Regularized lower incomplete gamma P(a, x).
    pub fn gammaP(a: f64, x: f64) -> f64 {
        if x <= 0.0 {
            return 0.0;
        }
        if x < a + 1.0 {
            let mut ap: f64 = a;
            let mut sum: f64 = 1.0 / a;
            let mut del: f64 = sum;
            let mut n: i64 = 0;
            while n < 1000 {
                ap += 1.0;
                del *= x / ap;
                sum += del;
                if FormulaStats::absD(del) < FormulaStats::absD(sum) * 1.0e-16 {
                    break;
                }
                n += 1;
            }
            return sum * FormulaStats::exp(0.0 - x + a * FormulaStats::ln(x) - FormulaStats::lnGamma(a));
        }
        1.0 - FormulaStats::gammaQcf(a, x)
    }
    pub fn gammaQ(a: f64, x: f64) -> f64 {
        if x <= 0.0 {
            return 1.0;
        }
        if x < a + 1.0 {
            return 1.0 - FormulaStats::gammaP(a, x);
        }
        FormulaStats::gammaQcf(a, x)
    }
    pub fn gammaQcf(a: f64, x: f64) -> f64 {
        let tiny: f64 = 1.0e-300;
        let mut b: f64 = x + 1.0 - a;
        let mut c: f64 = 1.0 / tiny;
        let mut d: f64 = 1.0 / b;
        let mut h: f64 = d;
        let mut i: i64 = 1;
        while i < 1000 {
            let an: f64 = 0.0 - (i as f64) * ((i as f64) - a);
            b += 2.0;
            d = an * d + b;
            if FormulaStats::absD(d) < tiny {
                d = tiny;
            }
            c = b + an / c;
            if FormulaStats::absD(c) < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let del: f64 = d * c;
            h *= del;
            if FormulaStats::absD(del - 1.0) < 1.0e-16 {
                break;
            }
            i += 1;
        }
        FormulaStats::exp(0.0 - x + a * FormulaStats::ln(x) - FormulaStats::lnGamma(a)) * h
    }
    pub fn lnBeta(a: f64, b: f64) -> f64 {
        FormulaStats::lnGamma(a) + FormulaStats::lnGamma(b) - FormulaStats::lnGamma(a + b)
    }
    /// Regularized incomplete beta I_x(a, b).
    pub fn betaI(a: f64, b: f64, x: f64) -> f64 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        let bt: f64 = FormulaStats::exp(a * FormulaStats::ln(x) + b * FormulaStats::ln(1.0 - x) - FormulaStats::lnBeta(a, b));
        if x < (a + 1.0) / (a + b + 2.0) {
            return bt * FormulaStats::betaCf(a, b, x) / a;
        }
        1.0 - bt * FormulaStats::betaCf(b, a, 1.0 - x) / b
    }
    pub fn betaCf(a: f64, b: f64, x: f64) -> f64 {
        let tiny: f64 = 1.0e-300;
        let qab: f64 = a + b;
        let qap: f64 = a + 1.0;
        let qam: f64 = a - 1.0;
        let mut c: f64 = 1.0;
        let mut d: f64 = 1.0 - qab * x / qap;
        if FormulaStats::absD(d) < tiny {
            d = tiny;
        }
        d = 1.0 / d;
        let mut h: f64 = d;
        let mut m: i64 = 1;
        while m < 1000 {
            let mf: f64 = m as f64;
            let m2: f64 = 2.0 * mf;
            let mut aa: f64 = mf * (b - mf) * x / ((qam + m2) * (a + m2));
            d = 1.0 + aa * d;
            if FormulaStats::absD(d) < tiny {
                d = tiny;
            }
            c = 1.0 + aa / c;
            if FormulaStats::absD(c) < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            h *= d * c;
            aa = 0.0 - (a + mf) * (qab + mf) * x / ((a + m2) * (qap + m2));
            d = 1.0 + aa * d;
            if FormulaStats::absD(d) < tiny {
                d = tiny;
            }
            c = 1.0 + aa / c;
            if FormulaStats::absD(c) < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let del: f64 = d * c;
            h *= del;
            if FormulaStats::absD(del - 1.0) < 1.0e-16 {
                break;
            }
            m += 1;
        }
        h
    }
    pub fn erf(x: f64) -> f64 {
        if x == 0.0 {
            return 0.0;
        }
        let p: f64 = FormulaStats::gammaP(0.5, x * x);
        if x < 0.0 {
            return 0.0 - p;
        }
        p
    }
    pub fn erfc(x: f64) -> f64 {
        if x < 0.0 {
            return 2.0 - FormulaStats::erfc(0.0 - x);
        }
        FormulaStats::gammaQ(0.5, x * x)
    }
    pub fn normCdf(z: f64) -> f64 {
        0.5 * FormulaStats::erfc((0.0 - z) / 1.4142135623730951)
    }
    pub fn normPdf(z: f64) -> f64 {
        FormulaStats::exp(0.0 - z * z / 2.0) / 2.5066282746310002
    }
    /// Inverse standard normal (Acklam), polished with one Halley step.
    pub fn normInv(p: f64) -> f64 {
        let a: Vec<f64> = vec![-3.969683028665376e+01, 2.209460984245205e+02, -2.759285104469687e+02, 1.383577518672690e+02, -3.066479806614716e+01, 2.506628277459239e+00];
        let b: Vec<f64> = vec![-5.447609879822406e+01, 1.615858368580409e+02, -1.556989798598866e+02, 6.680131188771972e+01, -1.328068155288572e+01];
        let c: Vec<f64> = vec![-7.784894002430293e-03, -3.223964580411365e-01, -2.400758277161838e+00, -2.549732539343734e+00, 4.374664141464968e+00, 2.938163982698783e+00];
        let d: Vec<f64> = vec![7.784695709041462e-03, 3.224671290700398e-01, 2.445134137142996e+00, 3.754408661907416e+00];
        let plow: f64 = 0.02425;
        let mut x: f64 = 0.0;
        if p < plow {
            let q: f64 = FormulaStats::sqrtD(-2.0 * FormulaStats::ln(p));
            x = (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0);
        } else if p <= 1.0 - plow {
            let q: f64 = p - 0.5;
            let r: f64 = q * q;
            x = (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0);
        } else {
            let q: f64 = FormulaStats::sqrtD(-2.0 * FormulaStats::ln(1.0 - p));
            x = 0.0 - (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0);
        }
        let mut k: i64 = 0;
        while k < 2 {
            let e: f64 = FormulaStats::normCdf(x) - p;
            let u: f64 = e * 2.5066282746310002 * FormulaStats::exp(x * x / 2.0);
            x = x - u / (1.0 + x * u / 2.0);
            k += 1;
        }
        x
    }
    /// The CDF of a distribution named by `kind`:
    /// 1 chi-squared(a) 2 Student t(a) 3 F(a, b) 4 gamma(a, b) 5 beta(a, b)
    pub fn cdf(kind: i64, x: f64, a: f64, b: f64) -> f64 {
        if kind == 1 {
            return FormulaStats::gammaP(a / 2.0, x / 2.0);
        }
        if kind == 2 {
            let ib: f64 = FormulaStats::betaI(a / 2.0, 0.5, a / (a + x * x));
            if x > 0.0 {
                return 1.0 - 0.5 * ib;
            }
            return 0.5 * ib;
        }
        if kind == 3 {
            if x <= 0.0 {
                return 0.0;
            }
            return FormulaStats::betaI(a / 2.0, b / 2.0, a * x / (a * x + b));
        }
        if kind == 4 {
            return FormulaStats::gammaP(a, x / b);
        }
        FormulaStats::betaI(a, b, x)
    }
    /// x with cdf(x) = p, by bisection over a bracket that grows until it
    /// holds the answer.
    pub fn invCdf(kind: i64, p: f64, a: f64, b: f64) -> f64 {
        let mut lo: f64 = 0.0;
        let mut hi: f64 = 1.0;
        if kind == 2 {
            lo = -1.0;
            while FormulaStats::cdf(kind, lo, a, b) > p && lo > -1.0e10 {
                lo *= 2.0;
            }
        }
        if kind != 5 {
            while FormulaStats::cdf(kind, hi, a, b) < p && hi < 1.0e10 {
                hi *= 2.0;
            }
        }
        let mut k: i64 = 0;
        while k < 200 {
            let mid: f64 = (lo + hi) / 2.0;
            if FormulaStats::cdf(kind, mid, a, b) < p {
                lo = mid;
            } else {
                hi = mid;
            }
            if hi - lo <= 1.0e-15 * FormulaStats::absD(mid) {
                break;
            }
            k += 1;
        }
        (lo + hi) / 2.0
    }
    pub fn lnCombin(n: f64, k: f64) -> f64 {
        FormulaStats::lnGamma(n + 1.0) - FormulaStats::lnGamma(k + 1.0) - FormulaStats::lnGamma(n - k + 1.0)
    }
    pub fn binomPmf(k: f64, n: f64, p: f64) -> f64 {
        if p == 0.0 {
            if k == 0.0 {
                return 1.0;
            }
            return 0.0;
        }
        if p == 1.0 {
            if k == n {
                return 1.0;
            }
            return 0.0;
        }
        FormulaStats::exp(FormulaStats::lnCombin(n, k) + k * FormulaStats::ln(p) + (n - k) * FormulaStats::ln(1.0 - p))
    }
    pub fn binomCdf(k: f64, n: f64, p: f64) -> f64 {
        let mut s: f64 = 0.0;
        let mut i: f64 = 0.0;
        while i <= k {
            s += FormulaStats::binomPmf(i, n, p);
            i += 1.0;
        }
        if s > 1.0 {
            return 1.0;
        }
        s
    }
    pub fn poissonPmf(k: f64, l: f64) -> f64 {
        if l == 0.0 {
            if k == 0.0 {
                return 1.0;
            }
            return 0.0;
        }
        FormulaStats::exp(0.0 - l + k * FormulaStats::ln(l) - FormulaStats::lnGamma(k + 1.0))
    }
    pub fn hypPmf(s: f64, n: f64, m: f64, pop: f64) -> f64 {
        FormulaStats::exp(FormulaStats::lnCombin(m, s) + FormulaStats::lnCombin(pop - m, n - s) - FormulaStats::lnCombin(pop, n))
    }
    pub fn tPdf(x: f64, v: f64) -> f64 {
        FormulaStats::exp(FormulaStats::lnGamma((v + 1.0) / 2.0) - FormulaStats::lnGamma(v / 2.0)) / FormulaStats::sqrtD(v * FormulaMore::PI()) * FormulaStats::pow(1.0 + x * x / v, 0.0 - (v + 1.0) / 2.0)
    }
    pub fn chiPdf(x: f64, k: f64) -> f64 {
        if x <= 0.0 {
            return 0.0;
        }
        FormulaStats::exp((k / 2.0 - 1.0) * FormulaStats::ln(x) - x / 2.0 - (k / 2.0) * FormulaStats::ln(2.0) - FormulaStats::lnGamma(k / 2.0))
    }
    pub fn gammaPdf(x: f64, a: f64, b: f64) -> f64 {
        if x < 0.0 {
            return 0.0;
        }
        if x == 0.0 {
            if a == 1.0 {
                return 1.0 / b;
            }
            return 0.0;
        }
        FormulaStats::exp((a - 1.0) * FormulaStats::ln(x) - x / b - a * FormulaStats::ln(b) - FormulaStats::lnGamma(a))
    }
    pub fn fPdf(x: f64, d1: f64, d2: f64) -> f64 {
        if x <= 0.0 {
            return 0.0;
        }
        let l: f64 = 0.5 * (d1 * FormulaStats::ln(d1 * x) + d2 * FormulaStats::ln(d2) - (d1 + d2) * FormulaStats::ln(d1 * x + d2)) - FormulaStats::ln(x) - FormulaStats::lnBeta(d1 / 2.0, d2 / 2.0);
        FormulaStats::exp(l)
    }
    pub fn betaPdf(x: f64, a: f64, b: f64) -> f64 {
        if x <= 0.0 || x >= 1.0 {
            if x == 0.0 && a == 1.0 {
                return FormulaStats::exp(0.0 - FormulaStats::lnBeta(a, b));
            }
            if x == 1.0 && b == 1.0 {
                return FormulaStats::exp(0.0 - FormulaStats::lnBeta(a, b));
            }
            return 0.0;
        }
        FormulaStats::exp((a - 1.0) * FormulaStats::ln(x) + (b - 1.0) * FormulaStats::ln(1.0 - x) - FormulaStats::lnBeta(a, b))
    }

    // --- argument helpers ----------------------------------------------------------

    pub fn num(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64, dflt: f64) -> f64 {
        FormulaMore::num(args, i, dflt)
    }
    pub fn flag(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64, dflt: bool) -> bool {
        if !FormulaMore::has(args, i) {
            return dflt;
        }
        let b: bool = args[i as usize].borrow().single().borrow().isTrue();
        b
    }
    pub fn err(e: &str) -> Rc<RefCell<FormulaValue>> {
        FormulaValue::fromError(e.to_string())
    }
    pub fn numV(x: f64) -> Rc<RefCell<FormulaValue>> {
        FormulaValue::fromNumber(x)
    }
    /// The numbers of argument i (an array's numbers, or one value).
    pub fn numbers(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64) -> Vec<f64> {
        if i >= (args.len() as i64) {
            let empty: Vec<f64> = Vec::new();
            return empty;
        }
        FormulaMore::numbersOf(&args[i as usize].clone())
    }
    pub fn mean(v: &Vec<f64>) -> f64 {
        let mut s: f64 = 0.0;
        for x in v.iter() {
            s += *x;
        }
        if v.is_empty() {
            return 0.0;
        }
        s / (v.len() as f64)
    }
    /// Sum of squared deviations.
    pub fn ssd(v: &Vec<f64>) -> f64 {
        let m: f64 = FormulaStats::mean(v);
        let mut s: f64 = 0.0;
        for x in v.iter() {
            s += (*x - m) * (*x - m);
        }
        s
    }
    /// Values for the A-functions (STDEVA, VARA ...): in a range, text and
    /// FALSE count as 0 and TRUE as 1; blanks are skipped.
    pub fn valuesA(args: &Vec<Rc<RefCell<FormulaValue>>>) -> Vec<f64> {
        let mut out: Vec<f64> = Vec::new();
        for a in args.iter() {
            for x in FormulaMore::flatOf(a).iter() {
                let k: i64 = x.borrow().kind;
                if k == 1 {
                    out.push(x.borrow().num);
                } else if k == 3 {
                    out.push(if x.borrow().flag { 1.0 } else { 0.0 });
                } else if k == 2 {
                    if a.borrow().isArray() {
                        out.push(0.0);
                    } else {
                        if let Some(v) = FormulaValue::parseNumberText(&x.borrow().text.clone()) {
                            out.push(v);
                        } else {
                            out.push(0.0);
                        }
                    }
                }
            }
        }
        out
    }
    pub fn column(vals: &Vec<f64>) -> Rc<RefCell<FormulaValue>> {
        let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        for x in vals.iter() {
            items.push(FormulaValue::fromNumber(*x));
        }
        let n: i64 = items.len() as i64;
        FormulaValue::fromArray(n, 1, &items)
    }

    // --- dispatch ------------------------------------------------------------------

    pub fn invoke(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if !FormulaStats::isKnown(n) {
            return FormulaValue::blank();
        }
        // Database functions, text and a few others look at errors themselves.
        let r0: Rc<RefCell<FormulaValue>> = FormulaStats::otherFn(n, args);
        if !r0.borrow().isBlank() {
            return r0;
        }
        let e: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        if e.borrow().isError() {
            return e;
        }
        let r1: Rc<RefCell<FormulaValue>> = FormulaStats::distFn(n, args);
        if !r1.borrow().isBlank() {
            return r1;
        }
        let r2: Rc<RefCell<FormulaValue>> = FormulaStats::sampleFn(n, args);
        if !r2.borrow().isBlank() {
            return r2;
        }
        let r3: Rc<RefCell<FormulaValue>> = FormulaStats::mathFn(n, args);
        if !r3.borrow().isBlank() {
            return r3;
        }
        FormulaStats::moneyFn(n, args)
    }

    // --- distributions -------------------------------------------------------------

    pub fn distFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let na: i64 = args.len() as i64;
        let x: f64 = FormulaStats::num(args, 0, 0.0);
        if n == "NORM.DIST" || n == "NORMDIST" {
            if na < 4 {
                return FormulaStats::err("#VALUE!");
            }
            let m: f64 = FormulaStats::num(args, 1, 0.0);
            let s: f64 = FormulaStats::num(args, 2, 1.0);
            if s <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            let z: f64 = (x - m) / s;
            if FormulaStats::flag(args, 3, false) {
                return FormulaStats::numV(FormulaStats::normCdf(z));
            }
            return FormulaStats::numV(FormulaStats::normPdf(z) / s);
        }
        if n == "NORM.S.DIST" {
            if na < 2 {
                return FormulaStats::err("#VALUE!");
            }
            if FormulaStats::flag(args, 1, false) {
                return FormulaStats::numV(FormulaStats::normCdf(x));
            }
            return FormulaStats::numV(FormulaStats::normPdf(x));
        }
        if n == "NORMSDIST" {
            return FormulaStats::numV(FormulaStats::normCdf(x));
        }
        if n == "PHI" {
            return FormulaStats::numV(FormulaStats::normPdf(x));
        }
        if n == "GAUSS" {
            return FormulaStats::numV(FormulaStats::normCdf(x) - 0.5);
        }
        if n == "NORM.INV" || n == "NORMINV" || n == "NORM.S.INV" || n == "NORMSINV" {
            if x <= 0.0 || x >= 1.0 {
                return FormulaStats::err("#NUM!");
            }
            let z: f64 = FormulaStats::normInv(x);
            if n == "NORM.S.INV" || n == "NORMSINV" {
                return FormulaStats::numV(z);
            }
            if na < 3 {
                return FormulaStats::err("#VALUE!");
            }
            let s: f64 = FormulaStats::num(args, 2, 1.0);
            if s <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(FormulaStats::num(args, 1, 0.0) + s * z);
        }
        if n == "LOGNORM.DIST" || n == "LOGNORMDIST" {
            let m: f64 = FormulaStats::num(args, 1, 0.0);
            let s: f64 = FormulaStats::num(args, 2, 1.0);
            if x <= 0.0 || s <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            let z: f64 = (FormulaStats::ln(x) - m) / s;
            if n == "LOGNORMDIST" || FormulaStats::flag(args, 3, false) {
                return FormulaStats::numV(FormulaStats::normCdf(z));
            }
            return FormulaStats::numV(FormulaStats::normPdf(z) / (x * s));
        }
        if n == "LOGNORM.INV" || n == "LOGINV" {
            let s: f64 = FormulaStats::num(args, 2, 1.0);
            if x <= 0.0 || x >= 1.0 || s <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(FormulaStats::exp(FormulaStats::num(args, 1, 0.0) + s * FormulaStats::normInv(x)));
        }
        if n == "EXPON.DIST" || n == "EXPONDIST" {
            let l: f64 = FormulaStats::num(args, 1, 1.0);
            if x < 0.0 || l <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            if FormulaStats::flag(args, 2, false) {
                return FormulaStats::numV(1.0 - FormulaStats::exp(0.0 - l * x));
            }
            return FormulaStats::numV(l * FormulaStats::exp(0.0 - l * x));
        }
        if n == "POISSON.DIST" || n == "POISSON" {
            let k: f64 = x.floor();
            let l: f64 = FormulaStats::num(args, 1, 0.0);
            if k < 0.0 || l < 0.0 {
                return FormulaStats::err("#NUM!");
            }
            if FormulaStats::flag(args, 2, false) {
                if l == 0.0 {
                    return FormulaStats::numV(1.0);
                }
                return FormulaStats::numV(FormulaStats::gammaQ(k + 1.0, l));
            }
            return FormulaStats::numV(FormulaStats::poissonPmf(k, l));
        }
        if n == "BINOM.DIST" || n == "BINOMDIST" {
            let k: f64 = x.floor();
            let tn: f64 = FormulaStats::num(args, 1, 0.0).floor();
            let p: f64 = FormulaStats::num(args, 2, 0.0);
            if k < 0.0 || k > tn || p < 0.0 || p > 1.0 {
                return FormulaStats::err("#NUM!");
            }
            if FormulaStats::flag(args, 3, false) {
                return FormulaStats::numV(FormulaStats::binomCdf(k, tn, p));
            }
            return FormulaStats::numV(FormulaStats::binomPmf(k, tn, p));
        }
        if n == "BINOM.DIST.RANGE" {
            let tn: f64 = x.floor();
            let p: f64 = FormulaStats::num(args, 1, 0.0);
            let s1: f64 = FormulaStats::num(args, 2, 0.0).floor();
            let mut s2: f64 = s1;
            if FormulaMore::has(args, 3) {
                s2 = FormulaStats::num(args, 3, 0.0).floor();
            }
            if p < 0.0 || p > 1.0 || s1 < 0.0 || s1 > tn || s2 < s1 || s2 > tn {
                return FormulaStats::err("#NUM!");
            }
            let mut s: f64 = 0.0;
            let mut i: f64 = s1;
            while i <= s2 {
                s += FormulaStats::binomPmf(i, tn, p);
                i += 1.0;
            }
            return FormulaStats::numV(s);
        }
        if n == "BINOM.INV" || n == "CRITBINOM" {
            let tn: f64 = x.floor();
            let p: f64 = FormulaStats::num(args, 1, 0.0);
            let alpha: f64 = FormulaStats::num(args, 2, 0.0);
            if tn < 0.0 || p < 0.0 || p > 1.0 || alpha <= 0.0 || alpha >= 1.0 {
                return FormulaStats::err("#NUM!");
            }
            let mut s: f64 = 0.0;
            let mut k: f64 = 0.0;
            while k <= tn {
                s += FormulaStats::binomPmf(k, tn, p);
                if s >= alpha * (1.0 - 1.0e-14) {
                    return FormulaStats::numV(k);
                }
                k += 1.0;
            }
            return FormulaStats::numV(tn);
        }
        if n == "NEGBINOM.DIST" || n == "NEGBINOMDIST" {
            let f: f64 = x.floor();
            let r: f64 = FormulaStats::num(args, 1, 0.0).floor();
            let p: f64 = FormulaStats::num(args, 2, 0.0);
            if f < 0.0 || r < 1.0 || p <= 0.0 || p >= 1.0 {
                return FormulaStats::err("#NUM!");
            }
            if n == "NEGBINOM.DIST" && FormulaStats::flag(args, 3, false) {
                return FormulaStats::numV(FormulaStats::betaI(r, f + 1.0, p));
            }
            return FormulaStats::numV(FormulaStats::exp(FormulaStats::lnCombin(f + r - 1.0, f) + r * FormulaStats::ln(p) + f * FormulaStats::ln(1.0 - p)));
        }
        if n == "HYPGEOM.DIST" || n == "HYPGEOMDIST" {
            let s: f64 = x.floor();
            let sn: f64 = FormulaStats::num(args, 1, 0.0).floor();
            let m: f64 = FormulaStats::num(args, 2, 0.0).floor();
            let pop: f64 = FormulaStats::num(args, 3, 0.0).floor();
            let mut lo: f64 = sn - pop + m;
            if lo < 0.0 {
                lo = 0.0;
            }
            if s < 0.0 || s > sn || s > m || sn > pop || m > pop || sn <= 0.0 || m <= 0.0 || pop <= 0.0 || s < lo {
                return FormulaStats::err("#NUM!");
            }
            if n == "HYPGEOM.DIST" && FormulaStats::flag(args, 4, false) {
                let mut t: f64 = 0.0;
                let mut i: f64 = lo;
                while i <= s {
                    t += FormulaStats::hypPmf(i, sn, m, pop);
                    i += 1.0;
                }
                return FormulaStats::numV(t);
            }
            return FormulaStats::numV(FormulaStats::hypPmf(s, sn, m, pop));
        }
        if n == "WEIBULL.DIST" || n == "WEIBULL" {
            let a: f64 = FormulaStats::num(args, 1, 0.0);
            let b: f64 = FormulaStats::num(args, 2, 0.0);
            if x < 0.0 || a <= 0.0 || b <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            let t: f64 = FormulaStats::exp(0.0 - FormulaStats::pow(x / b, a));
            if FormulaStats::flag(args, 3, false) {
                return FormulaStats::numV(1.0 - t);
            }
            return FormulaStats::numV(a / FormulaStats::pow(b, a) * FormulaStats::pow(x, a - 1.0) * t);
        }
        if n == "GAMMA" {
            let xi: f64 = x.floor();
            if x == 0.0 || (x < 0.0 && xi == x) {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(FormulaStats::gammaFn(x));
        }
        if n == "GAMMALN" || n == "GAMMALN.PRECISE" {
            if x <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(FormulaStats::lnGamma(x));
        }
        if n == "GAMMA.DIST" || n == "GAMMADIST" {
            let a: f64 = FormulaStats::num(args, 1, 0.0);
            let b: f64 = FormulaStats::num(args, 2, 0.0);
            if x < 0.0 || a <= 0.0 || b <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            if FormulaStats::flag(args, 3, false) {
                return FormulaStats::numV(FormulaStats::gammaP(a, x / b));
            }
            return FormulaStats::numV(FormulaStats::gammaPdf(x, a, b));
        }
        if n == "GAMMA.INV" || n == "GAMMAINV" {
            let a: f64 = FormulaStats::num(args, 1, 0.0);
            let b: f64 = FormulaStats::num(args, 2, 0.0);
            if x < 0.0 || x >= 1.0 || a <= 0.0 || b <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            if x == 0.0 {
                return FormulaStats::numV(0.0);
            }
            return FormulaStats::numV(FormulaStats::invCdf(4, x, a, b));
        }
        if n == "BETA.DIST" || n == "BETADIST" {
            let a: f64 = FormulaStats::num(args, 1, 0.0);
            let b: f64 = FormulaStats::num(args, 2, 0.0);
            let mut ai: i64 = 3;
            let mut cumulative: bool = true;
            if n == "BETA.DIST" {
                cumulative = FormulaStats::flag(args, 3, false);
                ai = 4;
            }
            let lo: f64 = FormulaStats::num(args, ai, 0.0);
            let hi: f64 = FormulaStats::num(args, ai + 1, 1.0);
            if a <= 0.0 || b <= 0.0 || x < lo || x > hi || lo >= hi {
                return FormulaStats::err("#NUM!");
            }
            let t: f64 = (x - lo) / (hi - lo);
            if cumulative {
                return FormulaStats::numV(FormulaStats::betaI(a, b, t));
            }
            return FormulaStats::numV(FormulaStats::betaPdf(t, a, b) / (hi - lo));
        }
        if n == "BETA.INV" || n == "BETAINV" {
            let a: f64 = FormulaStats::num(args, 1, 0.0);
            let b: f64 = FormulaStats::num(args, 2, 0.0);
            let lo: f64 = FormulaStats::num(args, 3, 0.0);
            let hi: f64 = FormulaStats::num(args, 4, 1.0);
            if x <= 0.0 || x > 1.0 || a <= 0.0 || b <= 0.0 || lo >= hi {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(lo + (hi - lo) * FormulaStats::invCdf(5, x, a, b));
        }
        if n == "CHISQ.DIST" || n == "CHISQ.DIST.RT" || n == "CHIDIST" {
            let k: f64 = FormulaStats::num(args, 1, 0.0).floor();
            if x < 0.0 || k < 1.0 {
                return FormulaStats::err("#NUM!");
            }
            if n == "CHISQ.DIST" {
                if FormulaStats::flag(args, 2, false) {
                    return FormulaStats::numV(FormulaStats::gammaP(k / 2.0, x / 2.0));
                }
                return FormulaStats::numV(FormulaStats::chiPdf(x, k));
            }
            return FormulaStats::numV(FormulaStats::gammaQ(k / 2.0, x / 2.0));
        }
        if n == "CHISQ.INV" || n == "CHISQ.INV.RT" || n == "CHIINV" {
            let k: f64 = FormulaStats::num(args, 1, 0.0).floor();
            if x < 0.0 || x > 1.0 || k < 1.0 {
                return FormulaStats::err("#NUM!");
            }
            let mut p: f64 = x;
            if n != "CHISQ.INV" {
                p = 1.0 - x;
            }
            if p <= 0.0 {
                return FormulaStats::numV(0.0);
            }
            if p >= 1.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(FormulaStats::invCdf(1, p, k, 0.0));
        }
        if n == "T.DIST" || n == "T.DIST.2T" || n == "T.DIST.RT" || n == "TDIST" {
            let v: f64 = FormulaStats::num(args, 1, 0.0).floor();
            if v < 1.0 {
                return FormulaStats::err("#NUM!");
            }
            if n == "T.DIST" {
                if FormulaStats::flag(args, 2, false) {
                    return FormulaStats::numV(FormulaStats::cdf(2, x, v, 0.0));
                }
                return FormulaStats::numV(FormulaStats::tPdf(x, v));
            }
            if n == "T.DIST.RT" {
                return FormulaStats::numV(1.0 - FormulaStats::cdf(2, x, v, 0.0));
            }
            if x < 0.0 {
                return FormulaStats::err("#NUM!");
            }
            let mut tails: f64 = 2.0;
            if n == "TDIST" {
                tails = FormulaStats::num(args, 2, 0.0).floor();
                if tails != 1.0 && tails != 2.0 {
                    return FormulaStats::err("#NUM!");
                }
            }
            return FormulaStats::numV(tails * (1.0 - FormulaStats::cdf(2, x, v, 0.0)));
        }
        if n == "T.INV" || n == "T.INV.2T" || n == "TINV" {
            let v: f64 = FormulaStats::num(args, 1, 0.0).floor();
            if x <= 0.0 || x > 1.0 || v < 1.0 {
                return FormulaStats::err("#NUM!");
            }
            if n == "T.INV" {
                if x >= 1.0 {
                    return FormulaStats::err("#NUM!");
                }
                return FormulaStats::numV(FormulaStats::invCdf(2, x, v, 0.0));
            }
            return FormulaStats::numV(FormulaStats::invCdf(2, 1.0 - x / 2.0, v, 0.0));
        }
        if n == "F.DIST" || n == "F.DIST.RT" || n == "FDIST" {
            let d1: f64 = FormulaStats::num(args, 1, 0.0).floor();
            let d2: f64 = FormulaStats::num(args, 2, 0.0).floor();
            if x < 0.0 || d1 < 1.0 || d2 < 1.0 {
                return FormulaStats::err("#NUM!");
            }
            if n == "F.DIST" {
                if FormulaStats::flag(args, 3, false) {
                    return FormulaStats::numV(FormulaStats::cdf(3, x, d1, d2));
                }
                return FormulaStats::numV(FormulaStats::fPdf(x, d1, d2));
            }
            return FormulaStats::numV(1.0 - FormulaStats::cdf(3, x, d1, d2));
        }
        if n == "F.INV" || n == "F.INV.RT" || n == "FINV" {
            let d1: f64 = FormulaStats::num(args, 1, 0.0).floor();
            let d2: f64 = FormulaStats::num(args, 2, 0.0).floor();
            if x < 0.0 || x > 1.0 || d1 < 1.0 || d2 < 1.0 {
                return FormulaStats::err("#NUM!");
            }
            let mut p: f64 = x;
            if n != "F.INV" {
                p = 1.0 - x;
            }
            if p <= 0.0 {
                return FormulaStats::numV(0.0);
            }
            if p >= 1.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(FormulaStats::invCdf(3, p, d1, d2));
        }
        if n == "CONFIDENCE" || n == "CONFIDENCE.NORM" || n == "CONFIDENCE.T" {
            let s: f64 = FormulaStats::num(args, 1, 0.0);
            let sz: f64 = FormulaStats::num(args, 2, 0.0).floor();
            if x <= 0.0 || x >= 1.0 || s <= 0.0 || sz < 1.0 {
                return FormulaStats::err("#NUM!");
            }
            if n == "CONFIDENCE.T" {
                if sz < 2.0 {
                    return FormulaStats::err("#DIV/0!");
                }
                return FormulaStats::numV(FormulaStats::invCdf(2, 1.0 - x / 2.0, sz - 1.0, 0.0) * s / FormulaStats::sqrtD(sz));
            }
            return FormulaStats::numV(FormulaStats::normInv(1.0 - x / 2.0) * s / FormulaStats::sqrtD(sz));
        }
        if n == "ERF" || n == "ERF.PRECISE" {
            if n == "ERF" && FormulaMore::has(args, 1) {
                return FormulaStats::numV(FormulaStats::erf(FormulaStats::num(args, 1, 0.0)) - FormulaStats::erf(x));
            }
            return FormulaStats::numV(FormulaStats::erf(x));
        }
        if n == "ERFC" || n == "ERFC.PRECISE" {
            return FormulaStats::numV(FormulaStats::erfc(x));
        }
        if n == "FISHER" {
            if x <= -1.0 || x >= 1.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(0.5 * FormulaStats::ln((1.0 + x) / (1.0 - x)));
        }
        if n == "FISHERINV" {
            let e2: f64 = FormulaStats::exp(2.0 * x);
            return FormulaStats::numV((e2 - 1.0) / (e2 + 1.0));
        }
        FormulaValue::blank()
    }

    // --- samples, tests and regression ------------------------------------------

    pub fn moments(v: &Vec<f64>, power: i64, sd: f64) -> f64 {
        let m: f64 = FormulaStats::mean(v);
        let mut s: f64 = 0.0;
        for x in v.iter() {
            let z: f64 = (*x - m) / sd;
            let mut t: f64 = 1.0;
            let mut k: i64 = 0;
            while k < power {
                t *= z;
                k += 1;
            }
            s += t;
        }
        s
    }
    /// Least squares through the points (x, y): out[0] slope, out[1]
    /// intercept, then for LINEST's statistics se slope, se intercept, r2,
    /// se y, F, df, ss reg, ss resid.
    pub fn fitLine(xs: &Vec<f64>, ys: &Vec<f64>, with_const: bool) -> Vec<f64> {
        let n: f64 = xs.len() as f64;
        let mut mx: f64 = 0.0;
        let mut my: f64 = 0.0;
        if with_const {
            mx = FormulaStats::mean(xs);
            my = FormulaStats::mean(ys);
        }
        let mut sxx: f64 = 0.0;
        let mut sxy: f64 = 0.0;
        let mut i: i64 = 0;
        while i < (xs.len() as i64) {
            let dx: f64 = xs[i as usize] - mx;
            sxx += dx * dx;
            sxy += dx * (ys[i as usize] - my);
            i += 1;
        }
        let mut out: Vec<f64> = Vec::new();
        if sxx == 0.0 {
            return out;
        }
        let m: f64 = sxy / sxx;
        let b: f64 = my - m * mx;
        let mut ssres: f64 = 0.0;
        let mut sstot: f64 = 0.0;
        i = 0;
        while i < (xs.len() as i64) {
            let e: f64 = ys[i as usize] - (m * xs[i as usize] + b);
            ssres += e * e;
            let d: f64 = ys[i as usize] - my;
            sstot += d * d;
            i += 1;
        }
        let mut df: f64 = n - 1.0;
        if with_const {
            df = n - 2.0;
        }
        let ssreg: f64 = sstot - ssres;
        let mut sey: f64 = 0.0;
        if df > 0.0 {
            sey = FormulaStats::sqrtD(ssres / df);
        }
        let sem: f64 = sey / FormulaStats::sqrtD(sxx);
        let mut seb: f64 = 0.0;
        if with_const {
            let mut sx2: f64 = 0.0;
            for x in xs.iter() {
                sx2 += *x * *x;
            }
            seb = sey * FormulaStats::sqrtD(sx2 / (n * sxx));
        }
        let mut r2: f64 = 1.0;
        if sstot > 0.0 {
            r2 = ssreg / sstot;
        }
        let mut f: f64 = 0.0;
        if ssres > 0.0 {
            f = ssreg / (ssres / df);
        }
        out.push(m);
        out.push(b);
        out.push(sem);
        out.push(seb);
        out.push(r2);
        out.push(sey);
        out.push(f);
        out.push(df);
        out.push(ssreg);
        out.push(ssres);
        out
    }
    /// LINEST/LOGEST/TREND/GROWTH with several x variables: known_x with as
    /// many rows (y a column) or columns (y a row) as known_y and more than
    /// one in the other direction. Blank when the shapes are a single x.
    pub fn multiFit(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>, log_y: bool) -> Rc<RefCell<FormulaValue>> {
        if (args.len() as i64) < 2 || !FormulaMore::has(args, 1) {
            return FormulaValue::blank();
        }
        let yg: Rc<RefCell<FormulaValue>> = FormulaMore::grid(&args[0].clone());
        let xg: Rc<RefCell<FormulaValue>> = FormulaMore::grid(&args[1].clone());
        let yr: i64 = yg.borrow().rows;
        let yc: i64 = yg.borrow().cols;
        let xr: i64 = xg.borrow().rows;
        let xc: i64 = xg.borrow().cols;
        let mut by_col: bool = false;
        if yc == 1 && yr > 1 && xr == yr && xc > 1 {
            by_col = true;
        } else if !(yr == 1 && yc > 1 && xc == yc && xr > 1) {
            return FormulaValue::blank();
        }
        let cnt: i64 = yr * yc;
        let mut k: i64 = xr;
        if by_col {
            k = xc;
        }
        let mut ys: Vec<f64> = Vec::new();
        let mut xs: Vec<f64> = Vec::new();
        let mut i: i64 = 0;
        while i < cnt {
            let y: Rc<RefCell<FormulaValue>> = yg.borrow().items[i as usize].clone();
            if y.borrow().kind != 1 {
                return FormulaStats::err("#VALUE!");
            }
            let mut yy: f64 = y.borrow().num;
            if log_y {
                if yy <= 0.0 {
                    return FormulaStats::err("#NUM!");
                }
                yy = FormulaStats::ln(yy);
            }
            ys.push(yy);
            let mut j: i64 = 0;
            while j < k {
                let mut at: i64 = j * xc + i;
                if by_col {
                    at = i * xc + j;
                }
                let x: Rc<RefCell<FormulaValue>> = xg.borrow().items[at as usize].clone();
                if x.borrow().kind != 1 {
                    return FormulaStats::err("#VALUE!");
                }
                xs.push(x.borrow().num);
                j += 1;
            }
            i += 1;
        }
        let mut ci: i64 = 3;
        if n == "LINEST" || n == "LOGEST" {
            ci = 2;
        }
        let with_const: bool = FormulaStats::flag(args, ci, true);
        // centred columns (with a constant), normal equations, Gauss-Jordan inverse
        let nf: f64 = cnt as f64;
        let mut mx: Vec<f64> = Vec::new();
        let mut j: i64 = 0;
        while j < k {
            let mut sm: f64 = 0.0;
            i = 0;
            while i < cnt {
                sm += xs[(i * k + j) as usize];
                i += 1;
            }
            if with_const {
                mx.push(sm / nf);
            } else {
                mx.push(0.0);
            }
            j += 1;
        }
        let mut my: f64 = 0.0;
        if with_const {
            my = FormulaStats::mean(&ys);
        }
        // augmented [A | I | c] with A = Xc'Xc, c = Xc'yc
        let w: i64 = 2 * k + 1;
        let mut m: Vec<f64> = Vec::new();
        let mut r: i64 = 0;
        while r < k {
            let mut c: i64 = 0;
            while c < k {
                let mut sm: f64 = 0.0;
                i = 0;
                while i < cnt {
                    sm += (xs[(i * k + r) as usize] - mx[r as usize]) * (xs[(i * k + c) as usize] - mx[c as usize]);
                    i += 1;
                }
                m.push(sm);
                c += 1;
            }
            c = 0;
            while c < k {
                if c == r {
                    m.push(1.0);
                } else {
                    m.push(0.0);
                }
                c += 1;
            }
            let mut sy: f64 = 0.0;
            i = 0;
            while i < cnt {
                sy += (xs[(i * k + r) as usize] - mx[r as usize]) * (ys[i as usize] - my);
                i += 1;
            }
            m.push(sy);
            r += 1;
        }
        let mut dropped: Vec<bool> = Vec::new();
        let mut scale: f64 = 0.0;
        r = 0;
        while r < k {
            dropped.push(false);
            let d: f64 = FormulaStats::absD(m[(r * w + r) as usize]);
            if d > scale {
                scale = d;
            }
            r += 1;
        }
        let mut p: i64 = 0;
        while p < k {
            let piv: f64 = m[(p * w + p) as usize];
            if FormulaStats::absD(piv) <= scale * 1.0e-13 {
                // collinear with the earlier columns: Excel leaves it out (0)
                dropped[p as usize] = true;
                let mut c: i64 = 0;
                while c < w {
                    m[(p * w + c) as usize] = 0.0;
                    c += 1;
                }
                r = 0;
                while r < k {
                    m[(r * w + p) as usize] = 0.0;
                    m[(r * w + k + p) as usize] = 0.0;
                    r += 1;
                }
                p += 1;
                continue;
            }
            let mut c: i64 = 0;
            while c < w {
                m[(p * w + c) as usize] = m[(p * w + c) as usize] / piv;
                c += 1;
            }
            r = 0;
            while r < k {
                if r != p {
                    let f: f64 = m[(r * w + p) as usize];
                    if f != 0.0 {
                        c = 0;
                        while c < w {
                            m[(r * w + c) as usize] = m[(r * w + c) as usize] - f * m[(p * w + c) as usize];
                            c += 1;
                        }
                    }
                }
                r += 1;
            }
            p += 1;
        }
        let mut beta: Vec<f64> = Vec::new();
        let mut used: f64 = 0.0;
        let mut b0: f64 = my;
        j = 0;
        while j < k {
            let bj: f64 = m[(j * w + 2 * k) as usize];
            beta.push(bj);
            b0 -= bj * mx[j as usize];
            if !dropped[j as usize] {
                used += 1.0;
            }
            j += 1;
        }
        if !with_const {
            b0 = 0.0;
        }
        if n == "TREND" || n == "GROWTH" {
            let mut nx: Rc<RefCell<FormulaValue>> = xg.clone();
            if FormulaMore::has(args, 2) {
                nx = FormulaMore::grid(&args[2].clone());
            }
            let nr: i64 = nx.borrow().rows;
            let nc: i64 = nx.borrow().cols;
            if (by_col && nc != k) || (!by_col && nr != k) {
                return FormulaStats::err("#REF!");
            }
            let mut pts: i64 = nr;
            if !by_col {
                pts = nc;
            }
            let mut out: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            i = 0;
            while i < pts {
                let mut y: f64 = b0;
                j = 0;
                while j < k {
                    let mut at: i64 = j * nc + i;
                    if by_col {
                        at = i * nc + j;
                    }
                    let xv: Rc<RefCell<FormulaValue>> = nx.borrow().items[at as usize].clone();
                    if xv.borrow().kind != 1 {
                        return FormulaStats::err("#VALUE!");
                    }
                    y += beta[j as usize] * xv.borrow().num;
                    j += 1;
                }
                if log_y {
                    y = FormulaStats::exp(y);
                }
                out.push(FormulaValue::fromNumber(y));
                i += 1;
            }
            if by_col {
                return FormulaValue::fromArray(pts, 1, &out);
            }
            return FormulaValue::fromArray(1, pts, &out);
        }
        // LINEST / LOGEST: m_k … m_1, b and optionally the statistics
        let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        j = k - 1;
        while j >= 0 {
            let mut v: f64 = beta[j as usize];
            if log_y {
                v = FormulaStats::exp(v);
            }
            items.push(FormulaValue::fromNumber(v));
            j -= 1;
        }
        if log_y {
            items.push(FormulaValue::fromNumber(FormulaStats::exp(b0)));
        } else {
            items.push(FormulaValue::fromNumber(b0));
        }
        let cols: i64 = k + 1;
        if !FormulaStats::flag(args, 3, false) {
            return FormulaValue::fromArray(1, cols, &items);
        }
        let mut ssres: f64 = 0.0;
        let mut sstot: f64 = 0.0;
        i = 0;
        while i < cnt {
            let mut fit: f64 = b0;
            j = 0;
            while j < k {
                fit += beta[j as usize] * xs[(i * k + j) as usize];
                j += 1;
            }
            let e: f64 = ys[i as usize] - fit;
            ssres += e * e;
            let d: f64 = ys[i as usize] - my;
            sstot += d * d;
            i += 1;
        }
        let mut df: f64 = nf - used;
        if with_const {
            df -= 1.0;
        }
        let ssreg: f64 = sstot - ssres;
        let mut sey: f64 = 0.0;
        if df > 0.0 {
            sey = FormulaStats::sqrtD(ssres / df);
        }
        j = k - 1;
        while j >= 0 {
            let vjj: f64 = m[(j * w + k + j) as usize];
            items.push(FormulaValue::fromNumber(sey * FormulaStats::sqrtD(FormulaStats::absD(vjj))));
            j -= 1;
        }
        if with_const {
            // var(b) = sey^2 (1/n + mx' V mx)
            let mut q: f64 = 0.0;
            r = 0;
            while r < k {
                let mut c: i64 = 0;
                while c < k {
                    q += mx[r as usize] * m[(r * w + k + c) as usize] * mx[c as usize];
                    c += 1;
                }
                r += 1;
            }
            items.push(FormulaValue::fromNumber(sey * FormulaStats::sqrtD(1.0 / nf + q)));
        } else {
            items.push(FormulaValue::fromError("#N/A".to_string()));
        }
        let mut r2: f64 = 1.0;
        if sstot > 0.0 {
            r2 = ssreg / sstot;
        }
        let mut fstat: f64 = 0.0;
        if ssres > 0.0 && used > 0.0 {
            fstat = (ssreg / used) / (ssres / df);
        }
        let row3: Vec<f64> = vec![r2, sey];
        let row4: Vec<f64> = vec![fstat, df];
        let row5: Vec<f64> = vec![ssreg, ssres];
        let mut c: i64 = 0;
        while c < cols {
            if c < 2 {
                items.push(FormulaValue::fromNumber(row3[c as usize]));
            } else {
                items.push(FormulaValue::fromError("#N/A".to_string()));
            }
            c += 1;
        }
        c = 0;
        while c < cols {
            if c < 2 {
                items.push(FormulaValue::fromNumber(row4[c as usize]));
            } else {
                items.push(FormulaValue::fromError("#N/A".to_string()));
            }
            c += 1;
        }
        c = 0;
        while c < cols {
            if c < 2 {
                items.push(FormulaValue::fromNumber(row5[c as usize]));
            } else {
                items.push(FormulaValue::fromError("#N/A".to_string()));
            }
            c += 1;
        }
        FormulaValue::fromArray(5, cols, &items)
    }
    /// Known y and x from TREND/GROWTH/LINEST arguments: x defaults to
    /// 1, 2, 3 ... Returns false when they do not pair up.
    pub fn knownXY(args: &Vec<Rc<RefCell<FormulaValue>>>, log_y: bool, xs: &mut Vec<f64>, ys: &mut Vec<f64>) -> bool {
        let yv: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[0].clone());
        let mut xv: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let has_x: bool = FormulaMore::has(args, 1);
        if has_x {
            xv = FormulaMore::flatOf(&args[1].clone());
            if xv.len() != yv.len() {
                return false;
            }
        }
        let mut i: i64 = 0;
        while i < (yv.len() as i64) {
            let y: Rc<RefCell<FormulaValue>> = yv[i as usize].clone();
            if y.borrow().kind != 1 {
                return false;
            }
            let mut yy: f64 = y.borrow().num;
            if log_y {
                if yy <= 0.0 {
                    return false;
                }
                yy = FormulaStats::ln(yy);
            }
            let mut xx: f64 = (i + 1) as f64;
            if has_x {
                let x: Rc<RefCell<FormulaValue>> = xv[i as usize].clone();
                if x.borrow().kind != 1 {
                    return false;
                }
                xx = x.borrow().num;
            }
            xs.push(xx);
            ys.push(yy);
            i += 1;
        }
        !ys.is_empty()
    }
    pub fn sampleFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if n == "KURT" || n == "SKEW" || n == "SKEW.P" {
            let v: Vec<f64> = FormulaFunctions::collectNumbers(args);
            let cnt: f64 = v.len() as f64;
            if n == "SKEW.P" {
                if cnt < 1.0 {
                    return FormulaStats::err("#DIV/0!");
                }
                let sdp: f64 = FormulaStats::sqrtD(FormulaStats::ssd(&v) / cnt);
                if sdp == 0.0 {
                    return FormulaStats::err("#DIV/0!");
                }
                return FormulaStats::numV(FormulaStats::moments(&v, 3, sdp) / cnt);
            }
            let need: f64 = if n == "KURT" { 4.0 } else { 3.0 };
            if cnt < need {
                return FormulaStats::err("#DIV/0!");
            }
            let sd: f64 = FormulaStats::sqrtD(FormulaStats::ssd(&v) / (cnt - 1.0));
            if sd == 0.0 {
                return FormulaStats::err("#DIV/0!");
            }
            if n == "SKEW" {
                return FormulaStats::numV(cnt / ((cnt - 1.0) * (cnt - 2.0)) * FormulaStats::moments(&v, 3, sd));
            }
            let a: f64 = cnt * (cnt + 1.0) / ((cnt - 1.0) * (cnt - 2.0) * (cnt - 3.0));
            let b: f64 = 3.0 * (cnt - 1.0) * (cnt - 1.0) / ((cnt - 2.0) * (cnt - 3.0));
            return FormulaStats::numV(a * FormulaStats::moments(&v, 4, sd) - b);
        }
        if n == "STDEVA" || n == "STDEVPA" || n == "VARA" || n == "VARPA" {
            let v: Vec<f64> = FormulaStats::valuesA(args);
            let cnt: f64 = v.len() as f64;
            let pop: bool = n == "STDEVPA" || n == "VARPA";
            let den: f64 = if pop { cnt } else { cnt - 1.0 };
            if den <= 0.0 {
                return FormulaStats::err("#DIV/0!");
            }
            let var: f64 = FormulaStats::ssd(&v) / den;
            if n == "VARA" || n == "VARPA" {
                return FormulaStats::numV(var);
            }
            return FormulaStats::numV(FormulaStats::sqrtD(var));
        }
        if n == "MODE.MULT" {
            let v: Vec<f64> = FormulaFunctions::collectNumbers(args);
            let mut best: i64 = 1;
            let mut counts: Vec<i64> = Vec::new();
            for x in v.iter() {
                let mut c: i64 = 0;
                for y in v.iter() {
                    if *y == *x {
                        c += 1;
                    }
                }
                if c > best {
                    best = c;
                }
                counts.push(c);
            }
            if best < 2 {
                return FormulaStats::err("#N/A");
            }
            let mut modes: Vec<f64> = Vec::new();
            let mut i: i64 = 0;
            while i < (v.len() as i64) {
                if counts[i as usize] == best {
                    let mut seen: bool = false;
                    for m in modes.iter() {
                        if *m == v[i as usize] {
                            seen = true;
                        }
                    }
                    if !seen {
                        modes.push(v[i as usize]);
                    }
                }
                i += 1;
            }
            return FormulaStats::column(&modes);
        }
        if n == "PERCENTRANK.EXC" {
            let sorted: Vec<f64> = FormulaFunctions::sortDoubles(FormulaStats::numbers(args, 0));
            let cnt: i64 = sorted.len() as i64;
            let x: f64 = FormulaStats::num(args, 1, 0.0);
            let digits: i64 = FormulaMore::int(args, 2, 3);
            if cnt == 0 || digits < 1 {
                return FormulaStats::err("#NUM!");
            }
            if x < sorted[0] || x > sorted[(cnt - 1) as usize] {
                return FormulaStats::err("#N/A");
            }
            let mut i: i64 = 0;
            while i < cnt - 1 && sorted[(i + 1) as usize] <= x {
                i += 1;
            }
            let mut rank: f64 = i as f64;
            if sorted[i as usize] < x {
                rank += (x - sorted[i as usize]) / (sorted[(i + 1) as usize] - sorted[i as usize]);
            } else {
                let mut j: i64 = i;
                while j > 0 && sorted[(j - 1) as usize] == x {
                    j -= 1;
                }
                rank = j as f64;
            }
            let pr: f64 = (rank + 1.0) / ((cnt + 1) as f64);
            let p10: f64 = FormulaFunctions::powTen(digits);
            return FormulaStats::numV((pr * p10 + 1.0e-9).floor() / p10);
        }
        if n == "FREQUENCY" {
            let data: Vec<f64> = FormulaStats::numbers(args, 0);
            let bins: Vec<f64> = FormulaStats::numbers(args, 1);
            let mut counts: Vec<f64> = Vec::new();
            let mut i: i64 = 0;
            while i <= (bins.len() as i64) {
                counts.push(0.0);
                i += 1;
            }
            for d in data.iter() {
                // the smallest bin at or above the value takes it
                let mut at: i64 = -1;
                let mut j: i64 = 0;
                while j < (bins.len() as i64) {
                    if *d <= bins[j as usize] {
                        if at < 0 || bins[j as usize] < bins[at as usize] {
                            at = j;
                        }
                    }
                    j += 1;
                }
                if at < 0 {
                    at = bins.len() as i64;
                }
                counts[at as usize] += 1.0;
            }
            return FormulaStats::column(&counts);
        }
        if n == "PROB" {
            let xs: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[0].clone());
            let ps: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[1].clone());
            if xs.len() != ps.len() || (args.len() as i64) < 3 {
                return FormulaStats::err("#N/A");
            }
            let lo: f64 = FormulaStats::num(args, 2, 0.0);
            let mut hi: f64 = lo;
            if FormulaMore::has(args, 3) {
                hi = FormulaStats::num(args, 3, 0.0);
            }
            let mut total: f64 = 0.0;
            let mut s: f64 = 0.0;
            let mut i: i64 = 0;
            while i < (xs.len() as i64) {
                let p: f64 = ps[i as usize].borrow().numberValue();
                if p < 0.0 || p > 1.0 {
                    return FormulaStats::err("#NUM!");
                }
                total += p;
                let x: f64 = xs[i as usize].borrow().numberValue();
                if x >= lo && x <= hi {
                    s += p;
                }
                i += 1;
            }
            if FormulaStats::absD(total - 1.0) > 1.0e-7 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(s);
        }
        if n == "CHISQ.TEST" || n == "CHITEST" {
            let a: Rc<RefCell<FormulaValue>> = FormulaMore::grid(&args[0].clone());
            let e: Rc<RefCell<FormulaValue>> = FormulaMore::grid(&args[1].clone());
            let rows: i64 = a.borrow().rows;
            let cols: i64 = a.borrow().cols;
            if rows != e.borrow().rows || cols != e.borrow().cols || rows * cols < 2 {
                return FormulaStats::err("#N/A");
            }
            let mut chi: f64 = 0.0;
            let mut i: i64 = 0;
            while i < rows * cols {
                let av: Rc<RefCell<FormulaValue>> = a.borrow().items[i as usize].clone();
                let ev: Rc<RefCell<FormulaValue>> = e.borrow().items[i as usize].clone();
                if av.borrow().kind == 1 && ev.borrow().kind == 1 {
                    let ex: f64 = ev.borrow().num;
                    if ex == 0.0 {
                        return FormulaStats::err("#DIV/0!");
                    }
                    let d: f64 = av.borrow().num - ex;
                    chi += d * d / ex;
                }
                i += 1;
            }
            let mut df: f64 = ((rows - 1) * (cols - 1)) as f64;
            if rows == 1 || cols == 1 {
                df = (rows * cols - 1) as f64;
            }
            return FormulaStats::numV(FormulaStats::gammaQ(df / 2.0, chi / 2.0));
        }
        if n == "T.TEST" || n == "TTEST" {
            if (args.len() as i64) < 4 {
                return FormulaStats::err("#VALUE!");
            }
            let tails: f64 = FormulaStats::num(args, 2, 0.0).floor();
            let kind: f64 = FormulaStats::num(args, 3, 0.0).floor();
            if (tails != 1.0 && tails != 2.0) || kind < 1.0 || kind > 3.0 {
                return FormulaStats::err("#NUM!");
            }
            let mut t: f64 = 0.0;
            let mut df: f64 = 0.0;
            if kind == 1.0 {
                let av: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[0].clone());
                let bv: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(&args[1].clone());
                if av.len() != bv.len() {
                    return FormulaStats::err("#N/A");
                }
                let mut d: Vec<f64> = Vec::new();
                let mut i: i64 = 0;
                while i < (av.len() as i64) {
                    if av[i as usize].borrow().kind == 1 && bv[i as usize].borrow().kind == 1 {
                        d.push(av[i as usize].borrow().num - bv[i as usize].borrow().num);
                    }
                    i += 1;
                }
                let cnt: f64 = d.len() as f64;
                if cnt < 2.0 {
                    return FormulaStats::err("#DIV/0!");
                }
                let sd: f64 = FormulaStats::sqrtD(FormulaStats::ssd(&d) / (cnt - 1.0));
                if sd == 0.0 {
                    return FormulaStats::err("#DIV/0!");
                }
                t = FormulaStats::mean(&d) / (sd / FormulaStats::sqrtD(cnt));
                df = cnt - 1.0;
            } else {
                let a: Vec<f64> = FormulaStats::numbers(args, 0);
                let b: Vec<f64> = FormulaStats::numbers(args, 1);
                let n1: f64 = a.len() as f64;
                let n2: f64 = b.len() as f64;
                if n1 < 2.0 || n2 < 2.0 {
                    return FormulaStats::err("#DIV/0!");
                }
                let v1: f64 = FormulaStats::ssd(&a) / (n1 - 1.0);
                let v2: f64 = FormulaStats::ssd(&b) / (n2 - 1.0);
                let diff: f64 = FormulaStats::mean(&a) - FormulaStats::mean(&b);
                if kind == 2.0 {
                    df = n1 + n2 - 2.0;
                    let sp: f64 = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / df;
                    let se: f64 = FormulaStats::sqrtD(sp * (1.0 / n1 + 1.0 / n2));
                    if se == 0.0 {
                        return FormulaStats::err("#DIV/0!");
                    }
                    t = diff / se;
                } else {
                    let q1: f64 = v1 / n1;
                    let q2: f64 = v2 / n2;
                    if q1 + q2 == 0.0 {
                        return FormulaStats::err("#DIV/0!");
                    }
                    t = diff / FormulaStats::sqrtD(q1 + q2);
                    df = (q1 + q2) * (q1 + q2) / (q1 * q1 / (n1 - 1.0) + q2 * q2 / (n2 - 1.0));
                }
            }
            let tail: f64 = 1.0 - FormulaStats::cdf(2, FormulaStats::absD(t), df, 0.0);
            return FormulaStats::numV(tails * tail);
        }
        if n == "F.TEST" || n == "FTEST" {
            let a: Vec<f64> = FormulaStats::numbers(args, 0);
            let b: Vec<f64> = FormulaStats::numbers(args, 1);
            let n1: f64 = a.len() as f64;
            let n2: f64 = b.len() as f64;
            if n1 < 2.0 || n2 < 2.0 {
                return FormulaStats::err("#DIV/0!");
            }
            let v1: f64 = FormulaStats::ssd(&a) / (n1 - 1.0);
            let v2: f64 = FormulaStats::ssd(&b) / (n2 - 1.0);
            if v1 == 0.0 || v2 == 0.0 {
                return FormulaStats::err("#DIV/0!");
            }
            let c: f64 = FormulaStats::cdf(3, v1 / v2, n1 - 1.0, n2 - 1.0);
            let mut p: f64 = 2.0 * c;
            if c > 0.5 {
                p = 2.0 * (1.0 - c);
            }
            return FormulaStats::numV(p);
        }
        if n == "Z.TEST" || n == "ZTEST" {
            let a: Vec<f64> = FormulaStats::numbers(args, 0);
            let cnt: f64 = a.len() as f64;
            if cnt < 1.0 || (args.len() as i64) < 2 {
                return FormulaStats::err("#N/A");
            }
            let x: f64 = FormulaStats::num(args, 1, 0.0);
            let mut sigma: f64 = 0.0;
            if FormulaMore::has(args, 2) {
                sigma = FormulaStats::num(args, 2, 0.0);
            } else {
                if cnt < 2.0 {
                    return FormulaStats::err("#DIV/0!");
                }
                sigma = FormulaStats::sqrtD(FormulaStats::ssd(&a) / (cnt - 1.0));
            }
            if sigma <= 0.0 {
                return FormulaStats::err("#DIV/0!");
            }
            return FormulaStats::numV(1.0 - FormulaStats::normCdf((FormulaStats::mean(&a) - x) / (sigma / FormulaStats::sqrtD(cnt))));
        }
        if n == "TREND" || n == "GROWTH" || n == "LINEST" || n == "LOGEST" {
            let log_y: bool = n == "GROWTH" || n == "LOGEST";
            let multi: Rc<RefCell<FormulaValue>> = FormulaStats::multiFit(n, args, log_y);
            if !multi.borrow().isBlank() {
                return multi;
            }
            let mut xs: Vec<f64> = Vec::new();
            let mut ys: Vec<f64> = Vec::new();
            if (args.len() as i64) < 1 || !FormulaStats::knownXY(args, log_y, &mut xs, &mut ys) {
                if log_y {
                    return FormulaStats::err("#NUM!");
                }
                return FormulaStats::err("#REF!");
            }
            let mut ci: i64 = 3;
            if n == "LINEST" || n == "LOGEST" {
                ci = 2;
            }
            let with_const: bool = FormulaStats::flag(args, ci, true);
            let fit: Vec<f64> = FormulaStats::fitLine(&xs, &ys, with_const);
            if fit.is_empty() {
                return FormulaStats::err("#DIV/0!");
            }
            if n == "LINEST" || n == "LOGEST" {
                let mut m: f64 = fit[0];
                let mut b: f64 = fit[1];
                if log_y {
                    m = FormulaStats::exp(m);
                    b = FormulaStats::exp(b);
                }
                let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
                items.push(FormulaValue::fromNumber(m));
                items.push(FormulaValue::fromNumber(b));
                if !FormulaStats::flag(args, 3, false) {
                    return FormulaValue::fromArray(1, 2, &items);
                }
                items.push(FormulaValue::fromNumber(fit[2]));
                if with_const {
                    items.push(FormulaValue::fromNumber(fit[3]));
                } else {
                    items.push(FormulaValue::fromError("#N/A".to_string()));
                }
                let mut k: i64 = 4;
                while k < 10 {
                    items.push(FormulaValue::fromNumber(fit[k as usize]));
                    k += 1;
                }
                return FormulaValue::fromArray(5, 2, &items);
            }
            // TREND / GROWTH at new x (default: the known x), in its shape
            let mut nx: Rc<RefCell<FormulaValue>> = FormulaStats::column(&xs);
            if FormulaMore::has(args, 2) {
                nx = FormulaMore::grid(&args[2].clone());
            } else {
                if FormulaMore::has(args, 1) {
                    nx = FormulaMore::grid(&args[1].clone());
                } else {
                    let yg: Rc<RefCell<FormulaValue>> = FormulaMore::grid(&args[0].clone());
                    let mut seq: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
                    let mut i: i64 = 0;
                    while i < yg.borrow().count() {
                        seq.push(FormulaValue::fromNumber((i + 1) as f64));
                        i += 1;
                    }
                    nx = FormulaValue::fromArray(yg.borrow().rows, yg.borrow().cols, &seq);
                }
            }
            let mut out: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut i: i64 = 0;
            while i < nx.borrow().count() {
                let xv: Rc<RefCell<FormulaValue>> = nx.borrow().items[i as usize].clone();
                if xv.borrow().kind != 1 {
                    return FormulaStats::err("#VALUE!");
                }
                let mut y: f64 = fit[0] * xv.borrow().num + fit[1];
                if log_y {
                    y = FormulaStats::exp(y);
                }
                out.push(FormulaValue::fromNumber(y));
                i += 1;
            }
            if out.len() == 1 {
                return out[0].clone();
            }
            return FormulaValue::fromArray(nx.borrow().rows, nx.borrow().cols, &out);
        }
        FormulaValue::blank()
    }

    // --- math, bits and bases ------------------------------------------------------

    pub fn sinh(x: f64) -> f64 {
        (FormulaStats::exp(x) - FormulaStats::exp(0.0 - x)) / 2.0
    }
    pub fn cosh(x: f64) -> f64 {
        (FormulaStats::exp(x) + FormulaStats::exp(0.0 - x)) / 2.0
    }
    /// A whole number in [0, 2^48) for the BIT functions, or -1.
    pub fn bitArg(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64) -> f64 {
        let v: f64 = FormulaStats::num(args, i, -1.0);
        if v < 0.0 || v.floor() != v || v >= 281474976710656.0 {
            return -1.0;
        }
        v
    }
    /// Bitwise and (op 0), or (1), xor (2) of two whole numbers, bit by bit.
    pub fn bitOp(a0: f64, b0: f64, op: i64) -> f64 {
        let mut a: f64 = a0;
        let mut b: f64 = b0;
        let mut out: f64 = 0.0;
        let mut place: f64 = 1.0;
        while a > 0.0 || b > 0.0 {
            let ha: f64 = (a / 2.0).floor();
            let hb: f64 = (b / 2.0).floor();
            let ba: bool = a - ha * 2.0 > 0.5;
            let bb: bool = b - hb * 2.0 > 0.5;
            let mut bit: bool = ba && bb;
            if op == 1 {
                bit = ba || bb;
            }
            if op == 2 {
                bit = ba != bb;
            }
            if bit {
                out += place;
            }
            place *= 2.0;
            a = ha;
            b = hb;
        }
        out
    }
    pub fn mathFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let x: f64 = FormulaStats::num(args, 0, 0.0);
        if n == "PERMUTATIONA" {
            let k: f64 = FormulaStats::num(args, 1, 0.0).floor();
            let nn: f64 = x.floor();
            if nn < 0.0 || k < 0.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(FormulaStats::pow(nn, k));
        }
        if n == "SERIESSUM" {
            let p: f64 = FormulaStats::num(args, 1, 0.0);
            let m: f64 = FormulaStats::num(args, 2, 0.0);
            let coeffs: Vec<f64> = FormulaStats::numbers(args, 3);
            let mut s: f64 = 0.0;
            let mut i: i64 = 0;
            while i < (coeffs.len() as i64) {
                s += coeffs[i as usize] * FormulaStats::pow(x, p + (i as f64) * m);
                i += 1;
            }
            return FormulaStats::numV(s);
        }
        if n == "ACOSH" {
            if x < 1.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(FormulaStats::ln(x + FormulaStats::sqrtD(x * x - 1.0)));
        }
        if n == "ASINH" {
            let ax: f64 = FormulaStats::absD(x);
            let v: f64 = FormulaStats::ln(ax + FormulaStats::sqrtD(ax * ax + 1.0));
            if x < 0.0 {
                return FormulaStats::numV(0.0 - v);
            }
            return FormulaStats::numV(v);
        }
        if n == "ATANH" {
            if x <= -1.0 || x >= 1.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(0.5 * FormulaStats::ln((1.0 + x) / (1.0 - x)));
        }
        if n == "ACOT" {
            return FormulaStats::numV(FormulaMore::PI() / 2.0 - FormulaMore::atanD(x));
        }
        if n == "ACOTH" {
            if x >= -1.0 && x <= 1.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(0.5 * FormulaStats::ln((x + 1.0) / (x - 1.0)));
        }
        if n == "COTH" || n == "CSCH" {
            if x == 0.0 {
                return FormulaStats::err("#DIV/0!");
            }
            if n == "CSCH" {
                return FormulaStats::numV(1.0 / FormulaStats::sinh(x));
            }
            return FormulaStats::numV(FormulaStats::cosh(x) / FormulaStats::sinh(x));
        }
        if n == "SECH" {
            return FormulaStats::numV(1.0 / FormulaStats::cosh(x));
        }
        if n == "BITAND" || n == "BITOR" || n == "BITXOR" {
            let a: f64 = FormulaStats::bitArg(args, 0);
            let b: f64 = FormulaStats::bitArg(args, 1);
            if a < 0.0 || b < 0.0 {
                return FormulaStats::err("#NUM!");
            }
            let mut op: i64 = 0;
            if n == "BITOR" {
                op = 1;
            }
            if n == "BITXOR" {
                op = 2;
            }
            return FormulaStats::numV(FormulaStats::bitOp(a, b, op));
        }
        if n == "BITLSHIFT" || n == "BITRSHIFT" {
            let a: f64 = FormulaStats::bitArg(args, 0);
            let mut s: f64 = FormulaStats::num(args, 1, 0.0).floor();
            if a < 0.0 || FormulaStats::absD(s) > 53.0 {
                return FormulaStats::err("#NUM!");
            }
            if n == "BITRSHIFT" {
                s = 0.0 - s;
            }
            let mut v: f64 = a;
            if s >= 0.0 {
                v = a * FormulaStats::pow(2.0, s);
            } else {
                v = (a / FormulaStats::pow(2.0, 0.0 - s)).floor();
            }
            if v >= 281474976710656.0 {
                return FormulaStats::err("#NUM!");
            }
            return FormulaStats::numV(v);
        }
        if n == "BIN2HEX" || n == "BIN2OCT" || n == "HEX2BIN" || n == "HEX2OCT" || n == "OCT2BIN" || n == "OCT2HEX" {
            let from: String = DgNative::substr(n, 0, 3);
            let to: String = DgNative::substr(n, 4, 7);
            let mut one: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            one.push(args[0].clone());
            let dec: Rc<RefCell<FormulaValue>> = FormulaFunctions::invoke(&format!("{}2DEC", from), &one);
            if dec.borrow().isError() {
                return dec;
            }
            let mut two: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            two.push(dec.clone());
            if FormulaMore::has(args, 1) {
                two.push(args[1].clone());
            }
            return FormulaFunctions::invoke(&format!("DEC2{}", to), &two);
        }
        if n == "DELTA" {
            let b: f64 = FormulaStats::num(args, 1, 0.0);
            return FormulaStats::numV(if x == b { 1.0 } else { 0.0 });
        }
        if n == "GESTEP" {
            let b: f64 = FormulaStats::num(args, 1, 0.0);
            return FormulaStats::numV(if x >= b { 1.0 } else { 0.0 });
        }
        if n == "ECMA.CEILING" {
            return FormulaFunctions::invoke("CEILING", args);
        }
        FormulaValue::blank()
    }

    // --- money ---------------------------------------------------------------------

    pub fn yearFrac(args: &Vec<Rc<RefCell<FormulaValue>>>, basis_at: i64) -> Rc<RefCell<FormulaValue>> {
        let mut yf: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        yf.push(args[0].clone());
        yf.push(args[1].clone());
        if FormulaMore::has(args, basis_at) {
            yf.push(args[basis_at as usize].clone());
        }
        FormulaFunctions::invoke("YEARFRAC", &yf)
    }
    /// One period's depreciation on the VDB schedule: declining balance at
    /// `factor`, switching to straight line when that is larger.
    pub fn vdbCum(cost: f64, salvage: f64, life: f64, t: f64, factor: f64, no_switch: bool) -> f64 {
        let mut book: f64 = cost;
        let mut cum: f64 = 0.0;
        let mut k: f64 = 0.0;
        while k < t {
            let mut ddb: f64 = book * factor / life;
            if book - ddb < salvage {
                ddb = book - salvage;
            }
            if ddb < 0.0 {
                ddb = 0.0;
            }
            let mut dep: f64 = ddb;
            if !no_switch {
                let left: f64 = life - k;
                if left > 0.0 {
                    let sl: f64 = (book - salvage) / left;
                    if sl > dep {
                        dep = sl;
                    }
                }
            }
            let mut part: f64 = 1.0;
            if t - k < 1.0 {
                part = t - k;
            }
            cum += dep * part;
            book -= dep;
            k += 1.0;
        }
        cum
    }
    pub fn moneyFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let na: i64 = args.len() as i64;
        if n == "FVSCHEDULE" {
            let mut v: f64 = FormulaStats::num(args, 0, 0.0);
            for x in FormulaMore::flatOf(&args[1].clone()).iter() {
                if x.borrow().kind == 2 {
                    return FormulaStats::err("#VALUE!");
                }
                v *= 1.0 + x.borrow().numberValue();
            }
            return FormulaStats::numV(v);
        }
        if n == "ISPMT" {
            let r: f64 = FormulaStats::num(args, 0, 0.0);
            let per: f64 = FormulaStats::num(args, 1, 0.0);
            let nper: f64 = FormulaStats::num(args, 2, 0.0);
            let pv: f64 = FormulaStats::num(args, 3, 0.0);
            if nper == 0.0 {
                return FormulaStats::err("#DIV/0!");
            }
            return FormulaStats::numV(pv * r * (per / nper - 1.0));
        }
        if n == "DOLLARDE" || n == "DOLLARFR" {
            let v: f64 = FormulaStats::num(args, 0, 0.0);
            let f: f64 = FormulaStats::num(args, 1, 0.0).floor();
            if f < 0.0 {
                return FormulaStats::err("#NUM!");
            }
            if f == 0.0 {
                return FormulaStats::err("#DIV/0!");
            }
            let mut p: f64 = 1.0;
            while p < f {
                p *= 10.0;
            }
            let neg: bool = v < 0.0;
            let av: f64 = FormulaStats::absD(v);
            let ip: f64 = av.floor();
            let mut out: f64 = 0.0;
            if n == "DOLLARDE" {
                out = ip + (av - ip) * p / f;
            } else {
                out = ip + (av - ip) * f / p;
            }
            if neg {
                out = 0.0 - out;
            }
            return FormulaStats::numV(out);
        }
        if n == "TBILLEQ" || n == "TBILLPRICE" || n == "TBILLYIELD" {
            if na < 3 {
                return FormulaStats::err("#VALUE!");
            }
            let s: f64 = FormulaStats::num(args, 0, 0.0).floor();
            let m: f64 = FormulaStats::num(args, 1, 0.0).floor();
            let r: f64 = FormulaStats::num(args, 2, 0.0);
            let dsm: f64 = m - s;
            if dsm <= 0.0 || dsm > 366.0 || r <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            if n == "TBILLPRICE" {
                let pr: f64 = 100.0 * (1.0 - r * dsm / 360.0);
                if pr <= 0.0 {
                    return FormulaStats::err("#NUM!");
                }
                return FormulaStats::numV(pr);
            }
            if n == "TBILLYIELD" {
                return FormulaStats::numV((100.0 - r) / r * 360.0 / dsm);
            }
            if dsm <= 182.0 {
                return FormulaStats::numV(365.0 * r / (360.0 - r * dsm));
            }
            let price: f64 = 100.0 * (1.0 - r * dsm / 360.0);
            let term: f64 = dsm / 365.0;
            let inner: f64 = term * term - (2.0 * term - 1.0) * (1.0 - 100.0 / price);
            return FormulaStats::numV((0.0 - term + FormulaStats::sqrtD(inner)) / (term - 0.5));
        }
        if n == "DISC" || n == "INTRATE" || n == "RECEIVED" || n == "PRICEDISC" || n == "YIELDDISC" {
            if na < 4 {
                return FormulaStats::err("#VALUE!");
            }
            let s: f64 = FormulaStats::num(args, 0, 0.0).floor();
            let m: f64 = FormulaStats::num(args, 1, 0.0).floor();
            let a: f64 = FormulaStats::num(args, 2, 0.0);
            let b: f64 = FormulaStats::num(args, 3, 0.0);
            let basis: f64 = FormulaStats::num(args, 4, 0.0).floor();
            if s >= m || a <= 0.0 || b <= 0.0 || basis < 0.0 || basis > 4.0 {
                return FormulaStats::err("#NUM!");
            }
            let yfv: Rc<RefCell<FormulaValue>> = FormulaStats::yearFrac(args, 4);
            if yfv.borrow().isError() {
                return yfv;
            }
            let yf: f64 = yfv.borrow().num;
            if yf <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            if n == "DISC" {
                return FormulaStats::numV((b - a) / b / yf);
            }
            if n == "INTRATE" {
                return FormulaStats::numV((b - a) / a / yf);
            }
            if n == "RECEIVED" {
                let d: f64 = 1.0 - b * yf;
                if d <= 0.0 {
                    return FormulaStats::err("#NUM!");
                }
                return FormulaStats::numV(a / d);
            }
            if n == "PRICEDISC" {
                return FormulaStats::numV(b - a * b * yf);
            }
            return FormulaStats::numV((b - a) / a / yf);
        }
        if n == "VDB" {
            if na < 5 {
                return FormulaStats::err("#VALUE!");
            }
            let cost: f64 = FormulaStats::num(args, 0, 0.0);
            let salvage: f64 = FormulaStats::num(args, 1, 0.0);
            let life: f64 = FormulaStats::num(args, 2, 0.0);
            let st: f64 = FormulaStats::num(args, 3, 0.0);
            let en: f64 = FormulaStats::num(args, 4, 0.0);
            let factor: f64 = FormulaStats::num(args, 5, 2.0);
            let no_switch: bool = FormulaStats::flag(args, 6, false);
            if cost < 0.0 || salvage < 0.0 || life <= 0.0 || st < 0.0 || en < st || en > life || factor <= 0.0 {
                return FormulaStats::err("#NUM!");
            }
            let a: f64 = FormulaStats::vdbCum(cost, salvage, life, st, factor, no_switch);
            let b: f64 = FormulaStats::vdbCum(cost, salvage, life, en, factor, no_switch);
            return FormulaStats::numV(b - a);
        }
        FormulaValue::blank()
    }

    // --- database, text and the rest -----------------------------------------------

    /// Does row `r` of the database meet the criteria table: any criteria row
    /// whose filled cells all match (a text criterion without an operator
    /// matches values that begin with it, as in Excel).
    pub fn dbRowMatches(db: &Rc<RefCell<FormulaValue>>, r: i64, crit: &Rc<RefCell<FormulaValue>>) -> bool {
        let cr: i64 = crit.borrow().rows;
        let cc: i64 = crit.borrow().cols;
        let dc: i64 = db.borrow().cols;
        if cr < 2 {
            return true;
        }
        let mut k: i64 = 1;
        while k < cr {
            let mut all: bool = true;
            let mut j: i64 = 0;
            while j < cc {
                let cv: Rc<RefCell<FormulaValue>> = crit.borrow().at(k, j);
                if !cv.borrow().isBlank() {
                    let head: String = FormulaFunctions::upper(&crit.borrow().at(0, j).borrow().toText());
                    let mut col: i64 = -1;
                    let mut c: i64 = 0;
                    while c < dc {
                        if FormulaFunctions::upper(&db.borrow().at(0, c).borrow().toText()) == head {
                            col = c;
                        }
                        c += 1;
                    }
                    if col < 0 {
                        all = false;
                    } else {
                        let mut test: Rc<RefCell<FormulaValue>> = cv.clone();
                        if cv.borrow().kind == 2 {
                            let t: String = cv.borrow().text.clone();
                            let first: i64 = DgNative::char_at(&t, 0);
                            let ops: bool = first == 61 || first == 60 || first == 62;
                            if !ops && DgNative::index_of(&t, "*") < 0 {
                                test = FormulaValue::fromString(format!("{}*", t));
                            }
                        }
                        if !FormulaFunctions::criterionMatches(&db.borrow().at(r, col), &test) {
                            all = false;
                        }
                    }
                }
                j += 1;
            }
            if all {
                return true;
            }
            k += 1;
        }
        false
    }
    pub fn databaseFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if (args.len() as i64) < 3 {
            return FormulaStats::err("#VALUE!");
        }
        let db: Rc<RefCell<FormulaValue>> = FormulaMore::grid(&args[0].clone());
        let crit: Rc<RefCell<FormulaValue>> = FormulaMore::grid(&args[2].clone());
        let fv: Rc<RefCell<FormulaValue>> = args[1].borrow().single();
        if fv.borrow().isError() {
            return fv;
        }
        let dc: i64 = db.borrow().cols;
        let mut field: i64 = -1;
        if fv.borrow().kind == 1 {
            field = (fv.borrow().num.floor() as i64) - 1;
        } else {
            let want: String = FormulaFunctions::upper(&fv.borrow().toText());
            let mut c: i64 = 0;
            while c < dc {
                if FormulaFunctions::upper(&db.borrow().at(0, c).borrow().toText()) == want {
                    field = c;
                }
                c += 1;
            }
        }
        if field < 0 || field >= dc {
            return FormulaStats::err("#VALUE!");
        }
        let mut vals: Vec<f64> = Vec::new();
        let mut cells: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut r: i64 = 1;
        while r < db.borrow().rows {
            if FormulaStats::dbRowMatches(&db, r, &crit) {
                let v: Rc<RefCell<FormulaValue>> = db.borrow().at(r, field);
                cells.push(v.clone());
                if v.borrow().kind == 1 {
                    vals.push(v.borrow().num);
                }
            }
            r += 1;
        }
        let cnt: f64 = vals.len() as f64;
        if n == "DCOUNT" {
            return FormulaStats::numV(cnt);
        }
        if n == "DCOUNTA" {
            let mut k: f64 = 0.0;
            for c in cells.iter() {
                if !c.borrow().isBlank() {
                    k += 1.0;
                }
            }
            return FormulaStats::numV(k);
        }
        if n == "DGET" {
            if cells.is_empty() {
                return FormulaStats::err("#VALUE!");
            }
            if cells.len() > 1 {
                return FormulaStats::err("#NUM!");
            }
            return cells[0].clone();
        }
        if n == "DSUM" {
            let mut s: f64 = 0.0;
            for x in vals.iter() {
                s += *x;
            }
            return FormulaStats::numV(s);
        }
        if n == "DPRODUCT" {
            if vals.is_empty() {
                return FormulaStats::numV(0.0);
            }
            let mut p: f64 = 1.0;
            for x in vals.iter() {
                p *= *x;
            }
            return FormulaStats::numV(p);
        }
        if n == "DMAX" || n == "DMIN" {
            if vals.is_empty() {
                return FormulaStats::numV(0.0);
            }
            let mut best: f64 = vals[0];
            for x in vals.iter() {
                if (n == "DMAX" && *x > best) || (n == "DMIN" && *x < best) {
                    best = *x;
                }
            }
            return FormulaStats::numV(best);
        }
        if n == "DAVERAGE" {
            if cnt == 0.0 {
                return FormulaStats::err("#DIV/0!");
            }
            return FormulaStats::numV(FormulaStats::mean(&vals));
        }
        let pop: bool = n == "DSTDEVP" || n == "DVARP";
        let den: f64 = if pop { cnt } else { cnt - 1.0 };
        if den <= 0.0 {
            return FormulaStats::err("#DIV/0!");
        }
        let var: f64 = FormulaStats::ssd(&vals) / den;
        if n == "DVAR" || n == "DVARP" {
            return FormulaStats::numV(var);
        }
        FormulaStats::numV(FormulaStats::sqrtD(var))
    }
    pub fn mapWidth(s: &str, to_half: bool) -> String {
        let mut out: String = String::new();
        let len: i64 = DgNative::str_len(s);
        let mut i: i64 = 0;
        while i < len {
            let c: i64 = DgNative::char_at(s, i);
            let mut m: i64 = c;
            if to_half {
                if c >= 65281 && c <= 65374 {
                    m = c - 65248;
                }
                if c == 12288 {
                    m = 32;
                }
            } else {
                if c >= 33 && c <= 126 {
                    m = c + 65248;
                }
                if c == 32 {
                    m = 12288;
                }
            }
            out.push_str(&DgNative::from_code(m));
            i += 1;
        }
        out
    }
    pub fn hexByte(b: i64) -> String {
        let digits: String = "0123456789ABCDEF".to_string();
        format!("%{}{}", DgNative::substr(&digits, b / 16, b / 16 + 1), DgNative::substr(&digits, b % 16, b % 16 + 1))
    }
    pub fn encodeUrl(s: &str) -> String {
        let mut out: String = String::new();
        let len: i64 = DgNative::str_len(s);
        let mut i: i64 = 0;
        while i < len {
            let mut c: i64 = DgNative::char_at(s, i);
            let plain: bool = (c >= 48 && c <= 57) || (c >= 65 && c <= 90) || (c >= 97 && c <= 122) || c == 45 || c == 95 || c == 46 || c == 126;
            if plain {
                out.push_str(&DgNative::from_code(c));
            } else {
                if c >= 55296 && c <= 56319 && i + 1 < len {
                    let lo: i64 = DgNative::char_at(s, i + 1);
                    c = 65536 + (c - 55296) * 1024 + (lo - 56320);
                    i += 1;
                }
                if c < 128 {
                    out.push_str(&FormulaStats::hexByte(c));
                } else if c < 2048 {
                    out.push_str(&FormulaStats::hexByte(192 + c / 64));
                    out.push_str(&FormulaStats::hexByte(128 + c % 64));
                } else if c < 65536 {
                    out.push_str(&FormulaStats::hexByte(224 + c / 4096));
                    out.push_str(&FormulaStats::hexByte(128 + (c / 64) % 64));
                    out.push_str(&FormulaStats::hexByte(128 + c % 64));
                } else {
                    out.push_str(&FormulaStats::hexByte(240 + c / 262144));
                    out.push_str(&FormulaStats::hexByte(128 + (c / 4096) % 64));
                    out.push_str(&FormulaStats::hexByte(128 + (c / 64) % 64));
                    out.push_str(&FormulaStats::hexByte(128 + c % 64));
                }
            }
            i += 1;
        }
        out
    }
    pub fn otherFn(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        let dbs: String = " DSUM DAVERAGE DCOUNT DCOUNTA DMAX DMIN DGET DPRODUCT DSTDEV DSTDEVP DVAR DVARP ".to_string();
        if DgNative::index_of(&dbs, &format!(" {} ", n)) >= 0 {
            return FormulaStats::databaseFn(n, args);
        }
        if n == "LEFTB" || n == "RIGHTB" || n == "MIDB" || n == "LENB" || n == "FINDB" || n == "SEARCHB" || n == "REPLACEB" || n == "USDOLLAR" {
            let mut plain: String = DgNative::substr(n, 0, DgNative::str_len(n) - 1);
            if n == "USDOLLAR" {
                plain = "DOLLAR".to_string();
            }
            return FormulaFunctions::invoke(&plain, args);
        }
        if args.is_empty() {
            return FormulaValue::blank();
        }
        let a0: Rc<RefCell<FormulaValue>> = args[0].borrow().single();
        if n == "ASC" || n == "DBCS" || n == "JIS" || n == "PHONETIC" || n == "ENCODEURL" {
            if a0.borrow().isError() {
                return a0;
            }
            let t: String = a0.borrow().toText();
            if n == "ASC" {
                return FormulaValue::fromString(FormulaStats::mapWidth(&t, true));
            }
            if n == "DBCS" || n == "JIS" {
                return FormulaValue::fromString(FormulaStats::mapWidth(&t, false));
            }
            if n == "ENCODEURL" {
                return FormulaValue::fromString(FormulaStats::encodeUrl(&t));
            }
            return FormulaValue::fromString(t);
        }
        if n == "ARRAYTOTEXT" {
            let g: Rc<RefCell<FormulaValue>> = FormulaMore::grid(&args[0].clone());
            let strict: bool = FormulaMore::int(args, 1, 0) == 1;
            let mut out: String = String::new();
            if strict {
                out.push_str("{");
            }
            let mut r: i64 = 0;
            while r < g.borrow().rows {
                let mut c: i64 = 0;
                while c < g.borrow().cols {
                    let v: Rc<RefCell<FormulaValue>> = g.borrow().at(r, c);
                    if r + c > 0 {
                        if strict && c == 0 {
                            out.push_str(";");
                        } else {
                            if strict {
                                out.push_str(",");
                            } else {
                                out.push_str(", ");
                            }
                        }
                    }
                    let mut t: String = v.borrow().toText();
                    if v.borrow().isError() {
                        t = v.borrow().err.clone();
                    }
                    if strict && v.borrow().kind == 2 {
                        t = format!("\"{}\"", t.replace("\"", "\"\""));
                    }
                    out.push_str(&t);
                    c += 1;
                }
                r += 1;
            }
            if strict {
                out.push_str("}");
            }
            return FormulaValue::fromString(out);
        }
        if n == "PERCENTOF" {
            if (args.len() as i64) < 2 {
                return FormulaStats::err("#VALUE!");
            }
            let e: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
            if e.borrow().isError() {
                return e;
            }
            let mut s1: f64 = 0.0;
            for x in FormulaStats::numbers(args, 0).iter() {
                s1 += *x;
            }
            let mut s2: f64 = 0.0;
            for x in FormulaStats::numbers(args, 1).iter() {
                s2 += *x;
            }
            if s2 == 0.0 {
                return FormulaStats::err("#DIV/0!");
            }
            return FormulaStats::numV(s1 / s2);
        }
        FormulaValue::blank()
    }
}
