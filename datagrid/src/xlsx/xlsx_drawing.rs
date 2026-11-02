// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::{DgNative, OpcPackage, OpcRelationship};
use crate::sheet_image::SheetImage;
use crate::xlsx::xml_lite::{XmlLite, XmlTag};
/// XlsxDrawing.rgr — where the pictures are, and which file each one is.
///
/// A worksheet does not hold its images. It points at a DRAWING part, the drawing
/// part describes anchors, and each anchor names a relationship whose target is
/// the actual media file. Three hops, and every one of them can be missing, so
/// each is checked rather than assumed.
///
/// Two anchor shapes matter:
///
///   twoCellAnchor  from cell + offset, to cell + offset — the picture stretches
///                  with the columns and rows between them
///   oneCellAnchor  from cell + offset, plus its own extent in EMUs
///
/// A third, absoluteAnchor, positions in EMUs from the sheet's corner and moves
/// with nothing; it is read as a one-cell anchor at A1 so that it appears rather
/// than being silently dropped.
#[derive(Clone)]
pub struct XlsxDrawing {
    /// Every image the drawing described, in document order.
    pub images: Vec<Rc<RefCell<SheetImage>>>,
    /// Chart anchors (graphicFrame → c:chart r:id), in document order.
    pub charts: Vec<Rc<RefCell<SheetImage>>>,
    /// The drawing's own package path — what its relationship targets are
    /// relative to. Set by the loader before `parse`; the conventional name is
    /// only the fallback for a caller that does not know.
    pub partPath: String,
}

