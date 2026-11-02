// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rgr::{DgNative, OfficeColorMath, OfficeTheme};
use crate::cell_style::CellStyle;
use crate::xlsx::xml_lite::{XmlLite, XmlTag};
/// XlsxStyles - xl/styles.xml → resolved CellStyle table.
///
/// FortuneSheet "Formatting" parity slice: fill, font color/bold, horizontal
/// align, number format.
///
/// COLOURS. A `<color>` in a workbook says which colour one of four ways, and
/// this used to read exactly one of them:
///
///     rgb="FFCC0000"                 read
///     theme="4" tint="-0.25"         "" — dropped
///     indexed="10"                   "" — dropped
///     auto="1"                       "" — dropped
///
/// The second is not exotic. It is what Excel writes for every cell styled from
/// the palette: "Blue, Accent 1, Lighter 40%" IS a theme index and a tint. The
/// theme part was sitting in the package unread, so any workbook coloured the
/// way Excel colours things by default came out with no colours at all.
///
/// The palette and the maths are `gallery/office/drawing/OfficeColor.rgr`, which
/// the .pptx reader shares — but Excel's `tint` is NOT DrawingML's `tint`, and
/// the shared file keeps them as two functions for that reason.
pub struct XlsxStyles {
    /// The workbook's theme palette, out of `xl/theme/theme1.xml`. Empty when a
    /// workbook has no theme part, in which case a `theme=` colour has nothing
    /// to resolve to and is left alone rather than guessed at.
    theme: Rc<RefCell<OfficeTheme>>,
    pub styles: Vec<Rc<RefCell<CellStyle>>>,
    pub fontsBold: Vec<bool>,
    pub fontsItalic: Vec<bool>,
    pub fontsUnderline: Vec<bool>,
    pub fontsStrike: Vec<bool>,
    pub fontsVert: Vec<String>,
    pub fontsSize: Vec<f64>,
    pub fontsColor: Vec<String>,
    pub fontsName: Vec<String>,
    /// One entry per <border> id, four edges each.
    pub bTopStyle: Vec<String>,
    pub bTopRgb: Vec<String>,
    pub bRightStyle: Vec<String>,
    pub bRightRgb: Vec<String>,
    pub bBottomStyle: Vec<String>,
    pub bBottomRgb: Vec<String>,
    pub bLeftStyle: Vec<String>,
    pub bLeftRgb: Vec<String>,
    pub fillsRgb: Vec<String>,
    pub fillsHas: Vec<bool>,
    pub numFmtMap: HashMap<String,String>,
}

