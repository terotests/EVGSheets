// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::{DgNative, OpcPackage, OpcRelationship};
use crate::sheet_table::SheetTable;
use crate::spreadsheet_model::SpreadsheetModel;
use crate::xlsx::xml_lite::{XmlLite, XmlTag};
/// XlsxTable.rgr — `xl/tables/tableN.xml` becomes a SheetTable.
///
/// A table part is small and says exactly three things worth keeping:
///
///   <table id="1" name="Table1" displayName="Table1" ref="A1:C5"
///          headerRowCount="1" totalsRowCount="0">
///     <tableColumns count="3">
///       <tableColumn id="1" name="ProductID"/>
///       <tableColumn id="2" name="CategoryID"/>
///       <tableColumn id="3" name="Amount"/>
///     </tableColumns>
///   </table>
///
/// the NAME a formula writes, the RECTANGLE it covers, and what each COLUMN is
/// called. The autoFilter and tableStyleInfo beside them are presentation and
/// are not read; the filter state the grid has of its own is a different thing
/// from the one a table remembers.
///
/// TWO ATTRIBUTES FOR THE NAME. `displayName` is what formulas use and `name` is
/// an internal identifier; they are usually the same string and Excel keeps them
/// in step, but where a file has only one of them, or they differ, the formula
/// side is the one that matters — so displayName wins and name is the fallback.
///
/// `headerRowCount` DEFAULTS TO ONE, which is the opposite of how an absent
/// attribute usually reads. A table written without it has a header row; a table
/// genuinely without one says `headerRowCount="0"` explicitly. Getting that
/// backwards moves every data row up by one and quietly turns the header into
/// data.
#[derive(Clone)]
pub struct XlsxTable {
}

