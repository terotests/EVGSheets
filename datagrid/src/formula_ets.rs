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

/// FormulaEts - FORECAST.ETS, .CONFINT, .SEASONALITY and .STAT: additive
/// error, trend and seasonality (AAA) exponential smoothing over an evenly
/// spaced timeline (Hyndman, Koehler, Ord & Snyder 2008, ETS(A,A,A)):
///
///   e_t = y_t - (l + b + s_{t-m});  l += b + a e;  b += a B e;  s += g e
///
/// Initial states: seasonal indices from a centred moving average over the
/// first seasons, level and trend from a line through the first ten
/// seasonally adjusted values. a, B and g in [0, 1] minimise the squared
/// one-step errors (a coarse grid, then a Nelder-Mead simplex). Excel does not
/// document its optimiser, so on noisy data the last digits can differ; exact
/// data (lines, exactly repeating seasons) forecasts exactly.
///
/// The timeline is a constant numeric step (missing points allowed, up to 30%)
/// or whole-day dates a constant number of calendar months apart.
#[derive(Clone)]
pub struct FormulaEts {
}

impl FormulaEts {
    pub fn new() -> Self {
        Self {
        }
    }
    pub fn names() -> String {
        " FORECAST.ETS FORECAST.ETS.CONFINT FORECAST.ETS.SEASONALITY FORECAST.ETS.STAT ".to_string()
    }
    pub fn isKnown(n: &str) -> bool {
        DgNative::index_of(&FormulaEts::names(), &format!(" {} ", n)) >= 0
    }
    pub fn err(e: &str) -> Rc<RefCell<FormulaValue>> {
        FormulaValue::fromError(e.to_string())
    }

    // --- timeline ------------------------------------------------------------------

