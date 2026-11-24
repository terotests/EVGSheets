// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;
use crate::formula_value::FormulaValue;
use crate::formula_functions::FormulaFunctions;
use crate::formula_more::FormulaMore;

/// RegexProg - a small backtracking regular expression engine for REGEXTEST,
/// REGEXEXTRACT and REGEXREPLACE, close to the PCRE2 syntax Excel uses:
/// literals and escapes (\d \w \s \b \xHH \uHHHH …), classes with ranges and
/// POSIX names, groups (capturing, named, non-capturing), alternation, greedy
/// and lazy quantifiers including {m,n}, anchors, look-ahead and look-behind,
/// back references and the inline flags (?i) (?s) (?m).
///
/// The pattern compiles to instructions with RELATIVE jump offsets, so a
/// quantified atom's code is repeated by copying it. Matching walks the
/// instructions and recurses only where it has to come back: a split, a
/// capture save and a loop register.
#[derive(Clone)]
pub struct RegexProg {
    pub src: String,
    pub pos: i64,
    pub len: i64,
    pub op: Vec<i64>,
    pub a: Vec<i64>,
    pub b: Vec<i64>,
    pub cls: Vec<i64>,
    pub groups: i64,
    pub regs: i64,
    pub names: Vec<String>,
    pub nameIdx: Vec<i64>,
    pub icase: bool,
    pub dotall: bool,
    pub multiline: bool,
    pub ok: bool,
    pub text: String,
    pub tlen: i64,
    pub saves: Vec<i64>,
    pub regv: Vec<i64>,
    pub lookEnds: Vec<i64>,
    pub steps: i64,
}

// instruction codes
// 0 CHAR a=code b=icase | 1 ANY | 2 CLASS a=class b=icase | 3 SPLIT a b
// 4 JMP a | 5 SAVE a=slot | 6 MATCH | 7 BOL | 8 EOL | 9 WORDB | 10 NWORDB
// 11 SETR a=reg | 12 LOOPR a=reg b=back | 13 LOOK a=neg b=len
// 14 BACKREF a=group b=icase | 15 ANYNL | 16 TEXTSTART | 17 TEXTEND
// 18 LOOKB a=neg b=len | 19 ENDAT | 20 TEXTENDNL

impl RegexProg {
    pub fn new() -> Self {
        Self {
            src: String::new(),
            pos: 0,
            len: 0,
            op: Vec::new(),
            a: Vec::new(),
            b: Vec::new(),
            cls: Vec::new(),
            groups: 0,
            regs: 0,
            names: Vec::new(),
            nameIdx: Vec::new(),
            icase: false,
            dotall: false,
            multiline: false,
            ok: true,
            text: String::new(),
            tlen: 0,
            saves: Vec::new(),
            regv: Vec::new(),
            lookEnds: Vec::new(),
            steps: 0,
        }
    }

    // --- compiling -----------------------------------------------------------------

