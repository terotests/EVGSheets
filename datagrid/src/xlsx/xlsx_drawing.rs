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
        let mut plots: String = String::new();
        let mut scatter_style: String = String::new();
        let mut in_val_ax: bool = false;
        let mut in_ser: bool = false;
        let mut in_legend: bool = false;
        let mut ser_colour: String = String::new();
        let mut colours: Vec<String> = Vec::new();
        let mut legend: String = "none".to_string();
        let mut labels: bool = false;
        let mut v_min: String = String::new();
        let mut v_max: String = String::new();
        let mut v_log: bool = false;
        let mut v_fmt: String = String::new();
        let mut v_grid: bool = false;
        let mut in_tx: bool = false;
        let mut in_cat_ax: bool = false;
        while p.borrow().pos < p.borrow().srcLen {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            let local: String = p.borrow().localName(&t.borrow().name.clone());
            let opening: bool = !t.borrow().isEnd;
            if local == "catAx" || local == "dateAx" || local == "serAx" {
                in_cat_ax = opening;
            }
            // Only the chart's own title: an axis title is not the chart's.
            if local == "title" && !in_val_ax && !in_cat_ax {
                in_title = opening;
            }
            if local == "cat" {
                in_cat = opening;
            }
            if local == "val" || local == "xVal" || local == "yVal" || local == "bubbleSize" {
                in_val = opening;
            }
            if local == "valAx" {
                in_val_ax = opening;
            }
            if local == "legend" {
                in_legend = opening;
                if opening {
                    legend = "right".to_string();
                }
            }
            if local == "tx" && in_ser {
                in_tx = opening && !t.borrow().selfClose;
            }
            if local == "ser" {
                if opening {
                    in_ser = true;
                    ser_colour = String::new();
                } else {
                    in_ser = false;
                    // Excel's default colour for this position says nothing
                    // the chart would not do by itself.
                    let defaults: Vec<String> = vec!["#4472C4".to_string(), "#ED7D31".to_string(), "#A5A5A5".to_string(), "#FFC000".to_string(), "#5B9BD5".to_string(), "#70AD47".to_string()];
                    let pos: i64 = (colours.len() as i64) % 6;
                    if ser_colour.to_uppercase() == defaults[pos as usize] {
                        ser_colour = String::new();
                    }
                    colours.push(ser_colour.clone());
                }
            }
            if opening {
                if DgNative::str_len(&local) > 5 && DgNative::index_of(&local, "Chart") == DgNative::str_len(&local) - 5 && local != "chart" {
                    plots = format!("{} {}", plots, local);
                }
                if local == "legendPos" && in_legend {
                    let lp: String = t.borrow().attr("val");
                    if lp == "t" {
                        legend = "top".to_string();
                    }
                    if lp == "b" {
                        legend = "bottom".to_string();
                    }
                    if lp == "l" {
                        legend = "left".to_string();
                    }
                }
                if local == "showVal" && t.borrow().attr("val") == "1" {
                    labels = true;
                }
                if in_val_ax {
                    if local == "min" {
                        v_min = t.borrow().attr("val");
                    }
                    if local == "max" {
                        v_max = t.borrow().attr("val");
                    }
                    if local == "logBase" {
                        v_log = true;
                    }
                    if local == "majorGridlines" {
                        v_grid = true;
                    }
                    if local == "numFmt" && t.borrow().attr("sourceLinked") == "0" {
                        v_fmt = t.borrow().attr("formatCode");
                    }
                }
                if local == "srgbClr" && in_ser && !in_val && !in_cat && ser_colour.is_empty() {
                    ser_colour = format!("#{}", t.borrow().attr("val"));
                }
                if local == "f" && in_tx && !t.borrow().selfClose {
                    let nref: String = p.borrow_mut().readTextUntilEnd(&t.borrow().name.clone());
                    img.borrow_mut().nameRefs.push(nref.clone());
                }
                if local == "scatterStyle" {
                    scatter_style = t.borrow().attr("val");
                }
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
                if local == "f" && !t.borrow().selfClose && !in_tx {
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
        let mut style: String = format!("legend={}", legend);
        if labels {
            style = format!("{};dataLabels=1", style);
        }
        if !v_min.is_empty() {
            style = format!("{};min={}", style, v_min);
        }
        if !v_max.is_empty() {
            style = format!("{};max={}", style, v_max);
        }
        if v_log {
            style = format!("{};log=on", style);
        }
        if !v_fmt.is_empty() && DgNative::index_of(&v_fmt, ";") < 0 {
            style = format!("{};format={}", style, v_fmt);
        }
        if !v_grid {
            style = format!("{};gridlines=off", style);
        }
        let mut any_colour: bool = false;
        let mut ci: i64 = 0;
        while ci < (colours.len() as i64) {
            if !colours[ci as usize].is_empty() {
                any_colour = true;
            }
            ci += 1;
        }
        if any_colour {
            let ncol: i64 = colours.len() as i64;
            let mut last: i64 = ncol - 1;
            while last > 0 {
                if !colours[last as usize].is_empty() {
                    break;
                }
                last -= 1;
            }
            let mut kept: Vec<String> = Vec::new();
            let mut ki: i64 = 0;
            while ki <= last {
                kept.push(colours[ki as usize].clone());
                ki += 1;
            }
            style = format!("{};colors={}", style, kept.join(","));
        }
        img.borrow_mut().chartStyle = style.clone();
        img.borrow_mut().chartKind = 1;
        if plot == "barChart" || plot == "bar3DChart" {
            let horizontal: bool = bar_dir == "bar";
            if horizontal {
                img.borrow_mut().chartKind = 2;
            }
            if grouping == "stacked" {
                img.borrow_mut().chartKind = 3;
                if horizontal {
                    img.borrow_mut().chartKind = 21;
                }
            }
            if grouping == "percentStacked" {
                img.borrow_mut().chartKind = 4;
                if horizontal {
                    img.borrow_mut().chartKind = 22;
                }
            }
            // Columns and a line in one plot area: Excel's combo chart.
            if DgNative::index_of(&plots, "lineChart") >= 0 {
                img.borrow_mut().chartKind = 25;
            }
        }
        if (plot == "lineChart" || plot == "line3DChart") || plot == "radarChart" {
            img.borrow_mut().chartKind = 6;
            if grouping == "stacked" {
                img.borrow_mut().chartKind = 30;
            }
            if grouping == "percentStacked" {
                img.borrow_mut().chartKind = 31;
            }
            if DgNative::index_of(&plots, "barChart") >= 0 {
                img.borrow_mut().chartKind = 25;
            }
        }
        if plot == "stockChart" {
            img.borrow_mut().chartKind = 29;
        }
        if plot == "bubbleChart" {
            img.borrow_mut().chartKind = 14;
        }
        if plot == "pieChart" || plot == "pie3DChart" {
            img.borrow_mut().chartKind = 16;
        }
        if plot == "doughnutChart" {
            img.borrow_mut().chartKind = 17;
        }
        if plot == "areaChart" || plot == "area3DChart" {
            img.borrow_mut().chartKind = 10;
            if grouping == "stacked" {
                img.borrow_mut().chartKind = 11;
            }
            if grouping == "percentStacked" {
                img.borrow_mut().chartKind = 23;
            }
        }
        if plot == "scatterChart" {
            img.borrow_mut().chartKind = 13;
            // "lineMarker" is also what Excel writes for markers only (the
            // line hidden in the series' shape), so only a plain "line" is
            // taken as lines through the points.
            if scatter_style == "line" {
                img.borrow_mut().chartKind = 24;
            }
            if scatter_style == "smoothMarker" || scatter_style == "smooth" {
                img.borrow_mut().chartKind = 32;
            }
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