    /// Position of time t on the axis [kind, t0, step, month0, mstep, day]:
    /// kind 0 a numeric step, 1 calendar months (day -1 = month ends).
    pub fn position(axis: &Vec<f64>, t: f64) -> f64 {
        if axis[0] == 0.0 {
            return (t - axis[1]) / axis[2];
        }
        let ymd: Vec<i64> = DateSerial::toYMD(t.floor());
        let month: f64 = (ymd[0] * 12 + ymd[1] - 1) as f64;
        let dim: f64 = DateSerial::daysInMonth(ymd[0], ymd[1]) as f64;
        let mut anchor: f64 = dim;
        if axis[5] > 0.0 {
            anchor = axis[5];
            if anchor > dim {
                anchor = dim;
            }
        }
        let offset: f64 = ((ymd[2] as f64) - anchor + (t - t.floor())) / dim;
        (month - axis[3] + offset) / axis[4]
    }
    pub fn gcd(a0: i64, b0: i64) -> i64 {
        let mut a: i64 = a0;
        let mut b: i64 = b0;
        while b != 0 {
            let r: i64 = a - (a / b) * b;
            a = b;
            b = r;
        }
        if a < 0 {
            return 0 - a;
        }
        a
    }
    /// The axis of sorted distinct times, or empty when it has no constant step.
    pub fn detectAxis(times: &Vec<f64>) -> Vec<f64> {
        let mut axis: Vec<f64> = Vec::new();
        let n: usize = times.len();
        let mut step: f64 = 1.0e308;
        let mut i: usize = 1;
        while i < n {
            let d: f64 = times[i] - times[i - 1];
            if d < step {
                step = d;
            }
            i += 1;
        }
        if step > 0.0 && step < 1.0e308 {
            let mut even: bool = true;
            i = 1;
            while i < n {
                let r: f64 = (times[i] - times[i - 1]) / step;
                if FormulaStats::absD(r - (r + 0.5).floor()) > 1.0e-6 {
                    even = false;
                }
                i += 1;
            }
            if even {
                axis.push(0.0);
                axis.push(times[0]);
                axis.push(step);
                axis.push(0.0);
                axis.push(1.0);
                axis.push(0.0);
                return axis;
            }
        }
        // calendar months: whole days on one day of the month, or all month ends
        let mut months: Vec<i64> = Vec::new();
        let mut same_day: i64 = -1;
        let mut all_same: bool = true;
        let mut all_end: bool = true;
        for t in times.iter() {
            if t.floor() != *t || *t < 1.0 {
                return axis;
            }
            let ymd: Vec<i64> = DateSerial::toYMD(*t);
            months.push(ymd[0] * 12 + ymd[1] - 1);
            if same_day < 0 {
                same_day = ymd[2];
            } else if same_day != ymd[2] {
                all_same = false;
            }
            if ymd[2] != DateSerial::daysInMonth(ymd[0], ymd[1]) {
                all_end = false;
            }
        }
        let mut day: f64 = -1.0;
        if all_same {
            day = same_day as f64;
        } else if !all_end {
            return axis;
        }
        let mut g: i64 = 0;
        i = 1;
        while i < months.len() {
            let d: i64 = months[i] - months[i - 1];
            if d <= 0 {
                return axis;
            }
            g = FormulaEts::gcd(g, d);
            i += 1;
        }
        if g <= 0 {
            return axis;
        }
        axis.push(1.0);
        axis.push(times[0]);
        axis.push(0.0);
        axis.push(months[0] as f64);
        axis.push(g as f64);
        axis.push(day);
        axis
    }
    pub fn aggregate(kind: i64, xs: &Vec<f64>) -> f64 {
        let n: f64 = xs.len() as f64;
        if kind == 2 || kind == 3 {
            return n;
        }
        if xs.is_empty() {
            return -1.0e308;
        }
        let mut sum: f64 = 0.0;
        let mut mx: f64 = xs[0];
        let mut mn: f64 = xs[0];
        for x in xs.iter() {
            sum += *x;
            if *x > mx {
                mx = *x;
            }
            if *x < mn {
                mn = *x;
            }
        }
        if kind == 4 {
            return mx;
        }
        if kind == 6 {
            return mn;
        }
        if kind == 7 {
            return sum;
        }
        if kind == 5 {
            let mut s: Vec<f64> = xs.clone();
            FormulaEts::sortF(&mut s);
            let k: usize = s.len() / 2;
            if s.len() - (s.len() / 2) * 2 == 1 {
                return s[k];
            }
            return (s[k - 1] + s[k]) / 2.0;
        }
        sum / n
    }
    pub fn sortF(v: &mut Vec<f64>) {
        let mut i: usize = 1;
        while i < v.len() {
            let cur: f64 = v[i];
            let mut j: i64 = (i as i64) - 1;
            while j >= 0 && v[j as usize] > cur {
                v[(j + 1) as usize] = v[j as usize];
                j -= 1;
            }
            v[(j + 1) as usize] = cur;
            i += 1;
        }
    }
    /// Builds the evenly spaced series into `y` and the axis into `axis`
    /// (plus the step at axis[6]); "" or the error.
    pub fn series(values: &Rc<RefCell<FormulaValue>>, timeline: &Rc<RefCell<FormulaValue>>, interpolate: bool, agg: i64, y: &mut Vec<f64>, axis: &mut Vec<f64>) -> String {
        let va: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(values);
        let ta: Vec<Rc<RefCell<FormulaValue>>> = FormulaMore::flatOf(timeline);
        if va.len() != ta.len() {
            return "#N/A".to_string();
        }
        let mut pt: Vec<f64> = Vec::new();
        let mut pv: Vec<f64> = Vec::new();
        let mut ph: Vec<bool> = Vec::new();
        let mut i: usize = 0;
        while i < va.len() {
            let t: Rc<RefCell<FormulaValue>> = ta[i].clone();
            let v: Rc<RefCell<FormulaValue>> = va[i].clone();
            if t.borrow().kind == 4 {
                return t.borrow().err.clone();
            }
            if v.borrow().kind == 4 {
                return v.borrow().err.clone();
            }
            let vk: i64 = v.borrow().kind;
            if vk != 1 && vk != 0 {
                return "#VALUE!".to_string();
            }
            let tk: i64 = t.borrow().kind;
            if tk == 0 && vk == 0 {
                i += 1;
                continue;
            }
            if tk != 1 {
                return "#VALUE!".to_string();
            }
            pt.push(t.borrow().num);
            pv.push(v.borrow().num);
            ph.push(vk == 1);
            i += 1;
        }
        // sort the points by time (insertion sort keeps equal times in order)
        let mut k: usize = 1;
        while k < pt.len() {
            let ct: f64 = pt[k];
            let cv: f64 = pv[k];
            let ch: bool = ph[k];
            let mut j: i64 = (k as i64) - 1;
            while j >= 0 && pt[j as usize] > ct {
                pt[(j + 1) as usize] = pt[j as usize];
                pv[(j + 1) as usize] = pv[j as usize];
                ph[(j + 1) as usize] = ph[j as usize];
                j -= 1;
            }
            pt[(j + 1) as usize] = ct;
            pv[(j + 1) as usize] = cv;
            ph[(j + 1) as usize] = ch;
            k += 1;
        }
        let mut times: Vec<f64> = Vec::new();
        for t in pt.iter() {
            if times.is_empty() || times[times.len() - 1] != *t {
                times.push(*t);
            }
        }
        if times.len() < 2 {
            return "#NUM!".to_string();
        }
        let ax: Vec<f64> = FormulaEts::detectAxis(&times);
        if ax.is_empty() {
            return "#NUM!".to_string();
        }
        let span: f64 = (FormulaEts::position(&ax, times[times.len() - 1]) + 0.5).floor();
        if !(span >= 1.0 && span < 200000.0) {
            return "#NUM!".to_string();
        }
        let len: usize = (span as usize) + 1;
        let mut slots: Vec<f64> = Vec::new();
        let mut known: Vec<bool> = Vec::new();
        i = 0;
        while i < len {
            slots.push(0.0);
            known.push(false);
            i += 1;
        }
        let mut group: Vec<f64> = Vec::new();
        let mut group_at: i64 = -1;
        i = 0;
        while i <= pt.len() {
            let mut p: i64 = -2;
            if i < pt.len() {
                let pf: f64 = (FormulaEts::position(&ax, pt[i]) + 0.5).floor();
                if !(pf >= 0.0 && pf < len as f64) {
                    return "#NUM!".to_string();
                }
                p = pf as i64;
            }
            if p != group_at {
                if group_at >= 0 {
                    let a: f64 = FormulaEts::aggregate(agg, &group);
                    if a > -1.0e308 {
                        slots[group_at as usize] = a;
                        known[group_at as usize] = true;
                    }
                }
                group = Vec::new();
                group_at = p;
            }
            if i < pt.len() && ph[i] {
                group.push(pv[i]);
            }
            i += 1;
        }
        let mut cnt: f64 = 0.0;
        for b in known.iter() {
            if *b {
                cnt += 1.0;
            }
        }
        if cnt == 0.0 || (len as f64) - cnt > 0.3 * (len as f64) {
            return "#NUM!".to_string();
        }
        // missing points: the line between known neighbours (zero without completion)
        i = 0;
        while i < len {
            if known[i] {
                y.push(slots[i]);
            } else if !interpolate {
                y.push(0.0);
            } else {
                let mut a: i64 = (i as i64) - 1;
                while a >= 0 && !known[a as usize] {
                    a -= 1;
                }
                let mut b: usize = i + 1;
                while b < len && !known[b] {
                    b += 1;
                }
                if a >= 0 && b < len {
                    let va2: f64 = slots[a as usize];
                    let vb: f64 = slots[b];
                    y.push(va2 + (vb - va2) * ((i as f64) - (a as f64)) / ((b as f64) - (a as f64)));
                } else if a >= 0 {
                    y.push(slots[a as usize]);
                } else {
                    y.push(slots[b]);
                }
            }
            i += 1;
        }
        for x in ax.iter() {
            axis.push(*x);
        }
        if ax[0] == 0.0 {
            axis.push(ax[2]);
        } else {
            axis.push((times[times.len() - 1] - times[0]) / span);
        }
        String::new()
    }