    pub fn compile(&mut self, pattern: &str, icase: bool) -> bool {
        self.src = pattern.to_string();
        self.pos = 0;
        self.len = DgNative::str_len(pattern);
        self.icase = icase;
        self.ok = true;
        self.parseAlt();
        if self.pos < self.len {
            self.ok = false;
        }
        self.emit(6, 0, 0);
        self.ok
    }
    pub fn peek(&self) -> i64 {
        if self.pos >= self.len {
            return -1;
        }
        DgNative::char_at(&self.src, self.pos)
    }
    pub fn peekAt(&self, k: i64) -> i64 {
        if self.pos + k >= self.len {
            return -1;
        }
        DgNative::char_at(&self.src, self.pos + k)
    }
    pub fn emit(&mut self, o: i64, x: i64, y: i64) {
        self.op.push(o);
        self.a.push(x);
        self.b.push(y);
    }
    pub fn size(&self) -> i64 {
        self.op.len() as i64
    }
    pub fn cut(&mut self, n: i64) {
        while self.size() > n {
            self.op.pop();
            self.a.pop();
            self.b.pop();
        }
    }
    pub fn insertAt(&mut self, at: i64, o: i64, x: i64, y: i64) {
        self.emit(0, 0, 0);
        let mut i: i64 = self.size() - 1;
        while i > at {
            let p: usize = (i - 1) as usize;
            let q: usize = i as usize;
            self.op[q] = self.op[p];
            self.a[q] = self.a[p];
            self.b[q] = self.b[p];
            i -= 1;
        }
        let k: usize = at as usize;
        self.op[k] = o;
        self.a[k] = x;
        self.b[k] = y;
    }
    pub fn parseAlt(&mut self) {
        let start: i64 = self.size();
        self.parseSeq();
        while self.ok && self.peek() == 124 {
            self.pos += 1;
            let l1: i64 = self.size() - start;
            self.insertAt(start, 3, 1, l1 + 2);
            let jmp_at: i64 = self.size();
            self.emit(4, 0, 0);
            let s2: i64 = self.size();
            self.parseSeq();
            let l2: i64 = self.size() - s2;
            self.a[jmp_at as usize] = l2 + 1;
        }
    }
    pub fn parseSeq(&mut self) {
        while self.ok && self.pos < self.len {
            let c: i64 = self.peek();
            if c == 124 || c == 41 {
                return;
            }
            self.parseQuantified();
        }
    }
    pub fn readInt(&mut self) -> i64 {
        let mut v: i64 = -1;
        while self.pos < self.len {
            let c: i64 = self.peek();
            if c < 48 || c > 57 {
                break;
            }
            if v < 0 {
                v = 0;
            }
            v = v * 10 + (c - 48);
            if v > 100000 {
                v = 100000;
            }
            self.pos += 1;
        }
        v
    }
    pub fn parseQuantified(&mut self) {
        let start: i64 = self.size();
        self.parseAtom();
        if !self.ok {
            return;
        }
        let c: i64 = self.peek();
        let mut lo: i64 = -1;
        let mut hi: i64 = -1;
        if c == 42 {
            lo = 0;
            hi = -2;
            self.pos += 1;
        } else if c == 43 {
            lo = 1;
            hi = -2;
            self.pos += 1;
        } else if c == 63 {
            lo = 0;
            hi = 1;
            self.pos += 1;
        } else if c == 123 {
            let save: i64 = self.pos;
            self.pos += 1;
            let m: i64 = self.readInt();
            let mut good: bool = m >= 0;
            let mut n: i64 = m;
            if good && self.peek() == 44 {
                self.pos += 1;
                n = self.readInt();
                if n < 0 {
                    n = -2;
                }
            }
            if good && self.peek() == 125 {
                self.pos += 1;
            } else {
                good = false;
            }
            if good && n != -2 && n < m {
                self.ok = false;
                return;
            }
            if good {
                lo = m;
                hi = n;
            } else {
                self.pos = save;
            }
        }
        if lo < 0 {
            return;
        }
        let mut lazy: bool = false;
        if self.peek() == 63 {
            lazy = true;
            self.pos += 1;
        } else if self.peek() == 43 {
            self.pos += 1;
        }
        // the atom's code, removed and then emitted again as often as needed
        let mut co: Vec<i64> = Vec::new();
        let mut ca: Vec<i64> = Vec::new();
        let mut cb: Vec<i64> = Vec::new();
        let mut i: i64 = start;
        while i < self.size() {
            co.push(self.op[i as usize]);
            ca.push(self.a[i as usize]);
            cb.push(self.b[i as usize]);
            i += 1;
        }
        let l: i64 = co.len() as i64;
        self.cut(start);
        let mut copies: i64 = lo;
        if hi > lo {
            copies = hi;
        }
        if (copies + 1) * (l + 3) > 200000 {
            self.ok = false;
            return;
        }
        let mut k: i64 = 0;
        while k < lo {
            self.emitCode(&co, &ca, &cb);
            k += 1;
        }
        if hi == -2 {
            let r: i64 = self.regs;
            self.regs += 1;
            if lazy {
                self.emit(3, l + 3, 1);
            } else {
                self.emit(3, 1, l + 3);
            }
            self.emit(11, r, 0);
            self.emitCode(&co, &ca, &cb);
            self.emit(12, r, 0 - (l + 2));
            return;
        }
        while k < hi {
            if lazy {
                self.emit(3, l + 1, 1);
            } else {
                self.emit(3, 1, l + 1);
            }
            self.emitCode(&co, &ca, &cb);
            k += 1;
        }
    }
    pub fn emitCode(&mut self, co: &Vec<i64>, ca: &Vec<i64>, cb: &Vec<i64>) {
        let mut i: usize = 0;
        while i < co.len() {
            self.emit(co[i], ca[i], cb[i]);
            i += 1;
        }
    }
    pub fn hexVal(c: i64) -> i64 {
        if c >= 48 && c <= 57 {
            return c - 48;
        }
        if c >= 65 && c <= 70 {
            return c - 55;
        }
        if c >= 97 && c <= 102 {
            return c - 87;
        }
        -1
    }
    /// The code of an escaped character after the backslash, or -1 for a class
    /// shorthand (-10 \d, -11 \w, -12 \s, -13 \D, -14 \W, -15 \S).
    pub fn escapeCode(&mut self) -> i64 {
        let c: i64 = self.peek();
        if c < 0 {
            self.ok = false;
            return 0;
        }
        self.pos += 1;
        if c == 100 {
            return -10;
        }
        if c == 119 {
            return -11;
        }
        if c == 115 {
            return -12;
        }
        if c == 68 {
            return -13;
        }
        if c == 87 {
            return -14;
        }
        if c == 83 {
            return -15;
        }
        if c == 110 {
            return 10;
        }
        if c == 116 {
            return 9;
        }
        if c == 114 {
            return 13;
        }
        if c == 102 {
            return 12;
        }
        if c == 118 {
            return 11;
        }
        if c == 101 {
            return 27;
        }
        if c == 48 {
            return 0;
        }
        if c == 120 || c == 117 {
            let mut v: i64 = 0;
            if self.peek() == 123 {
                self.pos += 1;
                while self.pos < self.len && self.peek() != 125 {
                    let h: i64 = RegexProg::hexVal(self.peek());
                    if h < 0 {
                        self.ok = false;
                        return 0;
                    }
                    v = v * 16 + h;
                    self.pos += 1;
                }
                self.pos += 1;
                return v;
            }
            let mut want: i64 = 2;
            if c == 117 {
                want = 4;
            }
            let mut k: i64 = 0;
            while k < want {
                let h: i64 = RegexProg::hexVal(self.peek());
                if h < 0 {
                    self.ok = false;
                    return 0;
                }
                v = v * 16 + h;
                self.pos += 1;
                k += 1;
            }
            return v;
        }
        c
    }
    pub fn addShorthand(&mut self, code: i64) {
        // a shorthand as range pairs; the negated ones as a special pair
        self.cls.push(code);
        self.cls.push(code);
    }
    pub fn parseAtom(&mut self) {
        let c: i64 = self.peek();
        self.pos += 1;
        if c == 40 {
            self.parseGroup();
            return;
        }
        if c == 91 {
            let ci: i64 = self.parseClass();
            let ic: i64 = if self.icase { 1 } else { 0 };
            self.emit(2, ci, ic);
            return;
        }
        if c == 46 {
            if self.dotall {
                self.emit(15, 0, 0);
            } else {
                self.emit(1, 0, 0);
            }
            return;
        }
        if c == 94 {
            self.emit(7, 0, 0);
            return;
        }
        if c == 36 {
            self.emit(8, 0, 0);
            return;
        }
        if c == 41 || c == 42 || c == 43 || c == 63 {
            self.ok = false;
            return;
        }
        if c == 92 {
            let e: i64 = self.peek();
            if e == 98 || e == 66 || e == 65 || e == 122 || e == 90 {
                self.pos += 1;
                if e == 98 {
                    self.emit(9, 0, 0);
                } else if e == 66 {
                    self.emit(10, 0, 0);
                } else if e == 65 {
                    self.emit(16, 0, 0);
                } else if e == 122 {
                    self.emit(17, 0, 0);
                } else {
                    self.emit(20, 0, 0);
                }
                return;
            }
            if e >= 49 && e <= 57 {
                let g: i64 = self.readInt();
                let ic: i64 = if self.icase { 1 } else { 0 };
                self.emit(14, g, ic);
                return;
            }
            if e == 107 && self.peekAt(1) == 60 {
                // \k<name>
                self.pos += 2;
                let nm: String = self.readName(62);
                let g: i64 = self.groupOf(&nm);
                if g < 0 {
                    self.ok = false;
                    return;
                }
                let ic: i64 = if self.icase { 1 } else { 0 };
                self.emit(14, g, ic);
                return;
            }
            let code: i64 = self.escapeCode();
            if code <= -10 {
                let ci: i64 = self.cls.len() as i64;
                self.cls.push(0);
                self.cls.push(1);
                self.addShorthand(code);
                self.emit(2, ci, 0);
                return;
            }
            let ic: i64 = if self.icase { 1 } else { 0 };
            self.emit(0, code, ic);
            return;
        }
        let ic: i64 = if self.icase { 1 } else { 0 };
        self.emit(0, c, ic);
    }
    pub fn readName(&mut self, close: i64) -> String {
        let s: i64 = self.pos;
        while self.pos < self.len && self.peek() != close {
            self.pos += 1;
        }
        let nm: String = DgNative::substr(&self.src, s, self.pos);
        if self.pos < self.len {
            self.pos += 1;
        } else {
            self.ok = false;
        }
        nm
    }
    pub fn groupOf(&self, nm: &str) -> i64 {
        let mut i: usize = 0;
        while i < self.names.len() {
            if self.names[i] == nm {
                return self.nameIdx[i];
            }
            i += 1;
        }
        -1
    }
    pub fn parseGroup(&mut self) {
        if self.peek() != 63 {
            self.groups += 1;
            let g: i64 = self.groups;
            self.emit(5, g * 2, 0);
            self.parseAlt();
            self.closeParen();
            self.emit(5, g * 2 + 1, 0);
            return;
        }
        self.pos += 1;
        let c: i64 = self.peek();
        if c == 58 {
            self.pos += 1;
            self.parseAlt();
            self.closeParen();
            return;
        }
        if c == 61 || c == 33 {
            self.pos += 1;
            let at: i64 = self.size();
            let neg: i64 = if c == 33 { 1 } else { 0 };
            self.emit(13, neg, 0);
            self.parseAlt();
            self.closeParen();
            self.emit(6, 0, 0);
            self.b[at as usize] = self.size() - at - 1;
            return;
        }
        if c == 60 && (self.peekAt(1) == 61 || self.peekAt(1) == 33) {
            let neg: i64 = if self.peekAt(1) == 33 { 1 } else { 0 };
            self.pos += 2;
            let at: i64 = self.size();
            self.emit(18, neg, 0);
            self.parseAlt();
            self.closeParen();
            self.emit(19, 0, 0);
            self.emit(6, 0, 0);
            self.b[at as usize] = self.size() - at - 1;
            return;
        }
        if c == 60 || c == 39 || c == 80 {
            // (?<name>…) (?'name'…) (?P<name>…)
            if c == 80 {
                self.pos += 1;
            }
            let open: i64 = self.peek();
            self.pos += 1;
            let close: i64 = if open == 39 { 39 } else { 62 };
            let nm: String = self.readName(close);
            self.groups += 1;
            let g: i64 = self.groups;
            self.names.push(nm.clone());
            self.nameIdx.push(g);
            self.emit(5, g * 2, 0);
            self.parseAlt();
            self.closeParen();
            self.emit(5, g * 2 + 1, 0);
            return;
        }
        // inline flags (?ims-ims) or (?ims-ims:…)
        let mut on: bool = true;
        while self.pos < self.len {
            let f: i64 = self.peek();
            self.pos += 1;
            if f == 41 {
                return;
            }
            if f == 58 {
                self.parseAlt();
                self.closeParen();
                return;
            }
            if f == 45 {
                on = false;
            } else if f == 105 {
                self.icase = on;
            } else if f == 115 {
                self.dotall = on;
            } else if f == 109 {
                self.multiline = on;
            } else if f != 120 && f != 110 && f != 85 {
                self.ok = false;
                return;
            }
        }
        self.ok = false;
    }
    pub fn closeParen(&mut self) {
        if self.peek() == 41 {
            self.pos += 1;
        } else {
            self.ok = false;
        }
    }
    pub fn posixClass(&mut self, nm: &str) -> bool {
        if nm == "alpha" {
            self.cls.push(65);
            self.cls.push(90);
            self.cls.push(97);
            self.cls.push(122);
            return true;
        }
        if nm == "digit" {
            self.cls.push(48);
            self.cls.push(57);
            return true;
        }
        if nm == "alnum" {
            self.cls.push(48);
            self.cls.push(57);
            self.cls.push(65);
            self.cls.push(90);
            self.cls.push(97);
            self.cls.push(122);
            return true;
        }
        if nm == "upper" {
            self.cls.push(65);
            self.cls.push(90);
            return true;
        }
        if nm == "lower" {
            self.cls.push(97);
            self.cls.push(122);
            return true;
        }
        if nm == "space" {
            self.cls.push(9);
            self.cls.push(13);
            self.cls.push(32);
            self.cls.push(32);
            return true;
        }
        if nm == "xdigit" {
            self.cls.push(48);
            self.cls.push(57);
            self.cls.push(65);
            self.cls.push(70);
            self.cls.push(97);
            self.cls.push(102);
            return true;
        }
        if nm == "punct" {
            self.cls.push(33);
            self.cls.push(47);
            self.cls.push(58);
            self.cls.push(64);
            self.cls.push(91);
            self.cls.push(96);
            self.cls.push(123);
            self.cls.push(126);
            return true;
        }
        if nm == "word" {
            self.addShorthand(-11);
            return true;
        }
        false
    }
    /// Parses a class after '[' into `cls`: [negated, pairs, lo, hi, …].
    pub fn parseClass(&mut self) -> i64 {
        let ci: i64 = self.cls.len() as i64;
        let mut neg: i64 = 0;
        if self.peek() == 94 {
            neg = 1;
            self.pos += 1;
        }
        self.cls.push(neg);
        self.cls.push(0);
        let mut first: bool = true;
        while self.ok {
            if self.pos >= self.len {
                self.ok = false;
                break;
            }
            let c: i64 = self.peek();
            if c == 93 && !first {
                self.pos += 1;
                break;
            }
            first = false;
            if c == 91 && self.peekAt(1) == 58 {
                let save: i64 = self.pos;
                self.pos += 2;
                let nm: String = self.readName(58);
                if self.peek() == 93 && self.posixClass(&nm) {
                    self.pos += 1;
                    continue;
                }
                self.pos = save;
            }
            self.pos += 1;
            let mut lo: i64 = c;
            if c == 92 {
                lo = self.escapeCode();
                if lo <= -10 {
                    self.addShorthand(lo);
                    continue;
                }
            }
            let mut hi: i64 = lo;
            if self.peek() == 45 && self.peekAt(1) != 93 && self.peekAt(1) >= 0 {
                self.pos += 1;
                hi = self.peek();
                self.pos += 1;
                if hi == 92 {
                    hi = self.escapeCode();
                    if hi <= -10 {
                        self.ok = false;
                        return ci;
                    }
                }
                if hi < lo {
                    self.ok = false;
                    return ci;
                }
            }
            self.cls.push(lo);
            self.cls.push(hi);
        }
        let pairs: i64 = ((self.cls.len() as i64) - ci - 2) / 2;
        self.cls[(ci + 1) as usize] = pairs;
        ci
    }

