// SPDX-License-Identifier: AGPL-3.0-or-later

use ranger::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::rgr::DgNative;

/// GridScript.rgr — a script that travels inside the workbook
///
/// An .xlsx is a ZIP package, and OPC lets a package carry parts nobody but its
/// author understands. So a script does not live beside the workbook in some
/// sidecar file that goes missing when the workbook is mailed on: it lives in
/// the package, under `ranger/`, and the loader and the writer both know about
/// it. Excel opens such a file as an ordinary spreadsheet and ignores the part
/// (it will drop it if IT saves the file — see docs/SCRIPTING.md); this editor
/// reads it, runs it and writes it back.
///
///   report.xlsx
///     [Content_Types].xml     <- Default Extension="tsx" declared here
///     xl/…                    the spreadsheet, exactly as before
///     ranger/summary.tsx      the script
///
/// This file is only the record and the naming rules; running one is
/// GridScriptEngine, and reading the workbook from inside one is GridScriptData.
#[derive(Clone)]
pub struct GridScript {
    /// What the UI calls it, and what the part is named after: "summary".
    pub name: String,
    /// The script itself — TSX evaluated by ComponentEngine.
    pub source: String,
    /// The package part it came from / goes back to: "ranger/summary.tsx".
    pub part: String,
}

impl GridScript {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            source: String::new(),
            part: String::new(),
        }
    }
    pub fn create(name: String, source: String) -> Rc<RefCell<GridScript>> {
        let s: Rc<RefCell<GridScript>> = Rc::new(RefCell::new(GridScript::new()));
        s.borrow_mut().name = name.clone();
        s.borrow_mut().source = source.clone();
        s.borrow_mut().part = GridScript::partFor(&name);
        s
    }
    pub fn partFor(name: &str) -> String {
        format!("ranger/{}.tsx", name)
    }
    /// Not `starts_with`/`ends_with`: Ranger's Rust writer passes a `&str`
    /// argument to them as `.as_str()`, which rustc does not have on `&str`.
    pub fn hasPrefix(s: &str, prefix: &str) -> bool {
        DgNative::index_of(s, prefix) == 0
    }
    pub fn hasSuffix(s: &str, suffix: &str) -> bool {
        let n: i64 = DgNative::str_len(s);
        let m: i64 = DgNative::str_len(suffix);
        n >= m && DgNative::substr(s, n - m, n) == suffix
    }
    /// Which package entries are scripts. Two extensions, because a report is
    /// TSX and a plain transform is not obliged to be.
    pub fn isScriptPart(path: &str) -> bool {
        if !GridScript::hasPrefix(path, "ranger/") {
            return false;
        }
        if GridScript::hasSuffix(path, ".tsx") {
            return true;
        }
        GridScript::hasSuffix(path, ".js")
    }
    /// "ranger/summary.tsx" -> "summary"
    pub fn nameOfPart(path: &str) -> String {
        let mut start: i64 = 0;
        let mut i: i64 = 0;
        let n: i64 = DgNative::str_len(path);
        while i < n {
            let ch: i64 = DgNative::char_at(path, i);
            if ch == 47 {
                start = i + 1;
            }
            i += 1;
        }
        let mut end: i64 = n;
        let mut j: i64 = start;
        while j < n {
            let ch2: i64 = DgNative::char_at(path, j);
            if ch2 == 46 {
                end = j;
            }
            j += 1;
        }
        if end < start {
            end = n;
        }
        DgNative::substr(path, start, end)
    }
    /// The extension the part name should keep, so a `.js` script written back
    /// does not silently become `.tsx`.
    pub fn extensionOfPart(path: &str) -> String {
        if GridScript::hasSuffix(path, ".js") {
            return ".js".to_string();
        }
        ".tsx".to_string()
    }
    /// What "New report script" puts in the workbook: a report over the sheet
    /// the user is looking at, written the way the docs say to write one — the
    /// data comes from the workbook, the columns size themselves, and the whole
    /// thing is short enough to read before editing.
    /// What "New report script" puts in the workbook: a report over the sheet
    /// the user is looking at, paginated, written the way the docs say to
    /// write one — the data comes from the workbook, the columns size
    /// themselves, and it is short enough to read before editing it.
    pub fn starter(sheet_name: &str) -> String {
        let mut out: String = String::new();
        out.push_str(&"// A report over the '".to_string());
        out.push_str(&sheet_name);
        out.push_str(&"' sheet.\n".to_string());
        out.push_str(&"// Run it with the Report button; Ctrl+P writes the PDF.\n".to_string());
        out.push_str(&"// Everything it prints is read from the workbook: sheetText() gives the\n".to_string());
        out.push_str(&"// cells as the grid displays them, number formats and all.\n".to_string());
        out.push_str(&"const SHEET = '".to_string());
        out.push_str(&sheet_name);
        out.push_str(&"';\n".to_string());
        out.push_str(&"const ROWS_PER_PAGE = 30;\n".to_string());
        out.push_str(&"const INK = '#1f2933';\n".to_string());
        out.push_str(&"const MUTED = '#7b8794';\n".to_string());
        out.push_str(&"\n".to_string());
        out.push_str(&"function line(values, weight, background) {\n".to_string());
        out.push_str(&"  const cells = [];\n".to_string());
        out.push_str(&"  // A hair under an even split: 100% exactly, plus the row's own padding,\n".to_string());
        out.push_str(&"  // is wider than the row, and the last column wraps onto a second line.\n".to_string());
        out.push_str(&"  const w = (96 / values.length) + '%';\n".to_string());
        out.push_str(&"  for (let c = 0; c < values.length; c++) {\n".to_string());
        out.push_str(&"    cells.push(<Label width={w} fontSize='10px' fontWeight={weight} color={INK}>{values[c]}</Label>);\n".to_string());
        out.push_str(&"  }\n".to_string());
        out.push_str(&"  return (<View flexDirection='row' padding='4px' backgroundColor={background}>{cells}</View>);\n".to_string());
        out.push_str(&"}\n".to_string());
        out.push_str(&"\n".to_string());
        out.push_str(&"function page(rows, first, pageNo, pageCount) {\n".to_string());
        out.push_str(&"  const body = [];\n".to_string());
        out.push_str(&"  for (let k = 0; k < ROWS_PER_PAGE; k++) {\n".to_string());
        out.push_str(&"    const i = first + k;\n".to_string());
        out.push_str(&"    if (i < rows.length) {\n".to_string());
        out.push_str(&"      let bg = '#ffffff';\n".to_string());
        out.push_str(&"      if (k % 2 == 1) { bg = '#f4f7fa'; }\n".to_string());
        out.push_str(&"      body.push(line(rows[i], 'normal', bg));\n".to_string());
        out.push_str(&"    }\n".to_string());
        out.push_str(&"  }\n".to_string());
        out.push_str(&"  return (\n".to_string());
        out.push_str(&"    <Page>\n".to_string());
        out.push_str(&"      <View flexDirection='column'>\n".to_string());
        out.push_str(&"        <Label fontSize='22px' fontWeight='bold' color={INK}>{SHEET}</Label>\n".to_string());
        out.push_str(&"        <Label fontSize='10px' color={MUTED} marginBottom='12px'>{rows.length - 1} rows from the workbook</Label>\n".to_string());
        out.push_str(&"        {line(rows[0], 'bold', '#e8edf3')}\n".to_string());
        out.push_str(&"        {body}\n".to_string());
        out.push_str(&"        <Label fontSize='9px' color={MUTED} marginTop='10px'>page {pageNo} of {pageCount}</Label>\n".to_string());
        out.push_str(&"      </View>\n".to_string());
        out.push_str(&"    </Page>\n".to_string());
        out.push_str(&"  );\n".to_string());
        out.push_str(&"}\n".to_string());
        out.push_str(&"\n".to_string());
        out.push_str(&"function render() {\n".to_string());
        out.push_str(&"  const rows = sheetText(SHEET);\n".to_string());
        out.push_str(&"  const dataRows = rows.length - 1;\n".to_string());
        out.push_str(&"  let pageCount = 0;\n".to_string());
        out.push_str(&"  for (let i = 0; i < dataRows; i += ROWS_PER_PAGE) { pageCount = pageCount + 1; }\n".to_string());
        out.push_str(&"  if (pageCount < 1) { pageCount = 1; }\n".to_string());
        out.push_str(&"  const pages = [];\n".to_string());
        out.push_str(&"  for (let p = 0; p < pageCount; p++) {\n".to_string());
        out.push_str(&"    pages.push(page(rows, 1 + (p * ROWS_PER_PAGE), p + 1, pageCount));\n".to_string());
        out.push_str(&"  }\n".to_string());
        out.push_str(&"  return (<Print><Section>{pages}</Section></Print>);\n".to_string());
        out.push_str(&"}\n".to_string());
        out
    }
}
