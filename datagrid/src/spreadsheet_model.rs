// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rgr::DgNative;
use crate::cell_style::CellStyle;
use crate::data_validation::DataValidation;
use crate::rich_text::RichText;
use crate::sheet_history::SheetHistory;
use crate::sheet_image::SheetImage;
use crate::sheet_shape::SheetShape;
use crate::sheet_table::SheetTable;
use crate::xlsx_number_format::XlsxNumberFormat;
/// SpreadsheetModel - sparse cell store + row/column geometry for DataGrid.
///
/// EVG paints cells; EVG is NOT the spreadsheet model. Geometry (row heights,
/// column widths, prefix offsets) lives here so DataGrid can virtualize without
/// CSS grid / one View per cell.
///
/// Cell values are sparse by row, then by int column. Empty cells are absent —
/// a 100k×26 sheet costs almost nothing until cells are written.
/// Conditional formatting rule (parsed from xlsx cfRule).
/// One sparkline from the worksheet's x14 extension: a tiny chart of `src`
/// (a range on this sheet) drawn inside cell (row, col).
#[derive(Clone)]
pub struct Sparkline {
    pub row: i64,
    pub col: i64,
    pub r1: i64,
    pub c1: i64,
    pub r2: i64,
    pub c2: i64,
    /// "line" (default), "column" or "stacked"
    pub kind: String,
    pub color: String,
}

