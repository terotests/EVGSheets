// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;

/// SheetImage.rgr — a picture anchored to the grid.
///
/// An image in a spreadsheet is not in a cell; it FLOATS over one, anchored by a
/// cell plus an offset inside it, and either sized absolutely or stretched to a
/// second anchor. That is the whole of OOXML's drawing model that a viewer needs,
/// and it is why the geometry here is (cell, offset) rather than (x, y): the
/// picture has to move when the column above it is widened, which a pixel
/// position could not do.
///
/// Offsets and extents arrive in EMUs — English Metric Units, 914400 to the inch,
/// which is 9525 to a pixel at 96 DPI. Everything is converted on the way in so
/// nothing downstream has to know the word.
#[derive(Clone)]
pub struct SheetImage {
    /// Where the top-left corner sits: a cell, plus an offset in pixels inside it.
    pub fromRow: i64,
    pub fromCol: i64,
    pub offX: i64,
    pub offY: i64,
    /// A twoCellAnchor stretches to a second cell; a oneCellAnchor carries its
    /// own size instead. `stretched` says which of the two this is.
    pub stretched: bool,
    pub toRow: i64,
    pub toCol: i64,
    pub toOffX: i64,
    pub toOffY: i64,
    pub widthPx: i64,
    pub heightPx: i64,
    /// The package part the pixels are in, e.g. "xl/media/image1.png".
    pub src: String,
    pub name: String,
    /// Alt text (a picture's description for a screen reader).
    pub altText: String,
    /// The media file's own bytes, taken out of the package while it was open.
    /// They stay compressed-format here — a PNG is a PNG — and are decoded on
    /// first paint, once, by GridImages.
    pub bytes: Vec<u8>,
    /// A chart anchored here instead of a picture: its part, and what was read
    /// from it — the kind (ChartKind ids), title and series references.
    pub chartPart: String,
    pub chartKind: i64,
    pub chartTitle: String,
    pub catRef: String,
    pub valRefs: Vec<String>,
}

impl SheetImage {
    pub fn new() -> Self {
        Self {
            fromRow: 0,
            fromCol: 0,
            offX: 0,
            offY: 0,
            stretched: false,
            toRow: 0,
            toCol: 0,
            toOffX: 0,
            toOffY: 0,
            widthPx: 0,
            heightPx: 0,
            src: String::new(),
            name: String::new(),
            altText: String::new(),
            bytes: vec![0u8; 0],
            chartPart: String::new(),
            chartKind: 0,
            chartTitle: String::new(),
            catRef: String::new(),
            valRefs: Vec::new(),
        }
    }
    pub fn emuToPx(emu: i64) -> i64 {
        emu / 9525
    }
    pub fn copyFrom(&mut self, o: &Rc<RefCell<SheetImage>>) {
        self.fromRow = o.borrow().fromRow;
        self.fromCol = o.borrow().fromCol;
        self.offX = o.borrow().offX;
        self.offY = o.borrow().offY;
        self.stretched = o.borrow().stretched;
        self.toRow = o.borrow().toRow;
        self.toCol = o.borrow().toCol;
        self.toOffX = o.borrow().toOffX;
        self.toOffY = o.borrow().toOffY;
        self.widthPx = o.borrow().widthPx;
        self.heightPx = o.borrow().heightPx;
        self.src = o.borrow().src.clone();
        self.name = o.borrow().name.clone();
        self.bytes = o.borrow().bytes.clone();
        self.chartPart = o.borrow().chartPart.clone();
        self.chartKind = o.borrow().chartKind;
        self.chartTitle = o.borrow().chartTitle.clone();
        self.catRef = o.borrow().catRef.clone();
        self.valRefs.clear();
        let mut vi: i64 = 0;
        while vi < (o.borrow().valRefs.len() as i64) {
            self.valRefs.push(o.borrow().valRefs[vi as usize].clone());
            vi += 1;
        }
    }
    /// What kind of file the pixels are in, from the part name — the package
    /// does not say, and the decoder has to be told.
    pub fn kind(&self) -> String {
        let n: i64 = DgNative::str_len(&self.src);
        if n < 4 {
            return String::new();
        }
        let tail: String = DgNative::substr(&self.src, n - 4, n);
        if tail == ".png" {
            return "png".to_string();
        }
        if tail == ".jpg" {
            return "jpeg".to_string();
        }
        if n >= 5 {
            if DgNative::substr(&self.src, n - 5, n) == ".jpeg" {
                return "jpeg".to_string();
            }
        }
        String::new()
    }
}