    // --- model ---------------------------------------------------------------------

    /// (intercept, slope) of the least-squares line through (xs, ys).
    pub fn line(xs: &Vec<f64>, ys: &Vec<f64>, out: &mut Vec<f64>) {
        let n: f64 = xs.len() as f64;
        if n == 0.0 {
            out.push(0.0);
            out.push(0.0);
            return;
        }
        let mut sx: f64 = 0.0;
        let mut sy: f64 = 0.0;
        let mut i: usize = 0;
        while i < xs.len() {
            sx += xs[i];
            sy += ys[i];
            i += 1;
        }
        let mx: f64 = sx / n;
        let my: f64 = sy / n;
        let mut sxy: f64 = 0.0;
        let mut sxx: f64 = 0.0;
        i = 0;
        while i < xs.len() {
            sxy += (xs[i] - mx) * (ys[i] - my);
            sxx += (xs[i] - mx) * (xs[i] - mx);
            i += 1;
        }
        let mut b: f64 = 0.0;
        if sxx > 0.0 {
            b = sxy / sxx;
        }
        out.push(my - b * mx);
        out.push(b);
    }
    pub fn detrend(y: &Vec<f64>) -> Vec<f64> {
        let mut xs: Vec<f64> = Vec::new();
        let mut i: usize = 0;
        while i < y.len() {
            xs.push(i as f64);
            i += 1;
        }
        let mut ab: Vec<f64> = Vec::new();
        FormulaEts::line(&xs, y, &mut ab);
        let mut out: Vec<f64> = Vec::new();
        i = 0;
        while i < y.len() {
            out.push(y[i] - (ab[0] + ab[1] * (i as f64)));
            i += 1;
        }
        out
    }
    /// The detected season length, 0 for none: the highest local peak of the
    /// detrended autocorrelation above 2/sqrt(n), at a lag up to n/2.
    pub fn detectSeason(y: &Vec<f64>) -> i64 {
        let n: i64 = y.len() as i64;
        if n < 4 {
            return 0;
        }
        let r: Vec<f64> = FormulaEts::detrend(y);
        let mut var: f64 = 0.0;
        let mut scale: f64 = 0.0;
        let mut i: i64 = 0;
        while i < n {
            var += r[i as usize] * r[i as usize];
            scale += y[i as usize] * y[i as usize];
            i += 1;
        }
        if scale < 1.0e-300 {
            scale = 1.0e-300;
        }
        if !(var > 1.0e-20 * scale) {
            return 0;
        }
        let mut max_lag: i64 = n / 2;
        if max_lag > 8760 {
            max_lag = 8760;
        }
        let mut top: i64 = max_lag + 1;
        if top > n - 1 {
            top = n - 1;
        }
        let mut acf: Vec<f64> = Vec::new();
        let mut k: i64 = 0;
        while k <= top {
            let mut s: f64 = 0.0;
            let mut j: i64 = k;
            while j < n {
                s += r[j as usize] * r[(j - k) as usize];
                j += 1;
            }
            acf.push(s / var);
            k += 1;
        }
        let bound: f64 = 2.0 / FormulaStats::sqrtD(n as f64);
        let mut best: i64 = 0;
        let mut best_r: f64 = bound;
        k = 2;
        while k <= max_lag {
            if k as usize >= acf.len() {
                break;
            }
            let prev: f64 = acf[(k - 1) as usize];
            let here: f64 = acf[k as usize];
            let mut next: f64 = -1.0e308;
            if ((k + 1) as usize) < acf.len() {
                next = acf[(k + 1) as usize];
            }
            if here > prev && here >= next && here > best_r {
                best = k;
                best_r = here;
            }
            k += 1;
        }
        best
    }
    /// Initial states: [level, trend, s_0 … s_{m-1}].
    pub fn initialStates(y: &Vec<f64>, m: i64) -> Vec<f64> {
        let n: i64 = y.len() as i64;
        let mut season: Vec<f64> = Vec::new();
        let mut i: i64 = 0;
        while i < m {
            season.push(0.0);
            i += 1;
        }
        if m >= 2 {
            let mut counts: Vec<f64> = Vec::new();
            let mut totals: Vec<f64> = Vec::new();
            i = 0;
            while i < m {
                counts.push(0.0);
                totals.push(0.0);
                i += 1;
            }
            if n >= 2 * m {
                let mut len: i64 = n;
                if len > 4 * m {
                    len = 4 * m;
                }
                let mut prefix: Vec<f64> = Vec::new();
                prefix.push(0.0);
                let mut acc: f64 = 0.0;
                i = 0;
                while i < len {
                    acc += y[i as usize];
                    prefix.push(acc);
                    i += 1;
                }
                let h: i64 = m / 2;
                let mut t: i64 = h;
                while t < len - h {
                    let mut ma: f64 = 0.0;
                    if m - (m / 2) * 2 == 1 {
                        ma = (prefix[(t + h + 1) as usize] - prefix[(t - h) as usize]) / (m as f64);
                    } else {
                        let ends: f64 = y[(t - h) as usize] + y[(t + h) as usize];
                        ma = (prefix[(t + h) as usize] - prefix[(t - h + 1) as usize] + ends / 2.0) / (m as f64);
                    }
                    let slot: usize = (t - (t / m) * m) as usize;
                    totals[slot] += y[t as usize] - ma;
                    counts[slot] += 1.0;
                    t += 1;
                }
            } else {
                let r: Vec<f64> = FormulaEts::detrend(y);
                let mut t: i64 = 0;
                while t < n {
                    let slot: usize = (t - (t / m) * m) as usize;
                    totals[slot] += r[t as usize];
                    counts[slot] += 1.0;
                    t += 1;
                }
            }
            let mut mean: f64 = 0.0;
            i = 0;
            while i < m {
                if counts[i as usize] > 0.0 {
                    season[i as usize] = totals[i as usize] / counts[i as usize];
                }
                mean += season[i as usize];
                i += 1;
            }
            mean /= m as f64;
            i = 0;
            while i < m {
                season[i as usize] -= mean;
                i += 1;
            }
        }
        let mut k: i64 = n;
        if k > 10 {
            k = 10;
        }
        let mut xs: Vec<f64> = Vec::new();
        let mut ys: Vec<f64> = Vec::new();
        i = 0;
        while i < k {
            let mut s: f64 = 0.0;
            if m >= 2 {
                s = season[(i - (i / m) * m) as usize];
            }
            xs.push((i + 1) as f64);
            ys.push(y[i as usize] - s);
            i += 1;
        }
        let mut ab: Vec<f64> = Vec::new();
        FormulaEts::line(&xs, &ys, &mut ab);
        let mut out: Vec<f64> = Vec::new();
        out.push(ab[0]);
        out.push(ab[1]);
        for s in season.iter() {
            out.push(*s);
        }
        out
    }
    /// Runs the filter: state [level, trend, sse, s_0 …]; errors when `keep`.
    pub fn runFilter(y: &Vec<f64>, init: &Vec<f64>, p: &Vec<f64>, errors: &mut Vec<f64>, keep: bool) -> Vec<f64> {
        let m: i64 = (init.len() as i64) - 2;
        let mut l: f64 = init[0];
        let mut b: f64 = init[1];
        let mut season: Vec<f64> = Vec::new();
        let mut i: usize = 2;
        while i < init.len() {
            season.push(init[i]);
            i += 1;
        }
        let mut sse: f64 = 0.0;
        let mut t: i64 = 0;
        while t < (y.len() as i64) {
            let mut s: f64 = 0.0;
            let mut slot: usize = 0;
            if m > 0 {
                slot = (t - (t / m) * m) as usize;
                s = season[slot];
            }
            let e: f64 = y[t as usize] - (l + b + s);
            if m > 0 {
                season[slot] = s + p[2] * e;
            }
            l = l + b + p[0] * e;
            b += p[0] * p[1] * e;
            sse += e * e;
            if keep {
                errors.push(e);
            }
            t += 1;
        }
        let mut out: Vec<f64> = Vec::new();
        out.push(l);
        out.push(b);
        out.push(sse);
        for s in season.iter() {
            out.push(*s);
        }
        out
    }
    pub fn clamp01(x: f64) -> f64 {
        if x < 0.0 {
            return 0.0;
        }
        if x > 1.0 {
            return 1.0;
        }
        x
    }
    pub fn clampP(p: &Vec<f64>, active: i64) -> Vec<f64> {
        let mut q: Vec<f64> = Vec::new();
        let mut i: i64 = 0;
        while i < 3 {
            if i < active {
                q.push(FormulaEts::clamp01(p[i as usize]));
            } else {
                q.push(0.0);
            }
            i += 1;
        }
        q
    }
    pub fn objective(y: &Vec<f64>, init: &Vec<f64>, p: &Vec<f64>, active: i64) -> f64 {
        let q: Vec<f64> = FormulaEts::clampP(p, active);
        let mut none: Vec<f64> = Vec::new();
        let st: Vec<f64> = FormulaEts::runFilter(y, init, &q, &mut none, false);
        let sse: f64 = st[2];
        if sse >= 0.0 && sse < 1.0e300 {
            return sse;
        }
        1.0e308
    }
    pub fn lerp(a: &Vec<f64>, b: &Vec<f64>, t: f64, active: i64) -> Vec<f64> {
        let mut o: Vec<f64> = Vec::new();
        let mut i: usize = 0;
        while i < 3 {
            o.push(a[i] + t * (b[i] - a[i]));
            i += 1;
        }
        FormulaEts::clampP(&o, active)
    }
    /// The smoothing parameters [a, B, g] minimising the squared one-step errors.
    pub fn optimise(y: &Vec<f64>, init: &Vec<f64>) -> Vec<f64> {
        let mut active: i64 = 3;
        if init.len() <= 2 {
            active = 2;
        }
        let grid: Vec<f64> = vec![0.05, 0.25, 0.5, 0.75, 0.95];
        let mut best: Vec<f64> = vec![0.5, 0.1, 0.1];
        let mut best_f: f64 = 1.0e308;
        let mut ng: usize = 1;
        if active == 3 {
            ng = 5;
        }
        for a in grid.iter() {
            for b in grid.iter() {
                let mut gi: usize = 0;
                while gi < ng {
                    let p: Vec<f64> = FormulaEts::clampP(&vec![*a, *b, grid[gi]], active);
                    let v: f64 = FormulaEts::objective(y, init, &p, active);
                    if v < best_f {
                        best_f = v;
                        best = p.clone();
                    }
                    gi += 1;
                }
            }
        }
        // Nelder-Mead from the best grid point, clamped to the box
        let mut sp: Vec<Vec<f64>> = Vec::new();
        let mut sf: Vec<f64> = Vec::new();
        sp.push(best.clone());
        sf.push(best_f);
        let mut i: i64 = 0;
        while i < active {
            let mut p: Vec<f64> = best.clone();
            if p[i as usize] > 0.5 {
                p[i as usize] -= 0.1;
            } else {
                p[i as usize] += 0.1;
            }
            let q: Vec<f64> = FormulaEts::clampP(&p, active);
            sf.push(FormulaEts::objective(y, init, &q, active));
            sp.push(q);
            i += 1;
        }
        let cnt: usize = sp.len();
        let mut iter: i64 = 0;
        while iter < 400 {
            // sort the simplex by value (insertion sort, stable)
            let mut k: usize = 1;
            while k < cnt {
                let cp: Vec<f64> = sp[k].clone();
                let cf: f64 = sf[k];
                let mut j: i64 = (k as i64) - 1;
                while j >= 0 && sf[j as usize] > cf {
                    sp[(j + 1) as usize] = sp[j as usize].clone();
                    sf[(j + 1) as usize] = sf[j as usize];
                    j -= 1;
                }
                sp[(j + 1) as usize] = cp;
                sf[(j + 1) as usize] = cf;
                k += 1;
            }
            let lo: Vec<f64> = sp[0].clone();
            let lo_f: f64 = sf[0];
            let hi: Vec<f64> = sp[cnt - 1].clone();
            let hi_f: f64 = sf[cnt - 1];
            let second: f64 = sf[cnt - 2];
            let mut size: f64 = 0.0;
            for s in sp.iter() {
                let mut d: usize = 0;
                while d < 3 {
                    let x: f64 = FormulaStats::absD(s[d] - lo[d]);
                    if x > size {
                        size = x;
                    }
                    d += 1;
                }
            }
            if FormulaStats::absD(hi_f - lo_f) <= 1.0e-12 * FormulaStats::absD(lo_f) + 1.0e-300 || size < 1.0e-7 {
                break;
            }
            let kf: f64 = (cnt - 1) as f64;
            let mut c: Vec<f64> = vec![0.0, 0.0, 0.0];
            k = 0;
            while k < cnt - 1 {
                let mut d: usize = 0;
                while d < 3 {
                    c[d] += sp[k][d] / kf;
                    d += 1;
                }
                k += 1;
            }
            let reflected: Vec<f64> = FormulaEts::lerp(&c, &hi, -1.0, active);
            let fr: f64 = FormulaEts::objective(y, init, &reflected, active);
            let mut rep_p: Vec<f64> = Vec::new();
            let mut rep_f: f64 = 0.0;
            if fr < lo_f {
                let expanded: Vec<f64> = FormulaEts::lerp(&c, &hi, -2.0, active);
                let fe: f64 = FormulaEts::objective(y, init, &expanded, active);
                if fe < fr {
                    rep_p = expanded;
                    rep_f = fe;
                } else {
                    rep_p = reflected;
                    rep_f = fr;
                }
            } else if fr < second {
                rep_p = reflected;
                rep_f = fr;
            } else {
                let contracted: Vec<f64> = FormulaEts::lerp(&c, &hi, 0.5, active);
                let fc: f64 = FormulaEts::objective(y, init, &contracted, active);
                if fc < hi_f {
                    rep_p = contracted;
                    rep_f = fc;
                }
            }
            if !rep_p.is_empty() {
                sp[cnt - 1] = rep_p;
                sf[cnt - 1] = rep_f;
            } else {
                k = 1;
                while k < cnt {
                    let p: Vec<f64> = FormulaEts::lerp(&lo, &sp[k].clone(), 0.5, active);
                    sf[k] = FormulaEts::objective(y, init, &p, active);
                    sp[k] = p;
                    k += 1;
                }
            }
            iter += 1;
        }
        let mut bi: usize = 0;
        let mut k2: usize = 1;
        while k2 < cnt {
            if sf[k2] < sf[bi] {
                bi = k2;
            }
            k2 += 1;
        }
        FormulaEts::clampP(&sp[bi].clone(), active)
    }
    /// The fitted model: [a, B, g, m, n, level, trend, sse, s_0 …]; errors out.
    pub fn fit(y: &Vec<f64>, m0: i64, errors: &mut Vec<f64>) -> Vec<f64> {
        let mut m: i64 = 0;
        if m0 >= 2 {
            m = m0;
        }
        let init: Vec<f64> = FormulaEts::initialStates(y, m);
        let p: Vec<f64> = FormulaEts::optimise(y, &init);
        let st: Vec<f64> = FormulaEts::runFilter(y, &init, &p, errors, true);
        let mut out: Vec<f64> = Vec::new();
        out.push(p[0]);
        out.push(p[1]);
        out.push(p[2]);
        out.push(m as f64);
        out.push(y.len() as f64);
        out.push(st[0]);
        out.push(st[1]);
        out.push(st[2]);
        let mut i: usize = 3;
        while i < st.len() {
            out.push(st[i]);
            i += 1;
        }
        out
    }
    pub fn at(f: &Vec<f64>, k: f64) -> f64 {
        let m: f64 = f[3];
        let mut s: f64 = 0.0;
        if m > 0.0 {
            let x: f64 = (f[4] - 1.0) + k;
            let idx: f64 = x - (x / m).floor() * m;
            s = f[(8.0 + idx) as usize];
        }
        f[5] + k * f[6] + s
    }
    pub fn forecast(f: &Vec<f64>, h: f64) -> f64 {
        let lo: f64 = h.floor();
        let a: f64 = FormulaEts::at(f, lo);
        if h == lo {
            return a;
        }
        a + (h - lo) * (FormulaEts::at(f, lo + 1.0) - a)
    }
    pub fn forecastSd(f: &Vec<f64>, h: f64) -> f64 {
        let mut nn: f64 = f[4];
        if nn < 1.0 {
            nn = 1.0;
        }
        let sigma2: f64 = f[7] / nn;
        let mut hc: f64 = h.ceil();
        if hc < 1.0 {
            hc = 1.0;
        }
        let k: f64 = hc - 1.0;
        let a: f64 = f[0];
        let b: f64 = f[0] * f[1];
        let g: f64 = f[2];
        let mut sum: f64 = k * a * a + a * b * k * (k + 1.0) + b * b * k * (k + 1.0) * (2.0 * k + 1.0) / 6.0;
        if f[3] > 0.0 {
            let m: f64 = f[3];
            let q: f64 = (k / m).floor();
            sum += 2.0 * g * (q * a + b * m * q * (q + 1.0) / 2.0) + g * g * q;
        }
        let v: f64 = sigma2 * (1.0 + sum);
        if v <= 0.0 {
            return 0.0;
        }
        FormulaStats::sqrtD(v)
    }