impl Sparkline {
    pub fn new() -> Self {
        Self {
            row: 0,
            col: 0,
            r1: 0,
            c1: 0,
            r2: 0,
            c2: 0,
            kind: "line".to_string(),
            color: "#1F4E79".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct CfRule {
    /// 0 colorScale | 1 cellIs | 2 dataBar | 3 iconSet
    pub kind: i64,
    /// Icon sets: the set's name ("3Arrows", "3TrafficLights1", "5Rating"…)
    /// and the lower bound of each level above the first, in percent of the
    /// range (Excel's default cfvo type).
    pub iconSet: String,
    pub thresholds: Vec<f64>,
    pub r0: i64,
    pub c0: i64,
    pub r1: i64,
    pub c1: i64,
    pub op: String,
    pub v1: f64,
    pub v2: f64,
    pub fillRgb: String,
    pub color0: String,
    pub color1: String,
    pub color2: String,
}

impl CfRule {
    pub fn new() -> Self {
        Self {
            kind: 0,
            iconSet: String::new(),
            thresholds: Vec::new(),
            r0: 0,
            c0: 0,
            r1: 0,
            c1: 0,
            op: String::new(),
            v1: 0.0,
            v2: 0.0,
            fillRgb: String::new(),
            color0: "F8696B".to_string(),
            color1: "FFEB84".to_string(),
            color2: "63BE7B".to_string(),
        }
    }
    pub fn copy(&self) -> Rc<RefCell<CfRule>> {
        let r: Rc<RefCell<CfRule>> = Rc::new(RefCell::new(CfRule::new()));
        r.borrow_mut().kind = self.kind;
        r.borrow_mut().r0 = self.r0;
        r.borrow_mut().c0 = self.c0;
        r.borrow_mut().r1 = self.r1;
        r.borrow_mut().c1 = self.c1;
        r.borrow_mut().op = self.op.clone();
        r.borrow_mut().v1 = self.v1;
        r.borrow_mut().v2 = self.v2;
        r.borrow_mut().fillRgb = self.fillRgb.clone();
        r.borrow_mut().color0 = self.color0.clone();
        r.borrow_mut().color1 = self.color1.clone();
        r.borrow_mut().color2 = self.color2.clone();
        r.borrow_mut().iconSet = self.iconSet.clone();
        let mut i: i64 = 0;
        while i < (self.thresholds.len() as i64) {
            r.borrow_mut().thresholds.push(self.thresholds[i as usize]);
            i += 1;
        }
        r
    }
}

/// ------------------------------------------------------------------------------
/// A worksheet child the writer does not generate, kept verbatim.
///
/// This is the spreadsheet's half of the idea `PptxOpaque` carries for slides:
/// Ranger does not have to understand something in order to preserve it. What is
/// in here is the source text of one element and the name that says where in the
/// worksheet's element order it goes back.
///
/// What ends up here is exactly what `XlsxWriter.sheetXml` does NOT emit —
/// `<drawing>` and `<legacyDrawing>` (the references to the pictures and the
/// comments), `<pageSetup>`, `<headerFooter>`, `<conditionalFormatting>`,
/// `<sheetProtection>`, `<oleObjects>` and the rest. Without them a saved
/// workbook keeps the media parts and points at none of them, which is the same
/// as losing them.
/// ------------------------------------------------------------------------------
#[derive(Clone)]
pub struct SheetOpaque {
    /// The element name, which decides where it goes back. SpreadsheetML fixes
    /// the order of a worksheet's children and Excel refuses a file that gets it
    /// wrong, so this is not decoration.
    pub name: String,
    /// The element and everything inside it, exactly as it was read.
    pub source: String,
}

impl SheetOpaque {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            source: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct CellPos {
    pub row: i64,
    pub col: i64,
}

impl CellPos {
    pub fn new() -> Self {
        Self {
            row: 0,
            col: 0,
        }
    }
    pub fn create(row: i64, col: i64) -> Rc<RefCell<CellPos>> {
        let p: Rc<RefCell<CellPos>> = Rc::new(RefCell::new(CellPos::new()));
        p.borrow_mut().row = row;
        p.borrow_mut().col = col;
        p
    }
    pub fn copyFrom(&mut self, other: &Rc<RefCell<CellPos>>) {
        self.row = other.borrow().row;
        self.col = other.borrow().col;
    }
    pub fn equals(&self, other: &Rc<RefCell<CellPos>>) -> bool {
        if self.row != other.borrow().row {
            return false;
        }
        self.col == other.borrow().col
    }
}

/// One tracked cell edit. Both the display value and the formula text are
/// captured so undo of "=A1+B1" restores the formula, not just the number.
/// Ops recorded inside one begin/end undo and redo together (paste, fill,
/// delete range); the transaction id lives in the shared history rather than
/// on the op.
/// ------------------------------------------------------------------------------
/// Which structural change an undo op is.
///
/// Written as bare numbers at both ends, and the two ends already disagree on
/// how many there are: the model handles seven, and `reapplyStructFormulaAdjust`
/// in the host handles six. That happens to be right — a sort permutes rows
/// without shifting any reference, so there is nothing for it to adjust — but
/// nothing in the code says so, and the next kind added will land in the same
/// gap silently. This is the shape that produced four separate bugs in the
/// document editor's op kinds.
/// ------------------------------------------------------------------------------
#[derive(Clone)]
pub struct SheetStructKind {
}

impl SheetStructKind {
    pub fn new() -> Self {
        Self {
        }
    }
    /// Not a structural op at all: an ordinary cell edit.
    pub fn none() -> i64 {
        0
    }
    pub fn insertColumn() -> i64 {
        1
    }
    pub fn deleteColumn() -> i64 {
        2
    }
    pub fn moveColumn() -> i64 {
        3
    }
    pub fn insertRow() -> i64 {
        4
    }
    pub fn deleteRow() -> i64 {
        5
    }
    pub fn moveRow() -> i64 {
        6
    }
    /// A sort: every row moved at once, as ONE undoable action. The permutation
    /// is in `structOrder` rather than a sequence of moves, because a sort is
    /// one action to the person who asked for it.
    pub fn reorderRows() -> i64 {
        7
    }
    /// Do rows or columns move? The snapshot fields are shared between the two,
    /// and this is the question the code kept asking as `kind < 4`.
    pub fn isColumnOp(kind: i64) -> bool {
        if kind < 1 {
            return false;
        }
        kind < 4
    }
}

#[derive(Clone)]
pub struct SpreadsheetUndoOp {
    pub row: i64,
    pub col: i64,
    pub before: String,
    pub after: String,
    pub beforeF: String,
    pub afterF: String,
    pub beforeStyle: i64,
    pub afterStyle: i64,
    pub touchStyle: bool,
    /// Which structural change this op is, if any — see `SheetStructKind`. Rows
    /// and columns share the snapshot fields below: what they hold is "the line
    /// that was lifted out", and which direction it ran is what `structKind`
    /// says.
    pub structKind: i64,
    pub structAt: i64,
    pub structTo: i64,
    pub colValues: Vec<String>,
    pub colFormulas: Vec<String>,
    pub colStyles: Vec<i64>,
    pub colWidthSaved: i64,
    /// The permutation a sort applied: `structOrder[i]` is the row that ended
    /// up at i. Undo runs its inverse.
    pub structOrder: Vec<i64>,
    /// Every formula in the sheet before the op. A reference into a deleted
    /// column becomes #REF!, which no amount of re-shifting can undo, so the
    /// original text is the only way back.
    pub savedKeys: Vec<String>,
    pub savedTexts: Vec<String>,
    /// Formulas on OTHER workbook sheets that named this sheet (qualified
    /// refs). GridApp fills these before adjustFormulasFor; undo restores them
    /// because SpreadsheetModel only owns the active sheet's snapshot above.
    pub otherSheetIdx: Vec<i64>,
    pub otherKeys: Vec<String>,
    pub otherTexts: Vec<String>,
    /// A change to the sheet's conditional formatting rules: the whole list
    /// before and after. Rules are few, and a whole list is the only shape that
    /// covers add, edit, delete and clear alike.
    pub cfSwap: bool,
    pub cfBefore: Vec<Rc<RefCell<CfRule>>>,
    pub cfAfter: Vec<Rc<RefCell<CfRule>>>,
}

impl SpreadsheetUndoOp {
    pub fn new() -> Self {
        Self {
            row: 0,
            col: 0,
            before: String::new(),
            after: String::new(),
            beforeF: String::new(),
            afterF: String::new(),
            beforeStyle: -1,
            afterStyle: -1,
            touchStyle: false,
            structKind: SheetStructKind::none(),
            structAt: 0,
            structTo: 0,
            colValues: Vec::new(),
            colFormulas: Vec::new(),
            colStyles: Vec::new(),
            colWidthSaved: 0,
            structOrder: Vec::new(),
            savedKeys: Vec::new(),
            savedTexts: Vec::new(),
            otherSheetIdx: Vec::new(),
            otherKeys: Vec::new(),
            otherTexts: Vec::new(),
            cfSwap: false,
            cfBefore: Vec::new(),
            cfAfter: Vec::new(),
        }
    }
    pub fn copyFrom(&mut self, o: &Rc<RefCell<SpreadsheetUndoOp>>) {
        self.row = o.borrow().row;
        self.col = o.borrow().col;
        self.before = o.borrow().before.clone();
        self.after = o.borrow().after.clone();
        self.beforeF = o.borrow().beforeF.clone();
        self.afterF = o.borrow().afterF.clone();
        self.beforeStyle = o.borrow().beforeStyle;
        self.afterStyle = o.borrow().afterStyle;
        self.touchStyle = o.borrow().touchStyle;
        // The structural payload has to come too — an op that loses structKind
        // on its way to the redo stack is replayed as a plain cell edit.
        self.structKind = o.borrow().structKind;
        self.structAt = o.borrow().structAt;
        self.structTo = o.borrow().structTo;
        self.colWidthSaved = o.borrow().colWidthSaved;
        self.structOrder.clear();
        let mut so: i64 = 0;
        while so < (o.borrow().structOrder.len() as i64) {
            self.structOrder.push(o.borrow().structOrder[so as usize]);
            so += 1;
        }
        self.colValues.clear();
        self.colFormulas.clear();
        self.colStyles.clear();
        self.savedKeys.clear();
        self.savedTexts.clear();
        self.otherSheetIdx.clear();
        self.otherKeys.clear();
        self.otherTexts.clear();
        let mut i: i64 = 0;
        while i < (o.borrow().colValues.len() as i64) {
            self.colValues.push(o.borrow().colValues[i as usize].clone());
            i += 1;
        }
        let mut j: i64 = 0;
        while j < (o.borrow().colFormulas.len() as i64) {
            self.colFormulas.push(o.borrow().colFormulas[j as usize].clone());
            j += 1;
        }
        let mut k: i64 = 0;
        while k < (o.borrow().colStyles.len() as i64) {
            self.colStyles.push(o.borrow().colStyles[k as usize]);
            k += 1;
        }
        let mut m: i64 = 0;
        while m < (o.borrow().savedKeys.len() as i64) {
            self.savedKeys.push(o.borrow().savedKeys[m as usize].clone());
            m += 1;
        }
        let mut n: i64 = 0;
        while n < (o.borrow().savedTexts.len() as i64) {
            self.savedTexts.push(o.borrow().savedTexts[n as usize].clone());
            n += 1;
        }
        let mut oi: i64 = 0;
        while oi < (o.borrow().otherSheetIdx.len() as i64) {
            self.otherSheetIdx.push(o.borrow().otherSheetIdx[oi as usize]);
            oi += 1;
        }
        let mut ok: i64 = 0;
        while ok < (o.borrow().otherKeys.len() as i64) {
            self.otherKeys.push(o.borrow().otherKeys[ok as usize].clone());
            ok += 1;
        }
        let mut ot: i64 = 0;
        while ot < (o.borrow().otherTexts.len() as i64) {
            self.otherTexts.push(o.borrow().otherTexts[ot as usize].clone());
            ot += 1;
        }
        self.cfSwap = o.borrow().cfSwap;
        self.cfBefore = SpreadsheetModel::copyCfRules(&o.borrow().cfBefore.clone());
        self.cfAfter = SpreadsheetModel::copyCfRules(&o.borrow().cfAfter.clone());
    }
}

/// One row or column of a sheet, as `snapshotRow` / `snapshotColumn` read it.
pub struct LineSnapshot {
    pub values: Vec<String>,
    pub formulas: Vec<String>,
    pub styles: Vec<i64>,
}

impl LineSnapshot {
    pub fn new() -> Self {
        Self { values: Vec::new(), formulas: Vec::new(), styles: Vec::new() }
    }
}

#[derive(Clone)]
pub struct SpreadsheetModel {
    pub name: String,
    /// The query this sheet was built from, if it was built from one. A sheet
    /// remembers its own SQL because the box that runs queries is one box and
    /// the workbook holds many sheets: without this, coming back to a sheet
    /// and asking "what is this one?" has no answer, and every re-run had to
    /// make yet another sheet rather than refresh the one being looked at.
    pub dbSql: String,
    pub rowCount: i64,
    pub colCount: i64,
    /// Excel's own defaults, in CSS pixels: a row is 15 points and a column is
    /// 8.43 characters of the default font. This used to be 22 and 100 — a grid
    /// half again as wide per column as the one the same file opens as in Excel
    /// or Sheets, which reads as everything being zoomed in, and text that is
    /// the right size looking too big for the sheet around it.
    pub defaultRowH: i64,
    pub defaultColW: i64,
    /// What a cell with no style of its own is set in. A workbook states this
    /// once and every unstyled cell wears it: Arial 10 in a sheet written by
    /// Google, Calibri 11 in one written by Excel. Assuming 11 drew a 10pt
    /// sheet a tenth too large, and — since a row is as tall as the type in it
    /// — a tenth too tall as well.
    pub defaultFontPt: f64,
    /// The theme's body font (Page Layout › Fonts), for cells with no font of
    /// their own; "" keeps the view's.
    pub defaultFontName: String,
    pub freezeRows: i64,
    pub freezeCols: i64,
    /// Cell values, sparse by row then column. Was a flat `"row,col"` string map:
    /// that is fine in JS (shared string keys) and catastrophic in C++ (a new
    /// std::string key per cell — minutes to insert 4M entries). Row maps keyed
    /// by int column avoid the string churn; `set_at` writes the row map back
    /// because C++ `itemAt` returns a copy.
    pub cellRows: Vec<HashMap<i64,String>>,
    pub formulas: HashMap<String,String>,
    pub formulaKeys: Vec<String>,
    pub cellStyleIds: HashMap<String,i64>,
    pub styles: Vec<Rc<RefCell<CellStyle>>>,
    pub merges: Vec<String>,
    pub rowHeights: Vec<i64>,
    pub colWidths: Vec<i64>,
    pub rowOffsets: Vec<i64>,
    pub colOffsets: Vec<i64>,
    pub rowHidden: Vec<bool>,
    pub colHidden: Vec<bool>,
    pub rowSuppressed: Vec<bool>,
    pub filterCol: i64,
    pub filterValues: Vec<String>,
    pub filterEnabled: bool,
    pub sortCol: i64,
    pub sortAsc: bool,
    pub cfRules: Vec<Rc<RefCell<CfRule>>>,
    pub sparklines: Vec<Rc<RefCell<Sparkline>>>,
    /// Cell annotations that are not the cell's value: a link to somewhere, a
    /// note about it, and a rule about what may be typed into it. All three are
    /// keyed the same way as a cell, and all three are METADATA — they survive
    /// an edit of the value and are not on the undo stack, which is also why
    /// they are not in `cells`.
    pub validations: Vec<Rc<RefCell<DataValidation>>>,
    /// Where an array formula's result was laid out. The anchor's key maps to
    /// "rows,cols"; every cell it covers maps back to the anchor, so a paint or
    /// an edit can ask "is this mine?" without searching.
    pub spillSize: HashMap<String,String>,
    pub spillOwner: HashMap<String,String>,
    pub spillKeys: Vec<String>,
    /// Cells whose text is drawn in pieces. Keyed like everything else, and kept
    /// beside the value rather than inside it — see RichText.
    /// Pictures floating over this sheet, in the order the drawing listed them.
    pub images: Vec<Rc<RefCell<SheetImage>>>,
    /// Charts the .xlsx carried (anchor + series refs); GridApp opens them.
    pub fileCharts: Vec<Rc<RefCell<SheetImage>>>,
    /// Tables (ListObjects) on this sheet. A table's NAME is workbook-wide, so
    /// the lookup that formulas use lives on WorkbookModel; this is only where
    /// the ones belonging to this sheet are kept.
    pub tables: Vec<Rc<RefCell<SheetTable>>>,
    pub richCells: HashMap<String,Rc<RefCell<RichText>>>,
    pub richKeys: Vec<String>,
    pub links: HashMap<String,String>,
    pub linkKeys: Vec<String>,
    pub notes: HashMap<String,String>,
    pub noteKeys: Vec<String>,
    /// Threaded comments (Review › New Comment): a cell's thread is one
    /// "Author: text" line per reply. Notes are the older single-text kind.
    pub threads: HashMap<String,String>,
    pub threadKeys: Vec<String>,
    /// Notes kept open (Show/Hide Note, Show All Notes) instead of only on the
    /// selected cell.
    pub shownNotes: HashMap<String,bool>,
    pub showAllNotes: bool,
    /// Allow Edit Ranges: "A1:B4" ranges a protected sheet still lets you type in.
    pub editRanges: Vec<String>,
    /// Circle Invalid Data: row, col pairs drawn with a red ring.
    pub invalidCircles: Vec<i64>,
    /// Data › Group: each row's and column's outline level (0 = none), and
    /// whether a group's detail is hidden.
    pub rowLevels: HashMap<i64,i64>,
    pub colLevels: HashMap<i64,i64>,
    /// Drawing objects over the sheet (shapes, text boxes, ink, slicers),
    /// bottom of the z-order first.
    pub shapes: Vec<Rc<RefCell<SheetShape>>>,
    /// In-cell checkboxes, by cell key.
    pub checkboxes: HashMap<String,bool>,
    /// Page Layout › Background: a picture tiled under the cells.
    pub backgroundSrc: String,
    pub backgroundBytes: Vec<u8>,
    pub bgWidth: i64,
    pub bgHeight: i64,
    /// View › Ruler, shown in Page Layout view.
    pub showRuler: bool,
    pub sheetHidden: bool,
    /// The sheet view's switches (View tab) and the tab's colour. Written to the
    /// file as `sheetView` attributes and `sheetPr/tabColor`.
    pub showGridlines: bool,
    /// View > Zoom (sheetView zoomScale / 100). Sizes are stored at 100 %;
    /// colWidth / rowHeight and the geometry hand out zoomed pixels.
    pub zoom: f64,
    /// "normal" | "pageLayout" | "pageBreak" — the status bar's view buttons.
    pub viewMode: String,
    pub showHeadings: bool,
    pub showFormulas: bool,
    pub tabColor: String,
    /// Review > Protect Sheet: a locked cell (every cell, until Format Cells
    /// unlocks it) refuses edits while this is on.
    pub sheetProtected: bool,
    /// Page Layout. `printModelled` goes up the first time a command touches
    /// any of these; from then on the writer states them all and the file's own
    /// `printOptions`/`pageMargins`/`pageSetup`/`headerFooter`/`rowBreaks` are
    /// dropped, because two of either element would make the file invalid.
    pub printModelled: bool,
    pub pageOrientation: String,
    pub paperSize: i64,
    pub marginLeft: f64,
    pub marginRight: f64,
    pub marginTop: f64,
    pub marginBottom: f64,
    pub printArea: String,
    pub printTitleRows: String,
    pub printGridlines: bool,
    pub printHeadings: bool,
    pub fitToWidth: i64,
    pub fitToHeight: i64,
    pub pageHeader: String,
    pub pageFooter: String,
    pub rowBreaks: Vec<i64>,
    /// Worksheet children nothing here models, kept so a save can put them back.
    /// See `SheetOpaque`.
    pub opaque: Vec<Rc<RefCell<SheetOpaque>>>,
    /// Which part of the package this sheet was read from. A save that writes
    /// over the source has to put the sheet back where it was: the relationship
    /// part beside it is named after it, and a sheet written to a different name
    /// would leave its own pictures and comments pointing nowhere. Empty for a
    /// sheet that was made here rather than read.
    pub partPath: String,
    pub scrollXSaved: i64,
    pub scrollYSaved: i64,
    pub rowGeomDirty: bool,
    pub colGeomDirty: bool,
    pub touched: Vec<Rc<RefCell<CellPos>>>,
    /// Set when an undo / redo replayed a structural op, so the caller knows to
    /// rebuild the formula engine rather than sync a few cells.
    pub structDirty: bool,
    pub usedMaxRow: i64,
    pub usedMaxCol: i64,
    /// The ops and every rule about them — `gallery/office/editor`, shared with
    /// the document editor. The two stacks, the cap and the transaction
    /// counters used to be fields here; a type parameter lets the shared file
    /// hold `SpreadsheetUndoOp` without knowing anything about it.
    pub history: Rc<RefCell<SheetHistory>>,
    pub version: i64,
    /// When true, `setCellRaw` / style / height writers skip bumping `version`
    /// on every cell. Xlsx load writes millions of cells; a version stamp per
    /// write was free on the JS heap and expensive as native store traffic.
    pub quietWrites: bool,
    /// Which column the data was last sorted by, and which way. This is a
    /// record of what happened, not an instruction to the view: the rows are
    /// really in this order now, so the view sorts nothing. It is what the
    /// header's little arrow is drawn from.
    pub sortedCol: i64,
    pub sortedAsc: bool,
}

impl SpreadsheetModel {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            dbSql: String::new(),
            rowCount: 1000,
            colCount: 26,
            defaultRowH: 20,
            defaultColW: 64,
            defaultFontPt: 11.0,
            defaultFontName: String::new(),
            freezeRows: 0,
            freezeCols: 0,
            cellRows: Vec::new(),
            formulas: HashMap::new(),
            formulaKeys: Vec::new(),
            cellStyleIds: HashMap::new(),
            styles: Vec::new(),
            merges: Vec::new(),
            rowHeights: Vec::new(),
            colWidths: Vec::new(),
            rowOffsets: Vec::new(),
            colOffsets: Vec::new(),
            rowHidden: Vec::new(),
            colHidden: Vec::new(),
            rowSuppressed: Vec::new(),
            filterCol: -1,
            filterValues: Vec::new(),
            filterEnabled: false,
            sortCol: -1,
            sortAsc: true,
            cfRules: Vec::new(),
            sparklines: Vec::new(),
            validations: Vec::new(),
            spillSize: HashMap::new(),
            spillOwner: HashMap::new(),
            spillKeys: Vec::new(),
            images: Vec::new(),
            fileCharts: Vec::new(),
            tables: Vec::new(),
            richCells: HashMap::new(),
            richKeys: Vec::new(),
            links: HashMap::new(),
            linkKeys: Vec::new(),
            notes: HashMap::new(),
            noteKeys: Vec::new(),
            threads: HashMap::new(),
            threadKeys: Vec::new(),
            shownNotes: HashMap::new(),
            showAllNotes: false,
            editRanges: Vec::new(),
            invalidCircles: Vec::new(),
            rowLevels: HashMap::new(),
            colLevels: HashMap::new(),
            shapes: Vec::new(),
            checkboxes: HashMap::new(),
            backgroundSrc: String::new(),
            backgroundBytes: vec![0u8; 0],
            bgWidth: 0,
            bgHeight: 0,
            showRuler: true,
            sheetHidden: false,
            showGridlines: true,
            zoom: 1.0,
            viewMode: "normal".to_string(),
            showHeadings: true,
            showFormulas: false,
            tabColor: String::new(),
            sheetProtected: false,
            printModelled: false,
            pageOrientation: "portrait".to_string(),
            paperSize: 1,
            marginLeft: 0.7,
            marginRight: 0.7,
            marginTop: 0.75,
            marginBottom: 0.75,
            printArea: String::new(),
            printTitleRows: String::new(),
            printGridlines: false,
            printHeadings: false,
            fitToWidth: 0,
            fitToHeight: 0,
            pageHeader: String::new(),
            pageFooter: String::new(),
            rowBreaks: Vec::new(),
            opaque: Vec::new(),
            partPath: String::new(),
            scrollXSaved: 0,
            scrollYSaved: 0,
            rowGeomDirty: true,
            colGeomDirty: true,
            touched: Vec::new(),
            structDirty: false,
            usedMaxRow: 0,
            usedMaxCol: 0,
            history: Rc::new(RefCell::new(SheetHistory::new())),
            version: 0,
            quietWrites: false,
            sortedCol: -1,
            sortedAsc: true,
        }
    }
    /// A1, B17, AA3 — 0-based row/col in, Excel-style label out.
    pub fn colLabel(col: i64) -> String {
        let mut c: i64 = col;
        let mut out: String = String::new();
        while c >= 0 {
            let rem: i64 = c - ((c as f64 / 26.0).floor() as i64) * 26;
            out = format!("{}{}", char::from_u32((65 + rem) as u32).unwrap_or('\0'), out);
            c = ((c as f64 / 26.0).floor() as i64) - 1;
        }
        out
    }
    pub fn cellLabel(row: i64, col: i64) -> String {
        format!("{}{}", SpreadsheetModel::colLabel(col), row + 1)
    }
    /// "12,3" as its two numbers. A cell key is built often enough to be worth
    /// a parser rather than a split into strings.
    /// "row,col" -> [row, col]
    pub fn splitKey(key: &str) -> Vec<i64> {
        let mut out: Vec<i64> = Vec::new();
        let n: i64 = DgNative::str_len(key);
        let mut acc: i64 = 0;
        let mut i: i64 = 0;
        while i < n {
            let ch: i64 = DgNative::char_at(key, i);
            if ch == 44 {
                out.push(acc);
                acc = 0;
            } else if ch >= 48 && ch <= 57 {
                acc = acc * 10 + (ch - 48);
            }
            i += 1;
        }
        out.push(acc);
        out
    }
    /// --- conditional formatting rules -----------------------------------------
    /// Rules are sheet metadata rather than cell contents, so they live beside
    /// the cells, and undoing a paste must not silently drop a rule the paste
    /// never touched. A change made to the rules themselves (add, edit, delete,
    /// clear) is an action of its own on the undo stack: the caller takes
    /// `cfSnapshot()` first, changes the rules, then `pushCfOp`.
    pub fn copyCfRules(list: &Vec<Rc<RefCell<CfRule>>>) -> Vec<Rc<RefCell<CfRule>>> {
        let mut out: Vec<Rc<RefCell<CfRule>>> = Vec::new();
        let mut i: i64 = 0;
        while i < (list.len() as i64) {
            let rule: Rc<RefCell<CfRule>> = list[i as usize].clone();
            out.push(rule.borrow().copy());
            i += 1;
        }
        out
    }
    pub fn parseInt0(s: &str) -> i64 {
        let parsed: Option<i64> = s.parse::<i64>().ok();
        if parsed.is_some() {
            return parsed.unwrap();
        }
        0
    }
    pub fn clearSheet(&mut self) {
        let empty_rows: Vec<HashMap<i64,String>> = Vec::new();
        let empty_formulas: HashMap<String,String> = HashMap::new();
        let empty_style_ids: HashMap<String,i64> = HashMap::new();
        self.cellRows = empty_rows.clone();
        self.formulas = empty_formulas.clone();
        self.cellStyleIds = empty_style_ids.clone();
        self.formulaKeys.clear();
        self.styles.clear();
        self.merges.clear();
        self.cfRules.clear();
        self.opaque.clear();
        self.rowHeights.clear();
        self.colWidths.clear();
        self.rowOffsets.clear();
        self.colOffsets.clear();
        self.rowHidden.clear();
        self.colHidden.clear();
        self.rowSuppressed.clear();
        self.filterValues.clear();
        self.tables.clear();
        self.history.borrow_mut().reset();
        self.freezeRows = 0;
        self.freezeCols = 0;
        self.filterCol = -1;
        self.filterEnabled = false;
        self.sortCol = -1;
        self.sortAsc = true;
        self.sheetHidden = false;
        self.usedMaxRow = 0;
        self.usedMaxCol = 0;
        self.rowCount = 1;
        self.colCount = 1;
        self.rowGeomDirty = true;
        self.colGeomDirty = true;
        self.version += 1;
        self.ensureGeometry();
    }
    /// A copy of this sheet that shares nothing with it: what "Duplicate" on a
    /// tab makes. Cells, formulas, styles, sizes, merges, rules, links, notes
    /// and pictures come along. Tables do not — a table's name is workbook-wide,
    /// so a second one under the same name would make `Table1[Amount]` mean two
    /// things. The copy starts with an empty undo history and no source part.
    pub fn duplicate(&self) -> Rc<RefCell<SpreadsheetModel>> {
        let c: Rc<RefCell<SpreadsheetModel>> = Rc::new(RefCell::new(SpreadsheetModel::new()));
        c.borrow_mut().name = self.name.clone();
        c.borrow_mut().rowCount = self.rowCount;
        c.borrow_mut().colCount = self.colCount;
        c.borrow_mut().defaultRowH = self.defaultRowH;
        c.borrow_mut().defaultColW = self.defaultColW;
        c.borrow_mut().defaultFontPt = self.defaultFontPt;
        c.borrow_mut().defaultFontName = self.defaultFontName.clone();
        c.borrow_mut().freezeRows = self.freezeRows;
        c.borrow_mut().freezeCols = self.freezeCols;
        c.borrow_mut().filterCol = self.filterCol;
        c.borrow_mut().filterEnabled = self.filterEnabled;
        c.borrow_mut().sortCol = self.sortCol;
        c.borrow_mut().sortAsc = self.sortAsc;
        c.borrow_mut().usedMaxRow = self.usedMaxRow;
        c.borrow_mut().usedMaxCol = self.usedMaxCol;
        // A row map is keyed by column, and an int-keyed map has no `keys` on
        // every target, so the columns are walked: as far as any cell can be.
        let mut last_col: i64 = self.colCount;
        if self.usedMaxCol + 1 > last_col {
            last_col = self.usedMaxCol + 1;
        }
        let mut r: i64 = 0;
        while r < (self.cellRows.len() as i64) {
            let src: HashMap<i64,String> = self.cellRows[r as usize].clone();
            let mut row: HashMap<i64,String> = HashMap::new();
            let mut col: i64 = 0;
            while col < last_col {
                if src.contains_key(&col) {
                    row.insert(col, src.get(&col).cloned().unwrap());
                }
                col += 1;
            }
            c.borrow_mut().cellRows.push(row.clone());
            r += 1;
        }
        let mut fk: i64 = 0;
        while fk < (self.formulaKeys.len() as i64) {
            let k: String = self.formulaKeys[fk as usize].clone();
            if self.formulas.contains_key(&k) {
                c.borrow_mut().formulas.insert(k.clone(), self.formulas.get(&k).cloned().unwrap());
                c.borrow_mut().formulaKeys.push(k.clone());
            }
            fk += 1;
        }
        let sk: Vec<String> = self.cellStyleIds.keys()
            .cloned()
            .collect::<Vec<String>>();
        let mut si: i64 = 0;
        while si < (sk.len() as i64) {
            let k2: String = sk[si as usize].clone();
            c.borrow_mut().cellStyleIds.insert(k2.clone(), self.cellStyleIds.get(&k2).cloned().unwrap());
            si += 1;
        }
        let mut st: i64 = 0;
        while st < (self.styles.len() as i64) {
            let cs: Rc<RefCell<CellStyle>> = self.styles[st as usize].clone();
            c.borrow_mut().styles.push(cs.borrow().copy());
            st += 1;
        }
        let mut mi: i64 = 0;
        while mi < (self.merges.len() as i64) {
            c.borrow_mut().merges.push(self.merges[mi as usize].clone());
            mi += 1;
        }
        let mut ri: i64 = 0;
        while ri < (self.rowHeights.len() as i64) {
            c.borrow_mut().rowHeights.push(self.rowHeights[ri as usize]);
            ri += 1;
        }
        let mut wi: i64 = 0;
        while wi < (self.colWidths.len() as i64) {
            c.borrow_mut().colWidths.push(self.colWidths[wi as usize]);
            wi += 1;
        }
        let mut hi: i64 = 0;
        while hi < (self.rowHidden.len() as i64) {
            c.borrow_mut().rowHidden.push(self.rowHidden[hi as usize]);
            hi += 1;
        }
        let mut hc: i64 = 0;
        while hc < (self.colHidden.len() as i64) {
            c.borrow_mut().colHidden.push(self.colHidden[hc as usize]);
            hc += 1;
        }
        let mut rs: i64 = 0;
        while rs < (self.rowSuppressed.len() as i64) {
            c.borrow_mut().rowSuppressed.push(self.rowSuppressed[rs as usize]);
            rs += 1;
        }
        let mut fv: i64 = 0;
        while fv < (self.filterValues.len() as i64) {
            c.borrow_mut().filterValues.push(self.filterValues[fv as usize].clone());
            fv += 1;
        }
        let mut cf: i64 = 0;
        while cf < (self.cfRules.len() as i64) {
            let rule: Rc<RefCell<CfRule>> = self.cfRules[cf as usize].clone();
            c.borrow_mut().cfRules.push(rule.borrow().copy());
            cf += 1;
        }
        let mut dv: i64 = 0;
        while dv < (self.validations.len() as i64) {
            let v: Rc<RefCell<DataValidation>> = self.validations[dv as usize].clone();
            c.borrow_mut().validations.push(v.borrow().copy());
            dv += 1;
        }
        let mut sp: i64 = 0;
        while sp < (self.spillKeys.len() as i64) {
            let k3: String = self.spillKeys[sp as usize].clone();
            c.borrow_mut().spillKeys.push(k3.clone());
            if self.spillSize.contains_key(&k3) {
                c.borrow_mut().spillSize.insert(k3.clone(), self.spillSize.get(&k3).cloned().unwrap());
            }
            sp += 1;
        }
        let ok: Vec<String> = self.spillOwner.keys()
            .cloned()
            .collect::<Vec<String>>();
        let mut oi: i64 = 0;
        while oi < (ok.len() as i64) {
            let k4: String = ok[oi as usize].clone();
            c.borrow_mut().spillOwner.insert(k4.clone(), self.spillOwner.get(&k4).cloned().unwrap());
            oi += 1;
        }
        let mut im: i64 = 0;
        while im < (self.images.len() as i64) {
            let pic: Rc<RefCell<SheetImage>> = self.images[im as usize].clone();
            let pc: Rc<RefCell<SheetImage>> = Rc::new(RefCell::new(SheetImage::new()));
            pc.borrow_mut().copyFrom(&pic);
            c.borrow_mut().images.push(pc.clone());
            im += 1;
        }
        let mut shi: i64 = 0;
        while shi < (self.shapes.len() as i64) {
            let shp: Rc<RefCell<SheetShape>> = Rc::new(RefCell::new(SheetShape::new()));
            shp.borrow_mut().copyFrom(&self.shapes[shi as usize].clone());
            c.borrow_mut().shapes.push(shp.clone());
            shi += 1;
        }
        let mut rk: i64 = 0;
        while rk < (self.richKeys.len() as i64) {
            let k5: String = self.richKeys[rk as usize].clone();
            if self.richCells.contains_key(&k5) {
                let rt: Rc<RefCell<RichText>> = self.richCells.get(&k5)
                    .cloned()
                    .unwrap();
                c.borrow_mut().richCells.insert(k5.clone(), rt.borrow().copy());
                c.borrow_mut().richKeys.push(k5.clone());
            }
            rk += 1;
        }
        let mut lk: i64 = 0;
        while lk < (self.linkKeys.len() as i64) {
            let k6: String = self.linkKeys[lk as usize].clone();
            if self.links.contains_key(&k6) {
                c.borrow_mut().links.insert(k6.clone(), self.links.get(&k6).cloned().unwrap());
                c.borrow_mut().linkKeys.push(k6.clone());
            }
            lk += 1;
        }
        let mut nk: i64 = 0;
        while nk < (self.noteKeys.len() as i64) {
            let k7: String = self.noteKeys[nk as usize].clone();
            if self.notes.contains_key(&k7) {
                c.borrow_mut().notes.insert(k7.clone(), self.notes.get(&k7).cloned().unwrap());
                c.borrow_mut().noteKeys.push(k7.clone());
            }
            nk += 1;
        }
        c.borrow_mut().rowGeomDirty = true;
        c.borrow_mut().colGeomDirty = true;
        c.borrow_mut().version = c.borrow().version + 1;
        c.borrow_mut().ensureGeometry();
        c
    }
    pub fn cellKey(&self, row: i64, col: i64) -> String {
        format!("{},{}", row, col)
    }
    pub fn ensureCellRow(&mut self, row: i64) {
        while (self.cellRows.len() as i64) <= row {
            let blank: HashMap<i64,String> = HashMap::new();
            self.cellRows.push(blank.clone());
        }
    }
    /// Allocate per-row / per-column size arrays when missing. Does NOT rebuild
    /// prefix-sum offsets — `autoFitRows` (and any bulk height pass) calls
    /// `rowHeight` / `setRowHeight` once per row, and rebuilding offsets on
    /// every call is O(rows²). Paint paths still go through `ensureGeometry`.
    pub fn ensureSizeArrays(&mut self) {
        if (self.rowHeights.len() as i64) != self.rowCount {
            self.rowHeights.clear();
            self.rowHidden.clear();
            let mut i: i64 = 0;
            while i < self.rowCount {
                self.rowHeights.push(self.defaultRowH);
                self.rowHidden.push(false);
                i += 1;
            }
            self.rowGeomDirty = true;
        }
        if (self.colWidths.len() as i64) != self.colCount {
            self.colWidths.clear();
            self.colHidden.clear();
            let mut j: i64 = 0;
            while j < self.colCount {
                self.colWidths.push(self.defaultColW);
                self.colHidden.push(false);
                j += 1;
            }
            self.colGeomDirty = true;
        }
        if (self.rowHidden.len() as i64) != self.rowCount {
            self.rowHidden.clear();
            self.rowSuppressed.clear();
            let mut hi: i64 = 0;
            while hi < self.rowCount {
                self.rowHidden.push(false);
                self.rowSuppressed.push(false);
                hi += 1;
            }
        }
        if (self.colHidden.len() as i64) != self.colCount {
            self.colHidden.clear();
            let mut hj: i64 = 0;
            while hj < self.colCount {
                self.colHidden.push(false);
                hj += 1;
            }
        }
    }
    pub fn ensureGeometry(&mut self) {
        self.ensureSizeArrays();
        if self.rowGeomDirty {
            self.rowOffsets.clear();
            let mut acc: i64 = 0;
            self.rowOffsets.push(0);
            let mut r: i64 = 0;
            while r < self.rowCount {
                let mut rh: i64 = self.zoomed(self.rowHeights[r as usize]);
                if r < (self.rowHidden.len() as i64) && self.rowHidden[r as usize] {
                    rh = 0;
                }
                if r < (self.rowSuppressed.len() as i64) && self.rowSuppressed[r as usize] {
                    rh = 0;
                }
                acc += rh;
                self.rowOffsets.push(acc);
                r += 1;
            }
            self.rowGeomDirty = false;
        }
        if self.colGeomDirty {
            self.colOffsets.clear();
            let mut acc2: i64 = 0;
            self.colOffsets.push(0);
            let mut c: i64 = 0;
            while c < self.colCount {
                let mut cw: i64 = self.zoomed(self.colWidths[c as usize]);
                if c < (self.colHidden.len() as i64) && self.colHidden[c as usize] {
                    cw = 0;
                }
                acc2 += cw;
                self.colOffsets.push(acc2);
                c += 1;
            }
            self.colGeomDirty = false;
        }
    }
    /// Grow the sheet without touching what is already there. resize() clears
    /// rowHeights / colWidths, so it must never be used to extend a loaded
    /// workbook — that would throw away every column width from styles.xml.
    /// --- column structure -----------------------------------------------------
    /// Insert / delete / move operate on the used rectangle only, so a sheet that
    /// declares 100k rows does not pay for its empty ones. Cells, formulas, style
    /// ids, widths, hidden flags and merges all move together — anything left
    /// behind would show up as a value with someone else's formatting.
    pub fn moveCellRaw(&mut self, from_row: i64, from_col: i64, to_row: i64, to_col: i64) {
        let v: String = self.getCell(from_row, from_col);
        self.setCellRaw(to_row, to_col, &v);
        self.setCellRaw(from_row, from_col, "");
        let src: String = self.cellKey(from_row, from_col);
        let dst: String = self.cellKey(to_row, to_col);
        let mut f: String = String::new();
        if self.formulas.contains_key(&src) {
            f = self.formulas.get(&src).cloned().unwrap();
        }
        self.setFormulaText(to_row, to_col, &f);
        if self.formulas.contains_key(&src) {
            self.formulas.insert(src.clone(), String::new());
        }
        let mut sid: i64 = -1;
        if self.cellStyleIds.contains_key(&src) {
            sid = self.cellStyleIds.get(&src).cloned().unwrap();
        }
        self.cellStyleIds.insert(dst.clone(), sid);
        self.cellStyleIds.insert(src.clone(), -1);
    }
    pub fn clearCellAt(&mut self, row: i64, col: i64) {
        self.setCellRaw(row, col, "");
        let k: String = self.cellKey(row, col);
        if self.formulas.contains_key(&k) {
            self.formulas.insert(k.clone(), String::new());
        }
        self.cellStyleIds.insert(k.clone(), -1);
    }
    /// The cells that hold a formula, in the order they were written, as
    /// "row,col" keys; `formulaSnapshotTexts` gives their formulas.
    pub fn formulaSnapshotKeys(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for k in self.formulaKeys.iter() {
            let key: String = k.clone();
            if let Some(t) = self.formulas.get(&key) {
                if !t.is_empty() {
                    out.push(key);
                }
            }
        }
        out
    }
    /// The formulas of `formulaSnapshotKeys`, in the same order.
    pub fn formulaSnapshotTexts(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for k in self.formulaKeys.iter() {
            if let Some(t) = self.formulas.get(k) {
                if !t.is_empty() {
                    out.push(t.clone());
                }
            }
        }
        out
    }
    pub fn restoreFormulas(&mut self, keys: &[String], texts: &[String]) {
        // Wipe what is there now, then lay the snapshot back down.
        let mut i: i64 = 0;
        while i < (self.formulaKeys.len() as i64) {
            let k: String = self.formulaKeys[i as usize].clone();
            if self.formulas.contains_key(&k) {
                self.formulas.insert(k.clone(), String::new());
            }
            i += 1;
        }
        let mut j: i64 = 0;
        while j < (keys.len() as i64) {
            let key: String = keys[j as usize].clone();
            if !self.formulas.contains_key(&key) {
                self.formulaKeys.push(key.clone());
            }
            self.formulas.insert(key.clone(), texts[j as usize].clone());
            j += 1;
        }
        self.version += 1;
    }
    /// The values, formulas and style ids of one column, down to the last used row.
    pub fn snapshotColumn(&self, col: i64) -> LineSnapshot {
        let mut snap = LineSnapshot::new();
        let last: i64 = self.usedRow();
        let mut r: i64 = 0;
        while r <= last {
            snap.values.push(self.getCell(r, col));
            snap.formulas.push(self.getFormula(r, col));
            snap.styles.push(self.getCellStyleId(r, col));
            r += 1;
        }
        snap
    }
    pub fn restoreColumn(&mut self, col: i64, values: &[String], formulas: &[String], styles: &[i64]) {
        let n: i64 = values.len() as i64;
        let mut r: i64 = 0;
        while r < n {
            self.setCellRaw(r, col, &values[r as usize].clone());
            self.setFormulaText(r, col, &formulas[r as usize].clone());
            let k: String = self.cellKey(r, col);
            self.cellStyleIds.insert(k.clone(), styles[r as usize]);
            r += 1;
        }
    }
    pub fn remapMergesForColumn(&mut self, at: i64, delta: i64) {
        let mut kept: Vec<String> = Vec::new();
        let mut i: i64 = 0;
        while i < (self.merges.len() as i64) {
            let m: String = self.merges[i as usize].clone();
            let parts: Vec<String> = m.split(",").map(|s| s.to_string()).collect::<Vec<String>>();
            if (parts.len() as i64) >= 4 {
                let r1: i64 = SpreadsheetModel::parseInt0(&parts[0].clone());
                let mut c1: i64 = SpreadsheetModel::parseInt0(&parts[1].clone());
                let r2: i64 = SpreadsheetModel::parseInt0(&parts[2].clone());
                let mut c2: i64 = SpreadsheetModel::parseInt0(&parts[3].clone());
                let mut drop: bool = false;
                if delta < 0 {
                    if c1 == at && c2 == at {
                        drop = true;
                    }
                }
                if c1 >= at {
                    c1 += delta;
                }
                if c2 >= at {
                    c2 += delta;
                }
                if c2 < c1 {
                    drop = true;
                }
                if !drop {
                    kept.push(format!("{},{},{},{}", r1, c1, r2, c2));
                }
            }
            i += 1;
        }
        self.merges.clear();
        let mut j: i64 = 0;
        while j < (kept.len() as i64) {
            self.merges.push(kept[j as usize].clone());
            j += 1;
        }
    }
    pub fn insertColumnRaw(&mut self, at: i64) {
        if at < 0 {
            return;
        }
        if at > self.colCount {
            return;
        }
        let last_r: i64 = self.usedRow();
        let last_c: i64 = self.usedCol();
        // Right to left, so a destination is never read afterwards.
        let mut c: i64 = last_c;
        while c >= at {
            let mut r: i64 = 0;
            while r <= last_r {
                self.moveCellRaw(r, c, r, c + 1);
                r += 1;
            }
            c -= 1;
        }
        let mut r2: i64 = 0;
        while r2 <= last_r {
            self.clearCellAt(r2, at);
            r2 += 1;
        }
        // Geometry arrays must end up exactly colCount long, or ensureGeometry
        // decides they are stale and resets every width to the default.
        if at < (self.colWidths.len() as i64) {
            self.colWidths.insert(at as usize, self.defaultColW);
        } else {
            self.colWidths.push(self.defaultColW);
        }
        if at < (self.colHidden.len() as i64) {
            self.colHidden.insert(at as usize, false);
        } else {
            self.colHidden.push(false);
        }
        self.colCount += 1;
        if self.usedMaxCol >= at {
            self.usedMaxCol += 1;
        }
        self.remapMergesForColumn(at, 1);
        self.colGeomDirty = true;
        self.ensureGeometry();
        self.version += 1;
    }
    pub fn deleteColumnRaw(&mut self, at: i64) {
        if at < 0 {
            return;
        }
        if at >= self.colCount {
            return;
        }
        if self.colCount <= 1 {
            return;
        }
        let last_r: i64 = self.usedRow();
        let last_c: i64 = self.usedCol();
        let mut c: i64 = at;
        while c < last_c {
            let mut r: i64 = 0;
            while r <= last_r {
                self.moveCellRaw(r, c + 1, r, c);
                r += 1;
            }
            c += 1;
        }
        // Only the column the shift vacated gets cleared. Deleting a column
        // BEYOND the used range shifts nothing, and clearing lastC there would
        // wipe live data instead.
        if at <= last_c {
            let mut r3: i64 = 0;
            while r3 <= last_r {
                self.clearCellAt(r3, last_c);
                r3 += 1;
            }
        }
        if at < (self.colWidths.len() as i64) {
            self.colWidths.remove((at) as usize);
        }
        if at < (self.colHidden.len() as i64) {
            self.colHidden.remove((at) as usize);
        }
        self.colCount -= 1;
        if self.usedMaxCol >= at {
            if self.usedMaxCol > 0 {
                self.usedMaxCol -= 1;
            }
        }
        self.remapMergesForColumn(at, -1);
        self.colGeomDirty = true;
        self.ensureGeometry();
        self.version += 1;
    }
    /// Reorder: lift a column out and drop it at `to` (index in the sheet AFTER
    /// the lift, the way a drag reads).
    pub fn moveColumnRaw(&mut self, from: i64, to: i64) {
        if from == to {
            return;
        }
        if from < 0 {
            return;
        }
        if from >= self.colCount {
            return;
        }
        if to < 0 {
            return;
        }
        if to >= self.colCount {
            return;
        }
        let snap = self.snapshotColumn(from);
        let width: i64 = self.colWidth(from);
        let hidden: bool = self.isColHidden(from);
        self.deleteColumnRaw(from);
        self.insertColumnRaw(to);
        self.restoreColumn(to, &snap.values, &snap.formulas, &snap.styles);
        if to < (self.colWidths.len() as i64) {
            self.colWidths[to as usize] = width;
        }
        if to < (self.colHidden.len() as i64) {
            self.colHidden[to as usize] = hidden;
        }
        self.colGeomDirty = true;
        self.ensureGeometry();
        self.version += 1;
    }
    /// The values, formulas and style ids of one row, across to the last used column.
    pub fn snapshotRow(&self, row: i64) -> LineSnapshot {
        let mut snap = LineSnapshot::new();
        let last: i64 = self.usedCol();
        let mut c: i64 = 0;
        while c <= last {
            snap.values.push(self.getCell(row, c));
            snap.formulas.push(self.getFormula(row, c));
            snap.styles.push(self.getCellStyleId(row, c));
            c += 1;
        }
        snap
    }
    pub fn restoreRow(&mut self, row: i64, values: &[String], formulas: &[String], styles: &[i64]) {
        let n: i64 = values.len() as i64;
        let mut c: i64 = 0;
        while c < n {
            self.setCellRaw(row, c, &values[c as usize].clone());
            self.setFormulaText(row, c, &formulas[c as usize].clone());
            let k: String = self.cellKey(row, c);
            self.cellStyleIds.insert(k.clone(), styles[c as usize]);
            c += 1;
        }
    }
    pub fn remapMergesForRow(&mut self, at: i64, delta: i64) {
        let mut kept: Vec<String> = Vec::new();
        let mut i: i64 = 0;
        while i < (self.merges.len() as i64) {
            let m: String = self.merges[i as usize].clone();
            let parts: Vec<String> = m.split(",").map(|s| s.to_string()).collect::<Vec<String>>();
            if (parts.len() as i64) >= 4 {
                let mut r1: i64 = SpreadsheetModel::parseInt0(&parts[0].clone());
                let c1: i64 = SpreadsheetModel::parseInt0(&parts[1].clone());
                let mut r2: i64 = SpreadsheetModel::parseInt0(&parts[2].clone());
                let c2: i64 = SpreadsheetModel::parseInt0(&parts[3].clone());
                let mut drop: bool = false;
                if delta < 0 {
                    if r1 == at && r2 == at {
                        drop = true;
                    }
                }
                if r1 >= at {
                    r1 += delta;
                }
                if r2 >= at {
                    r2 += delta;
                }
                if r2 < r1 {
                    drop = true;
                }
                if !drop {
                    kept.push(format!("{},{},{},{}", r1, c1, r2, c2));
                }
            }
            i += 1;
        }
        self.merges.clear();
        let mut j: i64 = 0;
        while j < (kept.len() as i64) {
            self.merges.push(kept[j as usize].clone());
            j += 1;
        }
    }
    /// The column ops, transposed. Every remark on those applies here for the
    /// same reason: bottom to top so a destination is never read afterwards,
    /// geometry spliced BEFORE the count changes so `ensureGeometry` does not
    /// decide the arrays are stale and reset every height, and the vacated line
    /// cleared only when the shift actually reached it.
    pub fn insertRowRaw(&mut self, at: i64) {
        if at < 0 {
            return;
        }
        if at > self.rowCount {
            return;
        }
        let last_r: i64 = self.usedRow();
        let last_c: i64 = self.usedCol();
        let mut r: i64 = last_r;
        while r >= at {
            let mut c: i64 = 0;
            while c <= last_c {
                self.moveCellRaw(r, c, r + 1, c);
                c += 1;
            }
            r -= 1;
        }
        let mut c2: i64 = 0;
        while c2 <= last_c {
            self.clearCellAt(at, c2);
            c2 += 1;
        }
        if at < (self.rowHeights.len() as i64) {
            self.rowHeights.insert(at as usize, self.defaultRowH);
        } else {
            self.rowHeights.push(self.defaultRowH);
        }
        if at < (self.rowHidden.len() as i64) {
            self.rowHidden.insert(at as usize, false);
        } else {
            self.rowHidden.push(false);
        }
        self.rowCount += 1;
        if self.usedMaxRow >= at {
            self.usedMaxRow += 1;
        }
        self.remapMergesForRow(at, 1);
        self.rowGeomDirty = true;
        self.ensureGeometry();
        self.version += 1;
    }
    pub fn deleteRowRaw(&mut self, at: i64) {
        if at < 0 {
            return;
        }
        if at >= self.rowCount {
            return;
        }
        if self.rowCount <= 1 {
            return;
        }
        let last_r: i64 = self.usedRow();
        let last_c: i64 = self.usedCol();
        let mut r: i64 = at;
        while r < last_r {
            let mut c: i64 = 0;
            while c <= last_c {
                self.moveCellRaw(r + 1, c, r, c);
                c += 1;
            }
            r += 1;
        }
        if at <= last_r {
            let mut c3: i64 = 0;
            while c3 <= last_c {
                self.clearCellAt(last_r, c3);
                c3 += 1;
            }
        }
        if at < (self.rowHeights.len() as i64) {
            self.rowHeights.remove((at) as usize);
        }
        if at < (self.rowHidden.len() as i64) {
            self.rowHidden.remove((at) as usize);
        }
        self.rowCount -= 1;
        if self.usedMaxRow >= at {
            if self.usedMaxRow > 0 {
                self.usedMaxRow -= 1;
            }
        }
        self.remapMergesForRow(at, -1);
        self.rowGeomDirty = true;
        self.ensureGeometry();
        self.version += 1;
    }
    pub fn moveRowRaw(&mut self, from: i64, to: i64) {
        if from == to {
            return;
        }
        if from < 0 {
            return;
        }
        if from >= self.rowCount {
            return;
        }
        if to < 0 {
            return;
        }
        if to >= self.rowCount {
            return;
        }
        let snap = self.snapshotRow(from);
        let height: i64 = self.rowHeight(from);
        let hidden: bool = self.isRowHidden(from);
        self.deleteRowRaw(from);
        self.insertRowRaw(to);
        self.restoreRow(to, &snap.values, &snap.formulas, &snap.styles);
        if to < (self.rowHeights.len() as i64) {
            self.rowHeights[to as usize] = height;
        }
        if to < (self.rowHidden.len() as i64) {
            self.rowHidden[to as usize] = hidden;
        }
        self.rowGeomDirty = true;
        self.ensureGeometry();
        self.version += 1;
    }
    /// Put the rows in a new order, in one pass.
    ///
    /// `order[i]` is the row that should end up AT i, and every row is named
    /// exactly once. Everything a row owns travels with it: its values, its
    /// formulas, the styles of its cells, its height and whether it is hidden.
    ///
    /// Every row is read out before any is written back, because a permutation
    /// applied in place overwrites rows that have not been read yet. Doing this
    /// with repeated single moves would be correct and quadratic, and a sheet is
    /// sorted often enough for that to be the difference between instant and
    /// noticeable.
    pub fn reorderRowsRaw(&mut self, order: &[i64]) {
        let n: i64 = order.len() as i64;
        if n < 2 {
            return;
        }
        // A row's VALUES are already a row: `cellRows` is one map per row, so
        // reordering them is reordering an array of references. The old code
        // read every cell out through a "r,c" string key and wrote it back
        // through another — six key builds and six map operations per cell,
        // two and a half million of each on a 500k-row sheet, which is four
        // seconds of a frozen window for work the storage layout had already
        // done.
        //
        // Formulas and styles are keyed flat, so those do move cell by cell —
        // but only the cells that HAVE one, which on an ordinary sheet is a
        // small fraction of it.
        let mut moved: Vec<HashMap<i64,String>> = Vec::new();
        let mut i: i64 = 0;
        while i < n {
            let mut from: i64 = order[i as usize];
            if from < 0 {
                from = 0;
            }
            if from >= (self.cellRows.len() as i64) {
                from = (self.cellRows.len() as i64) - 1;
            }
            if from < 0 {
                let blank: HashMap<i64,String> = HashMap::new();
                moved.push(blank.clone());
            } else {
                moved.push(self.cellRows[from as usize].clone());
            }
            i += 1;
        }
        let mut heights: Vec<i64> = Vec::new();
        let mut hiddens: Vec<bool> = Vec::new();
        let mut h: i64 = 0;
        while h < n {
            let mut hf: i64 = order[h as usize];
            if hf < 0 {
                hf = 0;
            }
            if hf >= self.rowCount {
                hf = self.rowCount - 1;
            }
            heights.push(self.rowHeight(hf));
            hiddens.push(self.isRowHidden(hf));
            h += 1;
        }
        // Formulas and style ids, gathered before anything is written so a
        // permutation cannot read a cell it has already overwritten.
        let width: i64 = self.usedCol() + 1;
        let mut new_row_of: Vec<i64> = Vec::new();
        let mut z: i64 = 0;
        while z < n {
            new_row_of.push(z);
            z += 1;
        }
        let mut k2: i64 = 0;
        while k2 < n {
            let src: i64 = order[k2 as usize];
            if src >= 0 {
                if src < n {
                    new_row_of[src as usize] = k2;
                }
            }
            k2 += 1;
        }
        let mut f_keys: Vec<String> = Vec::new();
        let mut f_vals: Vec<String> = Vec::new();
        let mut fj: i64 = 0;
        while fj < (self.formulaKeys.len() as i64) {
            let key: String = self.formulaKeys[fj as usize].clone();
            let mut text: String = String::new();
            if self.formulas.contains_key(&key) {
                text = self.formulas.get(&key).cloned().unwrap();
            }
            if !text.is_empty() {
                f_keys.push(key.clone());
                f_vals.push(text.clone());
            }
            fj += 1;
        }
        let mut s_keys: Vec<String> = Vec::new();
        let mut s_vals: Vec<i64> = Vec::new();
        let mut sr: i64 = 0;
        while sr < n {
            let mut sc: i64 = 0;
            while sc < width {
                let sk: String = self.cellKey(sr, sc);
                if self.cellStyleIds.contains_key(&sk) {
                    s_keys.push(sk.clone());
                    s_vals.push(self.cellStyleIds.get(&sk).cloned().unwrap());
                }
                sc += 1;
            }
            sr += 1;
        }
        // Values: the whole permutation, in one pass over the row maps.
        let mut m: i64 = 0;
        while m < n {
            if m < (self.cellRows.len() as i64) {
                self.cellRows[m as usize] = moved[m as usize].clone();
            }
            if m < (self.rowHeights.len() as i64) {
                self.rowHeights[m as usize] = heights[m as usize];
            }
            if m < (self.rowHidden.len() as i64) {
                self.rowHidden[m as usize] = hiddens[m as usize];
            }
            m += 1;
        }
        // Formulas: cleared and laid back down at the rows they moved to.
        let mut cf: i64 = 0;
        while cf < (f_keys.len() as i64) {
            self.formulas.insert(f_keys[cf as usize].clone(), String::new());
            cf += 1;
        }
        let mut pf: i64 = 0;
        while pf < (f_keys.len() as i64) {
            let parts: Vec<i64> = SpreadsheetModel::splitKey(&f_keys[pf as usize]);
            if (parts.len() as i64) == 2 {
                let orow: i64 = parts[0];
                let ocol: i64 = parts[1];
                let mut nrow: i64 = orow;
                if orow >= 0 {
                    if orow < (new_row_of.len() as i64) {
                        nrow = new_row_of[orow as usize];
                    }
                }
                self.setFormulaText(nrow, ocol, &f_vals[pf as usize].clone());
            }
            pf += 1;
        }
        // Style ids, the same way.
        let mut cs: i64 = 0;
        while cs < (s_keys.len() as i64) {
            self.cellStyleIds.insert(s_keys[cs as usize].clone(), -1);
            cs += 1;
        }
        let mut ps: i64 = 0;
        while ps < (s_keys.len() as i64) {
            let sparts: Vec<i64> = SpreadsheetModel::splitKey(&s_keys[ps as usize]);
            if (sparts.len() as i64) == 2 {
                let srow: i64 = sparts[0];
                let scol: i64 = sparts[1];
                let mut nsrow: i64 = srow;
                if srow >= 0 {
                    if srow < (new_row_of.len() as i64) {
                        nsrow = new_row_of[srow as usize];
                    }
                }
                self.cellStyleIds.insert(self.cellKey(nsrow, scol), s_vals[ps as usize]);
            }
            ps += 1;
        }
        self.rowGeomDirty = true;
        self.ensureGeometry();
        self.version += 1;
    }
    pub fn growTo(&mut self, rows: i64, cols: i64) {
        let mut changed: bool = false;
        if rows > self.rowCount {
            let same_len: bool = (self.rowHeights.len() as i64) == self.rowCount;
            let supp_len: bool = (self.rowSuppressed.len() as i64) == self.rowCount;
            if same_len {
                let mut r: i64 = self.rowCount;
                while r < rows {
                    self.rowHeights.push(self.defaultRowH);
                    self.rowHidden.push(false);
                    if supp_len {
                        self.rowSuppressed.push(false);
                    }
                    r += 1;
                }
            }
            self.rowCount = rows;
            self.rowGeomDirty = true;
            changed = true;
        }
        if cols > self.colCount {
            let same_cols: bool = (self.colWidths.len() as i64) == self.colCount;
            if same_cols {
                let mut c: i64 = self.colCount;
                while c < cols {
                    self.colWidths.push(self.defaultColW);
                    self.colHidden.push(false);
                    c += 1;
                }
            }
            self.colCount = cols;
            self.colGeomDirty = true;
            changed = true;
        }
        if changed {
            self.version += 1;
            self.ensureGeometry();
        }
    }
    pub fn resize(&mut self, mut rows: i64, mut cols: i64) {
        if rows < 1 {
            rows = 1;
        }
        if cols < 1 {
            cols = 1;
        }
        self.rowCount = rows;
        self.colCount = cols;
        self.rowHeights.clear();
        self.colWidths.clear();
        self.rowGeomDirty = true;
        self.colGeomDirty = true;
        self.ensureGeometry();
        self.version += 1;
    }
    pub fn getCell(&self, row: i64, col: i64) -> String {
        if row < 0 {
            return String::new();
        }
        if row >= (self.cellRows.len() as i64) {
            return String::new();
        }
        let m: HashMap<i64,String> = self.cellRows[row as usize].clone();
        if m.contains_key(&col) {
            return m.get(&col).cloned().unwrap();
        }
        String::new()
    }
    pub fn setCellStyleId(&mut self, row: i64, col: i64, style_id: i64) {
        let k: String = self.cellKey(row, col);
        self.cellStyleIds.insert(k.clone(), style_id);
    }
    pub fn getCellStyleId(&self, row: i64, col: i64) -> i64 {
        let k: String = self.cellKey(row, col);
        if self.cellStyleIds.contains_key(&k) {
            return self.cellStyleIds.get(&k).cloned().unwrap();
        }
        -1
    }
    /// Index of an equal style in this sheet's table, adding it when new. Paste
    /// carries a resolved CellStyle rather than an index, so it survives a hop
    /// between sheets (each sheet holds its own copy of the style table).
    pub fn styleIdFor(&mut self, st: &Rc<RefCell<CellStyle>>) -> i64 {
        if st.borrow().isPlain() {
            return -1;
        }
        let mut i: i64 = 0;
        let n: i64 = self.styles.len() as i64;
        while i < n {
            let cand: Rc<RefCell<CellStyle>> = self.styles[i as usize].clone();
            if cand.borrow().sameAs(st) {
                return i;
            }
            i += 1;
        }
        self.styles.push(st.borrow().copy());
        (self.styles.len() as i64) - 1
    }
    pub fn setCellStyleResolved(&mut self, row: i64, col: i64, st: &Rc<RefCell<CellStyle>>) {
        let id: i64 = self.styleIdFor(st);
        let k: String = self.cellKey(row, col);
        self.cellStyleIds.insert(k.clone(), id);
        self.version += 1;
    }
    pub fn getCellStyle(&self, row: i64, col: i64) -> Rc<RefCell<CellStyle>> {
        let k: String = self.cellKey(row, col);
        if self.cellStyleIds.contains_key(&k) {
            let sid: i64 = self.cellStyleIds.get(&k).cloned().unwrap();
            if sid >= 0 && sid < (self.styles.len() as i64) {
                return self.styles[sid as usize].clone();
            }
        }
        let plain: Rc<RefCell<CellStyle>> = CellStyle::createDefault();
        plain.borrow_mut().fontSizePt = self.defaultFontPt;
        if !self.defaultFontName.is_empty() {
            plain.borrow_mut().fontName = self.defaultFontName.clone();
        }
        plain
    }
    /// FortuneSheet-style display value: raw cell + number format engine.
    pub fn getDisplay(&self, row: i64, col: i64) -> String {
        let raw: String = self.getCell(row, col);
        let st: Rc<RefCell<CellStyle>> = self.getCellStyle(row, col);
        let out: String = XlsxNumberFormat::format(&raw, &st.borrow().numFmt.clone());
        out
    }
    /// --- images ---------------------------------------------------------------
    pub fn addImage(&mut self, img: Rc<RefCell<SheetImage>>) {
        self.images.push(img.clone());
        self.version += 1;
    }
    pub fn imageCount(&self) -> i64 {
        self.images.len() as i64
    }
    pub fn isCheckbox(&self, row: i64, col: i64) -> bool {
        let k: String = self.cellKey(row, col);
        if self.checkboxes.contains_key(&k) {
            return self.checkboxes.get(&k).cloned().unwrap();
        }
        false
    }
    /// A slicer's filter: rows r0.. take the flags given (true = hidden).
    pub fn setRowsSuppressed(&mut self, r0: i64, flags: &[bool]) {
        self.ensureGeometry();
        if (self.rowSuppressed.len() as i64) != self.rowCount {
            self.rowSuppressed.clear();
            let mut hi: i64 = 0;
            while hi < self.rowCount {
                self.rowSuppressed.push(false);
                hi += 1;
            }
        }
        let mut i: i64 = 0;
        while i < (flags.len() as i64) {
            let r: i64 = r0 + i;
            if r >= 0 && r < self.rowCount {
                self.rowSuppressed[r as usize] = flags[i as usize];
            }
            i += 1;
        }
        self.rowGeomDirty = true;
        self.version += 1;
    }
    pub fn shapeCount(&self) -> i64 {
        self.shapes.len() as i64
    }
    /// --- tables ---------------------------------------------------------------
    pub fn addTable(&mut self, t: Rc<RefCell<SheetTable>>) {
        self.tables.push(t.clone());
        self.version += 1;
    }
    /// The table a cell belongs to, as an index into `tables`, or -1.
    /// Forget preserved worksheet children of this name: the model states them
    /// itself from now on.
    pub fn dropOpaque(&mut self, el_name: &str) {
        let mut keep: Vec<Rc<RefCell<SheetOpaque>>> = Vec::new();
        let mut i: i64 = 0;
        while i < (self.opaque.len() as i64) {
            let o: Rc<RefCell<SheetOpaque>> = self.opaque[i as usize].clone();
            if o.borrow().name != el_name {
                keep.push(o.clone());
            }
            i += 1;
        }
        self.opaque = keep.clone();
    }
    pub fn modelPrint(&mut self) {
        if self.printModelled {
            return;
        }
        self.printModelled = true;
        self.dropOpaque("printOptions");
        self.dropOpaque("pageMargins");
        self.dropOpaque("pageSetup");
        self.dropOpaque("headerFooter");
        self.dropOpaque("rowBreaks");
    }
    pub fn tableIndexAt(&self, row: i64, col: i64) -> i64 {
        let mut i: i64 = 0;
        while i < (self.tables.len() as i64) {
            let t: Rc<RefCell<SheetTable>> = self.tables[i as usize].clone();
            if t.borrow().contains(row, col) {
                return i;
            }
            i += 1;
        }
        -1
    }
    pub fn tableCount(&self) -> i64 {
        self.tables.len() as i64
    }
    pub fn tableAt(&self, i: i64) -> Rc<RefCell<SheetTable>> {
        if i < 0 {
            return Rc::new(RefCell::new(SheetTable::new()));
        }
        if i >= (self.tables.len() as i64) {
            return Rc::new(RefCell::new(SheetTable::new()));
        }
        self.tables[i as usize].clone()
    }
    pub fn clearTables(&mut self) {
        self.tables.clear();
    }
    pub fn imageAt(&self, i: i64) -> Rc<RefCell<SheetImage>> {
        if i < 0 {
            return Rc::new(RefCell::new(SheetImage::new()));
        }
        if i >= (self.images.len() as i64) {
            return Rc::new(RefCell::new(SheetImage::new()));
        }
        self.images[i as usize].clone()
    }
    /// --- rich text ------------------------------------------------------------
    /// Setting runs also sets the cell's value to what they spell, so a formula
    /// reading the cell and a paste copying it see the plain string.
    pub fn setRichText(&mut self, row: i64, col: i64, rt: Rc<RefCell<RichText>>) {
        let k: String = self.cellKey(row, col);
        if !self.richCells.contains_key(&k) {
            self.richKeys.push(k.clone());
        }
        self.richCells.insert(k.clone(), rt.clone());
        self.version += 1;
    }
    pub fn richTextAt(&self, row: i64, col: i64) -> Rc<RefCell<RichText>> {
        let k: String = self.cellKey(row, col);
        if self.richCells.contains_key(&k) {
            return self.richCells.get(&k).cloned().unwrap();
        }
        Rc::new(RefCell::new(RichText::new()))
    }
    pub fn hasRichText(&self, row: i64, col: i64) -> bool {
        let rt: Rc<RefCell<RichText>> = self.richTextAt(row, col);
        let out: bool = rt.borrow().isRich();
        out
    }
    pub fn clearRichText(&mut self, row: i64, col: i64) {
        let k: String = self.cellKey(row, col);
        if self.richCells.contains_key(&k) {
            self.richCells.insert(k.clone(), Rc::new(RefCell::new(RichText::new())));
        }
    }
    pub fn richCount(&self) -> i64 {
        let mut n: i64 = 0;
        let mut i: i64 = 0;
        while i < (self.richKeys.len() as i64) {
            let k: String = self.richKeys[i as usize].clone();
            if self.richCells.contains_key(&k) {
                let rt: Rc<RefCell<RichText>> = self.richCells.get(&k).cloned().unwrap();
                if rt.borrow().isRich() {
                    n += 1;
                }
            }
            i += 1;
        }
        n
    }
    /// --- spilled array results ------------------------------------------------
    pub fn setSpill(&mut self, row: i64, col: i64, rows: i64, cols: i64) {
        let k: String = self.cellKey(row, col);
        if !self.spillSize.contains_key(&k) {
            self.spillKeys.push(k.clone());
        }
        self.spillSize.insert(k.clone(), format!("{},{}", rows, cols));
        let mut r: i64 = 0;
        while r < rows {
            let mut c: i64 = 0;
            while c < cols {
                let ck: String = self.cellKey(row + r, col + c);
                if !self.spillOwner.contains_key(&ck) {
                    self.spillKeys.push(ck.clone());
                }
                self.spillOwner.insert(ck.clone(), k.clone());
                c += 1;
            }
            r += 1;
        }
    }
    pub fn isSpilled(&self, row: i64, col: i64) -> bool {
        let k: String = self.cellKey(row, col);
        if let Some(owner) = self.spillOwner.get(&k) {
            return DgNative::str_len(owner) > 0;
        }
        false
    }
    /// The anchor a spilled cell belongs to, or "" — what an editor asks before
    /// letting someone type into a cell a formula is filling.
    pub fn spillAnchor(&self, row: i64, col: i64) -> String {
        let k: String = self.cellKey(row, col);
        if self.spillOwner.contains_key(&k) {
            return self.spillOwner.get(&k).cloned().unwrap();
        }
        String::new()
    }
    /// Take back everything the anchor at (row, col) last laid out.
    pub fn clearSpillFrom(&mut self, row: i64, col: i64) {
        let k: String = self.cellKey(row, col);
        if !self.spillSize.contains_key(&k) {
            return;
        }
        let dims: String = self.spillSize.get(&k).cloned().unwrap();
        if dims.is_empty() {
            return;
        }
        let parts: Vec<String> = dims.split(",").map(|s| s.to_string()).collect::<Vec<String>>();
        if (parts.len() as i64) < 2 {
            return;
        }
        let rows: i64 = SpreadsheetModel::parseInt0(&parts[0].clone());
        let cols: i64 = SpreadsheetModel::parseInt0(&parts[1].clone());
        let mut r: i64 = 0;
        while r < rows {
            let mut c: i64 = 0;
            while c < cols {
                let ck: String = self.cellKey(row + r, col + c);
                if self.spillOwner.contains_key(&ck) {
                    self.spillOwner.insert(ck.clone(), String::new());
                }
                if r + c > 0 {
                    self.setCellRaw(row + r, col + c, "");
                }
                c += 1;
            }
            r += 1;
        }
        self.spillSize.insert(k.clone(), String::new());
    }
    /// --- hyperlinks -----------------------------------------------------------
    pub fn setHyperlink(&mut self, row: i64, col: i64, target: &str) {
        let k: String = self.cellKey(row, col);
        if target.is_empty() {
            self.clearHyperlink(row, col);
            return;
        }
        if !self.links.contains_key(&k) {
            self.linkKeys.push(k.clone());
        }
        self.links.insert(k.clone(), target.to_string());
        self.version += 1;
    }
    pub fn hyperlinkAt(&self, row: i64, col: i64) -> String {
        let k: String = self.cellKey(row, col);
        if self.links.contains_key(&k) {
            return self.links.get(&k).cloned().unwrap();
        }
        String::new()
    }
    pub fn hasHyperlink(&self, row: i64, col: i64) -> bool {
        DgNative::str_len(&self.hyperlinkAt(row, col)) > 0
    }
    pub fn clearHyperlink(&mut self, row: i64, col: i64) {
        let k: String = self.cellKey(row, col);
        if self.links.contains_key(&k) {
            self.links.insert(k.clone(), String::new());
        }
        self.version += 1;
    }
    pub fn hyperlinkCount(&self) -> i64 {
        let mut n: i64 = 0;
        let mut i: i64 = 0;
        while i < (self.linkKeys.len() as i64) {
            let k: String = self.linkKeys[i as usize].clone();
            if let Some(v) = self.links.get(&k) {
                if !v.is_empty() {
                    n += 1;
                }
            }
            i += 1;
        }
        n
    }
    /// --- notes ----------------------------------------------------------------
    pub fn setNote(&mut self, row: i64, col: i64, text: &str) {
        let k: String = self.cellKey(row, col);
        if text.is_empty() {
            self.clearNote(row, col);
            return;
        }
        if !self.notes.contains_key(&k) {
            self.noteKeys.push(k.clone());
        }
        self.notes.insert(k.clone(), text.to_string());
        self.version += 1;
    }
    pub fn noteAt(&self, row: i64, col: i64) -> String {
        let k: String = self.cellKey(row, col);
        if self.notes.contains_key(&k) {
            return self.notes.get(&k).cloned().unwrap();
        }
        String::new()
    }
    pub fn hasNote(&self, row: i64, col: i64) -> bool {
        DgNative::str_len(&self.noteAt(row, col)) > 0
    }
    pub fn clearNote(&mut self, row: i64, col: i64) {
        let k: String = self.cellKey(row, col);
        if self.notes.contains_key(&k) {
            self.notes.insert(k.clone(), String::new());
        }
        self.version += 1;
    }
    pub fn noteCount(&self) -> i64 {
        let mut n: i64 = 0;
        let mut i: i64 = 0;
        while i < (self.noteKeys.len() as i64) {
            let k: String = self.noteKeys[i as usize].clone();
            if let Some(v) = self.notes.get(&k) {
                if !v.is_empty() {
                    n += 1;
                }
            }
            i += 1;
        }
        n
    }
    /// --- threaded comments --------------------------------------------------------
    pub fn threadAt(&self, row: i64, col: i64) -> String {
        let k: String = self.cellKey(row, col);
        if self.threads.contains_key(&k) {
            return self.threads.get(&k).cloned().unwrap();
        }
        String::new()
    }
    pub fn hasThread(&self, row: i64, col: i64) -> bool {
        DgNative::str_len(&self.threadAt(row, col)) > 0
    }
    pub fn addReply(&mut self, row: i64, col: i64, author: &str, text: &str) {
        let k: String = self.cellKey(row, col);
        let was: String = self.threadAt(row, col);
        if !self.threads.contains_key(&k) {
            self.threadKeys.push(k.clone());
        }
        let line: String = format!("{}: {}", author, text);
        if !was.is_empty() {
            self.threads.insert(k.clone(), format!("{}\n{}", was, line));
        } else {
            self.threads.insert(k.clone(), line.clone());
        }
        self.version += 1;
    }
    pub fn clearThread(&mut self, row: i64, col: i64) {
        let k: String = self.cellKey(row, col);
        if self.threads.contains_key(&k) {
            self.threads.insert(k.clone(), String::new());
        }
        self.version += 1;
    }
    pub fn noteShown(&self, row: i64, col: i64) -> bool {
        if !self.hasNote(row, col) {
            return false;
        }
        if self.showAllNotes {
            return true;
        }
        let k: String = self.cellKey(row, col);
        if self.shownNotes.contains_key(&k) {
            return self.shownNotes.get(&k).cloned().unwrap();
        }
        false
    }
    pub fn setNoteShown(&mut self, row: i64, col: i64, on: bool) {
        self.shownNotes.insert(self.cellKey(row, col), on);
        self.version += 1;
    }
    /// --- data validation ------------------------------------------------------
    /// Rules are attached to ranges, so the last one that covers a cell wins —
    /// the same order Excel resolves overlapping rules in.
    pub fn addValidation(&mut self, v: Rc<RefCell<DataValidation>>) {
        self.validations.push(v.clone());
        self.version += 1;
    }
    pub fn validationAt(&self, row: i64, col: i64) -> Rc<RefCell<DataValidation>> {
        let mut i: i64 = (self.validations.len() as i64) - 1;
        while i >= 0 {
            let v: Rc<RefCell<DataValidation>> = self.validations[i as usize].clone();
            if v.borrow().covers(row, col) {
                return v;
            }
            i -= 1;
        }
        Rc::new(RefCell::new(DataValidation::new()))
    }
    pub fn hasValidation(&self, row: i64, col: i64) -> bool {
        let v: Rc<RefCell<DataValidation>> = self.validationAt(row, col);
        let out: bool = !v.borrow().kind.is_empty();
        out
    }
    pub fn validationCount(&self) -> i64 {
        self.validations.len() as i64
    }
    pub fn clearValidationsIn(&mut self, r0: i64, c0: i64, r1: i64, c1: i64) {
        let mut keep: Vec<Rc<RefCell<DataValidation>>> = Vec::new();
        let mut i: i64 = 0;
        while i < (self.validations.len() as i64) {
            let v: Rc<RefCell<DataValidation>> = self.validations[i as usize].clone();
            let mut inside: bool = true;
            if v.borrow().r0 < r0 {
                inside = false;
            }
            if v.borrow().c0 < c0 {
                inside = false;
            }
            if v.borrow().r0 > r1 {
                inside = false;
            }
            if v.borrow().c0 > c1 {
                inside = false;
            }
            if !inside {
                keep.push(v.clone());
            }
            i += 1;
        }
        self.validations.clear();
        let mut k: i64 = 0;
        while k < (keep.len() as i64) {
            self.validations.push(keep[k as usize].clone());
            k += 1;
        }
        self.version += 1;
    }
    pub fn cfSnapshot(&self) -> Vec<Rc<RefCell<CfRule>>> {
        SpreadsheetModel::copyCfRules(&self.cfRules.clone())
    }
    /// `row`, `col` is where the selection goes back to on undo.
    pub fn pushCfOp(&self, before: &Vec<Rc<RefCell<CfRule>>>, row: i64, col: i64) {
        let op: Rc<RefCell<SpreadsheetUndoOp>> = Rc::new(RefCell::new(SpreadsheetUndoOp::new()));
        op.borrow_mut().cfSwap = true;
        op.borrow_mut().row = row;
        op.borrow_mut().col = col;
        op.borrow_mut().cfBefore = SpreadsheetModel::copyCfRules(before);
        op.borrow_mut().cfAfter = self.cfSnapshot();
        self.history.borrow_mut().record(op.clone());
    }
    pub fn cfRuleAt(&self, i: i64) -> Rc<RefCell<CfRule>> {
        self.cfRules[i as usize].clone()
    }
    pub fn replaceCfRule(&mut self, i: i64, rule: &Rc<RefCell<CfRule>>) {
        if i < 0 || i >= (self.cfRules.len() as i64) {
            return;
        }
        self.cfRules[i as usize] = rule.clone();
        self.version += 1;
    }
    pub fn removeCfRuleAt(&mut self, i: i64) {
        if i < 0 || i >= (self.cfRules.len() as i64) {
            return;
        }
        let mut keep: Vec<Rc<RefCell<CfRule>>> = Vec::new();
        let mut k: i64 = 0;
        while k < (self.cfRules.len() as i64) {
            if k != i {
                keep.push(self.cfRules[k as usize].clone());
            }
            k += 1;
        }
        self.cfRules = keep.clone();
        self.version += 1;
    }
    pub fn addCfRule(&mut self, rule: Rc<RefCell<CfRule>>) {
        self.cfRules.push(rule.clone());
        self.version += 1;
    }
    pub fn cfRuleCount(&self) -> i64 {
        self.cfRules.len() as i64
    }
    /// Drop every rule whose range starts inside the given rectangle — what
    /// "clear the rules on this selection" means.
    pub fn clearCfRulesIn(&mut self, r0: i64, c0: i64, r1: i64, c1: i64) {
        let mut keep: Vec<Rc<RefCell<CfRule>>> = Vec::new();
        let mut i: i64 = 0;
        while i < (self.cfRules.len() as i64) {
            let rule: Rc<RefCell<CfRule>> = self.cfRules[i as usize].clone();
            let mut inside: bool = true;
            if rule.borrow().r0 < r0 {
                inside = false;
            }
            if rule.borrow().c0 < c0 {
                inside = false;
            }
            if rule.borrow().r0 > r1 {
                inside = false;
            }
            if rule.borrow().c0 > c1 {
                inside = false;
            }
            if !inside {
                keep.push(rule.clone());
            }
            i += 1;
        }
        self.cfRules.clear();
        let mut k: i64 = 0;
        while k < (keep.len() as i64) {
            self.cfRules.push(keep[k as usize].clone());
            k += 1;
        }
        self.version += 1;
    }
    pub fn setStylesFrom(&mut self, list: &Vec<Rc<RefCell<CellStyle>>>) {
        self.styles.clear();
        // The first entry is the workbook's own default format, which is what
        // a cell naming no style is drawn in.
        if !list.is_empty() {
            let base: Rc<RefCell<CellStyle>> = list[0].clone();
            if base.borrow().fontSizePt > 0.0 {
                self.defaultFontPt = base.borrow().fontSizePt;
            }
        }
        let mut i: i64 = 0;
        let n: i64 = list.len() as i64;
        while i < n {
            self.styles.push(list[i as usize].clone());
            i += 1;
        }
    }
    pub fn setFreeze(&mut self, mut rows: i64, mut cols: i64) {
        if rows < 0 {
            rows = 0;
        }
        if cols < 0 {
            cols = 0;
        }
        self.freezeRows = rows;
        self.freezeCols = cols;
        self.version += 1;
    }
    pub fn frozenHeight(&mut self) -> i64 {
        self.ensureGeometry();
        if self.freezeRows <= 0 {
            return 0;
        }
        if self.freezeRows >= self.rowCount {
            return self.totalHeight();
        }
        self.rowToY(self.freezeRows.clone())
    }
    pub fn frozenWidth(&mut self) -> i64 {
        self.ensureGeometry();
        if self.freezeCols <= 0 {
            return 0;
        }
        if self.freezeCols >= self.colCount {
            return self.totalWidth();
        }
        self.colToX(self.freezeCols.clone())
    }
    pub fn getFormula(&self, row: i64, col: i64) -> String {
        let k: String = self.cellKey(row, col);
        if self.formulas.contains_key(&k) {
            return self.formulas.get(&k).cloned().unwrap();
        }
        String::new()
    }
    pub fn setFormula(&mut self, row: i64, col: i64, formula: &str) {
        let k: String = self.cellKey(row, col);
        if self.formulas.contains_key(&k) {
        } else {
            self.formulaKeys.push(k.clone());
        }
        self.formulas.insert(k.clone(), formula.to_string());
    }
    pub fn clearFormula(&mut self, row: i64, col: i64) {
        let k: String = self.cellKey(row, col);
        if self.formulas.contains_key(&k) {
            self.formulas.insert(k.clone(), String::new());
        }
    }
    /// setFormula that never creates a key for an empty formula.
    pub fn setFormulaText(&mut self, row: i64, col: i64, formula: &str) {
        let k: String = self.cellKey(row, col);
        if self.formulas.contains_key(&k) {
            self.formulas.insert(k.clone(), formula.to_string());
            return;
        }
        if formula.is_empty() {
            return;
        }
        self.formulaKeys.push(k.clone());
        self.formulas.insert(k.clone(), formula.to_string());
    }
    pub fn hasFormula(&self, row: i64, col: i64) -> bool {
        let k: String = self.cellKey(row, col);
        if self.formulas.contains_key(&k) {
            let f: String = self.formulas.get(&k).cloned().unwrap();
            if !f.is_empty() {
                return true;
            }
        }
        false
    }
    /// Cleared cells keep a "" entry in `formulas`; skip them so the recalc
    /// scan does not grow with every deleted formula.
    pub fn collectFormulaKeys(&self, sheet_idx: i64) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for k in self.formulaKeys.iter() {
            let key: String = k.clone();
            if let Some(f) = self.formulas.get(&key) {
                if !f.is_empty() {
                    out.push(format!("{}!{}", sheet_idx, key));
                }
            }
        }
        out
    }
    pub fn setRowHidden(&mut self, row: i64, hidden: bool) {
        self.ensureGeometry();
        if row < 0 || row >= self.rowCount {
            return;
        }
        self.rowHidden[row as usize] = hidden;
        self.rowGeomDirty = true;
        self.version += 1;
    }
    pub fn setColHidden(&mut self, col: i64, hidden: bool) {
        self.ensureGeometry();
        if col < 0 || col >= self.colCount {
            return;
        }
        self.colHidden[col as usize] = hidden;
        self.colGeomDirty = true;
        self.version += 1;
    }
    pub fn isRowHidden(&mut self, row: i64) -> bool {
        self.ensureGeometry();
        if row < 0 || row >= (self.rowHidden.len() as i64) {
            return false;
        }
        self.rowHidden[row as usize]
    }
    pub fn isColHidden(&mut self, col: i64) -> bool {
        self.ensureGeometry();
        if col < 0 || col >= (self.colHidden.len() as i64) {
            return false;
        }
        self.colHidden[col as usize]
    }
    pub fn filterActiveOn(&self, col: i64) -> bool {
        self.filterEnabled && self.filterCol == col
    }
    pub fn setFilter(&mut self, col: i64, enabled: bool, values: &[String]) {
        self.filterCol = col;
        self.filterEnabled = enabled;
        self.filterValues.clear();
        let mut i: i64 = 0;
        let n: i64 = values.len() as i64;
        while i < n {
            self.filterValues.push(values[i as usize].clone());
            i += 1;
        }
        self.ensureGeometry();
        if (self.rowSuppressed.len() as i64) != self.rowCount {
            self.rowSuppressed.clear();
            let mut hi: i64 = 0;
            while hi < self.rowCount {
                self.rowSuppressed.push(false);
                hi += 1;
            }
        }
        let mut r: i64 = 0;
        while r < self.rowCount {
            let mut suppress: bool = false;
            if enabled {
                if r >= self.freezeRows {
                    let v: String = self.getCell(r, col);
                    let mut ok: bool = false;
                    let mut j: i64 = 0;
                    while j < n {
                        if v == self.filterValues[j as usize].clone() {
                            ok = true;
                        }
                        j += 1;
                    }
                    if n == 0 {
                        ok = true;
                    }
                    if !ok {
                        suppress = true;
                    }
                }
            }
            self.rowSuppressed[r as usize] = suppress;
            r += 1;
        }
        self.rowGeomDirty = true;
        self.version += 1;
    }
    pub fn setSort(&mut self, col: i64, ascending: bool) {
        self.sortCol = col;
        self.sortAsc = ascending;
        self.version += 1;
    }
    /// Formula bar text: "=SUM(...)" when present, else display value.
    pub fn formulaBarText(&self, row: i64, col: i64) -> String {
        let f: String = self.getFormula(row, col);
        if !f.is_empty() {
            if DgNative::char_at(&f, 0) == 61 {
                return f;
            }
            return format!("={}", f);
        }
        self.getDisplay(row, col)
    }
    pub fn addMerge(&mut self, mut r1: i64, mut c1: i64, mut r2: i64, mut c2: i64) {
        if r2 < r1 {
            let t: i64 = r1;
            r1 = r2;
            r2 = t;
        }
        if c2 < c1 {
            let t2: i64 = c1;
            c1 = c2;
            c2 = t2;
        }
        let key: String = format!("{},{},{},{}", r1, c1, r2, c2);
        self.merges.push(key);
    }
    /// Take apart the merge that contains this cell, if any. The same button
    /// that made it undoes it, which is how every spreadsheet behaves.
    pub fn removeMerge(&mut self, row: i64, col: i64) -> bool {
        let key: String = self.mergeAt(row, col);
        if key.is_empty() {
            return false;
        }
        let mut kept: Vec<String> = Vec::new();
        let mut i: i64 = 0;
        while i < (self.merges.len() as i64) {
            let m: String = self.merges[i as usize].clone();
            if m != key {
                kept.push(m.clone());
            }
            i += 1;
        }
        self.merges.clear();
        let mut j: i64 = 0;
        while j < (kept.len() as i64) {
            self.merges.push(kept[j as usize].clone());
            j += 1;
        }
        self.version += 1;
        true
    }
    /// Returns merge key containing (row,col), or "".
    pub fn mergeAt(&self, row: i64, col: i64) -> String {
        let mut i: i64 = 0;
        let n: i64 = self.merges.len() as i64;
        while i < n {
            let m: String = self.merges[i as usize].clone();
            let parts: Vec<String> = m.split(",").map(|s| s.to_string()).collect::<Vec<String>>();
            if (parts.len() as i64) >= 4 {
                let r1: i64 = SpreadsheetModel::parseInt0(&parts[0].clone());
                let c1: i64 = SpreadsheetModel::parseInt0(&parts[1].clone());
                let r2: i64 = SpreadsheetModel::parseInt0(&parts[2].clone());
                let c2: i64 = SpreadsheetModel::parseInt0(&parts[3].clone());
                if row >= r1 && row <= r2 && col >= c1 && col <= c2 {
                    return m;
                }
            }
            i += 1;
        }
        String::new()
    }
    pub fn isMergeOrigin(&self, row: i64, col: i64) -> bool {
        let m: String = self.mergeAt(row, col);
        if m.is_empty() {
            return true;
        }
        let parts: Vec<String> = m.split(",").map(|s| s.to_string()).collect::<Vec<String>>();
        let r1: i64 = SpreadsheetModel::parseInt0(&parts[0].clone());
        let c1: i64 = SpreadsheetModel::parseInt0(&parts[1].clone());
        if row != r1 {
            return false;
        }
        col == c1
    }
    pub fn setCellRaw(&mut self, row: i64, col: i64, value: &str) {
        if row < 0 {
            return;
        }
        if col < 0 {
            return;
        }
        self.ensureCellRow(row);
        let mut m: HashMap<i64,String> = self.cellRows[row as usize].clone();
        m.insert(col, value.to_string());
        self.cellRows[row as usize] = m.clone();
        // Used range, for Ctrl+End. Like Excel it only grows: clearing cells
        // does not shrink it, and it stays O(1) instead of rescanning a sheet
        // that may declare 100k rows.
        if !value.is_empty() {
            if row > self.usedMaxRow {
                self.usedMaxRow = row;
            }
            if col > self.usedMaxCol {
                self.usedMaxCol = col;
            }
        }
        if !self.quietWrites {
            self.version += 1;
        }
    }
    /// Bottom-right of the used range, clamped to the sheet.
    pub fn usedRow(&self) -> i64 {
        let mut r: i64 = self.usedMaxRow;
        if r >= self.rowCount {
            r = self.rowCount - 1;
        }
        if r < 0 {
            r = 0;
        }
        r
    }
    pub fn usedCol(&self) -> i64 {
        let mut c: i64 = self.usedMaxCol;
        if c >= self.colCount {
            c = self.colCount - 1;
        }
        if c < 0 {
            c = 0;
        }
        c
    }
    /// --- pages -----------------------------------------------------------------
    /// The printable area of one page in CSS pixels (96 per inch): the paper,
    /// turned for landscape, less the margins. Fit-to-width / height makes that
    /// direction one page.
    pub fn pagePrintable(&self) -> Vec<f64> {
        let mut pw: f64 = 8.5;
        let mut ph: f64 = 11.0;
        if self.paperSize == 9 {
            pw = 8.27;
            ph = 11.69;
        }
        if self.paperSize == 5 {
            ph = 14.0;
        }
        if self.paperSize == 8 {
            pw = 11.69;
            ph = 16.54;
        }
        if self.pageOrientation == "landscape" {
            let t: f64 = pw;
            pw = ph;
            ph = t;
        }
        let mut out: Vec<f64> = Vec::new();
        let mut w: f64 = ((pw - self.marginLeft) - self.marginRight) * 96.0;
        let mut h: f64 = ((ph - self.marginTop) - self.marginBottom) * 96.0;
        if self.fitToWidth > 0 {
            w = 1000000.0;
        }
        if self.fitToHeight > 0 {
            h = 1000000.0;
        }
        out.push(w);
        out.push(h);
        out
    }
    /// Where each page after the first starts: rows (or columns) of the used
    /// range filled at 100 % until the next would not fit, plus manual breaks.
    pub fn pageBreaks(&mut self, cols: bool) -> Vec<i64> {
        let room: Vec<f64> = self.pagePrintable();
        let mut cap: f64 = room[1];
        let mut last: i64 = self.usedRow();
        if cols {
            cap = room[0];
            last = self.usedCol();
        }
        let mut out: Vec<i64> = Vec::new();
        let mut acc: f64 = 0.0;
        let mut i: i64 = 0;
        while i <= last {
            let mut sz: f64 = (self.rawRowHeight(i)) as f64;
            if cols {
                sz = (self.rawColWidth(i)) as f64;
                if self.isColHidden(i) {
                    sz = 0.0;
                }
            } else {
                if self.isRowHidden(i) {
                    sz = 0.0;
                }
            }
            let mut manual: bool = false;
            if !cols {
                let mut k: i64 = 0;
                while k < (self.rowBreaks.len() as i64) {
                    if self.rowBreaks[k as usize] == i {
                        manual = true;
                    }
                    k += 1;
                }
            }
            if i > 0 && (manual || acc + sz > cap) {
                out.push(i);
                acc = 0.0;
            }
            acc += sz;
            i += 1;
        }
        out
    }
    /// --- transactions -------------------------------------------------------
    /// Ops recorded between beginTx / endTx undo as one Excel-style action.
    /// Nesting is counted so a paste inside a larger op stays one transaction.
    /// What counts as one action, and how much falls off the bottom at the cap
    /// — `gallery/office/editor`, shared with the document editor. These rules
    /// were right here and missing there, which is how a paste of three lines
    /// in a .docx became five undos that never got back to the start.
    pub fn beginTx(&self) {
        self.history.borrow_mut().begin();
    }
    pub fn endTx(&self) {
        self.history.borrow_mut().end();
    }
    pub fn pushUndoOp(&self, row: i64, col: i64, before: String, after: String, before_f: String, after_f: String) {
        let op: Rc<RefCell<SpreadsheetUndoOp>> = Rc::new(RefCell::new(SpreadsheetUndoOp::new()));
        op.borrow_mut().row = row;
        op.borrow_mut().col = col;
        op.borrow_mut().before = before.clone();
        op.borrow_mut().after = after.clone();
        op.borrow_mut().beforeF = before_f.clone();
        op.borrow_mut().afterF = after_f.clone();
        // Stamped with the action it belongs to, never trimmed mid-action,
        // and the redo stack dropped because a new edit makes it unreachable.
        self.history.borrow_mut().record(op.clone());
    }
    /// Tracked edit of value + formula together. formula "" means "no formula",
    /// so a plain value overwrites any prior formula (Excel behaviour).
    pub fn applyEdit(&mut self, row: i64, col: i64, value: &str, formula: &str) {
        let before: String = self.getCell(row, col);
        let before_f: String = self.getFormula(row, col);
        if before == value && before_f == formula {
            return;
        }
        self.pushUndoOp(row,
            col,
            before.clone(),
            value.to_string(),
            before_f.clone(),
            formula.to_string()
        );
        self.setCellRaw(row, col, value);
        self.setFormulaText(row, col, formula);
    }
    /// Value + formula + formatting in ONE undo op — what a paste writes.
    pub fn applyEditStyled(&mut self, row: i64, col: i64, value: &str, formula: &str, st: &Rc<RefCell<CellStyle>>) {
        let before: String = self.getCell(row, col);
        let before_f: String = self.getFormula(row, col);
        let before_sid: i64 = self.getCellStyleId(row, col);
        let after_sid: i64 = self.styleIdFor(st);
        if (before == value && before_f == formula) && before_sid == after_sid {
            return;
        }
        self.pushUndoOp(row,
            col,
            before.clone(),
            value.to_string(),
            before_f.clone(),
            formula.to_string()
        );
        let n: i64 = self.history.borrow().depth();
        if n > 0 {
            let op: Rc<RefCell<SpreadsheetUndoOp>> = self.history.borrow().peekUndo();
            op.borrow_mut().beforeStyle = before_sid;
            op.borrow_mut().afterStyle = after_sid;
            op.borrow_mut().touchStyle = true;
        }
        self.setCellRaw(row, col, value);
        self.setFormulaText(row, col, formula);
        let k: String = self.cellKey(row, col);
        self.cellStyleIds.insert(k.clone(), after_sid);
    }
    pub fn setCell(&mut self, row: i64, col: i64, value: &str) {
        self.applyEdit(row, col, value, "");
    }
    /// Retyping a cell drops the pieces its text was drawn in — a spreadsheet
    /// does the same, because there is no answer to which run new text belongs
    /// to.
    pub fn applyEditPlain(&mut self, row: i64, col: i64, value: &str, formula: &str) {
        self.clearRichText(row, col);
        self.applyEdit(row, col, value, formula);
    }
    /// Record a structural column op so it can be undone as one action.
    /// A sort, as one undoable step. The order is kept whole rather than as a
    /// sequence of moves: a sort is one action to the person who asked for it,
    /// and one Ctrl+Z has to put the sheet back the way it was.
    pub fn pushReorderOp(&self, order: &[i64]) {
        let op: Rc<RefCell<SpreadsheetUndoOp>> = Rc::new(RefCell::new(SpreadsheetUndoOp::new()));
        op.borrow_mut().structKind = SheetStructKind::reorderRows();
        let mut i: i64 = 0;
        while i < (order.len() as i64) {
            op.borrow_mut().structOrder.push(order[i as usize]);
            i += 1;
        }
        op.borrow_mut().savedKeys = self.formulaSnapshotKeys();
        op.borrow_mut().savedTexts = self.formulaSnapshotTexts();
        self.history.borrow_mut().record(op.clone());
    }
    pub fn pushStructOp(&mut self, kind: i64, at: i64, to: i64) {
        let op: Rc<RefCell<SpreadsheetUndoOp>> = Rc::new(RefCell::new(SpreadsheetUndoOp::new()));
        op.borrow_mut().structKind = kind;
        op.borrow_mut().structAt = at;
        op.borrow_mut().structTo = to;
        if SheetStructKind::isColumnOp(kind) {
            op.borrow_mut().colWidthSaved = self.colWidth(at);
        } else {
            op.borrow_mut().colWidthSaved = self.rowHeight(at);
        }
        if kind == 2 {
            let snap = self.snapshotColumn(at);
            op.borrow_mut().colValues = snap.values;
            op.borrow_mut().colFormulas = snap.formulas;
            op.borrow_mut().colStyles = snap.styles;
        }
        if kind == 5 {
            let snap = self.snapshotRow(at);
            op.borrow_mut().colValues = snap.values;
            op.borrow_mut().colFormulas = snap.formulas;
            op.borrow_mut().colStyles = snap.styles;
        }
        op.borrow_mut().savedKeys = self.formulaSnapshotKeys();
        op.borrow_mut().savedTexts = self.formulaSnapshotTexts();
        self.history.borrow_mut().record(op.clone());
    }
    pub fn applyStructOp(&mut self, op: &Rc<RefCell<SpreadsheetUndoOp>>, to_after: bool) {
        if to_after {
            if op.borrow().structKind == SheetStructKind::insertColumn() {
                self.insertColumnRaw(op.borrow().structAt.clone());
            }
            if op.borrow().structKind == SheetStructKind::deleteColumn() {
                self.deleteColumnRaw(op.borrow().structAt.clone());
            }
            if op.borrow().structKind == SheetStructKind::moveColumn() {
                let mut _tmp_439 = op.borrow().structAt.clone();
                self.moveColumnRaw(_tmp_439, op.borrow().structTo.clone());
            }
            if op.borrow().structKind == SheetStructKind::insertRow() {
                self.insertRowRaw(op.borrow().structAt.clone());
            }
            if op.borrow().structKind == SheetStructKind::deleteRow() {
                self.deleteRowRaw(op.borrow().structAt.clone());
            }
            if op.borrow().structKind == SheetStructKind::moveRow() {
                let mut _tmp_443 = op.borrow().structAt.clone();
                self.moveRowRaw(_tmp_443, op.borrow().structTo.clone());
            }
            if op.borrow().structKind == SheetStructKind::reorderRows() {
                self.reorderRowsRaw(&op.borrow().structOrder.clone());
            }
            return;
        }
        if op.borrow().structKind == SheetStructKind::insertColumn() {
            self.deleteColumnRaw(op.borrow().structAt.clone());
        }
        if op.borrow().structKind == SheetStructKind::deleteColumn() {
            self.insertColumnRaw(op.borrow().structAt.clone());
            let mut _tmp_448 = op.borrow().structAt.clone();
            let mut _tmp_449 = op.borrow().colValues.clone();
            let mut _tmp_450 = op.borrow().colFormulas.clone();
            self.restoreColumn(_tmp_448, &_tmp_449, &_tmp_450, &op.borrow().colStyles.clone());
            let mut _tmp_452 = op.borrow().structAt.clone();
            self.setColWidth(_tmp_452, op.borrow().colWidthSaved.clone());
        }
        if op.borrow().structKind == SheetStructKind::moveColumn() {
            let mut _tmp_454 = op.borrow().structTo.clone();
            self.moveColumnRaw(_tmp_454, op.borrow().structAt.clone());
        }
        if op.borrow().structKind == SheetStructKind::insertRow() {
            self.deleteRowRaw(op.borrow().structAt.clone());
        }
        if op.borrow().structKind == SheetStructKind::deleteRow() {
            self.insertRowRaw(op.borrow().structAt.clone());
            let mut _tmp_458 = op.borrow().structAt.clone();
            let mut _tmp_459 = op.borrow().colValues.clone();
            let mut _tmp_460 = op.borrow().colFormulas.clone();
            self.restoreRow(_tmp_458, &_tmp_459, &_tmp_460, &op.borrow().colStyles.clone());
            let mut _tmp_462 = op.borrow().structAt.clone();
            self.setRowHeight(_tmp_462, op.borrow().colWidthSaved.clone());
        }
        if op.borrow().structKind == SheetStructKind::moveRow() {
            let mut _tmp_464 = op.borrow().structTo.clone();
            self.moveRowRaw(_tmp_464, op.borrow().structAt.clone());
        }
        if op.borrow().structKind == SheetStructKind::reorderRows() {
            // The inverse permutation: whatever went to i came from
            // structOrder[i], so to put it back, row structOrder[i] takes i.
            let mut inv: Vec<i64> = Vec::new();
            let zn: i64 = op.borrow().structOrder.len() as i64;
            let mut z: i64 = 0;
            while z < zn {
                inv.push(0);
                z += 1;
            }
            z = 0;
            while z < zn {
                let came: i64 = op.borrow().structOrder[z as usize];
                if came >= 0 {
                    if came < zn {
                        inv[came as usize] = z;
                    }
                }
                z += 1;
            }
            self.reorderRowsRaw(&inv);
        }
        // Formula text is restored wholesale — see savedKeys.
        let mut _tmp_466 = op.borrow().savedKeys.clone();
        self.restoreFormulas(&_tmp_466, &op.borrow().savedTexts.clone());
    }
    pub fn applyOp(&mut self, op: &Rc<RefCell<SpreadsheetUndoOp>>, to_after: bool) {
        if op.borrow().cfSwap {
            if to_after {
                self.cfRules = SpreadsheetModel::copyCfRules(&op.borrow().cfAfter.clone());
            } else {
                self.cfRules = SpreadsheetModel::copyCfRules(&op.borrow().cfBefore.clone());
            }
            self.version += 1;
            return;
        }
        if op.borrow().structKind != SheetStructKind::none() {
            self.applyStructOp(op, to_after);
            self.structDirty = true;
            return;
        }
        if to_after {
            let mut _tmp_472 = op.borrow().row.clone();
            let mut _tmp_473 = op.borrow().col.clone();
            self.setCellRaw(_tmp_472, _tmp_473, &op.borrow().after.clone());
            let mut _tmp_475 = op.borrow().row.clone();
            let mut _tmp_476 = op.borrow().col.clone();
            self.setFormulaText(_tmp_475, _tmp_476, &op.borrow().afterF.clone());
        } else {
            let mut _tmp_478 = op.borrow().row.clone();
            let mut _tmp_479 = op.borrow().col.clone();
            self.setCellRaw(_tmp_478, _tmp_479, &op.borrow().before.clone());
            let mut _tmp_481 = op.borrow().row.clone();
            let mut _tmp_482 = op.borrow().col.clone();
            self.setFormulaText(_tmp_481, _tmp_482, &op.borrow().beforeF.clone());
        }
        if op.borrow().touchStyle {
            let mut sid: i64 = op.borrow().beforeStyle;
            if to_after {
                sid = op.borrow().afterStyle;
            }
            let mut _tmp_484 = op.borrow().row.clone();
            let k: String = self.cellKey(_tmp_484, op.borrow().col.clone());
            self.cellStyleIds.insert(k.clone(), sid);
        }
        let mut _tmp_486 = op.borrow().row.clone();
        let mut _tmp_488 = CellPos::create(_tmp_486, op.borrow().col.clone());
        self.touched.push(_tmp_488);
    }
    /// Undo the whole last transaction (or the single last op). Cells restored
    /// are reported in `touched` so the caller can resync the formula engine.
    pub fn undo(&mut self) -> Rc<RefCell<CellPos>> {
        self.touched.clear();
        if !self.history.borrow().canUndo() {
            return CellPos::create(0, 0);
        }
        // One press is one action: the top op and every op below it stamped
        // with the same transaction. The rule is `gallery/office/editor`'s now
        // rather than a second copy of it written here.
        let span: i64 = self.history.borrow().undoSpan();
        let mut last_row: i64 = 0;
        let mut last_col: i64 = 0;
        let mut moved: i64 = 0;
        while moved < span {
            let op: Rc<RefCell<SpreadsheetUndoOp>> = self.history.borrow().peekUndo();
            // The op crosses to the redo side carrying its stamp, so the
            // action is still one press on the way back.
            self.history.borrow_mut().moveUndoToRedo();
            self.applyOp(&op, false);
            last_row = op.borrow().row;
            last_col = op.borrow().col;
            moved += 1;
        }
        CellPos::create(last_row, last_col)
    }
    pub fn redo(&mut self) -> Rc<RefCell<CellPos>> {
        self.touched.clear();
        if !self.history.borrow().canRedo() {
            return CellPos::create(0, 0);
        }
        let span: i64 = self.history.borrow().redoSpan();
        let mut last_row: i64 = 0;
        let mut last_col: i64 = 0;
        let mut moved: i64 = 0;
        while moved < span {
            let op: Rc<RefCell<SpreadsheetUndoOp>> = self.history.borrow().peekRedo();
            self.history.borrow_mut().moveRedoToUndo();
            self.applyOp(&op, true);
            last_row = op.borrow().row;
            last_col = op.borrow().col;
            moved += 1;
        }
        CellPos::create(last_row, last_col)
    }
    pub fn rowHeight(&mut self, row: i64) -> i64 {
        let raw: i64 = self.rawRowHeight(row);
        self.zoomed(raw)
    }
    pub fn colWidth(&mut self, col: i64) -> i64 {
        let raw: i64 = self.rawColWidth(col);
        self.zoomed(raw)
    }
    /// At 100 %: what the file stores and the writer writes.
    pub fn rawRowHeight(&mut self, row: i64) -> i64 {
        self.ensureSizeArrays();
        if row < 0 {
            return self.defaultRowH;
        }
        if row >= self.rowCount {
            return self.defaultRowH;
        }
        self.rowHeights[row as usize]
    }
    pub fn rawColWidth(&mut self, col: i64) -> i64 {
        self.ensureSizeArrays();
        if col < 0 {
            return self.defaultColW;
        }
        if col >= self.colCount {
            return self.defaultColW;
        }
        self.colWidths[col as usize]
    }
    pub fn zoomed(&self, v: i64) -> i64 {
        if self.zoom == 1.0 {
            return v;
        }
        ((v as f64) * self.zoom + 0.5).floor() as i64
    }
    pub fn unzoomed(&self, v: i64) -> i64 {
        if self.zoom == 1.0 {
            return v;
        }
        ((v as f64) / self.zoom + 0.5).floor() as i64
    }
    /// 10 % .. 400 %, as Excel allows.
    pub fn setZoom(&mut self, z: f64) {
        let mut v: f64 = z;
        if v < 0.1 {
            v = 0.1;
        }
        if v > 4.0 {
            v = 4.0;
        }
        self.zoom = v;
        self.rowGeomDirty = true;
        self.colGeomDirty = true;
        self.version += 1;
    }
    /// The default row height and column width the FILE states, in pixels, as
    /// opposed to the ones this program would have chosen. A sheet designed at
    /// 12.75pt rows and 12.63-character columns, drawn at 20px and 64px, is a
    /// fifth taller and a third narrower than it was meant to be — enough that
    /// labels run into the cell beside them and numbers sit off their line.
    ///
    /// Rows and columns that already carry the OLD default follow it to the new
    /// one; anything the file gave an explicit size keeps it. Pass 0 for either
    /// to leave that one alone.
    pub fn setDefaultGeometry(&mut self, row_h: i64, col_w: i64) {
        let old_row: i64 = self.defaultRowH;
        let old_col: i64 = self.defaultColW;
        if row_h > 0 {
            self.defaultRowH = row_h;
        }
        if col_w > 0 {
            self.defaultColW = col_w;
        }
        let mut i: i64 = 0;
        while i < (self.rowHeights.len() as i64) {
            if self.rowHeights[i as usize] == old_row {
                self.rowHeights[i as usize] = self.defaultRowH;
            }
            i += 1;
        }
        let mut j: i64 = 0;
        while j < (self.colWidths.len() as i64) {
            if self.colWidths[j as usize] == old_col {
                self.colWidths[j as usize] = self.defaultColW;
            }
            j += 1;
        }
        self.rowGeomDirty = true;
        self.colGeomDirty = true;
    }
    pub fn setRowHeight(&mut self, row: i64, mut h: i64) {
        self.ensureSizeArrays();
        if row < 0 {
            return;
        }
        if row >= self.rowCount {
            return;
        }
        h = self.unzoomed(h);
        if h < 8 {
            h = 8;
        }
        if h > 400 {
            h = 400;
        }
        self.rowHeights[row as usize] = h;
        self.rowGeomDirty = true;
        if !self.quietWrites {
            self.version += 1;
        }
    }
    pub fn setColWidth(&mut self, col: i64, mut w: i64) {
        self.ensureSizeArrays();
        if col < 0 {
            return;
        }
        if col >= self.colCount {
            return;
        }
        w = self.unzoomed(w);
        if w < 24 {
            w = 24;
        }
        if w > 600 {
            w = 600;
        }
        self.colWidths[col as usize] = w;
        self.colGeomDirty = true;
        self.version += 1;
    }
    /// Content-space Y of the top edge of `row` (prefix sum).
    pub fn rowToY(&mut self, row: i64) -> i64 {
        self.ensureGeometry();
        if row <= 0 {
            return 0;
        }
        if row >= self.rowCount {
            return self.rowOffsets[(self.rowCount) as usize];
        }
        self.rowOffsets[row as usize]
    }
    pub fn colToX(&mut self, col: i64) -> i64 {
        self.ensureGeometry();
        if col <= 0 {
            return 0;
        }
        if col >= self.colCount {
            return self.colOffsets[(self.colCount) as usize];
        }
        self.colOffsets[col as usize]
    }
    pub fn totalHeight(&mut self) -> i64 {
        self.ensureGeometry();
        self.rowOffsets[(self.rowCount) as usize]
    }
    pub fn totalWidth(&mut self) -> i64 {
        self.ensureGeometry();
        self.colOffsets[(self.colCount) as usize]
    }
    /// Binary search on prefix offsets: content Y → row index.
    pub fn yToRow(&mut self, y: i64) -> i64 {
        self.ensureGeometry();
        if y <= 0 {
            return 0;
        }
        let total: i64 = self.rowOffsets[(self.rowCount) as usize];
        if y >= total {
            return self.rowCount - 1;
        }
        let mut lo: i64 = 0;
        let mut hi: i64 = self.rowCount - 1;
        while lo < hi {
            let mid: i64 = (((lo + hi) + 1) as f64 / 2.0).floor() as i64;
            if self.rowOffsets[mid as usize] <= y {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    }
    pub fn xToCol(&mut self, x: i64) -> i64 {
        self.ensureGeometry();
        if x <= 0 {
            return 0;
        }
        let total: i64 = self.colOffsets[(self.colCount) as usize];
        if x >= total {
            return self.colCount - 1;
        }
        let mut lo: i64 = 0;
        let mut hi: i64 = self.colCount - 1;
        while lo < hi {
            let mid: i64 = (((lo + hi) + 1) as f64 / 2.0).floor() as i64;
            if self.colOffsets[mid as usize] <= x {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    }
    pub fn clampPos(&self, p: &Rc<RefCell<CellPos>>) {
        if p.borrow().row < 0 {
            p.borrow_mut().row = 0;
        }
        if p.borrow().col < 0 {
            p.borrow_mut().col = 0;
        }
        if p.borrow().row >= self.rowCount {
            p.borrow_mut().row = self.rowCount - 1;
        }
        if p.borrow().col >= self.colCount {
            p.borrow_mut().col = self.colCount - 1;
        }
    }
    /// Seed an empty sheet: named Sheet1, sized like a new workbook, no sample data.
    pub fn seedBlank(&mut self) {
        self.clearSheet();
        self.name = "Sheet1".to_string();
        self.resize(1000, 26);
        self.history.borrow_mut().reset();
    }
    /// Seed a demo sheet: headers + a few sample rows. Sparse — rest stay empty.
    pub fn seedDemo(&mut self) {
        self.clearSheet();
        self.name = "Demo".to_string();
        self.resize(1000, 26);
        self.setCellRaw(0, 0, "Product");
        self.setCellRaw(0, 1, "Qty");
        self.setCellRaw(0, 2, "Price");
        self.setCellRaw(0, 3, "Total");
        self.setCellRaw(1, 0, "Widget");
        self.setCellRaw(1, 1, "12");
        self.setCellRaw(1, 2, "4.50");
        self.setCellRaw(1, 3, "54.00");
        self.setCellRaw(2, 0, "Gadget");
        self.setCellRaw(2, 1, "3");
        self.setCellRaw(2, 2, "19.90");
        self.setCellRaw(2, 3, "59.70");
        self.setCellRaw(3, 0, "Doohickey");
        self.setCellRaw(3, 1, "100");
        self.setCellRaw(3, 2, "0.25");
        self.setCellRaw(3, 3, "25.00");
        self.setCellRaw(5, 0, "Ranger EVG DataGrid");
        self.setCellRaw(5, 1, "virtualized");
        self.setCellRaw(5, 2, "batched");
        self.setCellRaw(6,
            0,
            "Type to replace, F2 to edit, Enter/Tab to step."
        );
        self.setColWidth(0, 160);
        self.setColWidth(3, 110);
        self.setRowHeight(0, 28);
        self.history.borrow_mut().reset();
    }
    /// Fill many cells for scroll benches without storing empty rows.
    pub fn seedBench(&mut self, rows: i64, cols: i64, fill_every: i64) {
        self.resize(rows, cols);
        let mut r: i64 = 0;
        while r < rows {
            if r - ((r as f64 / fill_every as f64).floor() as i64) * fill_every == 0 {
                let mut c: i64 = 0;
                while c < cols {
                    self.setCellRaw(r, c, &format!("R{}C{}", r, c));
                    c += 1;
                }
            }
            r += 1;
        }
        self.history.borrow_mut().reset();
    }
}