    // --- matching ------------------------------------------------------------------

    pub fn lower(c: i64) -> i64 {
        if c >= 65 && c <= 90 {
            return c + 32;
        }
        if c >= 192 && c <= 222 && c != 215 {
            return c + 32;
        }
        c
    }
    pub fn upper(c: i64) -> i64 {
        if c >= 97 && c <= 122 {
            return c - 32;
        }
        if c >= 224 && c <= 254 && c != 247 {
            return c - 32;
        }
        c
    }
    pub fn isWord(c: i64) -> bool {
        (c >= 48 && c <= 57) || (c >= 65 && c <= 90) || (c >= 97 && c <= 122) || c == 95
    }
    pub fn isSpace(c: i64) -> bool {
        (c >= 9 && c <= 13) || c == 32
    }
    pub fn shortHas(code: i64, c: i64) -> bool {
        if code == -10 {
            return c >= 48 && c <= 57;
        }
        if code == -11 {
            return RegexProg::isWord(c);
        }
        if code == -12 {
            return RegexProg::isSpace(c);
        }
        if code == -13 {
            return !(c >= 48 && c <= 57);
        }
        if code == -14 {
            return !RegexProg::isWord(c);
        }
        !RegexProg::isSpace(c)
    }
    pub fn classHas1(&self, ci: i64, c: i64) -> bool {
        let pairs: i64 = self.cls[(ci + 1) as usize];
        let mut k: i64 = 0;
        while k < pairs {
            let lo: i64 = self.cls[(ci + 2 + k * 2) as usize];
            let hi: i64 = self.cls[(ci + 3 + k * 2) as usize];
            if lo <= -10 {
                if RegexProg::shortHas(lo, c) {
                    return true;
                }
            } else if c >= lo && c <= hi {
                return true;
            }
            k += 1;
        }
        false
    }
    pub fn classHas(&self, ci: i64, c: i64, ic: bool) -> bool {
        let mut hit: bool = self.classHas1(ci, c);
        if !hit && ic {
            hit = self.classHas1(ci, RegexProg::lower(c)) || self.classHas1(ci, RegexProg::upper(c));
        }
        if self.cls[ci as usize] == 1 {
            return !hit;
        }
        hit
    }
    pub fn ch(&self, i: i64) -> i64 {
        if i < 0 || i >= self.tlen {
            return -1;
        }
        DgNative::char_at(&self.text, i)
    }
    /// Runs the program from `pc` at text position `sp`; the end of the match, or -1.
    pub fn run(&mut self, pc0: i64, sp0: i64) -> i64 {
        let mut pc: i64 = pc0;
        let mut sp: i64 = sp0;
        loop {
            self.steps += 1;
            if self.steps > 2000000 {
                self.ok = false;
                return -1;
            }
            let o: i64 = self.op[pc as usize];
            let x: i64 = self.a[pc as usize];
            let y: i64 = self.b[pc as usize];
            if o == 0 {
                let c: i64 = self.ch(sp);
                if c < 0 {
                    return -1;
                }
                if c != x && !(y == 1 && RegexProg::lower(c) == RegexProg::lower(x)) {
                    return -1;
                }
                pc += 1;
                sp += 1;
            } else if o == 1 || o == 15 {
                let c: i64 = self.ch(sp);
                if c < 0 || (o == 1 && c == 10) {
                    return -1;
                }
                pc += 1;
                sp += 1;
            } else if o == 2 {
                let c: i64 = self.ch(sp);
                if c < 0 || !self.classHas(x, c, y == 1) {
                    return -1;
                }
                pc += 1;
                sp += 1;
            } else if o == 3 {
                let r: i64 = self.run(pc + x, sp);
                if r >= 0 || !self.ok {
                    return r;
                }
                pc += y;
            } else if o == 4 {
                pc += x;
            } else if o == 5 {
                let old: i64 = self.saves[x as usize];
                self.saves[x as usize] = sp;
                let r: i64 = self.run(pc + 1, sp);
                if r >= 0 {
                    return r;
                }
                self.saves[x as usize] = old;
                return -1;
            } else if o == 6 {
                return sp;
            } else if o == 7 {
                if sp != 0 && !(self.multiline && self.ch(sp - 1) == 10) {
                    return -1;
                }
                pc += 1;
            } else if o == 8 {
                let c: i64 = self.ch(sp);
                let at_end: bool = sp == self.tlen || (sp == self.tlen - 1 && c == 10);
                if !at_end && !(self.multiline && c == 10) {
                    return -1;
                }
                pc += 1;
            } else if o == 9 || o == 10 {
                let w1: bool = RegexProg::isWord(self.ch(sp - 1));
                let w2: bool = RegexProg::isWord(self.ch(sp));
                if (w1 != w2) != (o == 9) {
                    return -1;
                }
                pc += 1;
            } else if o == 11 {
                let old: i64 = self.regv[x as usize];
                self.regv[x as usize] = sp;
                let r: i64 = self.run(pc + 1, sp);
                self.regv[x as usize] = old;
                return r;
            } else if o == 12 {
                if sp != self.regv[x as usize] {
                    pc += y;
                } else {
                    pc += 1;
                }
            } else if o == 13 {
                let r: i64 = self.run(pc + 1, sp);
                if !self.ok {
                    return -1;
                }
                if (r >= 0) == (x == 1) {
                    return -1;
                }
                pc += 1 + y;
            } else if o == 18 {
                let mut found: bool = false;
                let mut k: i64 = sp;
                while k >= 0 && !found {
                    self.lookEnds.push(sp);
                    let r: i64 = self.run(pc + 1, k);
                    self.lookEnds.pop();
                    if r >= 0 {
                        found = true;
                    }
                    k -= 1;
                }
                if !self.ok || found == (x == 1) {
                    return -1;
                }
                pc += 1 + y;
            } else if o == 19 {
                let n: usize = self.lookEnds.len();
                if n == 0 || self.lookEnds[n - 1] != sp {
                    return -1;
                }
                pc += 1;
            } else if o == 14 {
                let mut gs: i64 = -1;
                if x * 2 + 1 < (self.saves.len() as i64) {
                    gs = self.saves[(x * 2) as usize];
                }
                if gs < 0 {
                    return -1;
                }
                let ge: i64 = self.saves[(x * 2 + 1) as usize];
                if ge < gs {
                    return -1;
                }
                let mut k: i64 = 0;
                while k < ge - gs {
                    let c1: i64 = self.ch(gs + k);
                    let c2: i64 = self.ch(sp + k);
                    if c2 < 0 {
                        return -1;
                    }
                    if c1 != c2 && !(y == 1 && RegexProg::lower(c1) == RegexProg::lower(c2)) {
                        return -1;
                    }
                    k += 1;
                }
                sp += ge - gs;
                pc += 1;
            } else if o == 16 {
                if sp != 0 {
                    return -1;
                }
                pc += 1;
            } else if o == 17 {
                if sp != self.tlen {
                    return -1;
                }
                pc += 1;
            } else {
                if !(sp == self.tlen || (sp == self.tlen - 1 && self.ch(sp) == 10)) {
                    return -1;
                }
                pc += 1;
            }
        }
    }
    pub fn setText(&mut self, t: &str) {
        self.text = t.to_string();
        self.tlen = DgNative::str_len(t);
    }
    /// The first match at or after `from`: saves[0], saves[1] and the groups.
    pub fn exec(&mut self, from: i64) -> bool {
        let mut s: i64 = from;
        while s <= self.tlen {
            self.saves = Vec::new();
            let mut i: i64 = 0;
            while i < (self.groups + 1) * 2 {
                self.saves.push(-1);
                i += 1;
            }
            self.regv = Vec::new();
            i = 0;
            while i < self.regs {
                self.regv.push(-1);
                i += 1;
            }
            let r: i64 = self.run(0, s);
            if !self.ok {
                return false;
            }
            if r >= 0 {
                self.saves[0] = s;
                self.saves[1] = r;
                return true;
            }
            s += 1;
        }
        false
    }
    pub fn group(&self, g: i64) -> String {
        if g < 0 || g > self.groups {
            return String::new();
        }
        let s: i64 = self.saves[(g * 2) as usize];
        let e: i64 = self.saves[(g * 2 + 1) as usize];
        if s < 0 || e < s {
            return String::new();
        }
        DgNative::substr(&self.text, s, e)
    }
}

