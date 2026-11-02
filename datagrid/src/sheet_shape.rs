// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// SheetShape.rgr — a drawing object floating over the grid.
///
/// Shapes, text boxes, icons, WordArt, slicers and pen strokes are one kind of
/// thing to a sheet: a box anchored to a cell, painted over the cells, with a
/// z-order and a selection. Like SheetImage the anchor is a cell plus a pixel
/// offset, so a shape moves with the column it sits in.
///
/// `kind` says how the box is drawn:
///   rect roundRect ellipse triangle diamond line arrow   a geometric shape
///   textBox wordArt signature                            text first
///   icon                                                 `icon` names it
///   ink                                                  `points`, relative
///   slicer timeline                                      buttons over a column
#[derive(Clone)]
pub struct SheetShape {
    pub kind: String,
    pub name: String,
    pub fromRow: i64,
    pub fromCol: i64,
    pub offX: i64,
    pub offY: i64,
    pub widthPx: i64,
    pub heightPx: i64,
    pub fill: String,
    pub outline: String,
    pub textColor: String,
    pub text: String,
    pub fontSize: f64,
    pub rotation: i64,
    pub flipH: bool,
    pub groupId: i64,
    pub hidden: bool,
    pub icon: String,
    pub altText: String,
    /// A pen stroke: x,y pairs relative to the box's top-left corner.
    pub points: Vec<f64>,
    /// A slicer: the table column it filters, and the values it offers.
    pub column: i64,
    pub items: Vec<String>,
    pub picked: Vec<String>,
}