impl XlsxDrawing {
    pub fn new() -> Self {
        Self {
            images: Vec::new(),
            charts: Vec::new(),
            partPath: "xl/drawings/drawing1.xml".to_string(),
        }
    }
    pub fn intOf(s: &str) -> i64 {
        if s.is_empty() {
            return 0;
        }
        let v: Option<i64> = s.parse::<i64>().ok();
        if v.is_some() {
            return v.unwrap();
        }
        0
    }
    /// The text of the next <col>/<row>/<colOff>/<rowOff> style element.
    pub fn readInt(p: &Rc<RefCell<XmlLite>>, name: &str) -> i64 {
        XlsxDrawing::intOf(&p.borrow_mut().readTextUntilEnd(name))
    }
    /// What the chart part says, as far as a GridChart can use it: the plot's
    /// kind, the title's text, the category range and each series' values.
    pub fn readChart(img: &Rc<RefCell<SheetImage>>, xml: &str) {
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(xml.to_string());
        let mut in_title: bool = false;
        let mut in_cat: bool = false;
        let mut in_val: bool = false;
        let mut bar_dir: String = "col".to_string();
        let mut grouping: String = String::new();
        let mut plot: String = String::new();
        while p.borrow().pos < p.borrow().srcLen {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            let local: String = p.borrow().localName(&t.borrow().name.clone());
            let opening: bool = !t.borrow().isEnd;
            if local == "title" {
                in_title = opening;
            }
            if local == "cat" {
                in_cat = opening;
            }
            if local == "val" {
                in_val = opening;
            }
            if opening {
                if plot.is_empty() {
                    if DgNative::str_len(&local) > 5 {
                        if DgNative::index_of(&local, "Chart") == DgNative::str_len(&local) - 5 {
                            plot = local.clone();
                        }
                    }
                }
                if local == "barDir" {
                    bar_dir = t.borrow().attr("val");
                }
                if local == "grouping" {
                    grouping = t.borrow().attr("val");
                }
                if (local == "t" && in_title) && !t.borrow().selfClose {
                    img.borrow_mut().chartTitle = format!("{}{}", img.borrow().chartTitle, p.borrow_mut().readTextUntilEnd(&t.borrow().name.clone()));
                    in_title = true;
                }
                if local == "f" && !t.borrow().selfClose {
                    let fref: String = p.borrow_mut().readTextUntilEnd(&t.borrow().name.clone());
                    if in_cat {
                        img.borrow_mut().catRef = fref.clone();
                    }
                    if in_val {
                        img.borrow_mut().valRefs.push(fref.clone());
                    }
                }
            }
        }
        img.borrow_mut().chartKind = 1;
        if plot == "barChart" || plot == "bar3DChart" {
            if bar_dir == "bar" {
                img.borrow_mut().chartKind = 2;
            }
            if grouping == "stacked" || grouping == "percentStacked" {
                img.borrow_mut().chartKind = 3;
            }
        }
        if (plot == "lineChart" || plot == "line3DChart") || plot == "radarChart" {
            img.borrow_mut().chartKind = 6;
        }
        if plot == "pieChart" || plot == "pie3DChart" {
            img.borrow_mut().chartKind = 16;
        }
        if plot == "doughnutChart" {
            img.borrow_mut().chartKind = 17;
        }
        if plot == "areaChart" {
            img.borrow_mut().chartKind = 10;
        }
        if plot == "scatterChart" {
            img.borrow_mut().chartKind = 13;
        }
    }
    /// One anchor, up to its closing tag. `endName` is the anchor's own element
    /// name, which is also what tells this walk where to stop.
    pub fn parseAnchor(&mut self, p: &Rc<RefCell<XmlLite>>, end_name: &str, rels: &str) {
        let img: Rc<RefCell<SheetImage>> = Rc::new(RefCell::new(SheetImage::new()));
        let mut in_from: bool = false;
        let mut in_to: bool = false;
        let mut saw_to: bool = false;
        let mut depth: i64 = 1;
        while p.borrow().pos < p.borrow().srcLen && depth > 0 {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            let local: String = p.borrow().localName(&t.borrow().name.clone());
            if local == end_name {
                if t.borrow().isEnd {
                    depth -= 1;
                } else {
                    if !t.borrow().selfClose {
                        depth += 1;
                    }
                }
            } else {
                if local == "from" {
                    in_from = !t.borrow().isEnd;
                }
                if local == "to" {
                    in_to = !t.borrow().isEnd;
                    if !t.borrow().isEnd {
                        saw_to = true;
                    }
                }
                if !t.borrow().isEnd {
                    if local == "col" {
                        let cv: i64 = XlsxDrawing::readInt(p, &t.borrow().name.clone());
                        if in_from {
                            img.borrow_mut().fromCol = cv;
                        }
                        if in_to {
                            img.borrow_mut().toCol = cv;
                        }
                    }
                    if local == "row" {
                        let rv: i64 = XlsxDrawing::readInt(p, &t.borrow().name.clone());
                        if in_from {
                            img.borrow_mut().fromRow = rv;
                        }
                        if in_to {
                            img.borrow_mut().toRow = rv;
                        }
                    }
                    if local == "colOff" {
                        let co: i64 = SheetImage::emuToPx(XlsxDrawing::readInt(p, &t.borrow().name.clone()));
                        if in_from {
                            img.borrow_mut().offX = co;
                        }
                        if in_to {
                            img.borrow_mut().toOffX = co;
                        }
                    }
                    if local == "rowOff" {
                        let ro: i64 = SheetImage::emuToPx(XlsxDrawing::readInt(p, &t.borrow().name.clone()));
                        if in_from {
                            img.borrow_mut().offY = ro;
                        }
                        if in_to {
                            img.borrow_mut().toOffY = ro;
                        }
                    }
                    // <xdr:ext cx=".." cy=".."> is the size a one-cell anchor
                    // carries instead of a second corner.
                    if local == "ext" {
                        let cx: String = t.borrow().attr("cx");
                        if !cx.is_empty() {
                            img.borrow_mut().widthPx = SheetImage::emuToPx(XlsxDrawing::intOf(&cx));
                            img.borrow_mut().heightPx = SheetImage::emuToPx(XlsxDrawing::intOf(&t.borrow().attr("cy")));
                        }
                    }
                    if local == "cNvPr" {
                        img.borrow_mut().name = t.borrow().attr("name");
                    }
                    if local == "chart" {
                        let crid: String = t.borrow().attrAny("r:id", "id");
                        if !crid.is_empty() {
                            img.borrow_mut().chartPart = self.relTarget(rels, &crid);
                        }
                    }
                    // The relationship id of the media part.
                    if local == "blip" {
                        let rid: String = t.borrow().attrAny("r:embed", "embed");
                        if !rid.is_empty() {
                            img.borrow_mut().src = self.relTarget(rels, &rid);
                        }
                    }
                }
            }
        }
        img.borrow_mut().stretched = saw_to;
        if !img.borrow().src.is_empty() {
            self.images.push(img.clone());
        }
        if !img.borrow().chartPart.is_empty() {
            self.charts.push(img.clone());
        }
    }
    /// rId → the media part it names. A drawing writes its target relative to
    /// itself — "../media/image1.png" — so it is resolved against the drawing's
    /// own path rather than against a directory this file assumes. The scan of
    /// the `.rels` text is `OpcPackage`'s, which is the same one the .docx and
    /// .pptx readers use; this file used to have its own, and its own `../`
    /// walker, which climbed exactly one level however many the target asked for.
    pub fn relTarget(&self, rels: &str, id: &str) -> String {
        if rels.is_empty() {
            return String::new();
        }
        let list: Vec<Rc<RefCell<OpcRelationship>>> = OpcPackage::parse_relationships(rels);
        let mut i: i64 = 0;
        while i < (list.len() as i64) {
            let rel: Rc<RefCell<OpcRelationship>> = list[i as usize].clone();
            if rel.borrow().id == id {
                if OpcRelationship::is_external(&rel) {
                    // A linked picture lives outside the package; there is no
                    // part to read, so there is nothing to draw.
                    return String::new();
                }
                let mut _tmp_795 = self.partPath.clone();
                return OpcPackage::join_part_path(&_tmp_795, &rel.borrow().target.clone());
            }
            i += 1;
        }
        String::new()
    }
    pub fn parse(&mut self, xml: &str, rels: &str) {
        self.images.clear();
        self.charts.clear();
        if xml.is_empty() {
            return;
        }
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(xml.to_string());
        while p.borrow().pos < p.borrow().srcLen {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            if !t.borrow().isEnd {
                let local: String = p.borrow().localName(&t.borrow().name.clone());
                if local == "twoCellAnchor" {
                    self.parseAnchor(&p, &local, rels);
                }
                if local == "oneCellAnchor" {
                    self.parseAnchor(&p, &local, rels);
                }
                if local == "absoluteAnchor" {
                    self.parseAnchor(&p, &local, rels);
                }
            }
        }
    }
}
