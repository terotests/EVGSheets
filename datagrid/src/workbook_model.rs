// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rgr::DgNative;
use crate::script::grid_script::GridScript;
use crate::sheet_table::SheetTable;
use crate::spreadsheet_model::SpreadsheetModel;
/// WorkbookModel - multi-sheet container around SpreadsheetModel.
#[derive(Clone)]
pub struct WorkbookModel {
    pub sheets: Vec<Rc<RefCell<SpreadsheetModel>>>,
    pub activeIndex: i64,
    pub sourcePath: String,
    /// The package this workbook was read from, kept as bytes.
    ///
    /// A spreadsheet writer that always builds a fresh package writes a CLEAN
    /// file, which is sometimes exactly what is wanted — and drops everything
    /// the model does not carry, which mostly is not. Keeping the source is what
    /// lets `XlsxWriter.writeOver` copy those parts through: the pictures, the
    /// comments, the theme, the document properties, and anything else a future
    /// version of Excel put in there that nothing here has ever heard of.
    ///
    /// It costs a copy of the file's bytes, which is small beside the parsed
    /// model of the same file. A host that wants that memory back calls
    /// `dropSource()`, and save-over then falls back to a clean write and says
    /// so rather than silently writing a different kind of file.
    pub sourceBytes: Vec<u8>,
    /// Scripts carried by the package (see script/GridScript.rgr). The loader
    /// fills this from the `ranger/` parts and the writer puts it back, so a
    /// script is part of the document rather than something beside it.
    pub scripts: Vec<Rc<RefCell<GridScript>>>,
    /// Review > Protect Workbook: sheets cannot be added, removed, renamed,
    /// moved, hidden or shown while this is on.
    pub structureProtected: bool,
    /// Defined names (Formulas > Name Manager): NAME (upper case) -> what it
    /// refers to, a reference such as "Sheet1!$B$2:$B$9" or a constant;
    /// `nameOrder` keeps them as typed, in the order they were made.
    pub names: HashMap<String,String>,
    pub nameOrder: Vec<String>,
    /// File › Info › Properties (docProps/core.xml). `propsSet` goes up when one
    /// is edited, so a save over the source rewrites its core part.
    /// Page Layout › Themes: the theme's name (the colours and fonts it set are
    /// the app's; this is what the workbook says it uses).
    pub themeName: String,
    pub propTitle: String,
    pub propSubject: String,
    pub propAuthor: String,
    pub propKeywords: String,
    pub propsSet: bool,
}

