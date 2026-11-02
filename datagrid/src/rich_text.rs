// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::cell_style::CellStyle;

/// RichText.rgr — several styles inside one cell.
///
/// A cell's style says how the cell is drawn. Rich text says how PARTS of it are:
/// "Reviewer:" in bold followed by the note in plain, a units suffix in a smaller
/// face, one word in red. OOXML calls the pieces runs, and a shared string is
/// either one plain piece of text or a list of them.
///
/// The runs are kept beside the cell's value rather than inside it, for the same
/// reason notes and links are: the value is what formulas read and what a paste
/// carries, and it must stay a plain string. `text()` reassembles it, and that is
/// what the model stores as the cell's value — so everything that does not know
/// about runs keeps working, and everything that does can ask.
///
/// Editing a cell drops its runs. That is what a spreadsheet does when you retype
/// a cell rather than a pretence that the pieces could survive arbitrary text
/// being typed over them.
#[derive(Clone)]
pub struct CellRun {
    pub text: String,
    /// What the run SAID, alongside what it said. `<b val="0"/>` takes a word
    /// out of a bold cell; no `<b>` at all lets the cell decide. `bold == false`
    /// is true of both, and `styleOver` below used to test exactly that — so a
    /// run that un-bolded itself came back bold.
    ///
    /// The size has always had this, spelled as a sentinel: 0 means "whatever
    /// the cell's own style says". A boolean has no spare value to spend that
    /// way, which is why these need a companion each.
    pub boldSet: bool,
    pub bold: bool,
    pub italicSet: bool,
    pub italic: bool,
    pub underlineSet: bool,
    pub underline: bool,
    pub strikeSet: bool,
    pub strike: bool,
    pub sizePt: f64,
    pub rgb: String,
}

impl CellRun {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            boldSet: false,
            bold: false,
            italicSet: false,
            italic: false,
            underlineSet: false,
            underline: false,
            strikeSet: false,
            strike: false,
            sizePt: 0.0,
            rgb: String::new(),
        }
    }
    pub fn copyFrom(&mut self, o: &Rc<RefCell<CellRun>>) {
        self.text = o.borrow().text.clone();
        self.boldSet = o.borrow().boldSet;
        self.bold = o.borrow().bold;
        self.italicSet = o.borrow().italicSet;
        self.italic = o.borrow().italic;
        self.underlineSet = o.borrow().underlineSet;
        self.underline = o.borrow().underline;
        self.strikeSet = o.borrow().strikeSet;
        self.strike = o.borrow().strike;
        self.sizePt = o.borrow().sizePt;
        self.rgb = o.borrow().rgb.clone();
    }
    /// Set a flag AND record that it was set. Every reader and every editor goes
    /// through these, so a caller cannot set the value and forget the companion.
    pub fn setBold(&mut self, v: bool) {
        self.bold = v;
        self.boldSet = true;
    }
    pub fn setItalic(&mut self, v: bool) {
        self.italic = v;
        self.italicSet = true;
    }
    pub fn setUnderline(&mut self, v: bool) {
        self.underline = v;
        self.underlineSet = true;
    }
    pub fn setStrike(&mut self, v: bool) {
        self.strike = v;
        self.strikeSet = true;
    }
    /// Does this run ask for anything the cell's own style would not give it?
    /// A string whose runs all answer false is not rich text — it is text that
    /// happened to be written in pieces, and it is stored as one.
    pub fn isStyled(&self) -> bool {
        // STATED, not true: a run that says "not bold" inside a bold cell is
        // asking for something the cell would not give it, and a string holding
        // one is rich text.
        if self.boldSet {
            return true;
        }
        if self.italicSet {
            return true;
        }
        if self.underlineSet {
            return true;
        }
        if self.strikeSet {
            return true;
        }
        if self.sizePt > 0.0 {
            return true;
        }
        !self.rgb.is_empty()
    }
    /// The face this run is painted in: the cell's style, overridden by whatever
    /// the run says. Handing back a whole CellStyle means the painter needs no
    /// special path — it draws a run exactly as it draws a cell.
    pub fn styleOver(&self, base: &Rc<RefCell<CellStyle>>) -> Rc<RefCell<CellStyle>> {
        let st: Rc<RefCell<CellStyle>> = base.borrow().copy();
        // What the run STATED replaces the cell's; what it did not state leaves
        // the cell's alone. This used to only ever turn things on, so a run
        // that said `b val="0"` in a bold cell drew bold.
        if self.boldSet {
            st.borrow_mut().bold = self.bold;
        }
        if self.italicSet {
            st.borrow_mut().italic = self.italic;
        }
        if self.underlineSet {
            st.borrow_mut().underline = self.underline;
        }
        if self.strikeSet {
            st.borrow_mut().strike = self.strike;
        }
        if self.sizePt > 0.0 {
            st.borrow_mut().fontSizePt = self.sizePt;
        }
        if !self.rgb.is_empty() {
            st.borrow_mut().textRgb = self.rgb.clone();
        }
        st
    }
}

#[derive(Clone)]
pub struct RichText {
    pub runs: Vec<Rc<RefCell<CellRun>>>,
}

impl RichText {
    pub fn new() -> Self {
        Self {
            runs: Vec::new(),
        }
    }
    pub fn count(&self) -> i64 {
        self.runs.len() as i64
    }
    pub fn add(&mut self, r: Rc<RefCell<CellRun>>) {
        self.runs.push(r.clone());
    }
    pub fn addPlain(&mut self, t: String) {
        let r: Rc<RefCell<CellRun>> = Rc::new(RefCell::new(CellRun::new()));
        r.borrow_mut().text = t.clone();
        self.runs.push(r.clone());
    }
    /// The plain string the runs spell out — what the cell stores as its value.
    pub fn text(&self) -> String {
        let mut out: String = String::new();
        let mut i: i64 = 0;
        while i < (self.runs.len() as i64) {
            let r: Rc<RefCell<CellRun>> = self.runs[i as usize].clone();
            out = format!("{}{}", out, r.borrow().text);
            i += 1;
        }
        out
    }
    /// Worth keeping only when at least one run differs from the rest of the
    /// cell. Otherwise the pieces carry no information the value does not.
    pub fn isRich(&self) -> bool {
        if self.runs.is_empty() {
            return false;
        }
        let mut i: i64 = 0;
        while i < (self.runs.len() as i64) {
            let r: Rc<RefCell<CellRun>> = self.runs[i as usize].clone();
            if r.borrow().isStyled() {
                return true;
            }
            i += 1;
        }
        false
    }
    pub fn copy(&self) -> Rc<RefCell<RichText>> {
        let o: Rc<RefCell<RichText>> = Rc::new(RefCell::new(RichText::new()));
        let mut i: i64 = 0;
        while i < (self.runs.len() as i64) {
            let src: Rc<RefCell<CellRun>> = self.runs[i as usize].clone();
            let r: Rc<RefCell<CellRun>> = Rc::new(RefCell::new(CellRun::new()));
            r.borrow_mut().copyFrom(&src);
            o.borrow_mut().runs.push(r.clone());
            i += 1;
        }
        o
    }
}
