// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::xlsx_number_format::XlsxNumberFormat;

/// CellStyle - resolved cell appearance (FortuneSheet "Formatting" subset).
///
/// XLSX style index → this struct → EVG fill/text. Number format is applied when
/// producing the display string via XlsxNumberFormat.
/// One cell edge from styles.xml <border>. style "" means no border.
/// OOXML styles: hair thin medium thick double dashed dotted dashDot
/// dashDotDot mediumDashed mediumDashDot mediumDashDotDot slantDashDot.
#[derive(Clone)]
pub struct CellBorder {
    pub style: String,
    pub rgb: String,
}

impl CellBorder {
    pub fn new() -> Self {
        Self {
            style: String::new(),
            rgb: "#000000".to_string(),
        }
    }
    pub fn has(&self) -> bool {
        !self.style.is_empty()
    }
    pub fn copyFrom(&mut self, o: &Rc<RefCell<CellBorder>>) {
        self.style = o.borrow().style.clone();
        self.rgb = o.borrow().rgb.clone();
    }
    pub fn sameAs(&self, o: &Rc<RefCell<CellBorder>>) -> bool {
        if self.style != o.borrow().style {
            return false;
        }
        self.rgb == o.borrow().rgb
    }
    /// Painted thickness in px.
    pub fn width(&self) -> i64 {
        if self.style == "thick" {
            return 3;
        }
        if self.style == "double" {
            return 3;
        }
        if self.style == "medium" {
            return 2;
        }
        if self.style == "mediumDashed" {
            return 2;
        }
        if self.style == "mediumDashDot" {
            return 2;
        }
        if self.style == "mediumDashDotDot" {
            return 2;
        }
        1
    }
    /// 0 = solid, 1 = dashed, 2 = dotted, 3 = double (two hairlines).
    pub fn pattern(&self) -> i64 {
        if self.style == "double" {
            return 3;
        }
        if self.style == "dotted" {
            return 2;
        }
        if self.style == "hair" {
            return 2;
        }
        if self.style == "dashed" {
            return 1;
        }
        if self.style == "dashDot" {
            return 1;
        }
        if self.style == "dashDotDot" {
            return 1;
        }
        if self.style == "mediumDashed" {
            return 1;
        }
        if self.style == "mediumDashDot" {
            return 1;
        }
        if self.style == "mediumDashDotDot" {
            return 1;
        }
        if self.style == "slantDashDot" {
            return 1;
        }
        0
    }
}

#[derive(Clone)]
pub struct CellStyle {
    pub fillRgb: String,
    pub textRgb: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub fontSizePt: f64,
    /// Typeface name as OOXML writes it (`<name val="Calibri"/>`). Empty means
    /// the workbook default; the painter falls back to Open Sans when the face
    /// is not loaded.
    pub fontName: String,
    pub align: String,
    /// Whether that alignment was STATED.
    ///
    /// Excel's default is `general`, not left: a cell with no `horizontal`
    /// takes its side from its reading order, and a right-to-left one belongs
    /// against the right edge. `align` alone cannot express that — "left" is
    /// both the default and a real value someone may have chosen — so an
    /// Arabic cell that said nothing was indistinguishable from one that asked
    /// for the left margin, and both went left.
    pub alignSet: bool,
    /// `<alignment readingOrder="…">`: 0 or absent is "work it out from the
    /// text", 1 is left to right, 2 is right to left. Excel's own numbering,
    /// kept rather than translated, so the value that was read is the value
    /// that gets written back.
    ///
    /// It is what an author sets on a cell whose text alone would answer
    /// wrongly — a cell of Arabic that opens with a Latin product code, which
    /// reads left to right by the algorithm and right to left by intent.
    pub readingOrder: i64,
    pub valign: String,
    pub wrapText: bool,
    /// Excel's textRotation, as OOXML writes it: 0..90 turns the text
    /// anticlockwise, 91..180 means 1..90 clockwise, and 255 is the stacked
    /// "vertical text" that reads downwards one letter at a time. Stored raw so
    /// a round trip keeps what the file said; `rotationDegrees` is what a
    /// painter wants.
    pub textRotation: i64,
    /// `<alignment indent="n">`: n steps of about three characters in from the
    /// side the text is aligned to.
    pub indent: i64,
    /// `<protection locked="0">` unlocks a cell; every cell is locked until
    /// then, which only matters while the sheet is protected.
    pub locked: bool,
    /// `<u val="double"/>`. `underline` stays true as well, so everything that
    /// only knows about one line still draws one.
    pub doubleUnderline: bool,
    /// `<vertAlign val="superscript|subscript"/>`; "" is the baseline.
    pub vertAlign: String,
    pub numFmt: String,
    pub hasFill: bool,
    pub hasBorder: bool,
    pub borderTop: Rc<RefCell<CellBorder>>,
    pub borderRight: Rc<RefCell<CellBorder>>,
    pub borderBottom: Rc<RefCell<CellBorder>>,
    pub borderLeft: Rc<RefCell<CellBorder>>,
}