impl WorkbookModel {
    pub fn new() -> Self {
        Self {
            sheets: Vec::new(),
            activeIndex: 0,
            sourcePath: String::new(),
            sourceBytes: vec![0u8; 0],
            scripts: Vec::new(),
            structureProtected: false,
            names: HashMap::new(),
            nameOrder: Vec::new(),
            themeName: "Office".to_string(),
            propTitle: String::new(),
            propSubject: String::new(),
            propAuthor: String::new(),
            propKeywords: String::new(),
            propsSet: false,
        }
    }
    /// Where the sheet that was at `idx` ends up when the one at `from` is moved
    /// to `to`. Everything between the two shifts by one, in the direction
    /// opposite to the move.
    ///
    /// Anything holding a sheet NUMBER has to be put through this after a move —
    /// the active sheet here, and a chart, which remembers which sheet it
    /// belongs to by index. A number that is not remapped does not become
    /// invalid; it quietly starts naming a different sheet, which is worse.
    pub fn remapIndex(idx: i64, from: i64, to: i64) -> i64 {
        if idx == from {
            return to;
        }
        if from < to {
            if idx > from && idx <= to {
                return idx - 1;
            }
        }
        if from > to {
            if idx >= to && idx < from {
                return idx + 1;
            }
        }
        idx
    }
    pub fn setName(&mut self, name: &str, r#ref: &str) {
        let key: String = name.to_uppercase();
        if self.nameRef(name).is_empty() {
            self.nameOrder.push(name.to_string());
        }
        self.names.insert(key.clone(), r#ref.to_string());
    }
    pub fn nameRef(&self, name: &str) -> String {
        let key: String = name.to_uppercase();
        if let Some(r) = self.names.get(&key) {
            return r.clone();
        }
        String::new()
    }
    pub fn hasName(&self, name: &str) -> bool {
        !self.nameRef(name).is_empty()
    }
    pub fn removeName(&mut self, name: &str) -> bool {
        let key: String = name.to_uppercase();
        if self.nameRef(name).is_empty() {
            return false;
        }
        self.names.insert(key.clone(), String::new());
        let mut keep: Vec<String> = Vec::new();
        let mut i: i64 = 0;
        while i < (self.nameOrder.len() as i64) {
            if self.nameOrder[i as usize].clone().to_uppercase() != key {
                keep.push(self.nameOrder[i as usize].clone());
            }
            i += 1;
        }
        self.nameOrder = keep.clone();
        true
    }
    pub fn clear(&mut self) {
        self.sheets.clear();
        self.scripts.clear();
        let no_names: HashMap<String,String> = HashMap::new();
        self.names = no_names.clone();
        self.nameOrder.clear();
        self.activeIndex = 0;
        self.sourcePath = String::new();
        self.sourceBytes = vec![0u8; 0];
    }
    /// Is there a package to preserve? A workbook made in the editor has none,
    /// and there is nothing to preserve about a file that never existed.
    pub fn hasSource(&self) -> bool {
        !self.sourceBytes.is_empty()
    }
    pub fn dropSource(&mut self) {
        self.sourceBytes = vec![0u8; 0];
    }
    pub fn addSheet(&mut self, sheet: Rc<RefCell<SpreadsheetModel>>) {
        self.sheets.push(sheet.clone());
    }
    /// --- adding, renaming, reordering -----------------------------------------
    /// The three things a tab strip has to be able to do. All of them are here
    /// rather than in the app because they are facts about the WORKBOOK, and
    /// because two of them have consequences a caller must not have to remember:
    /// a rename moves every formula that names the sheet, and a move renumbers
    /// every index anyone was holding.
    pub fn insertSheet(&mut self, at: i64, sheet: &Rc<RefCell<SpreadsheetModel>>) {
        let n: i64 = self.sheets.len() as i64;
        let mut i: i64 = at;
        if i < 0 {
            i = 0;
        }
        if i > n {
            i = n;
        }
        let mut kept: Vec<Rc<RefCell<SpreadsheetModel>>> = Vec::new();
        let mut k: i64 = 0;
        while k < n {
            if k == i {
                kept.push(sheet.clone());
            }
            kept.push(self.sheets[k as usize].clone());
            k += 1;
        }
        if i >= n {
            kept.push(sheet.clone());
        }
        self.sheets.clear();
        let mut m: i64 = 0;
        while m < (kept.len() as i64) {
            self.sheets.push(kept[m as usize].clone());
            m += 1;
        }
        // A sheet inserted at or before the active one pushes it along, so the
        // sheet you were looking at is still the sheet you are looking at.
        if i <= self.activeIndex {
            self.activeIndex += 1;
        }
    }
    /// Move the sheet at `from` so that it ends up at `to`. Excel's drag: the
    /// tab you are holding lands where you dropped it and everything else
    /// closes up behind it.
    pub fn moveSheet(&mut self, from: i64, to: i64) {
        let n: i64 = self.sheets.len() as i64;
        if from < 0 || from >= n {
            return;
        }
        let mut dest: i64 = to;
        if dest < 0 {
            dest = 0;
        }
        if dest >= n {
            dest = n - 1;
        }
        if dest == from {
            return;
        }
        let moved: Rc<RefCell<SpreadsheetModel>> = self.sheets[from as usize].clone();
        let mut rest: Vec<Rc<RefCell<SpreadsheetModel>>> = Vec::new();
        let mut i: i64 = 0;
        while i < n {
            if i != from {
                rest.push(self.sheets[i as usize].clone());
            }
            i += 1;
        }
        self.sheets.clear();
        let mut k: i64 = 0;
        let mut placed: bool = false;
        while k < (rest.len() as i64) {
            if k == dest && !placed {
                self.sheets.push(moved.clone());
                placed = true;
            }
            self.sheets.push(rest[k as usize].clone());
            k += 1;
        }
        if !placed {
            self.sheets.push(moved.clone());
        }
        // Whatever was active is still active — the same sheet, under whatever
        // number it now wears.
        self.activeIndex = WorkbookModel::remapIndex(self.activeIndex.clone(), from, dest);
    }
    /// Take the sheet at `at` out. The last sheet stays: a workbook with no
    /// sheet has nothing to show and Excel refuses it too. The sheet you were
    /// on stays the one you are on; if it was the one removed, the one that
    /// slid into its place is (the one before it, at the end).
    pub fn removeSheet(&mut self, at: i64) -> bool {
        let n: i64 = self.sheets.len() as i64;
        if (at < 0 || at >= n) || n <= 1 {
            return false;
        }
        let mut kept: Vec<Rc<RefCell<SpreadsheetModel>>> = Vec::new();
        let mut i: i64 = 0;
        while i < n {
            if i != at {
                kept.push(self.sheets[i as usize].clone());
            }
            i += 1;
        }
        self.sheets.clear();
        let mut k: i64 = 0;
        while k < (kept.len() as i64) {
            self.sheets.push(kept[k as usize].clone());
            k += 1;
        }
        if self.activeIndex > at {
            self.activeIndex -= 1;
        }
        if self.activeIndex >= (self.sheets.len() as i64) {
            self.activeIndex = (self.sheets.len() as i64) - 1;
        }
        true
    }
    /// The name a copy of `base` gets: Excel's "Sheet1 (2)", counting up, and
    /// cut so the whole still fits the 31 characters a name may have.
    pub fn copyName(&self, base: &str) -> String {
        let mut n: i64 = 2;
        while n < 10000 {
            let tail: String = format!(" ({})", n);
            let mut head: String = base.to_string();
            let room: i64 = 31 - DgNative::str_len(&tail);
            if DgNative::str_len(&head) > room {
                head = DgNative::substr(&head, 0, room);
            }
            let candidate: String = format!("{}{}", head, tail);
            if self.indexOfName(&candidate) < 0 {
                return candidate;
            }
            n += 1;
        }
        self.nextSheetName()
    }
    pub fn indexOfName(&self, name: &str) -> i64 {
        let want: String = SheetTable::fold(name);
        let mut i: i64 = 0;
        while i < (self.sheets.len() as i64) {
            let sh: Rc<RefCell<SpreadsheetModel>> = self.sheets[i as usize].clone();
            if SheetTable::fold(&sh.borrow().name.clone()) == want {
                return i;
            }
            i += 1;
        }
        -1
    }
    /// A name no sheet is using, starting from `base` and counting up. Excel
    /// compares names without case, and so does this: a workbook cannot hold
    /// both "Data" and "data", because a formula saying `data!A1` would have no
    /// way to mean one of them.
    pub fn uniqueName(&self, base: &str) -> String {
        if self.indexOfName(base) < 0 {
            return base.to_string();
        }
        let mut n: i64 = 2;
        while n < 10000 {
            let candidate: String = format!("{}{}", base, n);
            if self.indexOfName(&candidate) < 0 {
                return candidate;
            }
            n += 1;
        }
        base.to_string()
    }
    /// The name Excel would offer for a new sheet: the first "SheetN" free.
    pub fn nextSheetName(&self) -> String {
        let mut n: i64 = (self.sheets.len() as i64) + 1;
        while n < 10000 {
            let candidate: String = format!("Sheet{}", n);
            if self.indexOfName(&candidate) < 0 {
                return candidate;
            }
            n += 1;
        }
        "Sheet".to_string()
    }
    /// Whether a name may be given to the sheet at `at`. Excel's rules, and the
    /// reasons are all "a formula has to be able to name it": not empty, no
    /// longer than 31, none of the characters a reference uses as syntax, and
    /// not a name another sheet already has.
    pub fn nameIsValid(&self, at: i64, name: &str) -> bool {
        let n: i64 = DgNative::str_len(name);
        if n == 0 {
            return false;
        }
        if n > 31 {
            return false;
        }
        let mut i: i64 = 0;
        while i < n {
            let ch: i64 = DgNative::char_at(&name, i);
            // : \ / ? * [ ]
            if (((((ch == 58 || ch == 92) || ch == 47) || ch == 63) || ch == 42) || ch == 91) || ch == 93 {
                return false;
            }
            i += 1;
        }
        // A leading or trailing apostrophe is refused too: a quoted reference
        // is written `'My Sheet'!A1`, and a name that starts with one cannot
        // be written at all.
        if DgNative::char_at(&name, 0) == 39 {
            return false;
        }
        if DgNative::char_at(&name, n - 1) == 39 {
            return false;
        }
        let taken: i64 = self.indexOfName(name);
        if taken < 0 {
            return true;
        }
        taken == at
    }
    pub fn sheetCount(&self) -> i64 {
        self.sheets.len() as i64
    }
    pub fn sheetAt(&self, i: i64) -> Rc<RefCell<SpreadsheetModel>> {
        self.sheets[i as usize].clone()
    }
    /// --- tables ---------------------------------------------------------------
    /// A table belongs to a sheet, but its NAME is workbook-wide: `Table1[Amount]`
    /// means the same thing written on any sheet. So the lookup is here, and it
    /// answers with both the table and the sheet it sits on — a caller resolving
    /// a reference needs the second as much as the first.
    pub fn tableIndexByName(&self, name: &str) -> i64 {
        let want: String = SheetTable::fold(name);
        let mut si: i64 = 0;
        while si < (self.sheets.len() as i64) {
            let sh: Rc<RefCell<SpreadsheetModel>> = self.sheets[si as usize].clone();
            let mut ti: i64 = 0;
            while ti < sh.borrow().tableCount() {
                let t: Rc<RefCell<SheetTable>> = sh.borrow().tableAt(ti);
                if SheetTable::fold(&t.borrow().name.clone()) == want {
                    return si;
                }
                ti += 1;
            }
            si += 1;
        }
        -1
    }
    pub fn tableByName(&self, name: &str) -> Rc<RefCell<SheetTable>> {
        let want: String = SheetTable::fold(name);
        let mut si: i64 = 0;
        while si < (self.sheets.len() as i64) {
            let sh: Rc<RefCell<SpreadsheetModel>> = self.sheets[si as usize].clone();
            let mut ti: i64 = 0;
            while ti < sh.borrow().tableCount() {
                let t: Rc<RefCell<SheetTable>> = sh.borrow().tableAt(ti);
                if SheetTable::fold(&t.borrow().name.clone()) == want {
                    return t;
                }
                ti += 1;
            }
            si += 1;
        }
        Rc::new(RefCell::new(SheetTable::new()))
    }
    pub fn hasTable(&self, name: &str) -> bool {
        self.tableIndexByName(name) >= 0
    }
    pub fn setActive(&mut self, mut i: i64) {
        if i < 0 {
            i = 0;
        }
        let n: i64 = self.sheets.len() as i64;
        if n == 0 {
            self.activeIndex = 0;
            return;
        }
        if i >= n {
            i = n - 1;
        }
        self.activeIndex = i;
    }
    pub fn activeSheet(&self) -> Rc<RefCell<SpreadsheetModel>> {
        if self.sheets.is_empty() {
            let empty: Rc<RefCell<SpreadsheetModel>> = Rc::new(RefCell::new(SpreadsheetModel::new()));
            return empty;
        }
        self.sheets[(self.activeIndex) as usize].clone()
    }
    /// --- scripts --------------------------------------------------------------
    pub fn scriptCount(&self) -> i64 {
        self.scripts.len() as i64
    }
    pub fn scriptAt(&self, i: i64) -> Rc<RefCell<GridScript>> {
        self.scripts[i as usize].clone()
    }
    pub fn scriptIndexOf(&self, name: &str) -> i64 {
        let mut i: i64 = 0;
        let n: i64 = self.scripts.len() as i64;
        while i < n {
            let s: Rc<RefCell<GridScript>> = self.scripts[i as usize].clone();
            if s.borrow().name == name {
                return i;
            }
            i += 1;
        }
        -1
    }
    /// Adding a script twice under one name replaces it rather than growing a
    /// second part that would overwrite the first in the ZIP.
    pub fn addScript(&mut self, s: Rc<RefCell<GridScript>>) {
        let at: i64 = self.scriptIndexOf(&s.borrow().name.clone());
        if at >= 0 {
            self.scripts[at as usize] = s.clone();
            return;
        }
        self.scripts.push(s.clone());
    }
    pub fn removeScript(&mut self, name: &str) -> bool {
        let at: i64 = self.scriptIndexOf(name);
        if at < 0 {
            return false;
        }
        self.scripts.remove((at) as usize);
        true
    }
    pub fn activeName(&self) -> String {
        if self.sheets.is_empty() {
            return String::new();
        }
        let s: Rc<RefCell<SpreadsheetModel>> = self.sheets[(self.activeIndex) as usize].clone();
        let out: String = s.borrow().name.clone();
        out
    }
}