impl SheetShape {
    pub fn new() -> Self {
        Self {
            kind: "rect".to_string(),
            name: String::new(),
            fromRow: 0,
            fromCol: 0,
            offX: 0,
            offY: 0,
            widthPx: 120,
            heightPx: 80,
            fill: "#4472C4".to_string(),
            outline: "#2F528F".to_string(),
            textColor: "#FFFFFF".to_string(),
            text: String::new(),
            fontSize: 14.0,
            rotation: 0,
            flipH: false,
            groupId: 0,
            hidden: false,
            icon: String::new(),
            altText: String::new(),
            points: Vec::new(),
            column: -1,
            items: Vec::new(),
            picked: Vec::new(),
        }
    }
    /// |x|^(1/4) with the sign of x: pushes a circle out toward its square.
    pub fn softSign(x: f64) -> f64 {
        let mut ax: f64 = x;
        if ax < 0.0 {
            ax = 0.0 - ax;
        }
        let r: f64 = ax.sqrt().sqrt();
        if x < 0.0 {
            return 0.0 - r;
        }
        r
    }
    pub fn copyFrom(&mut self, o: &Rc<RefCell<SheetShape>>) {
        self.kind = o.borrow().kind.clone();
        self.name = o.borrow().name.clone();
        self.fromRow = o.borrow().fromRow;
        self.fromCol = o.borrow().fromCol;
        self.offX = o.borrow().offX;
        self.offY = o.borrow().offY;
        self.widthPx = o.borrow().widthPx;
        self.heightPx = o.borrow().heightPx;
        self.fill = o.borrow().fill.clone();
        self.outline = o.borrow().outline.clone();
        self.textColor = o.borrow().textColor.clone();
        self.text = o.borrow().text.clone();
        self.fontSize = o.borrow().fontSize;
        self.rotation = o.borrow().rotation;
        self.flipH = o.borrow().flipH;
        self.groupId = o.borrow().groupId;
        self.hidden = o.borrow().hidden;
        self.icon = o.borrow().icon.clone();
        self.altText = o.borrow().altText.clone();
        self.points.clear();
        let mut i: i64 = 0;
        while i < (o.borrow().points.len() as i64) {
            self.points.push(o.borrow().points[i as usize]);
            i += 1;
        }
        self.column = o.borrow().column;
        self.items.clear();
        let mut j: i64 = 0;
        while j < (o.borrow().items.len() as i64) {
            self.items.push(o.borrow().items[j as usize].clone());
            j += 1;
        }
        self.picked.clear();
        let mut k: i64 = 0;
        while k < (o.borrow().picked.len() as i64) {
            self.picked.push(o.borrow().picked[k as usize].clone());
            k += 1;
        }
    }
    pub fn isPicked(&self, v: &str) -> bool {
        let mut i: i64 = 0;
        while i < (self.picked.len() as i64) {
            if self.picked[i as usize].clone() == v {
                return true;
            }
            i += 1;
        }
        false
    }
    /// The outline of a geometric kind as a polygon in box space (0..w, 0..h),
    /// before rotation. Lines and ink are strokes, not polygons.
    pub fn outlinePoints(&self) -> Vec<f64> {
        let w: f64 = self.widthPx as f64;
        let h: f64 = self.heightPx as f64;
        let mut pts: Vec<f64> = Vec::new();
        if self.kind == "ellipse" || self.kind == "roundRect" {
            let n: i64 = 32;
            let mut i: i64 = 0;
            while i < n {
                let a: f64 = (6.283185307179586 * (i as f64)) / (n as f64);
                if self.kind == "ellipse" {
                    pts.push(w / 2.0 + (w / 2.0) * a.cos());
                    pts.push(h / 2.0 + (h / 2.0) * a.sin());
                } else {
                    // A rounded rectangle: the corners of a superellipse.
                    let ca: f64 = a.cos();
                    let sa: f64 = a.sin();
                    let px: f64 = SheetShape::softSign(ca);
                    let py: f64 = SheetShape::softSign(sa);
                    pts.push(w / 2.0 + (w / 2.0) * px);
                    pts.push(h / 2.0 + (h / 2.0) * py);
                }
                i += 1;
            }
            return pts;
        }
        if self.kind == "triangle" {
            pts.push(w / 2.0);
            pts.push(0.0);
            pts.push(w);
            pts.push(h);
            pts.push(0.0);
            pts.push(h);
            return pts;
        }
        if self.kind == "diamond" {
            pts.push(w / 2.0);
            pts.push(0.0);
            pts.push(w);
            pts.push(h / 2.0);
            pts.push(w / 2.0);
            pts.push(h);
            pts.push(0.0);
            pts.push(h / 2.0);
            return pts;
        }
        if self.kind == "arrow" {
            // A block arrow pointing right.
            pts.push(0.0);
            pts.push(h * 0.3);
            pts.push(w * 0.65);
            pts.push(h * 0.3);
            pts.push(w * 0.65);
            pts.push(0.0);
            pts.push(w);
            pts.push(h / 2.0);
            pts.push(w * 0.65);
            pts.push(h);
            pts.push(w * 0.65);
            pts.push(h * 0.7);
            pts.push(0.0);
            pts.push(h * 0.7);
            return pts;
        }
        if self.kind == "icon" {
            return self.iconPoints(w, h);
        }
        pts.push(0.0);
        pts.push(0.0);
        pts.push(w);
        pts.push(0.0);
        pts.push(w);
        pts.push(h);
        pts.push(0.0);
        pts.push(h);
        pts
    }
    pub fn iconPoints(&self, w: f64, h: f64) -> Vec<f64> {
        let mut pts: Vec<f64> = Vec::new();
        if self.icon == "check" {
            pts.push(0.0);
            pts.push(h * 0.55);
            pts.push(w * 0.15);
            pts.push(h * 0.4);
            pts.push(w * 0.4);
            pts.push(h * 0.65);
            pts.push(w * 0.85);
            pts.push(0.0);
            pts.push(w);
            pts.push(h * 0.15);
            pts.push(w * 0.4);
            pts.push(h);
            return pts;
        }
        if self.icon == "heart" {
            let mut i: i64 = 0;
            while i < 32 {
                let t: f64 = (6.283185307179586 * (i as f64)) / 32.0;
                let s: f64 = t.sin();
                let x: f64 = (16.0 * s) * (s * s);
                let y: f64 = (13.0 * t.cos() - 5.0 * ((2.0 * t).cos())) - (2.0 * ((3.0 * t).cos()) + ((4.0 * t).cos()));
                pts.push(w / 2.0 + (x / 34.0) * w);
                pts.push(h * 0.45 - (y / 34.0) * h);
                i += 1;
            }
            return pts;
        }
        if self.icon == "arrow" {
            pts.push(w / 2.0);
            pts.push(0.0);
            pts.push(w);
            pts.push(h * 0.5);
            pts.push(w * 0.7);
            pts.push(h * 0.5);
            pts.push(w * 0.7);
            pts.push(h);
            pts.push(w * 0.3);
            pts.push(h);
            pts.push(w * 0.3);
            pts.push(h * 0.5);
            pts.push(0.0);
            pts.push(h * 0.5);
            return pts;
        }
        // A five-pointed star, the default icon.
        let mut k: i64 = 0;
        while k < 10 {
            let a: f64 = -1.5707963267948966 + (3.141592653589793 * (k as f64)) / 5.0;
            let mut rr: f64 = 0.5;
            if k % 2 == 1 {
                rr = 0.2;
            }
            pts.push(w / 2.0 + (w * rr) * a.cos());
            pts.push(h * 0.52 + (h * rr) * a.sin());
            k += 1;
        }
        pts
    }
    /// Box-space points turned about the box's centre and flipped, then moved
    /// to the screen at (x, y).
    pub fn place(&self, pts: &[f64], x: f64, y: f64) -> Vec<f64> {
        let w: f64 = self.widthPx as f64;
        let h: f64 = self.heightPx as f64;
        let cx: f64 = w / 2.0;
        let cy: f64 = h / 2.0;
        let rad: f64 = ((self.rotation as f64) * 3.141592653589793) / 180.0;
        let c: f64 = rad.cos();
        let s: f64 = rad.sin();
        let mut out: Vec<f64> = Vec::new();
        let mut i: i64 = 0;
        while i + 1 < (pts.len() as i64) {
            let mut px: f64 = pts[i as usize] - cx;
            let py: f64 = pts[(i + 1) as usize] - cy;
            if self.flipH {
                px = 0.0 - px;
            }
            out.push((x + cx) + (px * c - py * s));
            out.push((y + cy) + (px * s + py * c));
            i += 2;
        }
        out
    }
    pub fn isStroke(&self) -> bool {
        self.kind == "line" || self.kind == "ink"
    }
    pub fn isTextFirst(&self) -> bool {
        (self.kind == "textBox" || self.kind == "wordArt") || self.kind == "signature"
    }
}