impl CellStyle {
    pub fn new() -> Self {
        Self {
            fillRgb: String::new(),
            textRgb: String::new(),
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            fontSizePt: 11.0,
            fontName: String::new(),
            align: "left".to_string(),
            alignSet: false,
            readingOrder: 0,
            valign: "bottom".to_string(),
            wrapText: false,
            textRotation: 0,
            indent: 0,
            locked: true,
            doubleUnderline: false,
            vertAlign: String::new(),
            numFmt: "General".to_string(),
            hasFill: false,
            hasBorder: false,
            borderTop: Rc::new(RefCell::new(CellBorder::new())),
            borderRight: Rc::new(RefCell::new(CellBorder::new())),
            borderBottom: Rc::new(RefCell::new(CellBorder::new())),
            borderLeft: Rc::new(RefCell::new(CellBorder::new())),
        }
    }
    /// The row height one line of this type needs, in pixels.
    ///
    /// Excel's own rule: the point size times 1.275, taken up to the next whole
    /// pixel. It is what makes 10pt type give the familiar 12.75pt row, and it
    /// is deliberately NOT the face's line box — a webfont declares a box far
    /// taller than its letters and a row sized by that is a fifth too tall.
    /// Loading a file and auto-fitting a row in the app have to agree about
    /// this, so they ask here.
    pub fn rowHeightForPt(pt: f64) -> i64 {
        let mut r#use: f64 = pt;
        if r#use <= 0.0 {
            r#use = 11.0;
        }
        let pts: f64 = r#use * 1.275;
        let px: f64 = (pts * 96.0) / 72.0;
        (px + 0.999).floor() as i64
    }
    pub fn createDefault() -> Rc<RefCell<CellStyle>> {
        Rc::new(RefCell::new(CellStyle::new()))
    }
    /// The reading order as the three states everything that PAINTS uses:
    /// -1 work it out from the text, 0 left to right, 1 right to left.
    ///
    /// Excel's own numbering is kept in the field rather than translated, so
    /// that the value read out of a file is the value written back into it;
    /// this is the translation, in one place, at the boundary.
    pub fn textDirection(&self) -> i64 {
        if self.readingOrder == 1 {
            return 0;
        }
        if self.readingOrder == 2 {
            return 1;
        }
        -1
    }
    pub fn copy(&self) -> Rc<RefCell<CellStyle>> {
        let o: Rc<RefCell<CellStyle>> = Rc::new(RefCell::new(CellStyle::new()));
        o.borrow_mut().fillRgb = self.fillRgb.clone();
        o.borrow_mut().textRgb = self.textRgb.clone();
        o.borrow_mut().bold = self.bold;
        o.borrow_mut().italic = self.italic;
        o.borrow_mut().underline = self.underline;
        o.borrow_mut().strike = self.strike;
        o.borrow_mut().fontSizePt = self.fontSizePt;
        o.borrow_mut().fontName = self.fontName.clone();
        o.borrow_mut().align = self.align.clone();
        o.borrow_mut().alignSet = self.alignSet;
        o.borrow_mut().readingOrder = self.readingOrder;
        o.borrow_mut().valign = self.valign.clone();
        o.borrow_mut().wrapText = self.wrapText;
        o.borrow_mut().textRotation = self.textRotation;
        o.borrow_mut().indent = self.indent;
        o.borrow_mut().locked = self.locked;
        o.borrow_mut().doubleUnderline = self.doubleUnderline;
        o.borrow_mut().vertAlign = self.vertAlign.clone();
        o.borrow_mut().numFmt = self.numFmt.clone();
        o.borrow_mut().hasFill = self.hasFill;
        o.borrow_mut().hasBorder = self.hasBorder;
        let mut _tmp_280 = self.borderTop.clone();
        o.borrow().borderTop.clone().borrow_mut().copyFrom(&_tmp_280);
        let mut _tmp_282 = self.borderRight.clone();
        o.borrow().borderRight.clone().borrow_mut().copyFrom(&_tmp_282);
        let mut _tmp_284 = self.borderBottom.clone();
        o.borrow().borderBottom.clone().borrow_mut().copyFrom(&_tmp_284);
        let mut _tmp_286 = self.borderLeft.clone();
        o.borrow().borderLeft.clone().borrow_mut().copyFrom(&_tmp_286);
        o
    }
    pub fn sameAs(&self, o: &Rc<RefCell<CellStyle>>) -> bool {
        if self.fillRgb != o.borrow().fillRgb {
            return false;
        }
        if self.textRgb != o.borrow().textRgb {
            return false;
        }
        if self.bold != o.borrow().bold {
            return false;
        }
        if self.italic != o.borrow().italic {
            return false;
        }
        if self.underline != o.borrow().underline {
            return false;
        }
        if self.strike != o.borrow().strike {
            return false;
        }
        if self.fontSizePt != o.borrow().fontSizePt {
            return false;
        }
        if self.fontName != o.borrow().fontName {
            return false;
        }
        if self.alignSet != o.borrow().alignSet {
            return false;
        }
        if self.align != o.borrow().align {
            return false;
        }
        // Two cells that differ only in reading order are two styles. Leaving
        // it out here would fold them into one and the direction of whichever
        // was interned first would be applied to both.
        if self.readingOrder != o.borrow().readingOrder {
            return false;
        }
        if self.valign != o.borrow().valign {
            return false;
        }
        if self.wrapText != o.borrow().wrapText {
            return false;
        }
        if self.textRotation != o.borrow().textRotation {
            return false;
        }
        if self.indent != o.borrow().indent {
            return false;
        }
        if self.locked != o.borrow().locked {
            return false;
        }
        if self.doubleUnderline != o.borrow().doubleUnderline {
            return false;
        }
        if self.vertAlign != o.borrow().vertAlign {
            return false;
        }
        if self.numFmt != o.borrow().numFmt {
            return false;
        }
        if self.hasFill != o.borrow().hasFill {
            return false;
        }
        if self.hasBorder != o.borrow().hasBorder {
            return false;
        }
        if !self.borderTop.borrow().sameAs(&o.borrow().borderTop.clone()) {
            return false;
        }
        if !self.borderRight.borrow().sameAs(&o.borrow().borderRight.clone()) {
            return false;
        }
        if !self.borderBottom.borrow().sameAs(&o.borrow().borderBottom.clone()) {
            return false;
        }
        if !self.borderLeft.borrow().sameAs(&o.borrow().borderLeft.clone()) {
            return false;
        }
        true
    }
    /// True when nothing is set — a default style needs no table entry.
    /// What a painter turns the text by: degrees CLOCKWISE, the direction every
    /// EVG backend rotates in. Excel counts anticlockwise up to 90 and then
    /// encodes clockwise as 91..180, so both halves fold onto one signed number.
    /// 255 (stacked) is not a rotation and stays 0.
    pub fn rotationDegrees(&self) -> f64 {
        if self.textRotation == 255 {
            return 0.0;
        }
        if self.textRotation > 90 {
            if self.textRotation <= 180 {
                return (self.textRotation - 90) as f64;
            }
            return 0.0;
        }
        if self.textRotation < 0 {
            return 0.0;
        }
        (0 - self.textRotation) as f64
    }
    pub fn isStacked(&self) -> bool {
        self.textRotation == 255
    }
    pub fn isPlain(&self) -> bool {
        let d: Rc<RefCell<CellStyle>> = Rc::new(RefCell::new(CellStyle::new()));
        self.sameAs(&d)
    }
}

/// Tiny number formatter kept as alias for older call sites.
#[derive(Clone)]
pub struct XlsxFormatter {
}

impl XlsxFormatter {
    pub fn new() -> Self {
        Self {
        }
    }
    pub fn format(raw: &str, num_fmt: &str) -> String {
        XlsxNumberFormat::format(raw, num_fmt)
    }
}
