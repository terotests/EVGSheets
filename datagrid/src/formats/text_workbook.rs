// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::rgr::{DgNative, EVGCodepoint, Utf8};
use crate::cell_style::CellStyle;
use crate::date_serial::DateSerial;
use crate::formats::sheet_bytes::SheetBytes;
use crate::spreadsheet_model::SpreadsheetModel;
use crate::workbook_model::WorkbookModel;
/// TextWorkbook: a .csv, .tsv or .txt file opened as a workbook of one sheet.
///
/// Data › From Text/CSV already adds a file's rows to the open workbook as a new
/// sheet. Opening one is the other thing Excel does with it: the file BECOMES
/// the workbook, named after the file, and Save offers .xlsx.
///
/// The bytes are decoded the way Excel guesses: a UTF-8 or UTF-16 byte-order
/// mark says so; otherwise UTF-8 when the bytes are valid UTF-8, else
/// Windows-1252. The separator is a tab for .tsv, and for the rest the one of
/// tab, semicolon and comma that splits the first lines most consistently.
/// Quoted fields may hold the separator, quotes ("") and line breaks.
///
/// Dates written the way DateSerial reads a typed date become dates with the
/// same look; everything else is kept as written and the sheet decides, as it
/// does for typed input, what is a number.
#[derive(Clone)]
pub struct TextWorkbook {
    pub lastError: String,
    pub separator: i64,
}

