// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;

/// SheetTable.rgr — an Excel table (a ListObject), and the names it gives things.
///
/// A table is a named rectangle whose COLUMNS have names, and that is the whole
/// point of it: once a range is a table, a formula can say what it means rather
/// than where it is.
///
///     =SUM(Table1[Amount])                  every amount
///     =Table1[[#This Row],[CategoryID]]     the category on this row
///     =Table1[@CategoryID]                  the same thing, written since 2010
///
/// Both spellings are ordinary in files people send: the long one is what Excel
/// 2007 wrote and what stays in a file written then, the short one is what Excel
/// writes today. Neither calculated here before, because nothing read the
/// `xl/tables/*.xml` parts where a table's name and its column names live —
/// the formula loaded as text and sat there.
///
/// The geometry is stored the way the file states it: `r0` is the FIRST row of
/// the table, which is the header row when there is one. Everything else is
/// derived, so a table cannot hold two answers to the same question:
///
///     header row     r0                          (when headerRowCount is 1)
///     first data row r0 + headerRowCount
///     last data row  r1 - totalsRowCount
///     totals row     r1                          (when totalsRowCount is 1)
///
/// A table belongs to one sheet, but its NAME is workbook-wide: `Table1[Amount]`
/// means the same thing written on any sheet, which is why the lookup lives on
/// WorkbookModel rather than here.
#[derive(Clone)]
pub struct SheetTable {
    /// `name` is what a formula writes — OOXML's `displayName`. The `name`
    /// attribute beside it is an internal identifier and is usually the same
    /// string; where they differ, formulas use displayName.
    pub name: String,
    pub r0: i64,
    pub c0: i64,
    pub r1: i64,
    pub c1: i64,
    pub headerRowCount: i64,
    pub totalsRowCount: i64,
    /// Column names, left to right, one per column between c0 and c1.
    pub columns: Vec<String>,
    /// How it is drawn (OOXML tableStyleInfo) and whether its header row carries
    /// filter buttons (an autoFilter element). Excel's own default for a new
    /// table is TableStyleMedium2 with banded rows and filter buttons.
    pub styleName: String,
    pub showRowStripes: bool,
    pub showColumnStripes: bool,
    pub showFirstColumn: bool,
    pub showLastColumn: bool,
    pub showFilter: bool,
}

impl SheetTable {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            r0: 0,
            c0: 0,
            r1: 0,
            c1: 0,
            headerRowCount: 1,
            totalsRowCount: 0,
            columns: Vec::new(),
            styleName: "TableStyleMedium2".to_string(),
            showRowStripes: true,
            showColumnStripes: false,
            showFirstColumn: false,
            showLastColumn: false,
            showFilter: true,
        }
    }
    /// Lower-cased and trimmed. Not `to_lowercase` alone: a header may have been
    /// written with a space around it, and a formula referring to it will not.
    pub fn fold(s: &str) -> String {
        let n: i64 = DgNative::str_len(s);
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
        DgNative::substr(&s, a, b).to_lowercase()
    }
    pub fn dataR0(&self) -> i64 {
        self.r0 + self.headerRowCount
    }
    pub fn dataR1(&self) -> i64 {
        self.r1 - self.totalsRowCount
    }
    pub fn hasHeader(&self) -> bool {
        self.headerRowCount > 0
    }
    pub fn hasTotals(&self) -> bool {
        self.totalsRowCount > 0
    }
    pub fn columnCount(&self) -> i64 {
        self.columns.len() as i64
    }
    /// Which column carries this name, as an offset from c0, or -1.
    ///
    /// Compared without case, because Excel does not care and neither do the
    /// people writing the formulas: a file whose header says "CategoryID" and a
    /// formula that says `[categoryid]` are the same reference.
    pub fn columnIndex(&self, col_name: &str) -> i64 {
        let want: String = SheetTable::fold(col_name);
        let mut i: i64 = 0;
        while i < (self.columns.len() as i64) {
            if SheetTable::fold(&self.columns[i as usize].clone()) == want {
                return i;
            }
            i += 1;
        }
        -1
    }
    pub fn columnAt(&self, i: i64) -> String {
        if i < 0 {
            return String::new();
        }
        if i >= (self.columns.len() as i64) {
            return String::new();
        }
        self.columns[i as usize].clone()
    }
    /// Does this row fall inside the table at all? `[#This Row]` is only
    /// meaningful where it does — a formula that says it from somewhere else is
    /// asking about a row the table does not have.
    pub fn contains(&self, row: i64, col: i64) -> bool {
        if row < self.r0 || row > self.r1 {
            return false;
        }
        col >= self.c0 && col <= self.c1
    }
    pub fn containsRow(&self, row: i64) -> bool {
        if row < self.r0 {
            return false;
        }
        row <= self.r1
    }
    pub fn copy(&self) -> Rc<RefCell<SheetTable>> {
        let t: Rc<RefCell<SheetTable>> = Rc::new(RefCell::new(SheetTable::new()));
        t.borrow_mut().name = self.name.clone();
        t.borrow_mut().r0 = self.r0;
        t.borrow_mut().c0 = self.c0;
        t.borrow_mut().r1 = self.r1;
        t.borrow_mut().c1 = self.c1;
        t.borrow_mut().headerRowCount = self.headerRowCount;
        t.borrow_mut().totalsRowCount = self.totalsRowCount;
        t.borrow_mut().styleName = self.styleName.clone();
        t.borrow_mut().showRowStripes = self.showRowStripes;
        t.borrow_mut().showColumnStripes = self.showColumnStripes;
        t.borrow_mut().showFirstColumn = self.showFirstColumn;
        t.borrow_mut().showLastColumn = self.showLastColumn;
        t.borrow_mut().showFilter = self.showFilter;
        let mut i: i64 = 0;
        while i < (self.columns.len() as i64) {
            t.borrow_mut().columns.push(self.columns[i as usize].clone());
            i += 1;
        }
        t
    }
}
