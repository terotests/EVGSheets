// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rgr::DgNative;
use crate::formula_functions::FormulaFunctions;
use crate::formula_value::FormulaValue;
use crate::sheet_table::SheetTable;
use crate::spreadsheet_model::SpreadsheetModel;
use crate::workbook_model::WorkbookModel;
/// FormulaEngine - AST + dependency graph + incremental recalc.
///
/// Function implementations live in FormulaFunctions.rgr (unit-testable).
/// This file: parse → deps → topological recalc → FormulaFunctions.invoke.
///
/// Operators: + - * / ^ &   = <> < > <= >=
/// Refs: A1, $A$1, A$1, $A1, Sheet2!A1, 'My Sheet'!A1, A1:B10
/// Errors: #DIV/0! #VALUE! #REF! #NAME? #CYCLE! #NUM!
///
/// AST helpers: FormulaParser.translateNode / serializeNode for fill/copy.
/// --- AST nodes (flattened: kind + fields) ------------------------------------
#[derive(Clone)]
pub struct FormulaNode {
    /// 0 num 1 str 2 bool 3 cell 4 range 5 unary 6 binary 7 call 8 err
    /// 9 structured reference — `Table1[[#This Row],[CategoryID]]`
    pub kind: i64,
    pub num: f64,
    pub text: String,
    pub flag: bool,
    pub sheet: String,
    pub row: i64,
    pub col: i64,
    pub row2: i64,
    pub col2: i64,
    pub absRow: bool,
    pub absCol: bool,
    pub absRow2: bool,
    pub absCol2: bool,
    pub op: String,
    pub fnName: String,
    pub kids: Vec<Rc<RefCell<FormulaNode>>>,
    /// --- structured reference (kind 9) ----------------------------------------
    /// Held as it was WRITTEN and resolved at evaluation time, not at parse
    /// time, for two reasons that both matter.
    ///
    /// The parser does not know the tables — they belong to the workbook — and
    /// `#This Row` does not have an answer until you know which cell is asking.
    /// And a structured reference is not translated when a formula is copied:
    /// that is the entire point of it, since `[#This Row]` re-anchors itself on
    /// whatever row it lands on. Keeping the text means a copy, a save and a
    /// round trip all give back the same reference rather than a rewritten one.
    ///
    /// `tableItem` is the specifier, folded to lower case ("#this row",
    /// "#headers", "#totals", "#data", "#all") or "" for none. `tableCol2` is
    /// set only for a span, `Table1[[Amount]:[Price]]`.
    pub tableName: String,
    pub tableItem: String,
    pub tableCol1: String,
    pub tableCol2: String,
    /// True where the reference was written with the `@` shorthand, so
    /// serializing gives back what the file said rather than the other spelling.
    pub tableAtSign: bool,
}

impl FormulaNode {
    pub fn new() -> Self {
        Self {
            kind: 0,
            num: 0.0,
            text: String::new(),
            flag: false,
            sheet: String::new(),
            row: 0,
            col: 0,
            row2: 0,
            col2: 0,
            absRow: false,
            absCol: false,
            absRow2: false,
            absCol2: false,
            op: String::new(),
            fnName: String::new(),
            kids: Vec::new(),
            tableName: String::new(),
            tableItem: String::new(),
            tableCol1: String::new(),
            tableCol2: String::new(),
            tableAtSign: false,
        }
    }
}

/// Where a structured reference lands, once a workbook has been consulted.
/// `err` is empty when it landed anywhere at all.
#[derive(Clone)]
pub struct TableRange {
    pub err: String,
    pub sheetIdx: i64,
    pub r0: i64,
    pub c0: i64,
    pub r1: i64,
    pub c1: i64,
}

impl TableRange {
    pub fn new() -> Self {
        Self {
            err: String::new(),
            sheetIdx: 0,
            r0: 0,
            c0: 0,
            r1: 0,
            c1: 0,
        }
    }
}

/// --- tokenizer / parser -------------------------------------------------------
#[derive(Clone)]
pub struct FormulaParser {
    pub src: String,
    pub pos: i64,
    pub srcLen: i64,
    pub ok: bool,
    pub err: String,
}