impl XlsxStyles {
    pub fn new() -> Self {
        Self {
            theme: Rc::new(RefCell::new(OfficeTheme::new())),
            styles: Vec::new(),
            fontsBold: Vec::new(),
            fontsItalic: Vec::new(),
            fontsUnderline: Vec::new(),
            fontsStrike: Vec::new(),
            fontsVert: Vec::new(),
            fontsSize: Vec::new(),
            fontsColor: Vec::new(),
            fontsName: Vec::new(),
            bTopStyle: Vec::new(),
            bTopRgb: Vec::new(),
            bRightStyle: Vec::new(),
            bRightRgb: Vec::new(),
            bBottomStyle: Vec::new(),
            bBottomRgb: Vec::new(),
            bLeftStyle: Vec::new(),
            bLeftRgb: Vec::new(),
            fillsRgb: Vec::new(),
            fillsHas: Vec::new(),
            numFmtMap: HashMap::new(),
        }
    }
    /// A border with no <color> defaults to black, as Excel does.
    pub fn borderColorOr(rgb: &str) -> String {
        if DgNative::str_len(&rgb) >= 4 {
            return rgb.to_string();
        }
        "#000000".to_string()
    }
    pub fn builtInNumFmt(id: i64) -> String {
        if id == 0 {
            return "General".to_string();
        }
        if id == 1 {
            return "0".to_string();
        }
        if id == 2 {
            return "0.00".to_string();
        }
        if id == 9 {
            return "0%".to_string();
        }
        if id == 10 {
            return "0.00%".to_string();
        }
        if id == 49 {
            return "@".to_string();
        }
        if id == 14 {
            return "mm/dd/yyyy".to_string();
        }
        if id == 15 {
            return "dd.mm.yyyy".to_string();
        }
        if id == 22 {
            return "hh:mm".to_string();
        }
        if id == 164 {
            return "yyyy-mm-dd".to_string();
        }
        "General".to_string()
    }
    pub fn parseInt0(s: &str) -> i64 {
        let parsed: Option<i64> = s.parse::<i64>().ok();
        if parsed.is_some() {
            return parsed.unwrap();
        }
        0
    }
    pub fn isSchemeSlot(name: &str) -> bool {
        if name == "dk1" {
            return true;
        }
        if name == "lt1" {
            return true;
        }
        if name == "dk2" {
            return true;
        }
        if name == "lt2" {
            return true;
        }
        if name == "accent1" {
            return true;
        }
        if name == "accent2" {
            return true;
        }
        if name == "accent3" {
            return true;
        }
        if name == "accent4" {
            return true;
        }
        if name == "accent5" {
            return true;
        }
        if name == "accent6" {
            return true;
        }
        if name == "hlink" {
            return true;
        }
        if name == "folHlink" {
            return true;
        }
        false
    }
    pub fn styleAt(&self, i: i64) -> Rc<RefCell<CellStyle>> {
        if i < 0 {
            return CellStyle::createDefault();
        }
        if i >= (self.styles.len() as i64) {
            return CellStyle::createDefault();
        }
        self.styles[i as usize].clone()
    }
    pub fn count(&self) -> i64 {
        self.styles.len() as i64
    }
    pub fn ensureBuiltinNumFmts(&mut self) {
        self.numFmtMap.insert("0".to_string(), "General".to_string());
        self.numFmtMap.insert("1".to_string(), "0".to_string());
        self.numFmtMap.insert("2".to_string(), "0.00".to_string());
        self.numFmtMap.insert("9".to_string(), "0%".to_string());
        self.numFmtMap.insert("10".to_string(), "0.00%".to_string());
        self.numFmtMap.insert("49".to_string(), "@".to_string());
    }
    /// All four ways a workbook names a colour, and the tint that may be on any
    /// of them.
    pub fn parseColorAttr(&self, t: &Rc<RefCell<XmlTag>>) -> String {
        let mut base: String = String::new();
        if t.borrow().has("rgb") {
            let rgb: String = t.borrow().attr("rgb");
            // ARGB in the file; the alpha is dropped because a cell colour has
            // nowhere to put it.
            if DgNative::str_len(&rgb) == 8 {
                base = DgNative::substr(&rgb, 2, 8);
            } else {
                base = rgb.clone();
            }
        }
        if base.is_empty() {
            if t.borrow().has("theme") {
                let ti: Option<i64> = t.borrow().attr("theme").parse::<i64>().ok();
                if ti.is_some() {
                    let name: String = OfficeTheme::scheme_for_excel_index(ti.unwrap());
                    if !name.is_empty() {
                        if OfficeTheme::has(&self.theme, &name) {
                            base = OfficeTheme::color_of(&self.theme, &name);
                        }
                    }
                }
            }
        }
        if base.is_empty() {
            if t.borrow().has("indexed") {
                let ii: Option<i64> = t.borrow().attr("indexed").parse::<i64>().ok();
                if ii.is_some() {
                    base = OfficeColorMath::indexed_color(ii.unwrap());
                }
            }
        }
        if DgNative::str_len(&base) != 6 {
            return String::new();
        }
        // `tint` rides on whichever of the three named the colour. Excel's
        // tint scales HLS luminance — it is not DrawingML's, which keeps a
        // fraction of each RGB channel, and using one for the other gives a
        // colour that is plausible and wrong.
        if t.borrow().has("tint") {
            let tv: Option<f64> = t.borrow().attr("tint").parse::<f64>().ok();
            if tv.is_some() {
                base = OfficeColorMath::apply_excel_tint(&base, tv.unwrap());
            }
        }
        format!("#{}", base)
    }
    /// `xl/theme/theme1.xml` — the same `<a:clrScheme>` a deck carries, so the
    /// twelve names mean the same twelve things. Read before the styles, because
    /// a `theme=` colour cannot be resolved without it.
    pub fn parseTheme(&self, xml: &str) {
        if xml.is_empty() {
            return;
        }
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(xml.to_string());
        let mut in_scheme: bool = false;
        let mut slot: String = String::new();
        while p.borrow().pos < p.borrow().srcLen {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            let local: String = p.borrow().localName(&t.borrow().name.clone());
            if local == "clrScheme" {
                in_scheme = !t.borrow().isEnd;
                if t.borrow().isEnd {
                    return;
                }
            }
            if in_scheme {
                if !t.borrow().isEnd {
                    // A slot element — `dk1`, `accent1` … — wraps the colour
                    // that fills it.
                    if local == "srgbClr" || local == "sysClr" {
                        if !slot.is_empty() {
                            let mut hex: String = String::new();
                            if local == "srgbClr" {
                                hex = t.borrow().attr("val");
                            } else {
                                // A system colour carries the value it had when
                                // the file was written, which is the only
                                // answer available here.
                                hex = t.borrow().attr("lastClr");
                            }
                            if DgNative::str_len(&hex) == 6 {
                                OfficeTheme::set(&self.theme, &slot, &hex);
                            }
                            slot = String::new();
                        }
                    } else {
                        if !OfficeTheme::scheme_for_excel_index(0).is_empty() {
                            // Any of the twelve names opens a slot.
                            if XlsxStyles::isSchemeSlot(&local) {
                                slot = local.clone();
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn parse(&mut self, xml: &str) {
        self.styles.clear();
        self.fontsBold.clear();
        self.fontsItalic.clear();
        self.fontsUnderline.clear();
        self.fontsStrike.clear();
        self.fontsVert.clear();
        self.fontsSize.clear();
        self.fontsColor.clear();
        self.fontsName.clear();
        self.fillsRgb.clear();
        self.fillsHas.clear();
        self.bTopStyle.clear();
        self.bTopRgb.clear();
        self.bRightStyle.clear();
        self.bRightRgb.clear();
        self.bBottomStyle.clear();
        self.bBottomRgb.clear();
        self.bLeftStyle.clear();
        self.bLeftRgb.clear();
        let empty_fmt: HashMap<String,String> = HashMap::new();
        self.numFmtMap = empty_fmt.clone();
        self.ensureBuiltinNumFmts();
        if xml.is_empty() {
            self.styles.push(CellStyle::createDefault());
            return;
        }
        // Pass 1: numFmts, fonts, fills
        let p: Rc<RefCell<XmlLite>> = Rc::new(RefCell::new(XmlLite::new()));
        p.borrow_mut().reset(xml.to_string());
        while p.borrow().pos < p.borrow().srcLen {
            let t: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t.borrow().valid {
                break;
            }
            if t.borrow().name == "numFmt" && !t.borrow().isEnd {
                let id: String = t.borrow().attr("numFmtId");
                let code: String = t.borrow().attr("formatCode");
                if !id.is_empty() && !code.is_empty() {
                    self.numFmtMap.insert(id.clone(), code.clone());
                }
            }
            // `<font/>` — no weight, no size, no colour — is a legal entry and
            // a common one: it means "the workbook default". Skipping it, as
            // the self-close guard here used to, did not merely lose one font;
            // it shifted every font AFTER it down an index, so cells came out
            // wearing their neighbour's weight and colour. A font element
            // always takes its place in the list, empty or not.
            if t.borrow().name == "font" && !t.borrow().isEnd {
                let mut bold: bool = false;
                let mut italic: bool = false;
                let mut underline: bool = false;
                let mut strike: bool = false;
                let mut vert: String = String::new();
                // Absent <sz> means the workbook default, which is font 0 —
                // 10pt Arial in a sheet written by Google, 11pt Calibri in one
                // written by Excel. Assuming 11 made a 10pt sheet a tenth too
                // large everywhere the size was left unsaid.
                let mut size_pt: f64 = 11.0;
                if !self.fontsSize.is_empty() {
                    size_pt = self.fontsSize[0];
                }
                let mut color: String = String::new();
                let mut face: String = String::new();
                let mut depth: i64 = 1;
                if t.borrow().selfClose {
                    depth = 0;
                }
                while p.borrow().pos < p.borrow().srcLen && depth > 0 {
                    let ft: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
                    if !ft.borrow().valid {
                        break;
                    }
                    if ft.borrow().name == "font" {
                        if ft.borrow().isEnd {
                            depth -= 1;
                        } else {
                            if !ft.borrow().selfClose {
                                depth += 1;
                            }
                        }
                    } else {
                        if ft.borrow().name == "b" && !ft.borrow().isEnd {
                            bold = true;
                        }
                        if ft.borrow().name == "i" && !ft.borrow().isEnd {
                            italic = true;
                        }
                        if ft.borrow().name == "u" && !ft.borrow().isEnd {
                            underline = true;
                        }
                        if ft.borrow().name == "strike" && !ft.borrow().isEnd {
                            strike = true;
                        }
                        if ft.borrow().name == "vertAlign" && !ft.borrow().isEnd {
                            let va: String = ft.borrow().attr("val");
                            if va == "superscript" || va == "subscript" {
                                vert = va.clone();
                            }
                        }
                        if ft.borrow().name == "sz" && !ft.borrow().isEnd {
                            let szv: String = ft.borrow().attr("val");
                            if !szv.is_empty() {
                                let szd: Option<f64> = szv.parse::<f64>().ok();
                                if szd.is_some() {
                                    size_pt = szd.unwrap();
                                }
                            }
                        }
                        if ft.borrow().name == "color" && !ft.borrow().isEnd {
                            color = self.parseColorAttr(&ft);
                        }
                        if ft.borrow().name == "name" && !ft.borrow().isEnd {
                            face = ft.borrow().attr("val");
                        }
                    }
                }
                self.fontsBold.push(bold);
                self.fontsItalic.push(italic);
                self.fontsUnderline.push(underline);
                self.fontsStrike.push(strike);
                self.fontsVert.push(vert.clone());
                self.fontsSize.push(size_pt);
                self.fontsColor.push(color.clone());
                self.fontsName.push(face.clone());
            }
            // <border><left style="thin"><color rgb="FFFF0000"/></left>…</border>
            if t.borrow().name == "border" && !t.borrow().isEnd {
                let mut t_s: String = String::new();
                let mut t_c: String = String::new();
                let mut r_s: String = String::new();
                let mut r_c: String = String::new();
                let mut b_s: String = String::new();
                let mut b_c: String = String::new();
                let mut l_s: String = String::new();
                let mut l_c: String = String::new();
                if !t.borrow().selfClose {
                    let mut edge: String = String::new();
                    let mut depth_b: i64 = 1;
                    while p.borrow().pos < p.borrow().srcLen && depth_b > 0 {
                        let bt: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
                        if !bt.borrow().valid {
                            break;
                        }
                        if bt.borrow().name == "border" {
                            if bt.borrow().isEnd {
                                depth_b -= 1;
                            } else {
                                if !bt.borrow().selfClose {
                                    depth_b += 1;
                                }
                            }
                        } else {
                            let mut is_edge: bool = false;
                            if bt.borrow().name == "left" || bt.borrow().name == "right" {
                                is_edge = true;
                            }
                            if bt.borrow().name == "top" || bt.borrow().name == "bottom" {
                                is_edge = true;
                            }
                            if is_edge {
                                if !bt.borrow().isEnd {
                                    edge = bt.borrow().name.clone();
                                    let stv: String = bt.borrow().attr("style");
                                    if bt.borrow().name == "top" {
                                        t_s = stv.clone();
                                    }
                                    if bt.borrow().name == "right" {
                                        r_s = stv.clone();
                                    }
                                    if bt.borrow().name == "bottom" {
                                        b_s = stv.clone();
                                    }
                                    if bt.borrow().name == "left" {
                                        l_s = stv.clone();
                                    }
                                } else {
                                    edge = String::new();
                                }
                            } else {
                                if bt.borrow().name == "color" && !bt.borrow().isEnd {
                                    let cv: String = self.parseColorAttr(&bt);
                                    if edge == "top" {
                                        t_c = cv.clone();
                                    }
                                    if edge == "right" {
                                        r_c = cv.clone();
                                    }
                                    if edge == "bottom" {
                                        b_c = cv.clone();
                                    }
                                    if edge == "left" {
                                        l_c = cv.clone();
                                    }
                                }
                            }
                        }
                    }
                }
                self.bTopStyle.push(t_s.clone());
                self.bTopRgb.push(t_c.clone());
                self.bRightStyle.push(r_s.clone());
                self.bRightRgb.push(r_c.clone());
                self.bBottomStyle.push(b_s.clone());
                self.bBottomRgb.push(b_c.clone());
                self.bLeftStyle.push(l_s.clone());
                self.bLeftRgb.push(l_c.clone());
            }
            // Same reasoning as fonts: an empty <fill/> still holds its index.
            if t.borrow().name == "fill" && !t.borrow().isEnd {
                let mut frgb: String = String::new();
                let mut fhas: bool = false;
                let mut depth2: i64 = 1;
                if t.borrow().selfClose {
                    depth2 = 0;
                }
                while p.borrow().pos < p.borrow().srcLen && depth2 > 0 {
                    let ft2: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
                    if !ft2.borrow().valid {
                        break;
                    }
                    if ft2.borrow().name == "fill" {
                        if ft2.borrow().isEnd {
                            depth2 -= 1;
                        } else {
                            if !ft2.borrow().selfClose {
                                depth2 += 1;
                            }
                        }
                    } else {
                        if ft2.borrow().name == "fgColor" && !ft2.borrow().isEnd {
                            frgb = self.parseColorAttr(&ft2);
                            if !frgb.is_empty() {
                                fhas = true;
                            }
                        }
                    }
                }
                self.fillsRgb.push(frgb.clone());
                self.fillsHas.push(fhas);
            }
        }
        // Pass 2: cellXfs / xf only (style index used by cells)
        p.borrow_mut().reset(xml.to_string());
        let mut in_cell_xfs: bool = false;
        while p.borrow().pos < p.borrow().srcLen {
            let t2: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
            if !t2.borrow().valid {
                break;
            }
            if t2.borrow().name == "cellXfs" && !t2.borrow().isEnd {
                in_cell_xfs = true;
            }
            if t2.borrow().name == "cellXfs" && t2.borrow().isEnd {
                in_cell_xfs = false;
            }
            if in_cell_xfs {
                if t2.borrow().name == "xf" && !t2.borrow().isEnd {
                    let st: Rc<RefCell<CellStyle>> = Rc::new(RefCell::new(CellStyle::new()));
                    let font_id: i64 = XlsxStyles::parseInt0(&t2.borrow().attr("fontId"));
                    let fill_id: i64 = XlsxStyles::parseInt0(&t2.borrow().attr("fillId"));
                    let num_id: i64 = XlsxStyles::parseInt0(&t2.borrow().attr("numFmtId"));
                    if font_id >= 0 && font_id < (self.fontsBold.len() as i64) {
                        st.borrow_mut().bold = self.fontsBold[font_id as usize];
                        st.borrow_mut().italic = self.fontsItalic[font_id as usize];
                        st.borrow_mut().underline = self.fontsUnderline[font_id as usize];
                        st.borrow_mut().strike = self.fontsStrike[font_id as usize];
                        if font_id < (self.fontsVert.len() as i64) {
                            st.borrow_mut()
                                .vertAlign = self.fontsVert[font_id as usize].clone();
                        }
                        st.borrow_mut().fontSizePt = self.fontsSize[font_id as usize];
                        st.borrow_mut().textRgb = self.fontsColor[font_id as usize].clone();
                        if font_id < (self.fontsName.len() as i64) {
                            st.borrow_mut().fontName = self.fontsName[font_id as usize].clone();
                        }
                    }
                    let border_id: i64 = XlsxStyles::parseInt0(&t2.borrow().attr("borderId"));
                    if border_id >= 0 && border_id < (self.bTopStyle.len() as i64) {
                        st.borrow_mut()
                            .borderTop.borrow_mut()
                            .style = self.bTopStyle[border_id as usize].clone();
                        st.borrow_mut().borderTop.borrow_mut().rgb = XlsxStyles::borderColorOr(&self.bTopRgb[border_id as usize].clone());
                        st.borrow_mut()
                            .borderRight.borrow_mut()
                            .style = self.bRightStyle[border_id as usize].clone();
                        st.borrow_mut().borderRight.borrow_mut().rgb = XlsxStyles::borderColorOr(&self.bRightRgb[border_id as usize].clone());
                        st.borrow_mut()
                            .borderBottom.borrow_mut()
                            .style = self.bBottomStyle[border_id as usize].clone();
                        st.borrow_mut().borderBottom.borrow_mut().rgb = XlsxStyles::borderColorOr(&self.bBottomRgb[border_id as usize].clone());
                        st.borrow_mut()
                            .borderLeft.borrow_mut()
                            .style = self.bLeftStyle[border_id as usize].clone();
                        st.borrow_mut().borderLeft.borrow_mut().rgb = XlsxStyles::borderColorOr(&self.bLeftRgb[border_id as usize].clone());
                        let mut any: bool = false;
                        if st.borrow().borderTop.borrow().has() {
                            any = true;
                        }
                        if st.borrow().borderRight.borrow().has() {
                            any = true;
                        }
                        if st.borrow().borderBottom.borrow().has() {
                            any = true;
                        }
                        if st.borrow().borderLeft.borrow().has() {
                            any = true;
                        }
                        st.borrow_mut().hasBorder = any;
                    }
                    if fill_id >= 0 && fill_id < (self.fillsHas.len() as i64) {
                        st.borrow_mut().hasFill = self.fillsHas[fill_id as usize];
                        st.borrow_mut().fillRgb = self.fillsRgb[fill_id as usize].clone();
                    }
                    let id_key: String = num_id.to_string();
                    if self.numFmtMap.contains_key(&id_key) {
                        st.borrow_mut().numFmt = self.numFmtMap.get(&id_key).cloned().unwrap();
                    } else {
                        st.borrow_mut().numFmt = XlsxStyles::builtInNumFmt(num_id);
                    }
                    if !t2.borrow().selfClose {
                        let mut depth3: i64 = 1;
                        while p.borrow().pos < p.borrow().srcLen && depth3 > 0 {
                            let at: Rc<RefCell<XmlTag>> = p.borrow_mut().nextTag();
                            if !at.borrow().valid {
                                break;
                            }
                            if at.borrow().name == "xf" {
                                if at.borrow().isEnd {
                                    depth3 -= 1;
                                } else {
                                    if !at.borrow().selfClose {
                                        depth3 += 1;
                                    }
                                }
                            } else {
                                if at.borrow().name == "protection" && !at.borrow().isEnd {
                                    let lk: String = at.borrow().attr("locked");
                                    if lk == "0" || lk == "false" {
                                        st.borrow_mut().locked = false;
                                    }
                                }
                                if at.borrow().name == "alignment" && !at.borrow().isEnd {
                                    let h: String = at.borrow().attr("horizontal");
                                    if !h.is_empty() {
                                        st.borrow_mut().align = h.clone();
                                        // `general` is Excel's way of saying
                                        // "work it out", which is what an
                                        // unstated alignment means too — so it
                                        // is recorded as not stated.
                                        if h != "general" {
                                            st.borrow_mut().alignSet = true;
                                        }
                                    }
                                    let vv: String = at.borrow().attr("vertical");
                                    if !vv.is_empty() {
                                        st.borrow_mut().valign = vv.clone();
                                    }
                                    let wt: String = at.borrow().attr("wrapText");
                                    if wt == "1" || wt == "true" {
                                        st.borrow_mut().wrapText = true;
                                    }
                                    let ind: String = at.borrow().attr("indent");
                                    if !ind.is_empty() {
                                        st.borrow_mut().indent = XlsxStyles::parseInt0(&ind);
                                    }
                                    let rot: String = at.borrow().attr("textRotation");
                                    if !rot.is_empty() {
                                        st.borrow_mut().textRotation = XlsxStyles::parseInt0(&rot);
                                    }
                                    // Which way the cell reads. Excel's own
                                    // numbering: 1 left to right, 2 right to
                                    // left, 0 or absent "work it out from the
                                    // text". Nothing read this, so a cell an
                                    // author had explicitly marked right to
                                    // left was drawn whichever way its own
                                    // characters happened to suggest.
                                    let ro: String = at.borrow().attr("readingOrder");
                                    if !ro.is_empty() {
                                        st.borrow_mut().readingOrder = XlsxStyles::parseInt0(&ro);
                                    }
                                }
                            }
                        }
                    }
                    self.styles.push(st.clone());
                }
            }
        }
        if self.styles.is_empty() {
            self.styles.push(CellStyle::createDefault());
        }
    }
}