impl XlsxTable {
    pub fn new() -> Self {
        Self {
        }
    }
    /// Parse one table part. Returns a table whose name is empty when the part
    /// is not one — a caller can test that rather than a separate flag.
    pub fn parse(xml: &str) -> Rc<RefCell<SheetTable>> {
        let t: Rc<RefCell<SheetTable>> = Rc::new(RefCell::new(SheetTable::new()));
        if xml.is_empty() {
            return t;
        }
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(xml.to_string());
        let mut in_columns: bool = false;
        // A table part with no autoFilter element has no filter buttons, and one
        // with no tableStyleInfo is drawn plain.
        t.borrow_mut().showFilter = false;
        t.borrow_mut().styleName = String::new();
        t.borrow_mut().showRowStripes = false;
        while p.borrow().pos < p.borrow().srcLen {
            let tag: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !tag.borrow().valid {
                break;
            }
            if tag.borrow().name == "table" && !tag.borrow().isEnd {
                let mut disp: String = tag.borrow().attr("displayName");
                if disp.is_empty() {
                    disp = tag.borrow().attr("name");
                }
                t.borrow_mut().name = disp.clone();
                XlsxTable::applyRef(&t, &tag.borrow().attr("ref"));
                // Absent means one, not none.
                let hdr: String = tag.borrow().attr("headerRowCount");
                if !hdr.is_empty() {
                    t.borrow_mut().headerRowCount = SpreadsheetModel::parseInt0(&hdr);
                } else {
                    t.borrow_mut().headerRowCount = 1;
                }
                let tot: String = tag.borrow().attr("totalsRowCount");
                if !tot.is_empty() {
                    t.borrow_mut().totalsRowCount = SpreadsheetModel::parseInt0(&tot);
                } else {
                    t.borrow_mut().totalsRowCount = 0;
                }
            }
            if tag.borrow().name == "autoFilter" && !tag.borrow().isEnd {
                t.borrow_mut().showFilter = true;
            }
            if tag.borrow().name == "tableStyleInfo" && !tag.borrow().isEnd {
                t.borrow_mut().styleName = tag.borrow().attr("name");
                t.borrow_mut().showRowStripes = tag.borrow().attr("showRowStripes") == "1";
                t.borrow_mut().showColumnStripes = tag.borrow().attr("showColumnStripes") == "1";
                t.borrow_mut().showFirstColumn = tag.borrow().attr("showFirstColumn") == "1";
                t.borrow_mut().showLastColumn = tag.borrow().attr("showLastColumn") == "1";
            }
            if tag.borrow().name == "tableColumns" {
                if tag.borrow().isEnd {
                    in_columns = false;
                } else {
                    in_columns = true;
                    if tag.borrow().selfClose {
                        in_columns = false;
                    }
                }
            }
            if tag.borrow().name == "tableColumn" && !tag.borrow().isEnd {
                if in_columns {
                    t.borrow_mut().columns.push(tag.borrow().attr("name"));
                }
            }
        }
        // A table whose part named no columns still has them — they are the
        // cells of its header row. Reading the sheet is not this function's
        // job, so the caller is left to fill them; what must not happen is a
        // column list that is shorter than the rectangle, because a column
        // index is looked up in it.
        t
    }
    /// "A1:C5" — or "A1" for a one-cell table, which is legal and rare.
    pub fn applyRef(t: &Rc<RefCell<SheetTable>>, r#ref: &str) {
        if r#ref.is_empty() {
            return;
        }
        let cut: i64 = DgNative::index_of(&r#ref, ":");
        let mut a: String = r#ref.to_string();
        let mut b: String = r#ref.to_string();
        if cut >= 0 {
            a = DgNative::substr(&r#ref, 0, cut);
            b = DgNative::substr(&r#ref, cut + 1, DgNative::str_len(&r#ref));
        }
        t.borrow_mut().r0 = XlsxTable::refRow(&a);
        t.borrow_mut().c0 = XlsxTable::refCol(&a);
        t.borrow_mut().r1 = XlsxTable::refRow(&b);
        t.borrow_mut().c1 = XlsxTable::refCol(&b);
        if t.borrow().r1 < t.borrow().r0 {
            let rt: i64 = t.borrow().r0;
            t.borrow_mut().r0 = t.borrow().r1;
            t.borrow_mut().r1 = rt;
        }
        if t.borrow().c1 < t.borrow().c0 {
            let ct: i64 = t.borrow().c0;
            t.borrow_mut().c0 = t.borrow().c1;
            t.borrow_mut().c1 = ct;
        }
    }
    /// A1 → 0-based row and column. Written here rather than borrowed from the
    /// formula parser: a table's `ref` never carries a `$` or a sheet name, and
    /// the loader importing the engine to read four numbers would be a cycle
    /// drawn for no reason.
    pub fn refCol(r#ref: &str) -> i64 {
        let mut col: i64 = 0;
        let mut saw: bool = false;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&r#ref);
        while i < n {
            let ch: i64 = DgNative::char_at(&r#ref, i);
            if ch >= 65 && ch <= 90 {
                col = col * 26 + ((ch - 65) + 1);
                saw = true;
            } else {
                if ch >= 97 && ch <= 122 {
                    col = col * 26 + ((ch - 97) + 1);
                    saw = true;
                } else {
                    if ch != 36 {
                        i = n;
                        continue;
                    }
                }
            }
            i += 1;
        }
        if !saw {
            return 0;
        }
        col - 1
    }
    pub fn refRow(r#ref: &str) -> i64 {
        let mut row: i64 = 0;
        let mut saw: bool = false;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&r#ref);
        while i < n {
            let ch: i64 = DgNative::char_at(&r#ref, i);
            if ch >= 48 && ch <= 57 {
                row = row * 10 + (ch - 48);
                saw = true;
            }
            i += 1;
        }
        if !saw {
            return 0;
        }
        row - 1
    }
    /// Every table relationship on a sheet, in the order the rels part lists
    /// them. A sheet may carry several tables, so `relTargetOfType` — which
    /// answers with the first — is not enough here. The scan is `OpcPackage`'s,
    /// the same one the .docx and .pptx readers use.
    pub fn tableTargets(rels: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if rels.is_empty() {
            return out;
        }
        let list: Vec<Rc<RefCell<OpcRelationship>>> = OpcPackage::parse_relationships(rels);
        let mut i: i64 = 0;
        while i < (list.len() as i64) {
            let rel: Rc<RefCell<OpcRelationship>> = list[i as usize].clone();
            if OpcPackage::type_ends_with(&rel.borrow().r#type.clone(), "/table") {
                if !rel.borrow().target.is_empty() {
                    out.push(rel.borrow().target.clone());
                }
            }
            i += 1;
        }
        out
    }
    /// Column names the part did not carry, taken from the header row the table
    /// says it has. A table with no header row has no names to take, and its
    /// columns can then only be reached by position — which is what Excel does
    /// too.
    pub fn fillColumnsFromSheet(t: &Rc<RefCell<SheetTable>>, model: &Rc<RefCell<SpreadsheetModel>>) {
        let want: i64 = (t.borrow().c1 - t.borrow().c0) + 1;
        if want < 1 {
            return;
        }
        if (t.borrow().columns.len() as i64) >= want {
            return;
        }
        let mut i: i64 = t.borrow().columns.len() as i64;
        while i < want {
            let mut name: String = String::new();
            if t.borrow().headerRowCount > 0 {
                name = model.borrow().getCell(t.borrow().r0, t.borrow().c0 + i);
            }
            t.borrow_mut().columns.push(name.clone());
            i += 1;
        }
    }
}