impl FormulaParser {
    pub fn new() -> Self {
        Self {
            src: String::new(),
            pos: 0,
            srcLen: 0,
            ok: true,
            err: String::new(),
        }
    }
    /// Split on commas that are not inside brackets.
    pub fn splitTopLevel(s: &str, out: &mut Vec<String>) {
        let mut depth: i64 = 0;
        let mut start: i64 = 0;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        while i < n {
            let ch: i64 = DgNative::char_at(&s, i);
            if ch == 39 {
                i += 2;
                continue;
            }
            if ch == 91 {
                depth += 1;
            }
            if ch == 93 {
                depth -= 1;
            }
            if ch == 44 && depth == 0 {
                out.push(DgNative::substr(&s, start, i));
                start = i + 1;
            }
            i += 1;
        }
        out.push(DgNative::substr(&s, start, n));
    }
    /// The position of a `:` that is not inside brackets, or -1.
    pub fn topLevelColon(s: &str) -> i64 {
        let mut depth: i64 = 0;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        while i < n {
            let ch: i64 = DgNative::char_at(&s, i);
            if ch == 39 {
                i += 2;
                continue;
            }
            if ch == 91 {
                depth += 1;
            }
            if ch == 93 {
                depth -= 1;
            }
            if ch == 58 && depth == 0 {
                return i;
            }
            i += 1;
        }
        -1
    }
    /// `[Amount]` → `Amount`; anything else is handed back untouched. Only one
    /// layer comes off, and only when the brackets wrap the WHOLE piece — in
    /// `[Amount]:[Price]` they do not.
    pub fn stripOuterBrackets(s: &str) -> String {
        let mut a: i64 = 0;
        let mut b: i64 = DgNative::str_len(&s);
        while a < b {
            let ch: i64 = DgNative::char_at(&s, a);
            if ch == 32 || ch == 9 {
                a += 1;
            } else {
                break;
            }
        }
        while b > a {
            let ch2: i64 = DgNative::char_at(&s, b - 1);
            if ch2 == 32 || ch2 == 9 {
                b -= 1;
            } else {
                break;
            }
        }
        if b - a < 2 {
            return DgNative::substr(&s, a, b);
        }
        if DgNative::char_at(&s, a) != 91 {
            return DgNative::substr(&s, a, b);
        }
        // Only if this `[` is closed by the LAST character.
        let mut depth: i64 = 0;
        let mut i: i64 = a;
        while i < b {
            let ch3: i64 = DgNative::char_at(&s, i);
            if ch3 == 39 {
                i += 2;
                continue;
            }
            if ch3 == 91 {
                depth += 1;
            }
            if ch3 == 93 {
                depth -= 1;
                if depth == 0 {
                    if i == b - 1 {
                        return DgNative::substr(&s, a + 1, b - 1);
                    }
                    return DgNative::substr(&s, a, b);
                }
            }
            i += 1;
        }
        DgNative::substr(&s, a, b)
    }
    /// `'` before a character means the character, not the syntax.
    pub fn unescapeColumn(s: &str) -> String {
        if DgNative::index_of(&s, "'") < 0 {
            return s.to_string();
        }
        let mut out: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        while i < n {
            let ch: i64 = DgNative::char_at(&s, i);
            if ch == 39 && i + 1 < n {
                let piece: String = DgNative::substr(&s, i + 1, i + 2);
                out.push_str(&piece);
                i += 2;
            } else {
                let piece: String = DgNative::substr(&s, i, i + 1);
                out.push_str(&piece);
                i += 1;
            }
        }
        out
    }
    pub fn upper(s: &str) -> String {
        let mut out: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        while i < n {
            let ch: i64 = DgNative::char_at(&s, i);
            if ch >= 97 && ch <= 122 {
                out = format!("{}{}", out, char::from_u32((ch - 32) as u32).unwrap_or('\0'));
            } else {
                // Copied, not rebuilt — see `lastAsText`.
                out = format!("{}{}", out, DgNative::substr(&s, i, i + 1));
            }
            i += 1;
        }
        out
    }
    /// Parse A1 / $A$1 / A$1 / $A1 into 0-based row/col + absolute flags.
    /// Row and column of an A1 reference, 0-based: `[row, col]`, -1 where a
    /// part is missing. The `$` marks are dropped; parseA1 gives them too.
    pub fn a1Position(r#ref: &str) -> Vec<i64> {
        let mut rows: Vec<i64> = Vec::new();
        let mut cols: Vec<i64> = Vec::new();
        let mut abs_rows: Vec<bool> = Vec::new();
        let mut abs_cols: Vec<bool> = Vec::new();
        FormulaParser::parseA1(r#ref, &mut rows, &mut cols, &mut abs_rows, &mut abs_cols);
        vec![rows[0], cols[0]]
    }
    pub fn parseA1(r#ref: &str, row_out: &mut Vec<i64>, col_out: &mut Vec<i64>, abs_row_out: &mut Vec<bool>, abs_col_out: &mut Vec<bool>) {
        row_out.clear();
        col_out.clear();
        abs_row_out.clear();
        abs_col_out.clear();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&r#ref);
        let mut col: i64 = 0;
        let mut saw_col: bool = false;
        let mut abs_col: bool = false;
        let mut abs_row: bool = false;
        while i < n {
            let ch: i64 = DgNative::char_at(&r#ref, i);
            if ch == 36 {
                let j: i64 = i + 1;
                if j < n {
                    let nch: i64 = DgNative::char_at(&r#ref, j);
                    if (nch >= 65 && nch <= 90) || (nch >= 97 && nch <= 122) {
                        abs_col = true;
                    } else {
                        if nch >= 48 && nch <= 57 {
                            abs_row = true;
                        }
                    }
                }
                i += 1;
            } else {
                if ch >= 65 && ch <= 90 {
                    col = col * 26 + ((ch - 65) + 1);
                    saw_col = true;
                    i += 1;
                } else {
                    if ch >= 97 && ch <= 122 {
                        col = col * 26 + ((ch - 97) + 1);
                        saw_col = true;
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
        }
        if saw_col {
            col -= 1;
        }
        let mut row: i64 = 0;
        let mut saw_row: bool = false;
        while i < n {
            let ch2: i64 = DgNative::char_at(&r#ref, i);
            if ch2 == 36 {
                abs_row = true;
                i += 1;
            } else {
                if ch2 >= 48 && ch2 <= 57 {
                    row = row * 10 + (ch2 - 48);
                    saw_row = true;
                    i += 1;
                } else {
                    break;
                }
            }
        }
        if saw_row {
            row -= 1;
        }
        row_out.push(row);
        col_out.push(col);
        abs_row_out.push(abs_row);
        abs_col_out.push(abs_col);
    }
    pub fn colLetters(col: i64) -> String {
        let mut c: i64 = col + 1;
        let mut out: String = String::new();
        while c > 0 {
            let rem: i64 = (c - 1) - ((c - 1) / 26) * 26;
            out = format!("{}{}", char::from_u32((65 + rem) as u32).unwrap_or('\0'), out);
            c = (c - 1) / 26;
        }
        out
    }
    pub fn formatA1(row: i64, col: i64, abs_row: bool, abs_col: bool) -> String {
        let mut s: String = String::new();
        if abs_col {
            s = format!("{}$", s);
        }
        s = format!("{}{}", s, FormulaParser::colLetters(col));
        if abs_row {
            s = format!("{}$", s);
        }
        s = format!("{}{}", s, row + 1);
        s
    }
    pub fn needsSheetQuotes(name: &str) -> bool {
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&name);
        while i < n {
            let ch: i64 = DgNative::char_at(&name, i);
            if (ch == 32 || ch == 39) || ch == 33 {
                return true;
            }
            i += 1;
        }
        false
    }
    pub fn formatSheetPrefix(sheet: &str) -> String {
        if sheet.is_empty() {
            return String::new();
        }
        let q: bool = FormulaParser::needsSheetQuotes(sheet);
        if q {
            let mut esc: String = String::new();
            let mut i: i64 = 0;
            let n: i64 = DgNative::str_len(&sheet);
            while i < n {
                let ch: i64 = DgNative::char_at(&sheet, i);
                if ch == 39 {
                    esc = format!("{}''", esc);
                } else {
                    esc = format!("{}{}", esc, DgNative::substr(&sheet, i, i + 1));
                }
                i += 1;
            }
            return format!("'{}'!", esc);
        }
        format!("{}!", sheet)
    }
    /// Shift relative refs by (dx, dy). Absolute flags stay fixed.
    /// Fill and copy: shift the references that are written as positions.
    ///
    /// A structured reference (kind 9) is deliberately NOT among them. It names
    /// a table and a column, and `[#This Row]` re-anchors itself on whatever row
    /// it lands on — shifting it would be shifting a name. That is the whole
    /// reason people write them, and it falls out of this function touching only
    /// kinds 3 and 4.
    pub fn translateNode(n: &Rc<RefCell<FormulaNode>>, dx: i64, dy: i64) {
        if n.borrow().kind == 3 || n.borrow().kind == 4 {
            if !n.borrow().absRow {
                n.borrow_mut().row = n.borrow().row + dy;
            }
            if !n.borrow().absCol {
                n.borrow_mut().col = n.borrow().col + dx;
            }
            if n.borrow().kind == 4 {
                if !n.borrow().absRow2 {
                    n.borrow_mut().row2 = n.borrow().row2 + dy;
                }
                if !n.borrow().absCol2 {
                    n.borrow_mut().col2 = n.borrow().col2 + dx;
                }
            }
        }
        let mut i: i64 = 0;
        let kn: i64 = n.borrow().kids.len() as i64;
        while i < kn {
            FormulaParser::translateNode(&n.borrow().kids[i as usize].clone(), dx, dy);
            i += 1;
        }
    }
    /// Operator precedence, mirroring the parse ladder below:
    ///   compare < concat < add < mul < pow < unary
    pub fn opPrec(op: &str) -> i64 {
        if op == "^" {
            return 5;
        }
        if op == "*" || op == "/" {
            return 4;
        }
        if op == "+" || op == "-" {
            return 3;
        }
        if op == "&" {
            return 2;
        }
        1
    }
    pub fn nodePrec(n: &Rc<RefCell<FormulaNode>>) -> i64 {
        if n.borrow().kind == 6 {
            return FormulaParser::opPrec(&n.borrow().op.clone());
        }
        if n.borrow().kind == 5 {
            return 6;
        }
        100
    }
    /// Adjust references for a column insert / delete on one sheet.
    ///   at    the column the operation happened at
    ///   delta +1 for an insert, -1 for a delete
    ///   sheet "" matches references with no sheet prefix (same sheet)
    /// A reference to a DELETED column cannot be repaired — it becomes #REF!,
    /// exactly as Excel does, rather than silently pointing at the neighbour.
    /// allowUnprefixed: true on the sheet being structurally edited (local
    /// A1 refs move with it). False on other sheets — only SheetName!A1 refs
    /// that name the edited sheet should shift; their local A1 must stay put.
    pub fn adjustColsNode(n: &Rc<RefCell<FormulaNode>>, sheet_name: &str, at: i64, delta: i64, allow_unprefixed: bool) -> bool {
        let mut broke: bool = false;
        if n.borrow().kind == 3 || n.borrow().kind == 4 {
            let mut applies: bool = false;
            if n.borrow().sheet.is_empty() {
                applies = allow_unprefixed;
            } else {
                if n.borrow().sheet == sheet_name {
                    applies = true;
                }
            }
            if applies {
                if delta < 0 {
                    if n.borrow().kind == 3 {
                        if n.borrow().col == at {
                            broke = true;
                        }
                    } else {
                        // A range collapses only when the whole span goes.
                        if n.borrow().col == at && n.borrow().col2 == at {
                            broke = true;
                        }
                    }
                }
                if !broke {
                    if n.borrow().col >= at {
                        let mut nc: i64 = n.borrow().col + delta;
                        if nc < 0 {
                            nc = 0;
                        }
                        n.borrow_mut().col = nc;
                    }
                    if n.borrow().kind == 4 {
                        if n.borrow().col2 >= at {
                            let mut nc2: i64 = n.borrow().col2 + delta;
                            if nc2 < 0 {
                                nc2 = 0;
                            }
                            n.borrow_mut().col2 = nc2;
                        }
                    }
                }
            }
        }
        let mut i: i64 = 0;
        let kn: i64 = n.borrow().kids.len() as i64;
        while i < kn {
            if FormulaParser::adjustColsNode(&n.borrow().kids[i as usize].clone(), sheet_name, at, delta, allow_unprefixed) {
                broke = true;
            }
            i += 1;
        }
        broke
    }
    /// The column walk, transposed. Same rules: a reference AT a deleted line
    /// breaks, a reference past it shifts, and a range breaks only when its
    /// whole span goes.
    /// --- reordering the rows --------------------------------------------------
    /// Sorting MOVES cells. A formula travels with its row — `reorderRowsRaw`
    /// carries the text along — and its references do not, so `B3+C3+D3` on the
    /// Rent row still added row 3 after Rent moved to row 6. Every total in the
    /// sheet then quietly belonged to a different row, which is worse than a
    /// sort that does nothing: the numbers are wrong and they look fine.
    ///
    /// `newRowOf[old]` is where the row that was at `old` ended up. A reference
    /// to a moved cell follows the cell — absolute as well as relative, because
    /// the cell physically MOVED, which is the cut-and-paste rule rather than
    /// the fill-down one.
    ///
    /// A RANGE is the interesting case. `SUM(B3:B7)` over the whole sorted block
    /// must stay `SUM(B3:B7)`: the block still holds the same rows, only in a
    /// different order, so remapping its ends would shrink it to an arbitrary
    /// sub-range. So a range whose rows map ONTO THEMSELVES is left alone, and
    /// only one that straddles the edge of what moved has its ends remapped.
    pub fn remapRowOf(new_row_of: &[i64], row: i64) -> i64 {
        if row < 0 {
            return row;
        }
        if row >= (new_row_of.len() as i64) {
            return row;
        }
        new_row_of[row as usize]
    }
    pub fn rangeIsClosed(new_row_of: &[i64], r0: i64, r1: i64) -> bool {
        if r1 < r0 {
            return false;
        }
        let mut lo: i64 = -1;
        let mut hi: i64 = -1;
        let mut r: i64 = r0;
        while r <= r1 {
            let m: i64 = FormulaParser::remapRowOf(new_row_of, r);
            if r == r0 {
                lo = m;
                hi = m;
            } else {
                if m < lo {
                    lo = m;
                }
                if m > hi {
                    hi = m;
                }
            }
            r += 1;
        }
        if lo != r0 {
            return false;
        }
        hi == r1
    }
    pub fn remapRowsNode(n: &Rc<RefCell<FormulaNode>>, sheet_name: &str, new_row_of: &[i64], allow_unprefixed: bool) -> bool {
        let mut touched: bool = false;
        if n.borrow().kind == 3 || n.borrow().kind == 4 {
            let mut applies: bool = false;
            if n.borrow().sheet.is_empty() {
                applies = allow_unprefixed;
            } else {
                if SheetTable::fold(&n.borrow().sheet.clone()) == SheetTable::fold(sheet_name) {
                    applies = true;
                }
            }
            if applies {
                if n.borrow().kind == 3 {
                    let nr: i64 = FormulaParser::remapRowOf(new_row_of, n.borrow().row.clone());
                    if nr != n.borrow().row {
                        n.borrow_mut().row = nr;
                        touched = true;
                    }
                } else {
                    if !FormulaParser::rangeIsClosed(new_row_of, n.borrow().row.clone(), n.borrow().row2.clone()) {
                        let mut a: i64 = FormulaParser::remapRowOf(new_row_of, n.borrow().row.clone());
                        let mut b: i64 = FormulaParser::remapRowOf(new_row_of, n.borrow().row2.clone());
                        if b < a {
                            let t: i64 = a;
                            a = b;
                            b = t;
                        }
                        if a != n.borrow().row || b != n.borrow().row2 {
                            n.borrow_mut().row = a;
                            n.borrow_mut().row2 = b;
                            touched = true;
                        }
                    }
                }
            }
        }
        let mut i: i64 = 0;
        let kn: i64 = n.borrow().kids.len() as i64;
        while i < kn {
            if FormulaParser::remapRowsNode(&n.borrow().kids[i as usize].clone(), sheet_name, new_row_of, allow_unprefixed) {
                touched = true;
            }
            i += 1;
        }
        touched
    }
    /// The rewritten formula, or "" when it did not need rewriting or could not
    /// be parsed — either way the caller leaves that cell alone.
    pub fn remapRows(expr: &str, sheet_name: &str, new_row_of: &[i64], allow_unprefixed: bool) -> String {
        let p: Rc<RefCell<FormulaParser>> = Rc::new(RefCell::new(FormulaParser::new()));
        let ast: Rc<RefCell<FormulaNode>> = p.borrow_mut().parse(expr);
        if !p.borrow().ok {
            return String::new();
        }
        if !FormulaParser::remapRowsNode(&ast, sheet_name, new_row_of, allow_unprefixed) {
            return String::new();
        }
        FormulaParser::serializeNode(&ast)
    }
    pub fn adjustRowsNode(n: &Rc<RefCell<FormulaNode>>, sheet_name: &str, at: i64, delta: i64, allow_unprefixed: bool) -> bool {
        let mut broke: bool = false;
        if n.borrow().kind == 3 || n.borrow().kind == 4 {
            let mut applies: bool = false;
            if n.borrow().sheet.is_empty() {
                applies = allow_unprefixed;
            } else {
                if n.borrow().sheet == sheet_name {
                    applies = true;
                }
            }
            if applies {
                if delta < 0 {
                    if n.borrow().kind == 3 {
                        if n.borrow().row == at {
                            broke = true;
                        }
                    } else {
                        if n.borrow().row == at && n.borrow().row2 == at {
                            broke = true;
                        }
                    }
                }
                if !broke {
                    if n.borrow().row >= at {
                        let mut nr: i64 = n.borrow().row + delta;
                        if nr < 0 {
                            nr = 0;
                        }
                        n.borrow_mut().row = nr;
                    }
                    if n.borrow().kind == 4 {
                        if n.borrow().row2 >= at {
                            let mut nr2: i64 = n.borrow().row2 + delta;
                            if nr2 < 0 {
                                nr2 = 0;
                            }
                            n.borrow_mut().row2 = nr2;
                        }
                    }
                }
            }
        }
        let mut i: i64 = 0;
        let kn: i64 = n.borrow().kids.len() as i64;
        while i < kn {
            if FormulaParser::adjustRowsNode(&n.borrow().kids[i as usize].clone(), sheet_name, at, delta, allow_unprefixed) {
                broke = true;
            }
            i += 1;
        }
        broke
    }
    /// --- Insert / Delete Cells: a block shifts ---------------------------------
    /// Cells shifted down (or up) inside columns c0..c1 from row `at`, by
    /// `delta` rows; with `columns`, cells shifted right (left) inside rows
    /// c0..c1 from column `at`. A reference moves with its cells when it lies
    /// wholly inside the band; one into cells a delete removed breaks.
    pub fn shiftBlockNode(n: &Rc<RefCell<FormulaNode>>, sheet_name: &str, columns: bool, at: i64, lo: i64, hi: i64, delta: i64, allow_unprefixed: bool) -> bool {
        let mut broke: bool = false;
        if n.borrow().kind == 3 || n.borrow().kind == 4 {
            let mut applies: bool = false;
            if n.borrow().sheet.is_empty() {
                applies = allow_unprefixed;
            } else {
                if n.borrow().sheet == sheet_name {
                    applies = true;
                }
            }
            let mut a0: i64 = n.borrow().col;
            let mut a1: i64 = n.borrow().col;
            let mut m0: i64 = n.borrow().row;
            let mut m1: i64 = n.borrow().row;
            if n.borrow().kind == 4 {
                a1 = n.borrow().col2;
                m1 = n.borrow().row2;
            }
            if columns {
                a0 = n.borrow().row;
                a1 = n.borrow().row;
                m0 = n.borrow().col;
                m1 = n.borrow().col;
                if n.borrow().kind == 4 {
                    a1 = n.borrow().row2;
                    m1 = n.borrow().col2;
                }
            }
            if (applies && a0 >= lo) && a1 <= hi {
                if delta < 0 {
                    // The removed cells are at .. at-delta-1.
                    let gone0: i64 = at;
                    let gone1: i64 = (at - delta) - 1;
                    if m0 >= gone0 && m1 <= gone1 {
                        broke = true;
                    }
                }
                if !broke {
                    let mut n0: i64 = m0;
                    let mut n1: i64 = m1;
                    if m0 >= at {
                        n0 = m0 + delta;
                    }
                    if m1 >= at {
                        n1 = m1 + delta;
                    }
                    if n0 < at {
                        if m0 >= at {
                            n0 = at;
                        }
                    }
                    if columns {
                        n.borrow_mut().col = n0;
                        if n.borrow().kind == 4 {
                            n.borrow_mut().col2 = n1;
                        }
                    } else {
                        n.borrow_mut().row = n0;
                        if n.borrow().kind == 4 {
                            n.borrow_mut().row2 = n1;
                        }
                    }
                }
            }
        }
        let mut i: i64 = 0;
        while i < (n.borrow().kids.len() as i64) {
            if FormulaParser::shiftBlockNode(&n.borrow().kids[i as usize].clone(), sheet_name, columns, at, lo, hi, delta, allow_unprefixed) {
                broke = true;
            }
            i += 1;
        }
        broke
    }
    pub fn shiftBlock(expr: &str, sheet_name: &str, columns: bool, at: i64, lo: i64, hi: i64, delta: i64, allow_unprefixed: bool) -> String {
        let p: Rc<RefCell<FormulaParser>> = Rc::new(RefCell::new(FormulaParser::new()));
        let ast: Rc<RefCell<FormulaNode>> = p.borrow_mut().parse(expr);
        if !p.borrow().ok {
            return String::new();
        }
        if FormulaParser::shiftBlockNode(&ast, sheet_name, columns, at, lo, hi, delta, allow_unprefixed) {
            return "#REF!".to_string();
        }
        FormulaParser::serializeNode(&ast)
    }
    /// --- renaming a sheet -----------------------------------------------------
    /// A cross-sheet reference names its sheet in TEXT — `Costs!B4`, or
    /// `'Q3 Forecast'!B4` when the name needs quoting — so renaming a sheet and
    /// leaving the formulas alone turns every one of them into `#NAME?`. Excel
    /// rewrites them, and so does this: parse, swap the name on every node that
    /// carries it, serialize back. Going through the AST rather than the text is
    /// what keeps `SUM(Costs!B4)` apart from a string literal that happens to
    /// say "Costs!", and what re-quotes a name that has become "Q3 Costs".
    pub fn renameSheetNode(n: &Rc<RefCell<FormulaNode>>, was: &str, now: String) -> bool {
        let mut touched: bool = false;
        if !n.borrow().sheet.is_empty() {
            if SheetTable::fold(&n.borrow().sheet.clone()) == SheetTable::fold(was) {
                n.borrow_mut().sheet = now.clone();
                touched = true;
            }
        }
        let mut i: i64 = 0;
        while i < (n.borrow().kids.len() as i64) {
            if FormulaParser::renameSheetNode(&n.borrow().kids[i as usize].clone(), was, now.clone()) {
                touched = true;
            }
            i += 1;
        }
        touched
    }
    /// The rewritten formula, or "" when it did not need rewriting or could not
    /// be parsed — either way the caller leaves the cell alone, which is the
    /// right answer for both.
    pub fn renameSheet(expr: &str, was: &str, now: &str) -> String {
        let p: Rc<RefCell<FormulaParser>> = Rc::new(RefCell::new(FormulaParser::new()));
        let ast: Rc<RefCell<FormulaNode>> = p.borrow_mut().parse(expr);
        if !p.borrow().ok {
            return String::new();
        }
        if !FormulaParser::renameSheetNode(&ast, was, now.to_string()) {
            return String::new();
        }
        FormulaParser::serializeNode(&ast)
    }
    pub fn adjustRows(expr: &str, sheet_name: &str, at: i64, delta: i64, allow_unprefixed: bool) -> String {
        let p: Rc<RefCell<FormulaParser>> = Rc::new(RefCell::new(FormulaParser::new()));
        let ast: Rc<RefCell<FormulaNode>> = p.borrow_mut().parse(expr);
        if !p.borrow().ok {
            return String::new();
        }
        let broke: bool = FormulaParser::adjustRowsNode(
            &ast,
            sheet_name,
            at,
            delta,
            allow_unprefixed
        );
        if broke {
            return "#REF!".to_string();
        }
        FormulaParser::serializeNode(&ast)
    }
    /// --- growing a total over an inserted row ---------------------------------
    /// A row inserted directly under a block, above the row that totals it,
    /// lands OUTSIDE `SUM(B2:B20)`: the plain shift only moves what is at or
    /// past the insert, and B20 is not. Excel fixes the obvious case for you, so
    /// this does too, and only that case: run after `adjustRows`, on a formula
    /// that sits below the new row, a range ending on the row just above it
    /// and covering the formula's own column grows by one to take the new row
    /// in. Everything else — a single-cell ref, a range elsewhere, a total in
    /// another column — is left as the shift wrote it.
    pub fn extendRowsEdgeNode(n: &Rc<RefCell<FormulaNode>>, sheet_name: &str, at: i64, f_row: i64, f_col: i64, allow_unprefixed: bool) -> bool {
        let mut touched: bool = false;
        if n.borrow().kind == 4 {
            let mut applies: bool = false;
            if n.borrow().sheet.is_empty() {
                applies = allow_unprefixed;
            } else {
                if n.borrow().sheet == sheet_name {
                    applies = true;
                }
            }
            if applies {
                if n.borrow().row2 == at - 1 && f_row > at {
                    if f_col >= n.borrow().col && f_col <= n.borrow().col2 {
                        n.borrow_mut().row2 = at;
                        touched = true;
                    }
                }
            }
        }
        let mut i: i64 = 0;
        let kn: i64 = n.borrow().kids.len() as i64;
        while i < kn {
            if FormulaParser::extendRowsEdgeNode(&n.borrow().kids[i as usize].clone(), sheet_name, at, f_row, f_col, allow_unprefixed) {
                touched = true;
            }
            i += 1;
        }
        touched
    }
    /// The rewritten formula, or "" when nothing grew or it could not be parsed.
    pub fn extendRowsEdge(expr: &str, sheet_name: &str, at: i64, f_row: i64, f_col: i64, allow_unprefixed: bool) -> String {
        let p: Rc<RefCell<FormulaParser>> = Rc::new(RefCell::new(FormulaParser::new()));
        let ast: Rc<RefCell<FormulaNode>> = p.borrow_mut().parse(expr);
        if !p.borrow().ok {
            return String::new();
        }
        if !FormulaParser::extendRowsEdgeNode(&ast, sheet_name, at, f_row, f_col, allow_unprefixed) {
            return String::new();
        }
        FormulaParser::serializeNode(&ast)
    }
    /// Returns the rewritten formula, or "#REF!" when a reference was destroyed.
    /// "" means the expression could not be parsed and should be left alone.
    pub fn adjustColumns(expr: &str, sheet_name: &str, at: i64, delta: i64, allow_unprefixed: bool) -> String {
        let p: Rc<RefCell<FormulaParser>> = Rc::new(RefCell::new(FormulaParser::new()));
        let ast: Rc<RefCell<FormulaNode>> = p.borrow_mut().parse(expr);
        if !p.borrow().ok {
            return String::new();
        }
        let broke: bool = FormulaParser::adjustColsNode(
            &ast,
            sheet_name,
            at,
            delta,
            allow_unprefixed
        );
        if broke {
            return "#REF!".to_string();
        }
        FormulaParser::serializeNode(&ast)
    }
    pub fn serializeNode(n: &Rc<RefCell<FormulaNode>>) -> String {
        if n.borrow().kind == 8 {
            return n.borrow().text.clone();
        }
        if n.borrow().kind == 0 {
            let iv: i64 = n.borrow().num.floor() as i64;
            if (iv as f64) == n.borrow().num {
                return iv.to_string();
            }
            return n.borrow().num.to_string();
        }
        if n.borrow().kind == 1 {
            return format!("\"{}\"", n.borrow().text);
        }
        if n.borrow().kind == 2 {
            let bf: bool = n.borrow().flag;
            if bf {
                return "TRUE".to_string();
            }
            return "FALSE".to_string();
        }
        if n.borrow().kind == 3 || n.borrow().kind == 4 {
            let mut s: String = FormulaParser::formatSheetPrefix(&n.borrow().sheet.clone());
            s = format!("{}{}", s, FormulaParser::formatA1(
                n.borrow().row,
                n.borrow().col,
                n.borrow().absRow,
                n.borrow().absCol
            ));
            if n.borrow().kind == 4 {
                s = format!("{}:", s);
                s = format!("{}{}", s, FormulaParser::formatA1(
                    n.borrow().row2,
                    n.borrow().col2,
                    n.borrow().absRow2,
                    n.borrow().absCol2
                ));
            }
            return s;
        }
        if n.borrow().kind == 5 {
            let k0: Rc<RefCell<FormulaNode>> = n.borrow().kids[0].clone();
            let mut s0: String = FormulaParser::serializeNode(&k0);
            if FormulaParser::nodePrec(&k0) < 6 {
                s0 = format!("({})", s0);
            }
            return format!("{}{}", n.borrow().op, s0);
        }
        // Parenthesize only where precedence (or left-associativity on the right
        // operand) requires it, so a filled "=A2+B2" stays "=A3+B3".
        if n.borrow().kind == 6 {
            let lk: Rc<RefCell<FormulaNode>> = n.borrow().kids[0].clone();
            let rk: Rc<RefCell<FormulaNode>> = n.borrow().kids[1].clone();
            let p: i64 = FormulaParser::opPrec(&n.borrow().op.clone());
            let mut L: String = FormulaParser::serializeNode(&lk);
            let mut R: String = FormulaParser::serializeNode(&rk);
            if FormulaParser::nodePrec(&lk) < p {
                L = format!("({})", L);
            }
            if FormulaParser::nodePrec(&rk) <= p {
                R = format!("({})", R);
            }
            return format!("{}{}{}", L, n.borrow().op, R);
        }
        if n.borrow().kind == 7 {
            let mut out: String = format!("{}(", n.borrow().fnName);
            let mut i: i64 = 0;
            let kn: i64 = n.borrow().kids.len() as i64;
            while i < kn {
                if i > 0 {
                    out = format!("{},", out);
                }
                out = format!("{}{}", out, FormulaParser::serializeNode(&n.borrow().kids[i as usize].clone()));
                i += 1;
            }
            return format!("{})", out);
        }
        if n.borrow().kind == 9 {
            return FormulaParser::serializeTableRef(n);
        }
        String::new()
    }
    /// A structured reference, written back the way the file spelled it. Which
    /// spelling that is matters more than it looks: `Table1[@Amount]` and
    /// `Table1[[#This Row],[Amount]]` mean the same thing, Excel 2007 wrote the
    /// second and Excel writes the first, and a round trip that swapped one for
    /// the other would rewrite formulas in a file it was only asked to open.
    pub fn serializeTableRef(n: &Rc<RefCell<FormulaNode>>) -> String {
        let mut out: String = n.borrow().tableName.clone();
        out.push_str(&"[".to_string());
        let has_item: bool = !n.borrow().tableItem.is_empty();
        let has_col: bool = !n.borrow().tableCol1.is_empty();
        let has_span: bool = !n.borrow().tableCol2.is_empty();
        if n.borrow().tableAtSign && has_col {
            out.push_str(&"@".to_string());
            out.push_str(&FormulaParser::escapeColumn(&n.borrow().tableCol1));
            out.push_str(&"]".to_string());
            return out;
        }
        let mut need_brackets: bool = false;
        if has_item && has_col {
            need_brackets = true;
        }
        if has_span {
            need_brackets = true;
        }
        if has_item {
            if need_brackets {
                out.push_str(&"[".to_string());
                out.push_str(&FormulaParser::itemLabel(&n.borrow().tableItem));
                out.push_str(&"]".to_string());
            } else {
                out.push_str(&FormulaParser::itemLabel(&n.borrow().tableItem));
            }
        }
        if has_item && has_col {
            out.push_str(&",".to_string());
        }
        if has_col {
            if need_brackets {
                out.push_str(&"[".to_string());
                out.push_str(&FormulaParser::escapeColumn(&n.borrow().tableCol1));
                out.push_str(&"]".to_string());
            } else {
                out.push_str(&FormulaParser::escapeColumn(&n.borrow().tableCol1));
            }
        }
        if has_span {
            out.push_str(&":[".to_string());
            out.push_str(&FormulaParser::escapeColumn(&n.borrow().tableCol2));
            out.push_str(&"]".to_string());
        }
        out.push_str(&"]".to_string());
        out
    }
    /// Specifiers are held folded so comparisons are simple, and written back in
    /// the capitalisation Excel uses.
    pub fn itemLabel(item: &str) -> String {
        if item == "#this row" {
            return "#This Row".to_string();
        }
        if item == "#headers" {
            return "#Headers".to_string();
        }
        if item == "#totals" {
            return "#Totals".to_string();
        }
        if item == "#data" {
            return "#Data".to_string();
        }
        if item == "#all" {
            return "#All".to_string();
        }
        item.to_string()
    }
    pub fn escapeColumn(s: &str) -> String {
        let mut out: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&s);
        while i < n {
            let ch: i64 = DgNative::char_at(&s, i);
            if (((ch == 91 || ch == 93) || ch == 35) || ch == 39) || ch == 64 {
                out.push_str(&"'".to_string());
            }
            let piece: String = DgNative::substr(&s, i, i + 1);
            out.push_str(&piece);
            i += 1;
        }
        out
    }
    /// Translate a formula string by (dx, dy); returns "" on parse failure.
    pub fn translateFormula(expr: &str, dx: i64, dy: i64) -> String {
        let p: Rc<RefCell<FormulaParser>> = Rc::new(RefCell::new(FormulaParser::new()));
        let ast: Rc<RefCell<FormulaNode>> = p.borrow_mut().parse(expr);
        if !p.borrow().ok {
            return String::new();
        }
        FormulaParser::translateNode(&ast, dx, dy);
        FormulaParser::serializeNode(&ast)
    }
    pub fn reset(&mut self, s: String) {
        self.src = s.clone();
        self.pos = 0;
        self.srcLen = DgNative::str_len(&s);
        self.ok = true;
        self.err = String::new();
        if self.srcLen > 0 {
            if DgNative::char_at(&self.src, 0) == 61 {
                self.pos = 1;
            }
        }
    }
    pub fn peek(&self) -> i64 {
        if self.pos >= self.srcLen {
            return -1;
        }
        DgNative::char_at(&self.src, self.pos)
    }
    pub fn next(&mut self) -> i64 {
        if self.pos >= self.srcLen {
            return -1;
        }
        let ch: i64 = DgNative::char_at(&self.src, self.pos);
        self.pos += 1;
        ch
    }
    /// The character `next` just consumed, as text.
    ///
    /// Not `(strfromcode (this.next()))`: that pair looks like an identity and
    /// is one in JavaScript, where a string is code units. In C++ a string is
    /// BYTES, so `charAt` hands back 0xC3 and `strfromcode` — whose job is a
    /// code POINT — encodes 195 as two bytes of UTF-8, and every accented
    /// letter inside a quoted formula string would be encoded twice.
    pub fn lastAsText(&self) -> String {
        if self.pos < 1 {
            return String::new();
        }
        DgNative::substr(&self.src, self.pos - 1, self.pos)
    }
    pub fn skipWs(&mut self) {
        while self.pos < self.srcLen {
            let ch: i64 = DgNative::char_at(&self.src, self.pos);
            if ((ch == 32 || ch == 9) || ch == 10) || ch == 13 {
                self.pos += 1;
            } else {
                return;
            }
        }
    }
    pub fn parse(&mut self, s: &str) -> Rc<RefCell<FormulaNode>> {
        self.reset(s.to_string());
        let n: Rc<RefCell<FormulaNode>> = self.parseExpr();
        self.skipWs();
        if self.pos < self.srcLen {
            self.ok = false;
            self.err = "#VALUE!".to_string();
        }
        if !self.ok {
            let e: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            e.borrow_mut().kind = 8;
            e.borrow_mut().text = self.err.clone();
            if e.borrow().text.is_empty() {
                e.borrow_mut().text = "#VALUE!".to_string();
            }
            return e;
        }
        n
    }
    pub fn parseExpr(&mut self) -> Rc<RefCell<FormulaNode>> {
        self.parseCompare()
    }
    pub fn parseCompare(&mut self) -> Rc<RefCell<FormulaNode>> {
        let mut left: Rc<RefCell<FormulaNode>> = self.parseConcat();
        self.skipWs();
        while self.pos < self.srcLen {
            let ch: i64 = self.peek();
            let mut op: &str = "";
            if ch == 60 {
                self.next();
                if self.peek() == 61 {
                    self.next();
                    op = "<=";
                } else {
                    if self.peek() == 62 {
                        self.next();
                        op = "<>";
                    } else {
                        op = "<";
                    }
                }
            } else {
                if ch == 62 {
                    self.next();
                    if self.peek() == 61 {
                        self.next();
                        op = ">=";
                    } else {
                        op = ">";
                    }
                } else {
                    if ch == 61 {
                        self.next();
                        op = "=";
                    } else {
                        break;
                    }
                }
            }
            let right: Rc<RefCell<FormulaNode>> = self.parseConcat();
            let b: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            b.borrow_mut().kind = 6;
            b.borrow_mut().op = op.to_string();
            b.borrow_mut().kids.push(left.clone());
            b.borrow_mut().kids.push(right.clone());
            left = b.clone();
            self.skipWs();
        }
        left
    }
    pub fn parseConcat(&mut self) -> Rc<RefCell<FormulaNode>> {
        let mut left: Rc<RefCell<FormulaNode>> = self.parseAdd();
        self.skipWs();
        while self.peek() == 38 {
            self.next();
            let right: Rc<RefCell<FormulaNode>> = self.parseAdd();
            let b: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            b.borrow_mut().kind = 6;
            b.borrow_mut().op = "&".to_string();
            b.borrow_mut().kids.push(left.clone());
            b.borrow_mut().kids.push(right.clone());
            left = b.clone();
            self.skipWs();
        }
        left
    }
    pub fn parseAdd(&mut self) -> Rc<RefCell<FormulaNode>> {
        let mut left: Rc<RefCell<FormulaNode>> = self.parseMul();
        self.skipWs();
        loop {
            let ch: i64 = self.peek();
            if ch != 43 && ch != 45 {
                break;
            }
            self.next();
            let right: Rc<RefCell<FormulaNode>> = self.parseMul();
            let b: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            b.borrow_mut().kind = 6;
            if ch == 43 {
                b.borrow_mut().op = "+".to_string();
            } else {
                b.borrow_mut().op = "-".to_string();
            }
            b.borrow_mut().kids.push(left.clone());
            b.borrow_mut().kids.push(right.clone());
            left = b.clone();
            self.skipWs();
        }
        left
    }
    pub fn parseMul(&mut self) -> Rc<RefCell<FormulaNode>> {
        let mut left: Rc<RefCell<FormulaNode>> = self.parsePow();
        self.skipWs();
        loop {
            let ch: i64 = self.peek();
            if ch != 42 && ch != 47 {
                break;
            }
            self.next();
            let right: Rc<RefCell<FormulaNode>> = self.parsePow();
            let b: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            b.borrow_mut().kind = 6;
            if ch == 42 {
                b.borrow_mut().op = "*".to_string();
            } else {
                b.borrow_mut().op = "/".to_string();
            }
            b.borrow_mut().kids.push(left.clone());
            b.borrow_mut().kids.push(right.clone());
            left = b.clone();
            self.skipWs();
        }
        left
    }
    pub fn parsePow(&mut self) -> Rc<RefCell<FormulaNode>> {
        let left: Rc<RefCell<FormulaNode>> = self.parseUnary();
        self.skipWs();
        if self.peek() == 94 {
            self.next();
            let right: Rc<RefCell<FormulaNode>> = self.parseUnary();
            let b: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            b.borrow_mut().kind = 6;
            b.borrow_mut().op = "^".to_string();
            b.borrow_mut().kids.push(left.clone());
            b.borrow_mut().kids.push(right.clone());
            return b;
        }
        left
    }
    pub fn parseUnary(&mut self) -> Rc<RefCell<FormulaNode>> {
        self.skipWs();
        let ch: i64 = self.peek();
        if ch == 43 || ch == 45 {
            self.next();
            let kid: Rc<RefCell<FormulaNode>> = self.parseUnary();
            let u: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            u.borrow_mut().kind = 5;
            if ch == 45 {
                u.borrow_mut().op = "-".to_string();
            } else {
                u.borrow_mut().op = "+".to_string();
            }
            u.borrow_mut().kids.push(kid.clone());
            return u;
        }
        self.parsePrimary()
    }
    pub fn parsePrimary(&mut self) -> Rc<RefCell<FormulaNode>> {
        self.skipWs();
        let ch: i64 = self.peek();
        if ch < 0 {
            self.ok = false;
            self.err = "#VALUE!".to_string();
            let e: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            e.borrow_mut().kind = 8;
            e.borrow_mut().text = "#VALUE!".to_string();
            return e;
        }
        // number
        if (ch >= 48 && ch <= 57) || ch == 46 {
            return self.parseNumber();
        }
        // string
        if ch == 34 {
            return self.parseString();
        }
        // quoted sheet name: 'My Sheet'!A1
        if ch == 39 {
            return self.parseQuotedSheetRef();
        }
        // paren
        if ch == 40 {
            self.next();
            let inner: Rc<RefCell<FormulaNode>> = self.parseExpr();
            self.skipWs();
            if self.peek() == 41 {
                self.next();
            } else {
                self.ok = false;
                self.err = "#VALUE!".to_string();
            }
            return inner;
        }
        // `[Column]` with no table in front: the table the formula sits in.
        if ch == 91 {
            return self.parseStructured(String::new());
        }
        // name / sheet / cell / function
        if (((ch >= 65 && ch <= 90) || (ch >= 97 && ch <= 122)) || ch == 95) || ch == 36 {
            return self.parseNameOrRef();
        }
        self.ok = false;
        self.err = "#VALUE!".to_string();
        let e2: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
        e2.borrow_mut().kind = 8;
        e2.borrow_mut().text = "#VALUE!".to_string();
        e2
    }
    pub fn parseNumber(&mut self) -> Rc<RefCell<FormulaNode>> {
        let start: i64 = self.pos;
        while self.pos < self.srcLen {
            let ch: i64 = DgNative::char_at(&self.src, self.pos);
            if (ch >= 48 && ch <= 57) || ch == 46 {
                self.pos += 1;
            } else {
                break;
            }
        }
        let s: String = DgNative::substr(&self.src, start, self.pos);
        let n: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
        n.borrow_mut().kind = 0;
        let p: Option<f64> = s.parse::<f64>().ok();
        if p.is_some() {
            n.borrow_mut().num = p.unwrap();
        } else {
            self.ok = false;
            self.err = "#VALUE!".to_string();
            n.borrow_mut().kind = 8;
            n.borrow_mut().text = "#VALUE!".to_string();
        }
        n
    }
    pub fn parseString(&mut self) -> Rc<RefCell<FormulaNode>> {
        self.next();
        let mut out: String = String::new();
        while self.pos < self.srcLen {
            let ch: i64 = self.next();
            if ch == 34 {
                if self.peek() == 34 {
                    self.next();
                    out = format!("{}\"", out);
                } else {
                    break;
                }
            } else {
                out = format!("{}{}", out, self.lastAsText());
            }
        }
        let n: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
        n.borrow_mut().kind = 1;
        n.borrow_mut().text = out.clone();
        n
    }
    /// --- structured references ------------------------------------------------
    /// `Table1[Amount]`, `Table1[[#This Row],[CategoryID]]`, `Table1[@Amount]`,
    /// `Table1[[Amount]:[Price]]`, `Table1[#All]`.
    ///
    /// The grammar is small but the bracketing is fiddly: the OUTER brackets
    /// wrap everything, and a reference naming more than one thing wraps each of
    /// them in brackets of their own and separates them with a comma. So
    /// `Table1[Amount]` and `Table1[[Amount]]` are the same reference written
    /// two ways, and both have to read.
    ///
    /// Inside a name, `'` escapes a character that would otherwise be syntax —
    /// a column genuinely called "#Amount" is written `['#Amount]`. Rare, and
    /// cheap to honour.
    pub fn parseStructured(&mut self, table_name: String) -> Rc<RefCell<FormulaNode>> {
        let n: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
        n.borrow_mut().kind = 9;
        n.borrow_mut().tableName = table_name.clone();
        let inner: String = self.readBracketed();
        if !self.ok {
            let e: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            e.borrow_mut().kind = 8;
            e.borrow_mut().text = "#VALUE!".to_string();
            return e;
        }
        let mut parts: Vec<String> = Vec::new();
        FormulaParser::splitTopLevel(&inner, &mut parts);
        let mut i: i64 = 0;
        while i < (parts.len() as i64) {
            let raw: String = parts[i as usize].clone();
            let piece: String = FormulaParser::stripOuterBrackets(&raw);
            if !piece.is_empty() {
                if DgNative::char_at(&piece, 0) == 64 {
                    // `@Amount` — the 2010 spelling of "this row, that column".
                    n.borrow_mut().tableItem = "#this row".to_string();
                    n.borrow_mut().tableAtSign = true;
                    let mut rest: String = DgNative::substr(&piece, 1, DgNative::str_len(&piece));
                    rest = FormulaParser::stripOuterBrackets(&rest);
                    if !rest.is_empty() {
                        n.borrow_mut().tableCol1 = FormulaParser::unescapeColumn(&rest);
                    }
                } else {
                    if DgNative::char_at(&piece, 0) == 35 {
                        n.borrow_mut().tableItem = piece.to_lowercase();
                    } else {
                        // A column, or a span of them: `[Amount]:[Price]`.
                        let cut: i64 = FormulaParser::topLevelColon(&raw);
                        if cut >= 0 {
                            let a: String = FormulaParser::stripOuterBrackets(&DgNative::substr(&raw, 0, cut));
                            let b: String = FormulaParser::stripOuterBrackets(&DgNative::substr(&raw, cut + 1, DgNative::str_len(&raw)));
                            n.borrow_mut().tableCol1 = FormulaParser::unescapeColumn(&a);
                            n.borrow_mut().tableCol2 = FormulaParser::unescapeColumn(&b);
                        } else {
                            n.borrow_mut().tableCol1 = FormulaParser::unescapeColumn(&piece);
                        }
                    }
                }
            }
            i += 1;
        }
        n
    }
    /// From a `[` to its matching `]`, returning what is between them. Nesting
    /// is counted so the outer bracket of `[[#This Row],[Col]]` closes last.
    pub fn readBracketed(&mut self) -> String {
        if self.peek() != 91 {
            self.ok = false;
            self.err = "#VALUE!".to_string();
            return String::new();
        }
        self.next();
        let start: i64 = self.pos;
        let mut depth: i64 = 1;
        while self.pos < self.srcLen {
            let ch: i64 = DgNative::char_at(&self.src, self.pos);
            if ch == 39 {
                // An escape takes the next character with it, whatever it is.
                self.pos += 2;
                continue;
            }
            if ch == 91 {
                depth += 1;
            }
            if ch == 93 {
                depth -= 1;
                if depth == 0 {
                    let inner: String = DgNative::substr(&self.src, start, self.pos);
                    self.pos += 1;
                    return inner;
                }
            }
            self.pos += 1;
        }
        self.ok = false;
        self.err = "#VALUE!".to_string();
        String::new()
    }
    pub fn parseNameOrRef(&mut self) -> Rc<RefCell<FormulaNode>> {
        // Optional Sheet! prefix
        let mut sheet: String = String::new();
        let mut start: i64 = self.pos;
        // read identifier or $A$1
        while self.pos < self.srcLen {
            let ch: i64 = DgNative::char_at(&self.src, self.pos);
            if (((((ch >= 65 && ch <= 90) || (ch >= 97 && ch <= 122)) || (ch >= 48 && ch <= 57)) || ch == 36) || ch == 95) || ch == 46 {
                self.pos += 1;
            } else {
                break;
            }
        }
        let mut ident: String = DgNative::substr(&self.src, start, self.pos);
        self.skipWs();
        if self.peek() == 33 {
            // sheet name
            self.next();
            sheet = ident.clone();
            start = self.pos;
            while self.pos < self.srcLen {
                let ch2: i64 = DgNative::char_at(&self.src, self.pos);
                if (((ch2 >= 65 && ch2 <= 90) || (ch2 >= 97 && ch2 <= 122)) || (ch2 >= 48 && ch2 <= 57)) || ch2 == 36 {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            ident = DgNative::substr(&self.src, start, self.pos);
            self.skipWs();
        }
        // A structured reference: `Table1[...]`. Checked before the function
        // call, because a `[` can only ever start one of these, and after the
        // sheet prefix, because `Sheet1!Table1[...]` is not a thing — a table
        // name is already workbook-wide.
        if self.peek() == 91 {
            if sheet.is_empty() {
                return self.parseStructured(ident.clone());
            }
        }
        // function call?
        if self.peek() == 40 {
            self.next();
            let call: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            call.borrow_mut().kind = 7;
            call.borrow_mut().fnName = FormulaParser::upper(&ident);
            self.skipWs();
            if self.peek() != 41 {
                loop {
                    let arg: Rc<RefCell<FormulaNode>> = self.parseExpr();
                    call.borrow_mut().kids.push(arg.clone());
                    self.skipWs();
                    if self.peek() == 44 {
                        self.next();
                        self.skipWs();
                    } else {
                        break;
                    }
                }
            }
            if self.peek() == 41 {
                self.next();
            } else {
                self.ok = false;
                self.err = "#VALUE!".to_string();
            }
            return call;
        }
        // Boolean literals
        let up_ident: String = FormulaParser::upper(&ident);
        if up_ident == "TRUE" {
            let bt: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            bt.borrow_mut().kind = 2;
            bt.borrow_mut().flag = true;
            return bt;
        }
        if up_ident == "FALSE" {
            let bf: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            bf.borrow_mut().kind = 2;
            bf.borrow_mut().flag = false;
            return bf;
        }
        // cell or range
        let mut r1: Vec<i64> = Vec::new();
        let mut c1: Vec<i64> = Vec::new();
        let mut ar1: Vec<bool> = Vec::new();
        let mut ac1: Vec<bool> = Vec::new();
        FormulaParser::parseA1(&ident, &mut r1, &mut c1, &mut ar1, &mut ac1);
        let cell: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
        cell.borrow_mut().kind = 3;
        cell.borrow_mut().sheet = sheet.clone();
        cell.borrow_mut().row = r1[0];
        cell.borrow_mut().col = c1[0];
        cell.borrow_mut().absRow = ar1[0];
        cell.borrow_mut().absCol = ac1[0];
        self.skipWs();
        if self.peek() == 58 {
            self.next();
            start = self.pos;
            while self.pos < self.srcLen {
                let ch3: i64 = DgNative::char_at(&self.src, self.pos);
                if (((ch3 >= 65 && ch3 <= 90) || (ch3 >= 97 && ch3 <= 122)) || (ch3 >= 48 && ch3 <= 57)) || ch3 == 36 {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let ident2: String = DgNative::substr(&self.src, start, self.pos);
            let mut r2: Vec<i64> = Vec::new();
            let mut c2: Vec<i64> = Vec::new();
            let mut ar2: Vec<bool> = Vec::new();
            let mut ac2: Vec<bool> = Vec::new();
            FormulaParser::parseA1(&ident2, &mut r2, &mut c2, &mut ar2, &mut ac2);
            cell.borrow_mut().kind = 4;
            cell.borrow_mut().row2 = r2[0];
            cell.borrow_mut().col2 = c2[0];
            cell.borrow_mut().absRow2 = ar2[0];
            cell.borrow_mut().absCol2 = ac2[0];
        }
        cell
    }
    pub fn parseQuotedSheetRef(&mut self) -> Rc<RefCell<FormulaNode>> {
        // leading '
        self.next();
        let mut sheet: String = String::new();
        while self.pos < self.srcLen {
            let ch: i64 = self.next();
            if ch == 39 {
                if self.peek() == 39 {
                    self.next();
                    sheet = format!("{}'", sheet);
                } else {
                    break;
                }
            } else {
                sheet = format!("{}{}", sheet, self.lastAsText());
            }
        }
        self.skipWs();
        if self.peek() != 33 {
            self.ok = false;
            self.err = "#REF!".to_string();
            let e: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
            e.borrow_mut().kind = 8;
            e.borrow_mut().text = "#REF!".to_string();
            return e;
        }
        self.next();
        self.skipWs();
        let mut start: i64 = self.pos;
        while self.pos < self.srcLen {
            let ch2: i64 = DgNative::char_at(&self.src, self.pos);
            if (((ch2 >= 65 && ch2 <= 90) || (ch2 >= 97 && ch2 <= 122)) || (ch2 >= 48 && ch2 <= 57)) || ch2 == 36 {
                self.pos += 1;
            } else {
                break;
            }
        }
        let ident: String = DgNative::substr(&self.src, start, self.pos);
        let mut r1: Vec<i64> = Vec::new();
        let mut c1: Vec<i64> = Vec::new();
        let mut ar1: Vec<bool> = Vec::new();
        let mut ac1: Vec<bool> = Vec::new();
        FormulaParser::parseA1(&ident, &mut r1, &mut c1, &mut ar1, &mut ac1);
        let cell: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
        cell.borrow_mut().kind = 3;
        cell.borrow_mut().sheet = sheet.clone();
        cell.borrow_mut().row = r1[0];
        cell.borrow_mut().col = c1[0];
        cell.borrow_mut().absRow = ar1[0];
        cell.borrow_mut().absCol = ac1[0];
        self.skipWs();
        if self.peek() == 58 {
            self.next();
            start = self.pos;
            while self.pos < self.srcLen {
                let ch3: i64 = DgNative::char_at(&self.src, self.pos);
                if (((ch3 >= 65 && ch3 <= 90) || (ch3 >= 97 && ch3 <= 122)) || (ch3 >= 48 && ch3 <= 57)) || ch3 == 36 {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let ident2: String = DgNative::substr(&self.src, start, self.pos);
            let mut r2: Vec<i64> = Vec::new();
            let mut c2: Vec<i64> = Vec::new();
            let mut ar2: Vec<bool> = Vec::new();
            let mut ac2: Vec<bool> = Vec::new();
            FormulaParser::parseA1(&ident2, &mut r2, &mut c2, &mut ar2, &mut ac2);
            cell.borrow_mut().kind = 4;
            cell.borrow_mut().row2 = r2[0];
            cell.borrow_mut().col2 = c2[0];
            cell.borrow_mut().absRow2 = ar2[0];
            cell.borrow_mut().absCol2 = ac2[0];
        }
        cell
    }
}

/// --- dependency graph + evaluator --------------------------------------------
#[derive(Clone)]
pub struct FormulaEngine {
    pub book: Option<Rc<RefCell<WorkbookModel>>>,
    pub parser: Rc<RefCell<FormulaParser>>,
    /// forward deps: cellKey → "depKey;depKey;..."
    pub depsOf: HashMap<String,String>,
    /// reverse: cellKey → dependents joined
    pub usedBy: HashMap<String,String>,
    pub manualCalc: bool,
    pub calcValues: HashMap<String,String>,
    pub calcStatus: HashMap<String,String>,
    pub lastError: String,
    /// What TODAY() and NOW() answer, as a serial. Zero means the host has not
    /// said, and those two functions then return #N/A rather than inventing a
    /// date — a spreadsheet that quietly disagrees with the calendar is worse
    /// than one that admits it does not know.
    pub todaySerial: f64,
    pub subtotalDepth: i64,
    /// Instrumentation for workbook-scale recalc demos / benches.
    pub dirtyMarked: i64,
    pub recalculated: i64,
    /// Which cell is being looked at. Only `[#This Row]` needs it — a structured
    /// reference is the one kind of reference whose meaning depends on where it
    /// is written — and threading two more arguments through every level of a
    /// recursive evaluator to serve one node kind would be paid for by every
    /// other kind. Set at the two entry points that know: `evalOne`, and
    /// `registerFormula` before it collects dependencies.
    pub curRow: i64,
    pub curCol: i64,
    pub curSheet: i64,
}

impl FormulaEngine {
    pub fn new() -> Self {
        Self {
            book: None,
            parser: Rc::new(RefCell::new(FormulaParser::new())),
            depsOf: HashMap::new(),
            usedBy: HashMap::new(),
            manualCalc: false,
            calcValues: HashMap::new(),
            calcStatus: HashMap::new(),
            lastError: String::new(),
            todaySerial: 0.0,
            subtotalDepth: 0,
            dirtyMarked: 0,
            recalculated: 0,
            curRow: 0,
            curCol: 0,
            curSheet: 0,
        }
    }
    /// A1, $B$7, XFD1048576: one to three letters then digits.
    pub fn looksLikeCell(word: &str) -> bool {
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&word);
        if i < n && DgNative::char_at(&word, i) == 36 {
            i += 1;
        }
        let mut letters: i64 = 0;
        while i < n && (DgNative::char_at(&word, i) >= 65 && DgNative::char_at(&word, i) <= 90 || DgNative::char_at(&word, i) >= 97 && DgNative::char_at(&word, i) <= 122) {
            letters += 1;
            i += 1;
        }
        if letters == 0 || letters > 3 {
            return false;
        }
        if i < n && DgNative::char_at(&word, i) == 36 {
            i += 1;
        }
        let mut digits: i64 = 0;
        while i < n && (DgNative::char_at(&word, i) >= 48 && DgNative::char_at(&word, i) <= 57) {
            digits += 1;
            i += 1;
        }
        digits > 0 && i == n
    }
    pub fn resetStats(&mut self) {
        self.dirtyMarked = 0;
        self.recalculated = 0;
    }
    pub fn attach(&mut self, b: Rc<RefCell<WorkbookModel>>) {
        self.book = Some(b.clone());
        let empty1: HashMap<String,String> = HashMap::new();
        let empty2: HashMap<String,String> = HashMap::new();
        let empty3: HashMap<String,String> = HashMap::new();
        let empty4: HashMap<String,String> = HashMap::new();
        self.depsOf = empty1.clone();
        self.usedBy = empty2.clone();
        self.calcValues = empty3.clone();
        self.calcStatus = empty4.clone();
        self.resetStats();
    }
    pub fn cellKey(&self, sheet_idx: i64, row: i64, col: i64) -> String {
        format!("{}!{},{}", sheet_idx, row, col)
    }
    pub fn sheetByName(&self, name: &str) -> i64 {
        let mut i: i64 = 0;
        let n: i64 = (self.book.as_ref().unwrap()).borrow().sheetCount();
        let want: String = FormulaFunctions::upper(name);
        while i < n {
            let s: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(i);
            if FormulaFunctions::upper(&s.borrow().name.clone()) == want {
                return i;
            }
            i += 1;
        }
        -1
    }
    pub fn rebuildAll(&mut self) {
        let empty1: HashMap<String,String> = HashMap::new();
        let empty2: HashMap<String,String> = HashMap::new();
        let empty3: HashMap<String,String> = HashMap::new();
        let empty4: HashMap<String,String> = HashMap::new();
        self.depsOf = empty1.clone();
        self.usedBy = empty2.clone();
        self.calcValues = empty3.clone();
        self.calcStatus = empty4.clone();
        let mut si: i64 = 0;
        let sn: i64 = (self.book.as_ref().unwrap()).borrow().sheetCount();
        while si < sn {
            let sheet: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(si);
            let mut i: i64 = 0;
            let n: i64 = sheet.borrow().formulaKeys.len() as i64;
            while i < n {
                let k: String = sheet.borrow().formulaKeys[i as usize].clone();
                let parts: Vec<String> = k.split(",").map(|s| s.to_string()).collect::<Vec<String>>();
                if (parts.len() as i64) >= 2 {
                    let row: i64 = SpreadsheetModel::parseInt0(&parts[0].clone());
                    let col: i64 = SpreadsheetModel::parseInt0(&parts[1].clone());
                    self.registerFormula(si, row, col, &sheet.borrow().getFormula(row, col).clone());
                }
                i += 1;
            }
            si += 1;
        }
        self.recalcAll();
    }
    /// Defined names, written out: each identifier that is a name (and not a
    /// function, a sheet prefix or a cell) becomes what it refers to. Done on
    /// the text so every later step reads plain references.
    pub fn expandNames(&self, expr: &str) -> String {
        let mut out: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&expr);
        let mut in_str: bool = false;
        let mut in_quote: bool = false;
        let mut depth: i64 = 0;
        while i < n {
            let ch: i64 = DgNative::char_at(&expr, i);
            // Structured references (`Tbl[[#Totals],[Qty]]`) name columns,
            // not defined names: copied as they are.
            if !in_str && (!in_quote && ch == 91) {
                depth += 1;
            }
            if !in_str && (!in_quote && (ch == 93 && depth > 0)) {
                depth -= 1;
                out = format!("{}{}", out, DgNative::substr(&expr, i, i + 1));
                i += 1;
                continue;
            }
            if depth > 0 {
                out = format!("{}{}", out, DgNative::substr(&expr, i, i + 1));
                i += 1;
                continue;
            }
            if ch == 39 && !in_str {
                in_quote = !in_quote;
                out = format!("{}{}", out, DgNative::substr(&expr, i, i + 1));
                i += 1;
                continue;
            }
            if in_quote {
                out = format!("{}{}", out, DgNative::substr(&expr, i, i + 1));
                i += 1;
                continue;
            }
            if ch == 34 {
                in_str = !in_str;
                out = format!("{}{}", out, DgNative::substr(&expr, i, i + 1));
                i += 1;
                continue;
            }
            let starts_word: bool = (ch >= 65 && ch <= 90) || ((ch >= 97 && ch <= 122) || ch == 95);
            if in_str || !starts_word {
                out = format!("{}{}", out, DgNative::substr(&expr, i, i + 1));
                i += 1;
                continue;
            }
            let mut j: i64 = i;
            while j < n {
                let cj: i64 = DgNative::char_at(&expr, j);
                let word_ch: bool = (cj >= 65 && cj <= 90) || ((cj >= 97 && cj <= 122) || ((cj >= 48 && cj <= 57) || (cj == 95 || (cj == 46 || cj == 36))));
                if !word_ch {
                    break;
                }
                j += 1;
            }
            let word: String = DgNative::substr(&expr, i, j);
            let mut next_ch: i64 = 0;
            if j < n {
                next_ch = DgNative::char_at(&expr, j);
            }
            let mut prev_ch: i64 = 0;
            if i > 0 {
                prev_ch = DgNative::char_at(&expr, i - 1);
            }
            let mut r#ref: String = String::new();
            let free: bool = next_ch != 40 && (next_ch != 33 && (next_ch != 91 && (prev_ch != 33 && (next_ch != 58 && prev_ch != 58))));
            if free {
                r#ref = (self.book.as_ref().unwrap()).borrow().nameRef(&word);
            }
            if !r#ref.is_empty() {
                out = format!("{}{}", out, r#ref);
            } else {
                // A word that is neither a name nor a cell is #NAME?, as in
                // Excel; an unknown function call says the same.
                if free && !FormulaEngine::looksLikeCell(&word) {
                    let up: String = word.to_uppercase();
                    if up == "TRUE" || up == "FALSE" {
                        out = format!("{}{}", out, word);
                    } else {
                        out = format!("{}NAME.ERROR()", out);
                    }
                } else {
                    out = format!("{}{}", out, word);
                }
            }
            i = j;
        }
        out
    }
    pub fn registerFormula(&mut self, sheet_idx: i64, row: i64, col: i64, expr: &str) {
        let key: String = self.cellKey(sheet_idx, row, col);
        self.pruneDeps(&key);
        let ast: Rc<RefCell<FormulaNode>> = self.parser.borrow_mut().parse(&self.expandNames(expr));
        let mut dep_list: Vec<String> = Vec::new();
        // `[#This Row]` resolves against the cell holding the formula, and
        // dependencies are worked out here rather than at evaluation time, so
        // this is the other place that has to say which cell that is.
        self.curRow = row;
        self.curCol = col;
        self.curSheet = sheet_idx;
        self.collectDeps(&ast, sheet_idx, &mut dep_list);
        let mut joined: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = dep_list.len() as i64;
        while i < n {
            if i > 0 {
                joined = format!("{};", joined);
            }
            joined = format!("{}{}", joined, dep_list[i as usize].clone());
            i += 1;
        }
        self.depsOf.insert(key.clone(), joined.clone());
        i = 0;
        while i < n {
            let d: String = dep_list[i as usize].clone();
            let mut cur: String = String::new();
            if self.usedBy.contains_key(&d) {
                cur = self.usedBy.get(&d).cloned().unwrap();
            }
            if cur.is_empty() {
                self.usedBy.insert(d.clone(), key.clone());
            } else {
                self.usedBy.insert(d.clone(), format!("{};{}", cur, key));
            }
            i += 1;
        }
        if self.parser.borrow().ok {
            self.calcStatus.insert(key.clone(), "pending".to_string());
        } else {
            self.calcStatus.insert(key.clone(), "unsupported".to_string());
        }
    }
    /// Remove key from previous reverse edges before re-registering.
    pub fn pruneDeps(&mut self, key: &str) {
        if self.depsOf.contains_key(key) {
            let mut old: Vec<String> = Vec::new();
            self.splitKeys(&self.depsOf.get(key).cloned().unwrap().clone(), &mut old);
            let mut i: i64 = 0;
            let n: i64 = old.len() as i64;
            while i < n {
                let d: String = old[i as usize].clone();
                if self.usedBy.contains_key(&d) {
                    let mut kept: Vec<String> = Vec::new();
                    let mut kids: Vec<String> = Vec::new();
                    self.splitKeys(&self.usedBy.get(&d).cloned().unwrap().clone(), &mut kids);
                    let mut j: i64 = 0;
                    let mut kn: i64 = kids.len() as i64;
                    while j < kn {
                        let k: String = kids[j as usize].clone();
                        if k != key {
                            kept.push(k.clone());
                        }
                        j += 1;
                    }
                    let mut joined: String = String::new();
                    j = 0;
                    kn = kept.len() as i64;
                    while j < kn {
                        if j > 0 {
                            joined = format!("{};", joined);
                        }
                        joined = format!("{}{}", joined, kept[j as usize].clone());
                        j += 1;
                    }
                    self.usedBy.insert(d.clone(), joined.clone());
                }
                i += 1;
            }
        }
    }
    pub fn clearFormula(&mut self, sheet_idx: i64, row: i64, col: i64) {
        let key: String = self.cellKey(sheet_idx, row, col);
        self.pruneDeps(&key);
        if self.depsOf.contains_key(&key) {
            self.depsOf.insert(key.clone(), String::new());
        }
        self.calcStatus.insert(key.clone(), "cached".to_string());
    }
    pub fn collectDeps(&self, n: &Rc<RefCell<FormulaNode>>, sheet_idx: i64, out: &mut Vec<String>) {
        if n.borrow().kind == 9 {
            // A structured reference depends on the cells it resolves to, and
            // nothing else can work out which those are. Without this the
            // formula computes correctly the first time and then never again:
            // editing an amount inside the table would leave every
            // `=SUM(Table1[Amount])` showing what it said before.
            let tr: Rc<RefCell<TableRange>> = self.resolveTableRef(n);
            if !tr.borrow().err.is_empty() {
                return;
            }
            let mut rr: i64 = tr.borrow().r0;
            while rr <= tr.borrow().r1 {
                let mut cc: i64 = tr.borrow().c0;
                while cc <= tr.borrow().c1 {
                    out.push(self.cellKey(tr.borrow().sheetIdx.clone(), rr, cc));
                    cc += 1;
                }
                rr += 1;
            }
            return;
        }
        if n.borrow().kind == 3 {
            let mut si: i64 = sheet_idx;
            if !n.borrow().sheet.is_empty() {
                si = self.sheetByName(&n.borrow().sheet);
                if si < 0 {
                    return;
                }
            }
            out.push(self.cellKey(si, n.borrow().row.clone(), n.borrow().col.clone()));
            return;
        }
        if n.borrow().kind == 4 {
            let mut si2: i64 = sheet_idx;
            if !n.borrow().sheet.is_empty() {
                si2 = self.sheetByName(&n.borrow().sheet);
                if si2 < 0 {
                    return;
                }
            }
            let mut r0: i64 = n.borrow().row;
            let mut r1: i64 = n.borrow().row2;
            let mut c0: i64 = n.borrow().col;
            let mut c1: i64 = n.borrow().col2;
            if r0 > r1 {
                let t: i64 = r0;
                r0 = r1;
                r1 = t;
            }
            if c0 > c1 {
                let t2: i64 = c0;
                c0 = c1;
                c1 = t2;
            }
            let mut r: i64 = r0;
            while r <= r1 {
                let mut c: i64 = c0;
                while c <= c1 {
                    out.push(self.cellKey(si2, r, c));
                    c += 1;
                }
                r += 1;
            }
            return;
        }
        let mut i: i64 = 0;
        let kn: i64 = n.borrow().kids.len() as i64;
        while i < kn {
            self.collectDeps(&n.borrow().kids[i as usize].clone(), sheet_idx, out);
            i += 1;
        }
    }
    pub fn splitKeys(&self, joined: &str, out: &mut Vec<String>) {
        out.clear();
        if joined.is_empty() {
            return;
        }
        let mut cur: String = String::new();
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&joined);
        while i < n {
            let ch: i64 = DgNative::char_at(&joined, i);
            if ch == 59 {
                if !cur.is_empty() {
                    out.push(cur.clone());
                }
                cur = String::new();
            } else {
                cur = format!("{}{}", cur, DgNative::substr(&joined, i, i + 1));
            }
            i += 1;
        }
        if !cur.is_empty() {
            out.push(cur.clone());
        }
    }
    pub fn markDirty(&mut self, key: &str, mut visited: &mut HashMap<String,bool>) {
        if visited.contains_key(key) {
            return;
        }
        visited.insert(key.to_string(), true);
        self.calcStatus.insert(key.to_string(), "dirty".to_string());
        self.dirtyMarked += 1;
        if self.usedBy.contains_key(key) {
            let mut kids: Vec<String> = Vec::new();
            self.splitKeys(&self.usedBy.get(key).cloned().unwrap().clone(), &mut kids);
            let mut i: i64 = 0;
            let n: i64 = kids.len() as i64;
            while i < n {
                self.markDirty(&kids[i as usize].clone(), &mut visited);
                i += 1;
            }
        }
    }
    /// Mark everything that reads this cell dirty, without recalculating yet.
    /// Batch editors (paste / fill / delete range / undo) call this per cell and
    /// then recalcDirty() once, instead of recalculating N times.
    pub fn markEdited(&mut self, sheet_idx: i64, row: i64, col: i64) {
        let key: String = self.cellKey(sheet_idx, row, col);
        let mut visited: HashMap<String,bool> = HashMap::new();
        // Mark dependents dirty (not the edited value cell itself — it is already current).
        if self.usedBy.contains_key(&key) {
            let mut kids: Vec<String> = Vec::new();
            self.splitKeys(&self.usedBy.get(&key).cloned().unwrap().clone(), &mut kids);
            let mut i: i64 = 0;
            let n: i64 = kids.len() as i64;
            while i < n {
                self.markDirty(&kids[i as usize].clone(), &mut visited);
                i += 1;
            }
        }
    }
    /// Re-read the cell's formula from the sheet and register or clear it in the
    /// dependency graph. Used after undo/redo and paste, where the model changed
    /// underneath the engine.
    pub fn syncCell(&mut self, sheet_idx: i64, row: i64, col: i64) {
        let sheet: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(sheet_idx);
        let f: String = sheet.borrow().getFormula(row, col);
        if !f.is_empty() {
            self.registerFormula(sheet_idx, row, col, &f);
        } else {
            self.clearFormula(sheet_idx, row, col);
        }
        self.markEdited(sheet_idx, row, col);
    }
    pub fn onCellEdited(&mut self, sheet_idx: i64, row: i64, col: i64) {
        self.resetStats();
        self.markEdited(sheet_idx, row, col);
        self.recalcDirty();
    }
    /// How many formulas ended up in an error state — a cycle, or an expression
    /// that could not be evaluated. Zero is what a healthy sheet reports, and a
    /// test can say so without knowing which cells they were.
    pub fn errorCount(&self) -> i64 {
        let mut n: i64 = 0;
        let mut si: i64 = 0;
        while si < (self.book.as_ref().unwrap()).borrow().sheetCount() {
            let sheet: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(si);
            let keys: Vec<String> = sheet.borrow().collectFormulaKeys(si);
            let mut i: i64 = 0;
            while i < (keys.len() as i64) {
                let k: String = keys[i as usize].clone();
                if self.calcStatus.contains_key(&k) {
                    let st: String = self.calcStatus.get(&k).cloned().unwrap();
                    if st == "error" {
                        n += 1;
                    }
                }
                i += 1;
            }
            si += 1;
        }
        n
    }
    pub fn recalcAll(&mut self) {
        let mut keys: Vec<String> = Vec::new();
        // gather all formula keys from depsOf — iterate sheets
        let mut si: i64 = 0;
        let sn: i64 = (self.book.as_ref().unwrap()).borrow().sheetCount();
        while si < sn {
            let sheet: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(si);
            let sheet_keys: Vec<String> = sheet.borrow().collectFormulaKeys(si);
            for sk in sheet_keys.iter().cloned() {
                keys.push(sk.clone());
            }
            si += 1;
        }
        let mut i: i64 = 0;
        let n: i64 = keys.len() as i64;
        while i < n {
            self.calcStatus.insert(keys[i as usize].clone(), "dirty".to_string());
            i += 1;
        }
        self.recalcDirty();
    }
    /// Recalculate everything dirty, dependencies first.
    ///
    /// This used to be a sweep: walk every formula in the workbook, evaluate the
    /// ones whose dependencies were already clean, and repeat — up to SIXTY-FOUR
    /// times, after which whatever was left was declared a cycle. A sweep settles
    /// one link of a chain per pass whenever the chain runs against the order the
    /// sheet is walked in, so a column of four hundred subtotals — each reading
    /// the one below it, which is how a report is usually built — got sixty-four
    /// rows deep and then reported #CYCLE! for the other three hundred and
    /// thirty-six. From the outside that looks exactly like sums that "stop being
    /// delegated after a while".
    ///
    /// So it is a depth-first walk now: to evaluate a cell, evaluate what it
    /// depends on first. Depth costs nothing, the work is proportional to the
    /// dependency graph rather than to (formulas x passes), and a cycle is a cell
    /// that is reached again while it is still being computed — which is what a
    /// cycle actually is, rather than what is left over when patience runs out.
    ///
    /// The walk carries its own stack. Recursion would have been shorter and a
    /// four-hundred-deep sheet is a perfectly ordinary thing to open.
    pub fn recalcDirty(&mut self) {
        // Formulas > Calculation Options > Manual: edits wait for F9.
        if self.manualCalc {
            return;
        }
        let mut si: i64 = 0;
        let sn: i64 = (self.book.as_ref().unwrap()).borrow().sheetCount();
        while si < sn {
            let sheet: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(si);
            let keys: Vec<String> = sheet.borrow().collectFormulaKeys(si);
            let mut i: i64 = 0;
            let n: i64 = keys.len() as i64;
            while i < n {
                let key: String = keys[i as usize].clone();
                if self.needsWork(&key) {
                    self.resolveFrom(&key);
                }
                i += 1;
            }
            si += 1;
        }
    }
    pub fn needsWork(&self, key: &str) -> bool {
        if self.calcStatus.contains_key(key) {
            let st: String = self.calcStatus.get(key).cloned().unwrap();
            if st == "dirty" {
                return true;
            }
            if st == "pending" {
                return true;
            }
            return false;
        }
        // A formula nothing has said anything about yet still has to be run.
        self.depsOf.contains_key(key)
    }
    /// One dependency-first walk from `root`. Everything reachable from it that
    /// is dirty comes out either evaluated or, if it is genuinely part of a
    /// cycle, marked as one.
    pub fn resolveFrom(&mut self, root: &str) {
        let mut stack: Vec<String> = Vec::new();
        let mut phase: Vec<i64> = Vec::new();
        stack.push(root.to_string());
        phase.push(0);
        while !stack.is_empty() {
            let top: i64 = (stack.len() as i64) - 1;
            let key: String = stack[top as usize].clone();
            let ph: i64 = phase[top as usize];
            if ph == 0 {
                if !self.needsWork(&key) {
                    stack.pop();
                    phase.pop();
                    continue;
                }
                // "visiting" means: on the stack right now. Anything that reaches
                // a visiting cell has closed a loop.
                self.calcStatus.insert(key.clone(), "visiting".to_string());
                phase[top as usize] = 1;
                if self.depsOf.contains_key(&key) {
                    let mut deps: Vec<String> = Vec::new();
                    self.splitKeys(&self.depsOf.get(&key).cloned().unwrap().clone(), &mut deps);
                    let mut i: i64 = 0;
                    let n: i64 = deps.len() as i64;
                    while i < n {
                        let d: String = deps[i as usize].clone();
                        let mut dst: String = String::new();
                        if self.calcStatus.contains_key(&d) {
                            dst = self.calcStatus.get(&d).cloned().unwrap();
                        }
                        if dst == "visiting" {
                            // The loop is real: this cell cannot be computed.
                            self.markCycle(&key);
                        } else {
                            if self.needsWork(&d) {
                                stack.push(d.clone());
                                phase.push(0);
                            }
                        }
                        i += 1;
                    }
                }
            } else {
                stack.pop();
                phase.pop();
                let mut st: String = String::new();
                if self.calcStatus.contains_key(&key) {
                    st = self.calcStatus.get(&key).cloned().unwrap();
                }
                // Still visiting means the dependencies are done and nothing
                // declared it cyclic, so now it can be evaluated.
                if st == "visiting" {
                    self.evalOne(&key);
                }
            }
        }
    }
    /// A cell in a cycle: say so in the cell as well as in the status, because
    /// the value that is there now was computed from something that no longer
    /// holds.
    pub fn markCycle(&mut self, key: &str) {
        self.calcStatus.insert(key.to_string(), "error".to_string());
        self.calcValues.insert(key.to_string(), "#CYCLE!".to_string());
        self.applyResult(key, &FormulaValue::fromError("#CYCLE!".to_string()).clone());
    }
    pub fn depsReady(&self, key: &str) -> bool {
        if self.depsOf.contains_key(key) {
            let mut deps: Vec<String> = Vec::new();
            self.splitKeys(&self.depsOf.get(key).cloned().unwrap().clone(), &mut deps);
            let mut i: i64 = 0;
            let n: i64 = deps.len() as i64;
            while i < n {
                let d: String = deps[i as usize].clone();
                if self.calcStatus.contains_key(&d) {
                    let st: String = self.calcStatus.get(&d).cloned().unwrap();
                    if st == "dirty" || st == "pending" {
                        return false;
                    }
                }
                i += 1;
            }
        }
        true
    }
    pub fn evalOne(&mut self, key: &str) {
        let parts: Vec<String> = key.split("!").map(|s| s.to_string()).collect::<Vec<String>>();
        if (parts.len() as i64) < 2 {
            return;
        }
        let si: i64 = SpreadsheetModel::parseInt0(&parts[0].clone());
        let rc: Vec<String> = parts[1].clone()
            .split(",")
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        if (rc.len() as i64) < 2 {
            return;
        }
        let row: i64 = SpreadsheetModel::parseInt0(&rc[0].clone());
        let col: i64 = SpreadsheetModel::parseInt0(&rc[1].clone());
        let sheet: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(si);
        let expr: String = sheet.borrow().getFormula(row, col);
        if expr.is_empty() {
            self.calcStatus.insert(key.to_string(), "cached".to_string());
            return;
        }
        let ast: Rc<RefCell<FormulaNode>> = self.parser.borrow_mut().parse(&self.expandNames(&expr));
        if !self.parser.borrow().ok {
            // keep cached XLSX value
            self.calcStatus.insert(key.to_string(), "unsupported".to_string());
            return;
        }
        self.curRow = row;
        self.curCol = col;
        self.curSheet = si;
        let v: Rc<RefCell<FormulaValue>> = self.evalNode(&ast, si);
        self.recalculated += 1;
        let _e_v: bool = v.borrow().isError();
        if _e_v {
            self.calcStatus.insert(key.to_string(), "error".to_string());
            self.calcValues.insert(key.to_string(), v.borrow().err.clone());
            self.applyResult(key, &v);
            return;
        }
        self.calcStatus.insert(key.to_string(), "calculated".to_string());
        self.calcValues.insert(key.to_string(), v.borrow().asRaw());
        self.applyResult(key, &v);
    }
    pub fn applyResult(&self, key: &str, v: &Rc<RefCell<FormulaValue>>) {
        let parts: Vec<String> = key.split("!").map(|s| s.to_string()).collect::<Vec<String>>();
        let si: i64 = SpreadsheetModel::parseInt0(&parts[0].clone());
        let rc: Vec<String> = parts[1].clone()
            .split(",")
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        let row: i64 = SpreadsheetModel::parseInt0(&rc[0].clone());
        let col: i64 = SpreadsheetModel::parseInt0(&rc[1].clone());
        let sheet: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(si);
        // A formula that came out as a RECTANGLE spills: the top-left lands in
        // the cell that holds the formula and the rest fills the block below and
        // to the right of it, which is what a modern spreadsheet does with an
        // array result. Whatever the last spill covered is cleared first, so a
        // formula that shrinks does not leave a tail behind.
        sheet.borrow_mut().clearSpillFrom(row, col);
        if v.borrow().isArray() {
            if v.borrow().count() > 1 {
                self.spillArray(&sheet, row, col, v);
                return;
            }
            sheet.borrow_mut().setCellRaw(row, col, &v.borrow().single().borrow().asRaw());
            return;
        }
        // Store raw calculated value; XlsxNumberFormat formats on display.
        sheet.borrow_mut().setCellRaw(row, col, &v.borrow().asRaw());
    }
    /// Lay an array out from its anchor. Cells that already hold something of
    /// their own are not overwritten — the spill stops rather than eating a
    /// value someone typed, and the block that WAS written is remembered so the
    /// next recalculation can take it back.
    pub fn spillArray(&self, sheet: &Rc<RefCell<SpreadsheetModel>>, row: i64, col: i64, v: &Rc<RefCell<FormulaValue>>) {
        let mut blocked: bool = false;
        let mut r: i64 = 0;
        while r < v.borrow().rows {
            let mut c: i64 = 0;
            while c < v.borrow().cols {
                if r + c > 0 {
                    let tr: i64 = row + r;
                    let tc: i64 = col + c;
                    if !sheet.borrow().getFormula(tr, tc).is_empty() {
                        blocked = true;
                    } else {
                        if !sheet.borrow().getCell(tr, tc).is_empty() {
                            if !sheet.borrow().isSpilled(tr, tc) {
                                blocked = true;
                            }
                        }
                    }
                }
                c += 1;
            }
            r += 1;
        }
        if blocked {
            sheet.borrow_mut().setCellRaw(row, col, "#SPILL!");
            return;
        }
        sheet.borrow_mut().setSpill(row, col, v.borrow().rows.clone(), v.borrow().cols.clone());
        let mut rr: i64 = 0;
        while rr < v.borrow().rows {
            let mut cc: i64 = 0;
            while cc < v.borrow().cols {
                let item: Rc<RefCell<FormulaValue>> = v.borrow().at(rr, cc);
                sheet.borrow_mut().setCellRaw(row + rr, col + cc, &item.borrow().asRaw());
                cc += 1;
            }
            rr += 1;
        }
    }
    /// --- structured references --------------------------------------------------
    /// Where `Table1[[#This Row],[CategoryID]]` actually points, given the
    /// workbook and the cell doing the asking.
    ///
    /// The errors are Excel's, and each says something different:
    ///   #NAME?  there is no table by that name
    ///   #REF!   the table has no such column, or no header / totals row to ask
    ///           about
    ///   #VALUE! `[#This Row]` from a row the table does not cover — the
    ///           reference is well formed, it just has nothing to point at here
    pub fn resolveTableRef(&self, n: &Rc<RefCell<FormulaNode>>) -> Rc<RefCell<TableRange>> {
        let out: Rc<RefCell<TableRange>> = Rc::new(RefCell::new(TableRange::new()));
        // `[Q1]` with no table name, as Excel writes it inside a table (its
        // totals row, a calculated column), means the table the formula is in.
        let mut tname: String = n.borrow().tableName.clone();
        if tname.is_empty() {
            if self.curSheet >= 0 && self.curSheet < (self.book.as_ref().unwrap()).borrow().sheetCount() {
                let host: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(self.curSheet.clone());
                let ht: i64 = host.borrow().tableIndexAt(self.curRow.clone(), self.curCol.clone());
                if ht >= 0 {
                    let host_t: Rc<RefCell<SheetTable>> = host.borrow().tableAt(ht);
                    tname = host_t.borrow().name.clone();
                }
            }
        }
        let si: i64 = (self.book.as_ref().unwrap()).borrow().tableIndexByName(&tname);
        if si < 0 {
            out.borrow_mut().err = "#NAME?".to_string();
            return out;
        }
        let t: Rc<RefCell<SheetTable>> = (self.book.as_ref().unwrap()).borrow().tableByName(&tname);
        out.borrow_mut().sheetIdx = si;
        // Columns first: a named column narrows the rectangle sideways, and a
        // span names both ends of it.
        let mut c0: i64 = t.borrow().c0;
        let mut c1: i64 = t.borrow().c1;
        if !n.borrow().tableCol1.is_empty() {
            let i1: i64 = t.borrow().columnIndex(&n.borrow().tableCol1.clone());
            if i1 < 0 {
                out.borrow_mut().err = "#REF!".to_string();
                return out;
            }
            c0 = t.borrow().c0 + i1;
            c1 = c0;
            if !n.borrow().tableCol2.is_empty() {
                let i2: i64 = t.borrow().columnIndex(&n.borrow().tableCol2.clone());
                if i2 < 0 {
                    out.borrow_mut().err = "#REF!".to_string();
                    return out;
                }
                c1 = t.borrow().c0 + i2;
                if c1 < c0 {
                    let sw: i64 = c0;
                    c0 = c1;
                    c1 = sw;
                }
            }
        }
        // Then rows, from the item specifier. No specifier means the data.
        let mut r0: i64 = t.borrow().dataR0();
        let mut r1: i64 = t.borrow().dataR1();
        let item: String = n.borrow().tableItem.clone();
        if item == "#all" {
            r0 = t.borrow().r0;
            r1 = t.borrow().r1;
        }
        if item == "#headers" {
            if !t.borrow().hasHeader() {
                out.borrow_mut().err = "#REF!".to_string();
                return out;
            }
            r0 = t.borrow().r0;
            r1 = t.borrow().r0;
        }
        if item == "#totals" {
            if !t.borrow().hasTotals() {
                out.borrow_mut().err = "#REF!".to_string();
                return out;
            }
            r0 = t.borrow().r1;
            r1 = t.borrow().r1;
        }
        if item == "#this row" {
            if !t.borrow().containsRow(self.curRow.clone()) {
                out.borrow_mut().err = "#VALUE!".to_string();
                return out;
            }
            r0 = self.curRow;
            r1 = self.curRow;
        }
        if r1 < r0 {
            // A table with a header and no data rows: asking for the data is
            // asking for nothing, which is an empty reference rather than an
            // error, so it reads as blank.
            r1 = r0;
        }
        out.borrow_mut().r0 = r0;
        out.borrow_mut().c0 = c0;
        out.borrow_mut().r1 = r1;
        out.borrow_mut().c1 = c1;
        out
    }
    /// The resolved rectangle as an ordinary cell or range node, so everything
    /// downstream — reading, spilling, array arithmetic — treats a structured
    /// reference exactly as it treats `$A$2:$A$5`, which it is.
    pub fn tableRefAsNode(&self, tr: &Rc<RefCell<TableRange>>) -> Rc<RefCell<FormulaNode>> {
        let n: Rc<RefCell<FormulaNode>> = Rc::new(RefCell::new(FormulaNode::new()));
        n.borrow_mut().sheet = String::new();
        n.borrow_mut().row = tr.borrow().r0;
        n.borrow_mut().col = tr.borrow().c0;
        n.borrow_mut().row2 = tr.borrow().r1;
        n.borrow_mut().col2 = tr.borrow().c1;
        n.borrow_mut().kind = 4;
        if tr.borrow().r0 == tr.borrow().r1 {
            if tr.borrow().c0 == tr.borrow().c1 {
                n.borrow_mut().kind = 3;
            }
        }
        n
    }
    pub fn evalNode(&mut self, n: &Rc<RefCell<FormulaNode>>, sheet_idx: i64) -> Rc<RefCell<FormulaValue>> {
        if n.borrow().kind == 8 {
            return FormulaValue::fromError(n.borrow().text.clone());
        }
        if n.borrow().kind == 0 {
            return FormulaValue::fromNumber(n.borrow().num.clone());
        }
        if n.borrow().kind == 1 {
            return FormulaValue::fromString(n.borrow().text.clone());
        }
        if n.borrow().kind == 2 {
            return FormulaValue::fromBool(n.borrow().flag.clone());
        }
        if n.borrow().kind == 3 {
            let sheet: String = n.borrow().sheet.clone();
            return self.readCell(&sheet, sheet_idx, n.borrow().row.clone(), n.borrow().col.clone());
        }
        if n.borrow().kind == 4 {
            // A range IS a value now: a rectangle of them. What it comes to
            // depends on where it is used — a function that wants a table gets
            // the rectangle, arithmetic works on it element by element, and a
            // formula that just wants one number takes the top-left.
            return self.rangeArray(n, sheet_idx);
        }
        if n.borrow().kind == 5 {
            let a: Rc<RefCell<FormulaValue>> = self.evalNode(&n.borrow().kids[0].clone(),
                sheet_idx
            );
            let _e_a: bool = a.borrow().isError();
            if _e_a {
                return a;
            }
            if n.borrow().op == "-" {
                let ca: Rc<RefCell<FormulaValue>> = a.borrow().coerceNumber();
                let ce: bool = ca.borrow().isError();
                if ce {
                    return ca;
                }
                return FormulaValue::fromNumber(0.0 - ca.borrow().num);
            }
            return a;
        }
        if n.borrow().kind == 6 {
            return self.evalBinary(n, sheet_idx);
        }
        if n.borrow().kind == 7 {
            return self.evalCall(n, sheet_idx);
        }
        if n.borrow().kind == 9 {
            let tr: Rc<RefCell<TableRange>> = self.resolveTableRef(n);
            if !tr.borrow().err.is_empty() {
                return FormulaValue::fromError(tr.borrow().err.clone());
            }
            let rn: Rc<RefCell<FormulaNode>> = self.tableRefAsNode(&tr);
            if rn.borrow().kind == 3 {
                return self.readCell("", tr.borrow().sheetIdx, rn.borrow().row.clone(), rn.borrow().col.clone());
            }
            return self.rangeArray(&rn, tr.borrow().sheetIdx.clone());
        }
        FormulaValue::blank()
    }
    pub fn isSubtotalCell(&self, si: i64, row: i64, col: i64) -> bool {
        if si < 0 || si >= (self.book.as_ref().unwrap()).borrow().sheetCount() {
            return false;
        }
        let sheet: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(si);
        let f: String = FormulaFunctions::upper(&sheet.borrow().getFormula(row, col));
        DgNative::index_of(&f, "SUBTOTAL(") >= 0
    }
    pub fn readCell(&self, sheet_name: &str, sheet_idx: i64, row: i64, col: i64) -> Rc<RefCell<FormulaValue>> {
        let mut si: i64 = sheet_idx;
        if !sheet_name.is_empty() {
            si = self.sheetByName(sheet_name);
            if si < 0 {
                return FormulaValue::fromError("#REF!".to_string());
            }
        }
        if si < 0 || si >= (self.book.as_ref().unwrap()).borrow().sheetCount() {
            return FormulaValue::fromError("#REF!".to_string());
        }
        let sheet: Rc<RefCell<SpreadsheetModel>> = (self.book.as_ref().unwrap()).borrow().sheetAt(si);
        if ((row < 0 || col < 0) || row >= sheet.borrow().rowCount) || col >= sheet.borrow().colCount {
            return FormulaValue::fromError("#REF!".to_string());
        }
        let raw: String = sheet.borrow().getCell(row, col);
        if raw.is_empty() {
            return FormulaValue::blank();
        }
        if raw == "TRUE" {
            return FormulaValue::fromBool(true);
        }
        if raw == "FALSE" {
            return FormulaValue::fromBool(false);
        }
        if DgNative::char_at(&raw, 0) == 35 {
            return FormulaValue::fromError(raw.clone());
        }
        let p: Option<f64> = raw.parse::<f64>().ok();
        if p.is_some() {
            return FormulaValue::fromNumber(p.unwrap());
        }
        FormulaValue::fromString(raw.clone())
    }
    pub fn evalBinary(&mut self, n: &Rc<RefCell<FormulaNode>>, sheet_idx: i64) -> Rc<RefCell<FormulaValue>> {
        let L: Rc<RefCell<FormulaValue>> = self.evalNode(&n.borrow().kids[0].clone(),
            sheet_idx
        );
        let R: Rc<RefCell<FormulaValue>> = self.evalNode(&n.borrow().kids[1].clone(),
            sheet_idx
        );
        // An operand that is a rectangle makes the whole operation a rectangle:
        // A1:A3*B1:B3 is three products, not one. The result is an array, and
        // what happens to it then is the caller's business — a plain cell keeps
        // the top-left, a spilling one lays the lot out.
        if L.borrow().isArray() {
            return self.binaryOverArrays(n, &L, &R, sheet_idx);
        }
        if R.borrow().isArray() {
            return self.binaryOverArrays(n, &L, &R, sheet_idx);
        }
        let _e_l: bool = L.borrow().isError();
        if _e_l {
            return L;
        }
        let _e_r: bool = R.borrow().isError();
        if _e_r {
            return R;
        }
        if n.borrow().op == "&" {
            let _ls: String = L.borrow().toText();
            let _rs: String = R.borrow().toText();
            return FormulaValue::fromString(format!("{}{}", _ls, _rs));
        }
        if n.borrow().op == "=" {
            return FormulaValue::fromBool(FormulaValue::valuesEqual(&L, &R));
        }
        if n.borrow().op == "<>" {
            let eq: bool = FormulaValue::valuesEqual(&L, &R);
            if eq {
                return FormulaValue::fromBool(false);
            }
            return FormulaValue::fromBool(true);
        }
        // Arithmetic + ordered compare: shared coerceNumber semantics.
        let a_v: Rc<RefCell<FormulaValue>> = L.borrow().coerceNumber();
        let ae: bool = a_v.borrow().isError();
        if ae {
            return a_v;
        }
        let b_v: Rc<RefCell<FormulaValue>> = R.borrow().coerceNumber();
        let be: bool = b_v.borrow().isError();
        if be {
            return b_v;
        }
        let a: f64 = a_v.borrow().num;
        let b: f64 = b_v.borrow().num;
        if n.borrow().op == "+" {
            return FormulaValue::fromNumber(a + b);
        }
        if n.borrow().op == "-" {
            return FormulaValue::fromNumber(a - b);
        }
        if n.borrow().op == "*" {
            return FormulaValue::fromNumber(a * b);
        }
        if n.borrow().op == "/" {
            if b == 0.0 {
                return FormulaValue::fromError("#DIV/0!".to_string());
            }
            return FormulaValue::fromNumber(a / b);
        }
        if n.borrow().op == "^" {
            let p: i64 = b.floor() as i64;
            let mut acc: f64 = 1.0;
            let mut i: i64 = 0;
            if p < 0 {
                return FormulaValue::fromError("#VALUE!".to_string());
            }
            while i < p {
                acc *= a;
                i += 1;
            }
            return FormulaValue::fromNumber(acc);
        }
        if n.borrow().op == "<" {
            return FormulaValue::fromBool(a < b);
        }
        if n.borrow().op == ">" {
            return FormulaValue::fromBool(a > b);
        }
        if n.borrow().op == "<=" {
            return FormulaValue::fromBool(a <= b);
        }
        if n.borrow().op == ">=" {
            return FormulaValue::fromBool(a >= b);
        }
        FormulaValue::fromError("#VALUE!".to_string())
    }
    /// Element by element, broadcasting a single value against a rectangle the
    /// way every spreadsheet does: A1:A3*2 multiplies each of the three.
    pub fn binaryOverArrays(&self, n: &Rc<RefCell<FormulaNode>>, L: &Rc<RefCell<FormulaValue>>, R: &Rc<RefCell<FormulaValue>>, _sheet_idx: i64) -> Rc<RefCell<FormulaValue>> {
        let mut rows: i64 = 1;
        let mut cols: i64 = 1;
        if L.borrow().isArray() {
            rows = L.borrow().rows;
            cols = L.borrow().cols;
        }
        if R.borrow().isArray() {
            if R.borrow().rows > rows {
                rows = R.borrow().rows;
            }
            if R.borrow().cols > cols {
                cols = R.borrow().cols;
            }
        }
        let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut r: i64 = 0;
        while r < rows {
            let mut c: i64 = 0;
            while c < cols {
                let mut lv: Rc<RefCell<FormulaValue>> = L.clone();
                if L.borrow().isArray() {
                    lv = L.borrow().at(r, c);
                    if L.borrow().rows == 1 {
                        lv = L.borrow().at(0, c);
                    }
                    if L.borrow().cols == 1 {
                        lv = L.borrow().at(r, 0);
                    }
                }
                let mut rv: Rc<RefCell<FormulaValue>> = R.clone();
                if R.borrow().isArray() {
                    rv = R.borrow().at(r, c);
                    if R.borrow().rows == 1 {
                        rv = R.borrow().at(0, c);
                    }
                    if R.borrow().cols == 1 {
                        rv = R.borrow().at(r, 0);
                    }
                }
                items.push(self.binaryOfValues(&n.borrow().op.clone(), &lv, &rv));
                c += 1;
            }
            r += 1;
        }
        FormulaValue::fromArray(rows, cols, &items)
    }
    /// One operation on two scalars. Split out of evalBinary so the array walk
    /// can reuse exactly the semantics a plain formula gets.
    pub fn binaryOfValues(&self, op: &str, l: &Rc<RefCell<FormulaValue>>, r: &Rc<RefCell<FormulaValue>>) -> Rc<RefCell<FormulaValue>> {
        let _e_l: bool = l.borrow().isError();
        if _e_l {
            return l.clone();
        }
        let _e_r: bool = r.borrow().isError();
        if _e_r {
            return r.clone();
        }
        if op == "&" {
            return FormulaValue::fromString(format!("{}{}", l.borrow().toText(), r.borrow().toText()));
        }
        if op == "=" {
            return FormulaValue::fromBool(FormulaValue::valuesEqual(l, r));
        }
        if op == "<>" {
            let eq: bool = FormulaValue::valuesEqual(l, r);
            if eq {
                return FormulaValue::fromBool(false);
            }
            return FormulaValue::fromBool(true);
        }
        let a_v: Rc<RefCell<FormulaValue>> = l.borrow().coerceNumber();
        let ae: bool = a_v.borrow().isError();
        if ae {
            return a_v;
        }
        let b_v: Rc<RefCell<FormulaValue>> = r.borrow().coerceNumber();
        let be: bool = b_v.borrow().isError();
        if be {
            return b_v;
        }
        let a: f64 = a_v.borrow().num;
        let b: f64 = b_v.borrow().num;
        if op == "+" {
            return FormulaValue::fromNumber(a + b);
        }
        if op == "-" {
            return FormulaValue::fromNumber(a - b);
        }
        if op == "*" {
            return FormulaValue::fromNumber(a * b);
        }
        if op == "/" {
            if b == 0.0 {
                return FormulaValue::fromError("#DIV/0!".to_string());
            }
            return FormulaValue::fromNumber(a / b);
        }
        if op == "^" {
            let p: i64 = b.floor() as i64;
            if p < 0 {
                return FormulaValue::fromError("#VALUE!".to_string());
            }
            let mut acc: f64 = 1.0;
            let mut i: i64 = 0;
            while i < p {
                acc *= a;
                i += 1;
            }
            return FormulaValue::fromNumber(acc);
        }
        if op == "<" {
            return FormulaValue::fromBool(a < b);
        }
        if op == ">" {
            return FormulaValue::fromBool(a > b);
        }
        if op == "<=" {
            return FormulaValue::fromBool(a <= b);
        }
        if op == ">=" {
            return FormulaValue::fromBool(a >= b);
        }
        FormulaValue::fromError("#VALUE!".to_string())
    }
    pub fn evalCall(&mut self, n: &Rc<RefCell<FormulaNode>>, sheet_idx: i64) -> Rc<RefCell<FormulaValue>> {
        let name: String = n.borrow().fnName.clone();
        let mut args: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut i: i64 = 0;
        let kn: i64 = n.borrow().kids.len() as i64;
        // SUBTOTAL leaves out the SUBTOTALs inside its ranges, as Excel's
        // does, so a grand total over grouped subtotals counts each row once.
        let is_sub: bool = FormulaFunctions::upper(&name) == "SUBTOTAL";
        if is_sub {
            self.subtotalDepth += 1;
        }
        while i < kn {
            let arg: Rc<RefCell<FormulaNode>> = n.borrow().kids[i as usize].clone();
            args.push(self.evalNode(&arg, sheet_idx));
            i += 1;
        }
        if is_sub {
            self.subtotalDepth -= 1;
        }
        // TODAY and NOW are not pure functions: they read a clock, and this
        // layer has none. The HOST sets one — `setToday` — which also makes
        // them testable, since a test can say what today is instead of hoping
        // the suite does not run over midnight. Unset, they answer #N/A rather
        // than a made-up date.
        let upper: String = FormulaFunctions::upper(&name);
        if upper == "TODAY" {
            if self.todaySerial <= 0.0 {
                return FormulaValue::fromError("#N/A".to_string());
            }
            return FormulaValue::fromNumber((self.todaySerial.floor() as i64) as f64);
        }
        if upper == "NOW" {
            if self.todaySerial <= 0.0 {
                return FormulaValue::fromError("#N/A".to_string());
            }
            return FormulaValue::fromNumber(self.todaySerial.clone());
        }
        // Unknown names → #NAME? (caller may keep XLSX cached <v>)
        if !FormulaFunctions::isKnown(&name) {
            return FormulaValue::fromError("#NAME?".to_string());
        }
        FormulaFunctions::invoke(&name, &args)
    }
    /// A range, read into an array value that remembers its shape. VLOOKUP
    /// reads down a column and across a row; INDEX picks by row and column;
    /// TRANSPOSE turns one into the other. None of that is possible once the
    /// rectangle has been flattened into a list of arguments, which is all the
    /// engine could hand over before.
    pub fn rangeArray(&self, n: &Rc<RefCell<FormulaNode>>, sheet_idx: i64) -> Rc<RefCell<FormulaValue>> {
        let mut si: i64 = sheet_idx;
        if !n.borrow().sheet.is_empty() {
            si = self.sheetByName(&n.borrow().sheet);
            if si < 0 {
                return FormulaValue::fromError("#REF!".to_string());
            }
        }
        let mut r0: i64 = n.borrow().row;
        let mut r1: i64 = n.borrow().row2;
        let mut c0: i64 = n.borrow().col;
        let mut c1: i64 = n.borrow().col2;
        if r0 > r1 {
            let t: i64 = r0;
            r0 = r1;
            r1 = t;
        }
        if c0 > c1 {
            let t2: i64 = c0;
            c0 = c1;
            c1 = t2;
        }
        let mut items: Vec<Rc<RefCell<FormulaValue>>> = Vec::new();
        let mut r: i64 = r0;
        while r <= r1 {
            let mut c: i64 = c0;
            while c <= c1 {
                if self.subtotalDepth > 0 && self.isSubtotalCell(si, r, c) {
                    items.push(FormulaValue::blank());
                } else {
                    items.push(self.readCell("", si, r, c));
                }
                c += 1;
            }
            r += 1;
        }
        FormulaValue::fromArray((r1 - r0) + 1, (c1 - c0) + 1, &items)
    }
    /// Expand A1:B2 into individual FormulaValue args for the function library.
    pub fn expandRangeArgs(&self, n: &Rc<RefCell<FormulaNode>>, sheet_idx: i64, args: &mut Vec<Rc<RefCell<FormulaValue>>>) {
        let mut si: i64 = sheet_idx;
        if !n.borrow().sheet.is_empty() {
            si = self.sheetByName(&n.borrow().sheet);
            if si < 0 {
                args.push(FormulaValue::fromError("#REF!".to_string()));
                return;
            }
        }
        let mut r0: i64 = n.borrow().row;
        let mut r1: i64 = n.borrow().row2;
        let mut c0: i64 = n.borrow().col;
        let mut c1: i64 = n.borrow().col2;
        if r0 > r1 {
            let t: i64 = r0;
            r0 = r1;
            r1 = t;
        }
        if c0 > c1 {
            let t2: i64 = c0;
            c0 = c1;
            c1 = t2;
        }
        let mut r: i64 = r0;
        while r <= r1 {
            let mut c: i64 = c0;
            while c <= c1 {
                args.push(self.readCell("", si, r, c));
                c += 1;
            }
            r += 1;
        }
    }
}