impl TextWorkbook {
    pub fn new() -> Self {
        Self {
            lastError: String::new(),
            separator: 44,
        }
    }
    pub fn validUtf8(b: &[u8], from: i64) -> bool {
        let n: i64 = b.len() as i64;
        let mut i: i64 = from;
        while i < n {
            let c: i64 = b[i as usize] as i64;
            if c < 128 {
                i += 1;
                continue;
            }
            let mut need: i64 = 0;
            if c >= 194 && c <= 223 {
                need = 1;
            }
            if c >= 224 && c <= 239 {
                need = 2;
            }
            if c >= 240 && c <= 244 {
                need = 3;
            }
            if need == 0 {
                return false;
            }
            if i + need >= n {
                // A sequence cut off by the end of the file is still UTF-8.
                return true;
            }
            let mut k: i64 = 1;
            while k <= need {
                let cc: i64 = b[(i + k) as usize] as i64;
                if cc < 128 || cc > 191 {
                    return false;
                }
                k += 1;
            }
            i = (i + need) + 1;
        }
        true
    }
    /// The file's text, decoded.
    pub fn decode(b: &[u8]) -> String {
        let n: i64 = b.len() as i64;
        if (n >= 2 && (b[0] as i64) == 255) && (b[1] as i64) == 254 {
            return SheetBytes::utf16(b, 2, ((n - 2) as f64 / 2.0).floor() as i64, true);
        }
        if (n >= 2 && (b[0] as i64) == 254) && (b[1] as i64) == 255 {
            // Big-endian: swap into little-endian and read that.
            let mut sw: Vec<u8> = vec![0u8; (n - 2) as usize];
            let mut i: i64 = 2;
            while i + 1 < n {
                sw[(i - 2) as usize] = (b[(i + 1) as usize] as i64) as u8;
                sw[(i - 1) as usize] = (b[i as usize] as i64) as u8;
                i += 2;
            }
            return SheetBytes::utf16(&sw, 0, ((n - 2) as f64 / 2.0).floor() as i64, true);
        }
        let mut start: i64 = 0;
        if ((n >= 3 && (b[0] as i64) == 239) && (b[1] as i64) == 187) && (b[2] as i64) == 191 {
            start = 3;
        }
        if TextWorkbook::validUtf8(b, start) {
            let mut raw: Vec<u8> = b.to_vec();
            if start > 0 {
                raw = b[(start) as usize..(n) as usize].to_vec();
            }
            return Utf8::decode(&String::from_utf8_lossy(&raw).to_string());
        }
        // Windows-1252: one character per byte.
        let mut parts: Vec<String> = Vec::new();
        let mut j: i64 = start;
        while j < n {
            let bv: i64 = b[j as usize] as i64;
            if bv < 128 {
                parts.push(char::from_u32(bv as u32).unwrap_or('\0').to_string());
            } else {
                parts.push(EVGCodepoint::to_str(Utf8::from_win_ansi(bv)));
            }
            j += 1;
        }
        parts.join("")
    }
    /// How many fields `sep` splits the first lines into, if it splits all of
    /// them alike; 0 when it does not.
    pub fn consistency(text: &str, sep: i64) -> i64 {
        let mut lines: i64 = 0;
        let mut fields: i64 = -1;
        let mut count: i64 = 1;
        let mut in_q: bool = false;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&text);
        while i < n && lines < 10 {
            let ch: i64 = DgNative::char_at(&text, i);
            if ch == 34 {
                in_q = !in_q;
            }
            if !in_q && ch == sep {
                count += 1;
            }
            if !in_q && ch == 10 {
                if fields < 0 {
                    fields = count;
                } else {
                    if count != fields {
                        return 0;
                    }
                }
                lines += 1;
                count = 1;
            }
            i += 1;
        }
        if fields < 0 {
            fields = count;
        }
        if fields < 2 {
            return 0;
        }
        fields
    }
    pub fn guessSeparator(text: &str) -> i64 {
        let mut best: i64 = 44;
        let mut best_n: i64 = 0;
        let cands: Vec<i64> = vec![9, 59, 44, 124];
        let mut i: i64 = 0;
        while i < (cands.len() as i64) {
            let s: i64 = cands[i as usize];
            let n: i64 = TextWorkbook::consistency(text, s);
            if n > best_n {
                best = s;
                best_n = n;
            }
            i += 1;
        }
        best
    }
    /// Every record of the text, each a list of fields.
    pub fn records(text: &str, sep: i64) -> Vec<Vec<String>> {
        let mut out: Vec<Vec<String>> = Vec::new();
        let mut row: Vec<String> = Vec::new();
        let mut cur: String = String::new();
        let mut in_q: bool = false;
        let mut run: i64 = 0;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(&text);
        while i < n {
            let ch: i64 = DgNative::char_at(&text, i);
            if in_q {
                if ch == 34 {
                    cur = format!("{}{}", cur, DgNative::substr(&text, run, i));
                    if i + 1 < n && DgNative::char_at(&text, i + 1) == 34 {
                        cur = format!("{}\"", cur);
                        i += 2;
                        run = i;
                        continue;
                    }
                    in_q = false;
                    i += 1;
                    run = i;
                    continue;
                }
                i += 1;
                continue;
            }
            if ch == 34 {
                cur = format!("{}{}", cur, DgNative::substr(&text, run, i));
                in_q = true;
                i += 1;
                run = i;
                continue;
            }
            if (ch == sep || ch == 10) || ch == 13 {
                cur = format!("{}{}", cur, DgNative::substr(&text, run, i));
                row.push(cur.clone());
                cur = String::new();
                if ch != sep {
                    if (ch == 13 && i + 1 < n) && DgNative::char_at(&text, i + 1) == 10 {
                        i += 1;
                    }
                    out.push(row.clone());
                    let fresh: Vec<String> = Vec::new();
                    row = fresh.clone();
                }
                i += 1;
                run = i;
                continue;
            }
            i += 1;
        }
        cur = format!("{}{}", cur, DgNative::substr(&text, run, n));
        if !cur.is_empty() || !row.is_empty() {
            row.push(cur.clone());
            out.push(row.clone());
        }
        out
    }
    /// The sheet as delimited text: what each cell shows, quoted where the
    /// separator, a quote or a line break would split it.
    pub fn sheetText(sh: &Rc<RefCell<SpreadsheetModel>>, sep: i64) -> String {
        let mut out: Vec<String> = Vec::new();
        let lr: i64 = sh.borrow().usedRow();
        let lc: i64 = sh.borrow().usedCol();
        let sep_text: String = char::from_u32(sep as u32).unwrap_or('\0').to_string();
        let mut r: i64 = 0;
        while r <= lr {
            let mut line: Vec<String> = Vec::new();
            let mut c: i64 = 0;
            while c <= lc {
                let v: String = sh.borrow().getDisplay(r, c);
                let q: bool = ((DgNative::index_of(&v, &sep_text) >= 0 || DgNative::index_of(&v, "\"") >= 0) || DgNative::index_of(&v, "\n") >= 0) || DgNative::index_of(&v, "\r") >= 0;
                if q {
                    line.push(format!("\"{}\"", (v.split("\"").map(|s| s.to_string()).collect::<Vec<String>>()).join("\"\"")));
                } else {
                    line.push(v.clone());
                }
                c += 1;
            }
            out.push(format!("{}\r\n", line.join(&*sep_text)));
            r += 1;
        }
        out.join("")
    }
    pub fn loadBytes(&mut self, bytes: &[u8], name: &str, kind: &str, book: &Rc<RefCell<WorkbookModel>>) -> bool {
        self.lastError = String::new();
        let text: String = TextWorkbook::decode(bytes);
        if kind == "tsv" {
            self.separator = 9;
        } else {
            self.separator = TextWorkbook::guessSeparator(&text);
        }
        let rows: Vec<Vec<String>> = TextWorkbook::records(&text, self.separator.clone());
        book.borrow_mut().clear();
        let sheet: Rc<RefCell<SpreadsheetModel>> = Rc::new(RefCell::new(SpreadsheetModel::new()));
        let mut base: String = SheetBytes::withExt(name, "");
        let mut slash: i64 = -1;
        let mut k: i64 = 0;
        while k < DgNative::str_len(&base) {
            if DgNative::char_at(&base, k) == 47 || DgNative::char_at(&base, k) == 92 {
                slash = k;
            }
            k += 1;
        }
        base = DgNative::substr(&base, slash + 1, DgNative::str_len(&base) - 1);
        if base.is_empty() {
            base = "Sheet1".to_string();
        }
        if DgNative::str_len(&base) > 31 {
            base = DgNative::substr(&base, 0, 31);
        }
        sheet.borrow_mut().name = base.clone();
        sheet.borrow_mut().quietWrites = true;
        let mut styles: Vec<Rc<RefCell<CellStyle>>> = Vec::new();
        styles.push(Rc::new(RefCell::new(CellStyle::new())));
        let mut date_style: HashMap<String,i64> = HashMap::new();
        let mut max_c: i64 = 0;
        let mut r: i64 = 0;
        while r < (rows.len() as i64) {
            let fields: Vec<String> = rows[r as usize].clone();
            if (fields.len() as i64) > max_c {
                max_c = fields.len() as i64;
            }
            r += 1;
        }
        if !rows.is_empty() {
            let mut nr: i64 = rows.len() as i64;
            let mut nc: i64 = max_c;
            if nr < sheet.borrow().rowCount {
                nr = sheet.borrow().rowCount;
            }
            if nc < sheet.borrow().colCount {
                nc = sheet.borrow().colCount;
            }
            sheet.borrow_mut().growTo(nr, nc);
        }
        let mut ri: i64 = 0;
        while ri < (rows.len() as i64) {
            let fs: Vec<String> = rows[ri as usize].clone();
            let mut ci: i64 = 0;
            while ci < (fs.len() as i64) {
                let v: String = fs[ci as usize].clone();
                if !v.is_empty() {
                    let mut serial: f64 = 0.0;
                    let mut is_date: bool = false;
                    if DgNative::str_len(&v) >= 6 && (DgNative::str_len(&v)) <= 10 {
                        is_date = DateSerial::isDate(&v);
                        serial = DateSerial::parseDate(&v);
                    }
                    if is_date {
                        let fmt: String = DateSerial::formatForTyped(&v);
                        if !date_style.contains_key(&fmt) {
                            let st: Rc<RefCell<CellStyle>> = Rc::new(RefCell::new(CellStyle::new()));
                            st.borrow_mut().numFmt = fmt.clone();
                            date_style.insert(fmt.clone(), styles.len() as i64);
                            styles.push(st.clone());
                        }
                        sheet.borrow_mut().setCellRaw(ri, ci, &(serial.floor() as i64).to_string());
                        sheet.borrow_mut().setCellStyleId(ri, ci, date_style.get(&fmt).cloned().unwrap());
                    } else {
                        sheet.borrow_mut().setCellRaw(ri, ci, &v);
                    }
                }
                ci += 1;
            }
            ri += 1;
        }
        sheet.borrow_mut().setStylesFrom(&styles);
        sheet.borrow_mut().quietWrites = false;
        sheet.borrow_mut().version = sheet.borrow().version + 1;
        book.borrow_mut().addSheet(sheet.clone());
        book.borrow_mut().setActive(0);
        true
    }
}