    // --- worksheet functions -------------------------------------------------------

    /// A whole-number option at i: -1 when missing, -2 when not a finite number.
    pub fn whole(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64) -> f64 {
        if !FormulaMore::has(args, i) {
            return -1.0;
        }
        let v: Rc<RefCell<FormulaValue>> = args[i as usize].borrow().single();
        if v.borrow().kind == 0 {
            return -1.0;
        }
        let x: f64 = v.borrow().asNumber();
        if v.borrow().kind == 2 && FormulaValue::parsePlainNumber(&v.borrow().text).is_none() {
            return -2.0;
        }
        x.floor()
    }
    pub fn invoke(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if !FormulaEts::isKnown(n) {
            return FormulaValue::blank();
        }
        let na: i64 = args.len() as i64;
        let mut vi: i64 = 1;
        let mut season_at: i64 = 3;
        let mut comp_at: i64 = 4;
        let mut agg_at: i64 = 5;
        let mut need: i64 = 3;
        if n == "FORECAST.ETS.CONFINT" {
            season_at = 4;
            comp_at = 5;
            agg_at = 6;
        } else if n == "FORECAST.ETS.SEASONALITY" {
            vi = 0;
            season_at = -1;
            comp_at = 2;
            agg_at = 3;
            need = 2;
        } else if n == "FORECAST.ETS.STAT" {
            vi = 0;
            season_at = 3;
            comp_at = 4;
            agg_at = 5;
        }
        if na < need {
            return FormulaEts::err("#VALUE!");
        }
        let mut i: i64 = 0;
        while i < na {
            if i != vi && i != vi + 1 {
                let one: Rc<RefCell<FormulaValue>> = args[i as usize].borrow().single();
                if one.borrow().kind == 4 {
                    return one;
                }
            }
            i += 1;
        }
        let mut season: f64 = 1.0;
        if season_at >= 0 {
            season = FormulaEts::whole(args, season_at);
            if season == -1.0 {
                season = 1.0;
            }
            if season < 0.0 || season > 8760.0 {
                return FormulaEts::err("#NUM!");
            }
        }
        let comp: f64 = FormulaEts::whole(args, comp_at);
        if comp != -1.0 && comp != 0.0 && comp != 1.0 {
            return FormulaEts::err("#NUM!");
        }
        let agg: f64 = FormulaEts::whole(args, agg_at);
        if agg != -1.0 && (agg < 0.0 || agg > 7.0) {
            return FormulaEts::err("#NUM!");
        }
        let mut agg_kind: i64 = 1;
        if agg >= 0.0 {
            agg_kind = agg as i64;
        }
        let mut y: Vec<f64> = Vec::new();
        let mut axis: Vec<f64> = Vec::new();
        let e: String = FormulaEts::series(&args[vi as usize].clone(), &args[(vi + 1) as usize].clone(), comp != 0.0, agg_kind, &mut y, &mut axis);
        if !e.is_empty() {
            return FormulaEts::err(&e);
        }
        let mut m: i64 = season as i64;
        if season == 1.0 {
            m = FormulaEts::detectSeason(&y);
        }
        if n == "FORECAST.ETS.SEASONALITY" {
            return FormulaValue::fromNumber(m as f64);
        }
        let mut errors: Vec<f64> = Vec::new();
        if n == "FORECAST.ETS.STAT" {
            let kind: f64 = FormulaMore::num(args, 2, 0.0).floor();
            if kind < 1.0 || kind > 8.0 {
                return FormulaEts::err("#NUM!");
            }
            if kind == 8.0 {
                return FormulaValue::fromNumber(axis[6]);
            }
            let f: Vec<f64> = FormulaEts::fit(&y, m, &mut errors);
            let mut cnt: f64 = errors.len() as f64;
            if cnt < 1.0 {
                cnt = 1.0;
            }
            let mut mae: f64 = 0.0;
            let mut sq: f64 = 0.0;
            for x in errors.iter() {
                mae += FormulaStats::absD(*x);
                sq += *x * *x;
            }
            mae /= cnt;
            if kind <= 3.0 {
                return FormulaValue::fromNumber(f[(kind - 1.0) as usize]);
            }
            if kind == 4.0 {
                let mut d: f64 = 0.0;
                let mut k: usize = 1;
                while k < y.len() {
                    d += FormulaStats::absD(y[k] - y[k - 1]);
                    k += 1;
                }
                let mut dn: f64 = (y.len() as f64) - 1.0;
                if dn < 1.0 {
                    dn = 1.0;
                }
                d /= dn;
                if d == 0.0 {
                    return FormulaEts::err("#DIV/0!");
                }
                return FormulaValue::fromNumber(mae / d);
            }
            if kind == 5.0 {
                let mut tot: f64 = 0.0;
                let mut tn: f64 = 0.0;
                let mut k: usize = 0;
                while k < y.len() {
                    let v: f64 = y[k];
                    let ee: f64 = errors[k];
                    let denom: f64 = (FormulaStats::absD(v) + FormulaStats::absD(v - ee)) / 2.0;
                    if denom > 0.0 {
                        tot += FormulaStats::absD(ee) / denom;
                        tn += 1.0;
                    }
                    k += 1;
                }
                if tn < 1.0 {
                    tn = 1.0;
                }
                return FormulaValue::fromNumber(tot / tn);
            }
            if kind == 6.0 {
                return FormulaValue::fromNumber(mae);
            }
            return FormulaValue::fromNumber(FormulaStats::sqrtD(sq / cnt));
        }
        // FORECAST.ETS / .CONFINT: the target's distance past the last point
        let tv: Rc<RefCell<FormulaValue>> = args[0].borrow().single();
        if tv.borrow().kind != 1 {
            return FormulaEts::err("#VALUE!");
        }
        let p: f64 = FormulaEts::position(&axis, tv.borrow().num);
        let mut h: f64 = p - ((y.len() as f64) - 1.0);
        if h < -1.0e-9 {
            return FormulaEts::err("#NUM!");
        }
        if h < 0.0 {
            h = 0.0;
        }
        let f: Vec<f64> = FormulaEts::fit(&y, m, &mut errors);
        if n == "FORECAST.ETS" {
            return FormulaValue::fromNumber(FormulaEts::forecast(&f, h));
        }
        let mut level: f64 = 0.95;
        if FormulaMore::has(args, 3) {
            level = FormulaMore::num(args, 3, 0.95);
        }
        if !(level > 0.0 && level < 1.0) {
            return FormulaEts::err("#NUM!");
        }
        let z: f64 = FormulaStats::normInv((1.0 + level) / 2.0);
        FormulaValue::fromNumber(z * FormulaEts::forecastSd(&f, h))
    }
}