/// FormulaRegex - REGEXTEST, REGEXEXTRACT and REGEXREPLACE on RegexProg.
#[derive(Clone)]
pub struct FormulaRegex {
}

impl FormulaRegex {
    pub fn new() -> Self {
        Self {
        }
    }
    pub fn names() -> String {
        " REGEXTEST REGEXEXTRACT REGEXREPLACE ".to_string()
    }
    pub fn isKnown(n: &str) -> bool {
        DgNative::index_of(&FormulaRegex::names(), &format!(" {} ", n)) >= 0
    }
    pub fn err(e: &str) -> Rc<RefCell<FormulaValue>> {
        FormulaValue::fromError(e.to_string())
    }
    /// The case argument at `i`: 0 (or missing) matches case, 1 ignores it, -1 is an error.
    pub fn caseArg(args: &Vec<Rc<RefCell<FormulaValue>>>, i: i64) -> i64 {
        if !FormulaMore::has(args, i) {
            return 0;
        }
        let v: f64 = FormulaMore::num(args, i, 0.0).floor();
        if v == 0.0 {
            return 0;
        }
        if v == 1.0 {
            return 1;
        }
        -1
    }
    pub fn invoke(n: &str, args: &Vec<Rc<RefCell<FormulaValue>>>) -> Rc<RefCell<FormulaValue>> {
        if !FormulaRegex::isKnown(n) {
            return FormulaValue::blank();
        }
        let e: Rc<RefCell<FormulaValue>> = FormulaFunctions::firstError(args);
        if e.borrow().isError() {
            return e;
        }
        let na: i64 = args.len() as i64;
        if na < 2 || (n == "REGEXREPLACE" && na < 3) {
            return FormulaRegex::err("#VALUE!");
        }
        let text: String = FormulaMore::txt(args, 0);
        let pattern: String = FormulaMore::txt(args, 1);
        let mut case_at: i64 = 2;
        if n == "REGEXEXTRACT" {
            case_at = 3;
        }
        if n == "REGEXREPLACE" {
            case_at = 4;
        }
        let cs: i64 = FormulaRegex::caseArg(args, case_at);
        if cs < 0 {
            return FormulaRegex::err("#VALUE!");
        }
        let re: Rc<RefCell<RegexProg>> = Rc::new(RefCell::new(RegexProg::new()));
        if !re.borrow_mut().compile(&pattern, cs == 1) {
            return FormulaRegex::err("#VALUE!");
        }
        re.borrow_mut().setText(&text);
        if n == "REGEXTEST" {
            let hit: bool = re.borrow_mut().exec(0);
            if !re.borrow().ok {
                return FormulaRegex::err("#VALUE!");
            }
            return FormulaValue::fromBool(hit);
        }
        if n == "REGEXEXTRACT" {
            let mode: f64 = FormulaMore::num(args, 2, 0.0).floor();
            if mode != 0.0 && mode != 1.0 && mode != 2.0 {
                return FormulaRegex::err("#VALUE!");
            }
            if mode == 1.0 {
                let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
                let mut from: i64 = 0;
                let tlen: i64 = re.borrow().tlen;
                while from <= tlen {
                    if !re.borrow_mut().exec(from) {
                        break;
                    }
                    let s: i64 = re.borrow().saves[0];
                    let f: i64 = re.borrow().saves[1];
                    items.push(FormulaValue::fromString(re.borrow().group(0)));
                    if f == s {
                        from = f + 1;
                    } else {
                        from = f;
                    }
                }
                if !re.borrow().ok {
                    return FormulaRegex::err("#VALUE!");
                }
                if items.is_empty() {
                    return FormulaRegex::err("#N/A");
                }
                let cnt: i64 = items.len() as i64;
                return FormulaValue::fromArray(cnt, 1, &items);
            }
            if !re.borrow_mut().exec(0) {
                if !re.borrow().ok {
                    return FormulaRegex::err("#VALUE!");
                }
                return FormulaRegex::err("#N/A");
            }
            let groups: i64 = re.borrow().groups;
            if mode == 0.0 || groups == 0 {
                return FormulaValue::fromString(re.borrow().group(0));
            }
            let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
            let mut g: i64 = 1;
            while g <= groups {
                items.push(FormulaValue::fromString(re.borrow().group(g)));
                g += 1;
            }
            return FormulaValue::fromArray(1, groups, &items);
        }
        // REGEXREPLACE(text, pattern, replacement, [occurrence], [case])
        let rep: String = FormulaMore::txt(args, 2);
        let occ: i64 = FormulaMore::num(args, 3, 0.0).floor() as i64;
        let mut starts: Vec<i64> = Vec::new();
        let mut ends: Vec<i64> = Vec::new();
        let mut subs: Vec<String> = Vec::new();
        let mut from: i64 = 0;
        let tlen: i64 = re.borrow().tlen;
        while from <= tlen {
            if !re.borrow_mut().exec(from) {
                break;
            }
            let s: i64 = re.borrow().saves[0];
            let f: i64 = re.borrow().saves[1];
            starts.push(s);
            ends.push(f);
            subs.push(FormulaRegex::expand(&re, &rep));
            if f == s {
                from = f + 1;
            } else {
                from = f;
            }
        }
        if !re.borrow().ok {
            return FormulaRegex::err("#VALUE!");
        }
        let cnt: i64 = starts.len() as i64;
        let mut pick: i64 = -1;
        if occ > 0 {
            pick = occ - 1;
        } else if occ < 0 {
            pick = cnt + occ;
        }
        let mut out: String = String::new();
        let mut last: i64 = 0;
        let mut k: i64 = 0;
        while k < cnt {
            if occ == 0 || k == pick {
                out = format!("{}{}{}", out, DgNative::substr(&text, last, starts[k as usize]), subs[k as usize]);
                last = ends[k as usize];
            }
            k += 1;
        }
        out = format!("{}{}", out, DgNative::substr(&text, last, tlen));
        FormulaValue::fromString(out)
    }
    pub fn nameChar(c: i64) -> bool {
        RegexProg::isWord(c)
    }
    /// The replacement for the current match: $n, ${n}, $name, ${name} and $$.
    pub fn expand(re: &Rc<RefCell<RegexProg>>, rep: &str) -> String {
        let n: i64 = DgNative::str_len(rep);
        let mut out: String = String::new();
        let mut i: i64 = 0;
        while i < n {
            let c: i64 = DgNative::char_at(rep, i);
            if c != 36 || i + 1 >= n {
                out = format!("{}{}", out, DgNative::substr(rep, i, i + 1));
                i += 1;
                continue;
            }
            let d: i64 = DgNative::char_at(rep, i + 1);
            if d == 36 {
                out = format!("{}$", out);
                i += 2;
                continue;
            }
            let mut name: String = String::new();
            let mut j: i64 = i + 1;
            let mut found: bool = false;
            if d == 123 {
                let close: i64 = DgNative::index_of_from(rep, "}", i + 2);
                if close >= 0 {
                    name = DgNative::substr(rep, i + 2, close);
                    j = close + 1;
                    found = true;
                }
            } else if d >= 48 && d <= 57 {
                while j < n && DgNative::char_at(rep, j) >= 48 && DgNative::char_at(rep, j) <= 57 {
                    j += 1;
                }
                name = DgNative::substr(rep, i + 1, j);
                found = true;
            } else {
                while j < n && FormulaRegex::nameChar(DgNative::char_at(rep, j)) {
                    j += 1;
                }
                if j > i + 1 {
                    name = DgNative::substr(rep, i + 1, j);
                    found = true;
                }
            }
            if !found {
                out = format!("{}$", out);
                i += 1;
                continue;
            }
            let num: f64 = FormulaRegex::digits(&name);
            let mut g: i64 = -1;
            if num >= 0.0 {
                g = num as i64;
            } else {
                g = re.borrow().groupOf(&name);
            }
            if g >= 0 {
                out = format!("{}{}", out, re.borrow().group(g));
            }
            i = j;
        }
        out
    }
    /// The value of an all-digit name, or -1.
    pub fn digits(s: &str) -> f64 {
        let n: i64 = DgNative::str_len(s);
        if n == 0 {
            return -1.0;
        }
        let mut v: f64 = 0.0;
        let mut i: i64 = 0;
        while i < n {
            let c: i64 = DgNative::char_at(s, i);
            if c < 48 || c > 57 {
                return -1.0;
            }
            v = v * 10.0 + ((c - 48) as f64);
            i += 1;
        }
        v
    }
}
