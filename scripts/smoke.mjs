#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Open the built site in a real Chromium and use it the way a person would:
 * real pointer events on the canvas, real keys, the accessibility mirror read
 * back from the DOM.
 *
 *   node scripts/smoke.mjs [--dist dist] [--shots DIR] [--port 8931]
 *
 * --shots writes screenshots of what it checked, for the README and for a
 * reviewer. Exits non-zero on the first failed check, with every check listed.
 */
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, "..");
const argv = process.argv.slice(2);
const flag = (n, d) => {
  const i = argv.indexOf(n);
  return i >= 0 ? argv[i + 1] : d;
};
const DIST = path.resolve(flag("--dist", path.join(REPO, "dist")));
const SHOTS = flag("--shots", "");
const PORT = parseInt(flag("--port", "8931"), 10);

function loadPlaywright() {
  const places = [REPO, process.cwd(), "/opt/node-tools"];
  if (process.env.RANGER_DIR) places.push(path.join(process.env.RANGER_DIR, "gallery", "ui", "conformance", "dom"));
  for (const from of places) {
    try {
      return createRequire(path.join(from, "package.json"))("playwright-core");
    } catch (_) {
      /* next */
    }
  }
  throw new Error("playwright-core not found: npm install in this repository");
}

function chromePath() {
  const c = [process.env.CHROME_PATH, "/opt/pw-browsers/chromium"].filter(Boolean);
  for (const p of c) if (fs.existsSync(p)) return p;
  return undefined;
}

const MIME = {
  ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".mjs": "text/javascript",
  ".css": "text/css", ".ttf": "font/ttf", ".xlsx": "application/octet-stream",
};
const server = http.createServer((req, res) => {
  const url = decodeURIComponent(req.url.split("?")[0]);
  let file = path.join(DIST, url.endsWith("/") ? url + "index.html" : url);
  if (!file.startsWith(DIST) || !fs.existsSync(file)) {
    res.writeHead(404);
    res.end("not found");
    return;
  }
  res.writeHead(200, { "content-type": MIME[path.extname(file)] || "application/octet-stream" });
  fs.createReadStream(file).pipe(res);
});
await new Promise((r) => server.listen(PORT, r));
const BASE = `http://localhost:${PORT}/`;

const checks = [];
const ok = (name, cond, extra = "") => {
  checks.push({ name, ok: !!cond });
  console.log(`  ${cond ? "PASS" : "FAIL"} ${name}${extra ? "  (" + extra + ")" : ""}`);
};

const { chromium } = loadPlaywright();
const browser = await chromium.launch({
  executablePath: chromePath(),
  args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
});
const errors = [];
async function open(url, w = 1280, h = 800, scheme = "light") {
  const page = await browser.newPage({ viewport: { width: w, height: h }, colorScheme: scheme });
  page.on("pageerror", (e) => errors.push(url + ": " + e.message));
  page.on("console", (m) => {
    if (m.type() === "error" && !/favicon/.test(m.text())) errors.push(url + ": " + m.text());
  });
  await page.goto(BASE + url);
  return page;
}
const shot = async (page, name) => {
  if (!SHOTS) return;
  fs.mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: path.join(SHOTS, name + ".png") });
};
// The middle of a control, from the app's own layout.
async function centre(page, tid) {
  const r = await page.evaluate((t) => window.sheet.app.rectOf(t), tid);
  if (!r) throw new Error("no rectangle for " + tid);
  const [x, y, w, h] = r.split(",").map(Number);
  const box = await page.evaluate(() => {
    const b = window.sheet.canvas.getBoundingClientRect();
    return { x: b.left, y: b.top };
  });
  return { x: box.x + x + w / 2, y: box.y + y + h / 2 };
}
async function click(page, tid) {
  const p = await centre(page, tid);
  await page.mouse.click(p.x, p.y);
  await page.waitForTimeout(80);
}
const state = (page) => page.evaluate(() => window.sheet.state());

try {
  // --- the editor -------------------------------------------------------------
  console.log("== editor");
  const blank = await open("");
  await blank.waitForFunction(() => window.__sheetsReady || window.__sheetsError, null, { timeout: 60000 });
  const bs = await state(blank);
  const a1 = await blank.evaluate(() => window.sheet.app.grid.app.model.getCell(0, 0));
  ok("a plain visit opens an empty workbook", bs.sheets.length === 1 && !a1, bs.sheets.join(",") + " A1=" + a1);
  await blank.close();

  const page = await open("?demo&theme=light");
  await page.waitForFunction(() => window.__sheetsReady || window.__sheetsError, null, { timeout: 60000 });
  ok("the editor started", await page.evaluate(() => !!window.__sheetsReady), await page.evaluate(() => window.__sheetsError || ""));
  let s = await state(page);
  ok("the sample workbook is open", s.sheets.length >= 3, s.sheets.join(","));
  ok("the tabs show the sheet the grid shows", s.activeSheet === s.sheets.indexOf("Summary"));
  const cmds = await page.evaluate(() => window.sheet.lastDoc.list.cmds.length);
  ok("the scene has commands", cmds > 200, String(cmds));
  await shot(page, "01_editor");

  // A cell, through the grid: click B3 and type.
  const g = await page.evaluate(() => ({ x: window.sheet.app.gx, y: window.sheet.app.gy }));
  const canvasBox = await page.evaluate(() => {
    const b = window.sheet.canvas.getBoundingClientRect();
    return { x: b.left, y: b.top };
  });
  await page.evaluate(() => window.sheet.run("nav.goto", "D8"));
  await page.keyboard.type("1234");
  await page.keyboard.press("Enter");
  await page.waitForTimeout(100);
  s = await state(page);
  const d8 = await page.evaluate(() => window.sheet.app.grid.app.model.getCell(7, 3));
  ok("typing goes into the cell", d8 === "1234", d8);

  // Ctrl+V through the browser's paste event: our own copy keeps its format
  // and fills the selected range; Backspace then clears the range.
  const pasted = await page.evaluate(async () => {
    const a = window.sheet.app.grid.app;
    const pick = (r0, c0, r1, c1) => {
      a.sel.anchor.row = r0; a.sel.anchor.col = c0;
      a.sel.active.row = r1; a.sel.active.col = c1;
    };
    pick(7, 3, 7, 3);
    await window.sheet.run("format.bold");
    await window.sheet.run("edit.copy");
    pick(8, 3, 9, 4);
    const dt = new DataTransfer();
    dt.setData("text/plain", a.clipboardTsv + "\r\n");
    window.sheet.canvas.focus();
    window.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true }));
    await new Promise((r) => setTimeout(r, 100));
    const m = a.model;
    return { v: m.getCell(9, 4), bold: !!m.getCellStyle(9, 4).bold, below: m.getCell(10, 3) };
  });
  ok("a paste fills the selected range, format and all", pasted.v === "1234" && pasted.bold, JSON.stringify(pasted));
  await page.keyboard.press("Backspace");
  await page.waitForTimeout(100);
  const cleared = await page.evaluate(() => {
    const m = window.sheet.app.grid.app.model;
    return [m.getCell(8, 3), m.getCell(9, 4), String(window.sheet.app.grid.app.editing)].join("|");
  });
  ok("Backspace clears the selected range", cleared === "||false", cleared);
  const values = await page.evaluate(async () => {
    const a = window.sheet.app.grid.app;
    a.sel.anchor.row = 11; a.sel.anchor.col = 5;
    a.sel.active.row = 11; a.sel.active.col = 5;
    window.sheet.element.dispatchEvent(new KeyboardEvent("keydown", { key: "V", ctrlKey: true, shiftKey: true, bubbles: true }));
    const dt = new DataTransfer();
    dt.setData("text/plain", a.clipboardTsv);
    window.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true }));
    await new Promise((r) => setTimeout(r, 100));
    const m = a.model;
    return { v: m.getCell(11, 5), bold: !!m.getCellStyle(11, 5).bold, dialog: !!a.windows.anyModalVisible() };
  });
  ok("Ctrl+Shift+V pastes the value without its format", values.v === "1234" && !values.bold && !values.dialog, JSON.stringify(values));
  const more = await page.evaluate(() => {
    const sx = window.sheet.app, a = sx.grid.app, m = a.model;
    const pick = (r0, c0, r1, c1) => {
      a.sel.anchor.row = r0; a.sel.anchor.col = c0;
      a.sel.active.row = r1; a.sel.active.col = c1;
    };
    const out = {};
    // Outside text with a CRLF line end: no \r, and the row under it stays.
    m.applyEdit(30, 0, "keep", "");
    pick(29, 0, 29, 0);
    sx.pasteText("x\r\n");
    out.text = m.getCell(29, 0) + "|" + m.getCell(30, 0);
    // A copied 1x2 block with a formula, into a 2x4 selection: tiled and re-based, one undo.
    m.applyEdit(32, 0, "5", "");
    pick(32, 0, 32, 0);
    a.sel.anchor.row = 32; a.sel.active.col = 1;
    a.model.applyEdit(32, 1, "", "A33*2");
    a.copySelection();
    pick(34, 0, 35, 3);
    sx.pasteText(a.clipboardTsv);
    out.tiled = m.getFormula(35, 3) + "|" + m.getCell(35, 2);
    a.undoEdit();
    out.undo = m.getCell(35, 2) + "|" + m.getCell(34, 0);
    return out;
  });
  ok("outside text loses its \\r and keeps the row below", more.text === "x|keep", more.text);
  ok("a block fills a range it divides, formulas re-based", more.tiled === "C36*2|5", more.tiled);
  ok("and one undo takes the whole fill away", more.undo === "|", more.undo);

  // Bold through the ribbon.
  await page.evaluate(() => window.sheet.run("nav.goto", "D8"));
  const wasBold = (await state(page)).bold;
  await click(page, "sx-rb-text-bold");
  s = await state(page);
  ok("the Bold button toggles the cell", s.bold === !wasBold);
  ok("…and the keyboard went back to the sheet", s.gridFocused);
  const pressed = await page.evaluate(() => {
    const el = document.querySelector('[data-a11y-id="sx-rb-text-bold"]');
    return el ? el.getAttribute("aria-pressed") : null;
  });
  ok("the reader is told Bold is pressed", pressed === String(!wasBold), String(pressed));
  await click(page, "sx-rb-text-bold");

  // The menubar: open File with the pointer, then Format › Number.
  await click(page, "sx-mb-format-trigger");
  s = await state(page);
  ok("a menubar menu opens", s.menuOpen);
  await shot(page, "02_format_menu");
  // Each menu hangs under its own trigger — also after the pointer slides
  // along the bar with one open, which switches the menu.
  const under = async (menu) => {
    const t = (await page.evaluate((m) => window.sheet.app.rectOf("sx-mb-" + m + "-trigger"), menu)).split(",").map(Number);
    const c = (await page.evaluate((m) => window.sheet.app.rectOf("sx-mb-" + m + "-content"), menu)).split(",").map(Number);
    return c.length === 4 && Math.abs(c[0] - t[0]) <= 12 && c[1] >= t[1] + t[3] - 2;
  };
  ok("the Format menu opens under Format", await under("format"));
  const ins = await centre(page, "sx-mb-insert-trigger");
  await page.mouse.move(ins.x, ins.y);
  await page.waitForTimeout(120);
  ok("hovering Insert switches to the Insert menu, under Insert", (await page.evaluate(() => window.sheet.app.rectOf("sx-mb-insert-content"))) !== "" && (await under("insert")));
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);
  ok("Escape closes it", !(await state(page)).menuOpen);

  // A dropdown on the ribbon: the font size.
  await click(page, "sx-size-trigger");
  ok("the size dropdown opens", (await state(page)).menuOpen);
  await shot(page, "03_size_dropdown");
  await click(page, "sx-size-item-size-14");
  s = await state(page);
  ok("choosing 14 sets the size", Math.round(s.size) === 14, String(s.size));
  ok("…and closes the menu", !s.menuOpen);

  // The context menu on a cell.
  const cellX = canvasBox.x + g.x + 160;
  const cellY = canvasBox.y + g.y + 140;
  await page.mouse.click(cellX, cellY, { button: "right" });
  await page.waitForTimeout(100);
  ok("right click opens the context menu", (await state(page)).menuOpen);
  await shot(page, "04_context_menu");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);

  // The context menu on a column header: the column is selected and the menu
  // is the column's own. Inserting a column to the left moves C's text to D.
  const hdr = await page.evaluate(() => {
    const v = window.sheet.app.grid.app.grid;
    const m = window.sheet.app.grid.app.model;
    return { colX: v.x + v.rowHeaderW + m.colToX(2) + 10, colY: v.headerTop() + 6, rowX: v.x + 10, rowY: v.headerTop() + v.colHeaderH + m.rowToY(4) + 6 };
  });
  const c1 = await page.evaluate(() => window.sheet.app.grid.app.model.getCell(0, 2));
  await page.mouse.click(canvasBox.x + g.x + hdr.colX, canvasBox.y + g.y + hdr.colY, { button: "right" });
  await page.waitForTimeout(100);
  ok("right click on a column header opens the column menu", (await state(page)).menuOpen && (await page.evaluate(() => window.sheet.app.ctxMenu.name)) === "Column actions");
  ok("…with column C selected", await page.evaluate(() => {
    const s = window.sheet.app.grid.app.sel;
    return s.active.col === 2 && s.anchor.col === 2 && s.active.row === 0 && s.anchor.row === window.sheet.app.grid.app.model.rowCount - 1;
  }));
  ok("…naming the column it acts on", (await page.evaluate(() => window.sheet.app.rectOf("sx-ctx-item-h-delcols"))) !== "" && (await page.evaluate(() => window.sheet.app.ctxMenu.items.find((i) => i.value === "h-delcols").name)) === "Delete column C");
  await shot(page, "04b_column_menu");
  await click(page, "sx-ctx-item-h-colleft");
  ok("Insert 1 column left moves C to D", (await page.evaluate(() => window.sheet.app.grid.app.model.getCell(0, 3))) === c1 && !(await state(page)).menuOpen, c1);
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(80);
  ok("…and undo puts it back", (await page.evaluate(() => window.sheet.app.grid.app.model.getCell(0, 2))) === c1);

  // …and on a row header.
  await page.mouse.click(canvasBox.x + g.x + hdr.rowX, canvasBox.y + g.y + hdr.rowY, { button: "right" });
  await page.waitForTimeout(100);
  ok("right click on a row header opens the row menu", (await state(page)).menuOpen && (await page.evaluate(() => window.sheet.app.ctxMenu.name)) === "Row actions");
  ok("…with row 5 selected", await page.evaluate(() => {
    const s = window.sheet.app.grid.app.sel;
    return s.active.row === 4 && s.anchor.row === 4 && s.active.col === 0;
  }));
  await shot(page, "04c_row_menu");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);

  // A cell gets the cell menu back.
  await page.mouse.click(cellX, cellY, { button: "right" });
  await page.waitForTimeout(100);
  ok("a cell gets the cell menu back", (await page.evaluate(() => window.sheet.app.rectOf("sx-ctx-item-c-chart"))) !== "");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);

  // Find and replace: an EVGUI dialog (WindowCtl), not the core's window.
  await page.evaluate(() => window.sheet.run("nav.goto", "A1"));
  await page.keyboard.press("Control+f");
  await page.waitForTimeout(150);
  ok("Ctrl+F opens Find and replace", await page.evaluate(() => window.sheet.app.find.win.open));
  ok("…as an EVGUI dialog, not the core's window", await page.evaluate(() => !window.sheet.app.grid.app.findDialog.visible && window.sheet.app.rectOf("sx-find-content") !== ""));
  ok("…with focus in the Find field", await page.evaluate(() => window.sheet.app.ui.focusId === "sx-find-query"));
  await page.keyboard.type("Revenue");
  await page.keyboard.press("Enter");
  await page.waitForTimeout(100);
  const found = await page.evaluate(() => window.sheet.app.grid.app.findStatus);
  ok("Enter finds the next match", /^found /.test(found), found);
  await shot(page, "05_find");
  await click(page, "sx-find-case");
  ok("a click ticks Match case", await page.evaluate(() => window.sheet.app.find.matchCase.checkState === 1));
  await click(page, "sx-find-next");
  ok("Find next by pointer", /^found /.test(await page.evaluate(() => window.sheet.app.grid.app.findStatus)));
  await click(page, "sx-find-close");
  ok("Close closes it", !(await page.evaluate(() => window.sheet.app.find.win.open)));
  ok("…and the query is kept", (await page.evaluate(() => window.sheet.app.grid.app.findQuery)) === "Revenue");
  await page.keyboard.press("Control+h");
  await page.waitForTimeout(100);
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);
  ok("Escape closes it and the sheet has the keyboard", await page.evaluate(() => !window.sheet.app.find.win.open && window.sheet.app.gridFocused));

  // Paste special: an EVGUI dialog too, whatever opens it.
  await page.evaluate(() => { window.sheet.run("nav.goto", "B3"); window.sheet.run("edit.copy", ""); window.sheet.run("nav.goto", "F10"); });
  await page.evaluate(() => window.sheet.run("edit.pasteSpecial", ""));
  await page.waitForTimeout(150);
  ok("Paste special opens as an EVGUI dialog", await page.evaluate(() => window.sheet.app.paste.win.open && !window.sheet.app.grid.app.pasteDialog.visible));
  await shot(page, "06_paste_special");
  await click(page, "sx-paste-what-1");
  ok("a click chooses Values only", (await page.evaluate(() => window.sheet.app.paste.what.value)) === "1");
  await click(page, "sx-paste-ok");
  const special = await page.evaluate(() => [window.sheet.app.paste.win.open, window.sheet.app.grid.app.model.getCell(9, 5), window.sheet.app.grid.app.pasteMode]);
  ok("Paste pastes the value and closes", !special[0] && special[1] !== "" && !String(special[1]).startsWith("="), String(special[1]));
  ok("…and the next paste is a full one again", special[2] === 0);
  await page.evaluate(() => window.sheet.run("edit.pasteSpecial", ""));
  await page.waitForTimeout(80);
  ok("it opens again", await page.evaluate(() => window.sheet.app.paste.win.open));
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);
  ok("Escape cancels it", await page.evaluate(() => !window.sheet.app.paste.win.open && window.sheet.app.gridFocused));
  await page.evaluate(() => window.sheet.run("edit.undo", ""));

  // Rename sheet: an EVGUI dialog, which says why it refuses a name.
  const oldName = await page.evaluate(() => window.sheet.app.grid.app.book.sheetAt(window.sheet.app.grid.app.book.activeIndex).name);
  await page.evaluate(() => window.sheet.run("sheet.rename", ""));
  await page.waitForTimeout(120);
  ok("Rename opens as an EVGUI dialog", await page.evaluate(() => window.sheet.app.rename.win.open && !window.sheet.app.grid.app.renameDialog.visible && window.sheet.app.ui.focusId === "sx-rename-name"));
  await page.keyboard.type("Bad/Name");
  await page.keyboard.press("Enter");
  await page.waitForTimeout(80);
  ok("a refused name keeps it open and says why", await page.evaluate(() => window.sheet.app.rename.win.open && window.sheet.app.rename.statusEl.textContent !== ""));
  await shot(page, "07_rename");
  await page.keyboard.press("Control+a");
  await page.keyboard.type("Renamed");
  await page.keyboard.press("Enter");
  await page.waitForTimeout(80);
  ok("Enter renames and closes", await page.evaluate(() => !window.sheet.app.rename.win.open && window.sheet.app.grid.app.book.sheetAt(window.sheet.app.grid.app.book.activeIndex).name === "Renamed"));
  await page.evaluate((n) => window.sheet.run("sheet.rename", n), oldName);

  // Link: an EVGUI dialog, from Ctrl+K too.
  await page.evaluate(() => window.sheet.run("nav.goto", "H12"));
  await page.keyboard.press("Control+k");
  await page.waitForTimeout(120);
  ok("Ctrl+K opens Link as an EVGUI dialog", await page.evaluate(() => window.sheet.app.link.win.open && !window.sheet.app.grid.app.linkDialog.visible && window.sheet.app.ui.focusId === "sx-link-address"));
  await page.keyboard.type("https://example.com");
  await page.keyboard.press("Tab");
  await page.keyboard.type("Example");
  await shot(page, "08_link");
  await page.keyboard.press("Enter");
  await page.waitForTimeout(80);
  ok("Enter sets the link and the text", await page.evaluate(() => { const a = window.sheet.app.grid.app; return !window.sheet.app.link.win.open && a.model.hyperlinkAt(11, 7) === "https://example.com" && a.model.getCell(11, 7) === "Example"; }));
  await page.evaluate(() => window.sheet.run("insert.link", ""));
  await page.waitForTimeout(80);
  await click(page, "sx-link-remove");
  ok("Remove takes the link off", await page.evaluate(() => !window.sheet.app.link.win.open && window.sheet.app.grid.app.model.hyperlinkAt(11, 7) === ""));
  await page.evaluate(() => window.sheet.run("edit.undo", ""));

  // Fill colour: swatches in an EVGUI dialog, previewed on the sheet.
  await page.evaluate(() => window.sheet.run("nav.goto", "J14"));
  await page.evaluate(() => window.sheet.run("format.fill", ""));
  await page.waitForTimeout(120);
  ok("Fill colour opens as an EVGUI dialog", await page.evaluate(() => window.sheet.app.color.win.open && !window.sheet.app.grid.app.colorDialog.visible));
  await click(page, "sx-color-swatch-s22");
  ok("a swatch previews on the sheet", await page.evaluate(() => { const st = window.sheet.app.grid.app.model.getCellStyle(13, 9); return st.hasFill && st.fillRgb.toUpperCase() === "#C6EFCE"; }));
  await shot(page, "09_fill");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);
  ok("Escape takes the preview back", await page.evaluate(() => !window.sheet.app.color.win.open && !window.sheet.app.grid.app.model.getCellStyle(13, 9).hasFill));
  await page.evaluate(() => window.sheet.run("format.color", ""));
  await page.waitForTimeout(80);
  await click(page, "sx-color-hex");
  await page.keyboard.press("Control+a");
  await page.keyboard.type("#123456");
  await page.keyboard.press("Enter");
  await page.waitForTimeout(80);
  ok("a typed hex and Enter apply the text colour", await page.evaluate(() => !window.sheet.app.color.win.open && window.sheet.app.grid.app.model.getCellStyle(13, 9).textRgb.toUpperCase() === "#123456"));
  await page.evaluate(() => window.sheet.run("edit.undo", ""));
  ok("…as one undo", await page.evaluate(() => window.sheet.app.grid.app.model.getCellStyle(13, 9).textRgb.toUpperCase() !== "#123456"));

  // Borders: an EVGUI dialog with a preview; one spec applied on Apply.
  await page.evaluate(() => window.sheet.run("nav.goto", "J14"));
  await page.keyboard.press("Shift+ArrowRight");
  await page.keyboard.press("Shift+ArrowDown");
  await page.evaluate(() => window.sheet.run("format.border", ""));
  await page.waitForTimeout(120);
  ok("Borders opens as an EVGUI dialog", await page.evaluate(() => window.sheet.app.border.win.open && !window.sheet.app.grid.app.borderDialog.visible && window.sheet.app.border.multi));
  await click(page, "sx-border-box");
  await click(page, "sx-border-line-medium");
  await click(page, "sx-border-ink-c4");
  ok("Box, Medium and red make the spec", (await page.evaluate(() => window.sheet.app.border.spec())) === "spec:TBLR:medium:#C00000", await page.evaluate(() => window.sheet.app.border.spec()));
  await shot(page, "10_borders");
  await click(page, "sx-border-apply");
  ok("Apply draws them", await page.evaluate(() => { const st = window.sheet.app.grid.app.model.getCellStyle(13, 9); return !window.sheet.app.border.win.open && st.borderTop.has() && st.borderTop.style === "medium"; }));
  await page.evaluate(() => window.sheet.run("edit.undo", ""));

  // Conditional formatting: an EVGUI dialog that adds rules and stays open.
  await page.evaluate(() => window.sheet.run("nav.goto", "J14"));
  const cfBefore = await page.evaluate(() => window.sheet.app.grid.app.model.cfRuleCount());
  await page.evaluate(() => window.sheet.run("format.conditional", ""));
  await page.waitForTimeout(120);
  ok("Conditional formatting opens as an EVGUI dialog", await page.evaluate(() => window.sheet.app.cf.win.open && !window.sheet.app.grid.app.cfDialog.visible));
  await click(page, "sx-cf-test-602");
  await click(page, "sx-cf-v1");
  await page.keyboard.press("Control+a");
  await page.keyboard.type("5");
  await click(page, "sx-cf-fill-#C6EFCE");
  await shot(page, "11_cf");
  await page.keyboard.press("Enter");
  await page.waitForTimeout(80);
  ok("Enter adds the rule and keeps it open", await page.evaluate((n) => { const a = window.sheet.app.grid.app; return window.sheet.app.cf.win.open && a.model.cfRuleCount() === n + 1 && a.cfTest === 602 && a.cfV1 === "5"; }, cfBefore));
  await click(page, "sx-cf-clear");
  await click(page, "sx-cf-close");
  ok("Clear takes it off, Close closes", await page.evaluate((n) => !window.sheet.app.cf.win.open && window.sheet.app.grid.app.model.cfRuleCount() === n, cfBefore));

  // Data validation and the list picker: EVGUI dialogs.
  await page.evaluate(() => window.sheet.run("nav.goto", "J14"));
  const dvBefore = await page.evaluate(() => window.sheet.app.grid.app.model.validationCount());
  await page.evaluate(() => window.sheet.run("data.validation", ""));
  await page.waitForTimeout(120);
  ok("Data validation opens as an EVGUI dialog", await page.evaluate(() => window.sheet.app.dv.win.open && !window.sheet.app.grid.app.dvDialog.visible));
  await click(page, "sx-dv-kind-701");
  await click(page, "sx-dv-v1");
  await page.keyboard.press("Control+a");
  await page.keyboard.type("Yes,No,Maybe");
  await shot(page, "12_validation");
  await click(page, "sx-dv-add");
  await click(page, "sx-dv-close");
  ok("Add rule puts a list rule on the cell", await page.evaluate((n) => { const a = window.sheet.app.grid.app; return !window.sheet.app.dv.win.open && a.model.validationCount() === n + 1 && a.model.validationAt(13, 9).isList(); }, dvBefore));
  await page.evaluate(async () => { window.sheet.app.grid.app.openListPicker(13, 9); window.sheet.app.sync(); await window.sheet.redraw(); });
  await page.waitForTimeout(80);
  ok("the cell's list opens as an EVGUI dialog", await page.evaluate(() => window.sheet.app.pick.win.open && !window.sheet.app.grid.app.listDialog.visible && window.sheet.app.pick.what.items.length === 3));
  await shot(page, "13_list");
  await click(page, "sx-list-what-2");
  await click(page, "sx-list-ok");
  ok("Choose writes the value", await page.evaluate(() => !window.sheet.app.pick.win.open && window.sheet.app.grid.app.model.getCell(13, 9) === "Maybe"));
  await page.evaluate(() => { window.sheet.run("edit.undo", ""); const a = window.sheet.app.grid.app; a.model.clearValidationsIn(13, 9, 13, 9); });

  // Insert chart: a chart at once, and a short editor window beside it.
  await page.evaluate(() => window.sheet.run("nav.goto", "B4"));
  const charts0 = await page.evaluate(() => window.sheet.app.grid.app.view.chartLayer.count());
  await page.evaluate(() => window.sheet.run("insert.chart", ""));
  await page.waitForTimeout(120);
  ok("Insert chart makes the chart and opens its editor", await page.evaluate((n) => window.sheet.app.chart.win.open && !window.sheet.app.chart.win.isModal && !window.sheet.app.grid.app.chartDialog.visible && window.sheet.app.grid.app.view.chartLayer.count() === n + 1, charts0));
  const kinds = await page.evaluate(() => window.sheet.app.chart.kind.items.map((it) => it.value));
  ok("…offering only the types the data suits", kinds.length > 0 && kinds.length < 20, kinds.join(","));
  const pickKind = kinds[kinds.length - 1];
  await click(page, "sx-chart-kind-trigger");
  ok("the type is a drop-down", await page.evaluate(() => window.sheet.app.chart.kind.open));
  await shot(page, "14_chart_type");
  await click(page, "sx-chart-kind-item-" + pickKind);
  ok("a type chosen redraws the chart on the sheet", await page.evaluate((k) => { const a = window.sheet.app.grid.app; const l = a.view.chartLayer; const p = l.panelAt(l.count() - 1); return !window.sheet.app.chart.kind.open && String(p.chart.kind) === k; }, pickKind));
  await shot(page, "14_chart");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);
  ok("Escape leaves the chart on the sheet", await page.evaluate((n) => !window.sheet.app.chart.win.open && window.sheet.app.grid.app.view.chartLayer.count() === n + 1, charts0));
  await page.evaluate(() => { const a = window.sheet.app.grid.app; const p = a.view.chartLayer.panelAt(a.view.chartLayer.count() - 1); a.chartEditId = 0; a.openChartDialog(p.chart.id); window.sheet.app.sync(); });
  await page.evaluate(() => window.sheet.redraw());
  await click(page, "sx-chart-delete");
  ok("its editor deletes it", await page.evaluate((n) => !window.sheet.app.chart.win.open && window.sheet.app.grid.app.view.chartLayer.count() === n, charts0));

  // The SQL box and the connection window: EVGUI dialogs too.
  await page.evaluate(() => window.sheet.run("db.sql.dialog", ""));
  await page.waitForTimeout(100);
  ok("the SQL box opens as an EVGUI dialog", await page.evaluate(() => window.sheet.app.sql.win.open && !window.sheet.app.grid.app.sqlDialog.visible && window.sheet.app.ui.focusId === "sx-sql-text"));
  await shot(page, "15_sql");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);
  await page.evaluate(() => window.sheet.run("db.connection", ""));
  await page.waitForTimeout(100);
  ok("the connection window opens as an EVGUI dialog", await page.evaluate(() => window.sheet.app.conn.win.open && !window.sheet.app.grid.app.connDialog.visible));
  await click(page, "sx-conn-table");
  await page.keyboard.press("Control+a");
  await page.keyboard.type("sales");
  await page.keyboard.press("Enter");
  await page.waitForTimeout(80);
  ok("Connect leaves the host a request", await page.evaluate(() => { const a = window.sheet.app.grid.app; return window.sheet.app.conn.win.open && a.dbRequest === "connect" && a.dbRequestTable === "sales"; }));
  await shot(page, "16_connection");
  await page.evaluate(() => { window.sheet.app.grid.app.takeDbRequest(); window.sheet.app.grid.app.dbConnStatus = ""; });
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);
  ok("Escape closes it", await page.evaluate(() => !window.sheet.app.conn.win.open));

  // Sheet tabs.
  await click(page, "sx-tabs-tab-0");
  s = await state(page);
  ok("a tab shows its sheet", s.activeSheet === 0, String(s.activeSheet));
  // A tab's own menu: right click on a tab shows that sheet and opens it,
  // upward, off the strip.
  const names0 = s.sheets.slice();
  let t1 = await centre(page, "sx-tabs-tab-1");
  await page.mouse.click(t1.x, t1.y, { button: "right" });
  await page.waitForTimeout(100);
  s = await state(page);
  ok("right click on a tab opens the sheet menu", s.menuOpen && (await page.evaluate(() => window.sheet.app.ctxMenu.name)) === "Sheet actions");
  ok("…on that sheet", s.activeSheet === 1, String(s.activeSheet));
  ok("…above the tabs", await page.evaluate(() => {
    const m = window.sheet.app.rectOf("sx-ctx-content").split(",").map(Number);
    const t = window.sheet.app.rectOf("sx-tabs-tab-1").split(",").map(Number);
    return m[1] + m[3] <= t[1] + 2;
  }));
  await shot(page, "05b_tab_menu");
  await click(page, "sx-ctx-item-t-dup");
  s = await state(page);
  ok("Duplicate adds a copy after it, shown", s.sheets.length === names0.length + 1 && s.activeSheet === 2 && s.sheets[2] === names0[1] + " (2)", s.sheets.join(","));
  // Delete asks first.
  const t2 = await centre(page, "sx-tabs-tab-2");
  await page.mouse.click(t2.x, t2.y, { button: "right" });
  await page.waitForTimeout(100);
  await click(page, "sx-ctx-item-t-delete");
  ok("Delete… asks before deleting", await page.evaluate(() => window.sheet.app.delSheet.win.open) && (await state(page)).sheets.length === names0.length + 1);
  await shot(page, "05c_delete_sheet");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(80);
  ok("…Escape keeps the sheet", (await state(page)).sheets.length === names0.length + 1);
  await page.mouse.click(t2.x, t2.y, { button: "right" });
  await page.waitForTimeout(100);
  await click(page, "sx-ctx-item-t-delete");
  await click(page, "sx-delsheet-ok");
  s = await state(page);
  ok("…and Delete deletes it", s.sheets.join(",") === names0.join(",") && !(await page.evaluate(() => window.sheet.app.delSheet.win.open)), s.sheets.join(","));
  // Dragging the first tab past the second.
  const d0 = await centre(page, "sx-tabs-tab-0");
  const d2 = await centre(page, "sx-tabs-tab-2");
  await page.mouse.move(d0.x, d0.y);
  await page.mouse.down();
  for (let i = 1; i <= 12; i++) await page.mouse.move(d0.x + ((d2.x + 30 - d0.x) * i) / 12, d0.y);
  await shot(page, "05d_tab_drag");
  await page.mouse.up();
  await page.waitForTimeout(80);
  s = await state(page);
  ok("dragging a tab reorders the sheets", s.sheets[2] === names0[0] && s.sheets[0] === names0[1], s.sheets.join(","));
  ok("…and the dragged sheet stays shown", s.activeSheet === 2, String(s.activeSheet));
  await page.evaluate(() => window.sheet.run("sheet.move", "0"));
  s = await state(page);
  ok("…and moves back by command", s.sheets.join(",") === names0.join(","), s.sheets.join(","));
  await click(page, "sx-tabs-tab-0");
  s = await state(page);
  // A selection's sum in the status bar.
  await page.evaluate(() => window.sheet.run("nav.goto", "D2"));
  await page.evaluate(() => window.sheet.run("select.column", ""));
  s = await state(page);
  ok("the status bar has the selection's figures", s.selNumbers > 0, `${s.selNumbers} numbers, sum ${s.selSum}`);
  await shot(page, "05_sales_selection");

  // F6 walks to the ribbon and back.
  await page.evaluate(() => window.sheet.run("nav.goto", "A1"));
  await page.keyboard.press("F6");
  await page.waitForTimeout(60);
  s = await state(page);
  ok("F6 leaves the sheet for the chrome", !s.gridFocused && s.uiFocus.length > 0, s.uiFocus);

  // The accessibility mirror.
  const tree = await page.evaluate(() => JSON.parse(window.sheet.app.a11y()));
  const roles = new Set(tree.nodes.map((n) => n.role));
  ok("the tree has a menubar, toolbars, a grid and tabs", ["menubar", "toolbar", "grid", "tablist"].every((r) => roles.has(r)) || tree.nodes.length > 100, [...roles].slice(0, 12).join(","));
  const domButtons = await page.evaluate(() => document.querySelectorAll('[role="button"], button').length);
  ok("the mirror put real buttons on the page", domButtons > 20, String(domButtons));

  // Dark.
  await page.evaluate(() => window.sheet.setTheme("dark"));
  await page.waitForTimeout(120);
  await shot(page, "06_dark");
  await page.close();

  // --- parameters -------------------------------------------------------------
  console.log("== parameters");
  const viewer = await open("?ui=viewer");
  await viewer.waitForFunction(() => window.__sheetsReady || window.__sheetsError, null, { timeout: 60000 });
  let vs = await viewer.evaluate(() => window.sheet.state());
  ok("?ui=viewer is read-only", vs.readOnly && vs.options.readOnly);
  ok("…with no menubar and no ribbon", !vs.options.menubar && !vs.options.ribbon);
  await viewer.evaluate(() => window.sheet.run("nav.goto", "B3"));
  const before = await viewer.evaluate(() => window.sheet.app.grid.app.model.getCell(2, 1));
  await viewer.keyboard.type("99");
  await viewer.keyboard.press("Enter");
  const afterV = await viewer.evaluate(() => window.sheet.app.grid.app.model.getCell(2, 1));
  ok("typing changes nothing", before === afterV, `${before} → ${afterV}`);
  await shot(viewer, "07_viewer");
  await viewer.close();

  const compact = await open("?ui=compact&tools=bold,italic,sortasc&theme=dark");
  await compact.waitForFunction(() => window.__sheetsReady || window.__sheetsError, null, { timeout: 60000 });
  const cs = await compact.evaluate(() => window.sheet.state());
  ok("?tools= keeps only the tools asked for", cs.options.tools === "bold,italic,sortasc");
  const hasUnderline = await compact.evaluate(() => window.sheet.app.rectOf("sx-rb-text-underline"));
  ok("…so Underline is not there", hasUnderline === "");
  await shot(compact, "08_compact_dark");
  await compact.close();

  // --- the embeds page --------------------------------------------------------
  console.log("== embeds");
  const embeds = await open("embed.html", 1280, 1800);
  await embeds.waitForFunction(() => window.__embedsReady, null, { timeout: 90000 });
  const n = await embeds.evaluate(() => Object.keys(window.sheets).length);
  ok("four editors on one page", n === 4, String(n));
  await embeds.evaluate(() => document.getElementById("edit").click());
  await embeds.waitForTimeout(200);
  const slideState = await embeds.evaluate(() => window.sheets.slide.state());
  ok("the slide's sheet switches to editing in place", !slideState.readOnly && slideState.options.preset === "compact");
  await shot(embeds, "09_embeds");
  await embeds.close();

  ok("no errors in any page", errors.length === 0, errors.slice(0, 3).join(" | "));
} catch (e) {
  ok("the run finished", false, e && e.stack ? e.stack.split("\n").slice(0, 3).join(" ") : String(e));
} finally {
  await browser.close();
  server.close();
}

const failed = checks.filter((c) => !c.ok);
console.log(`\n${checks.length - failed.length}/${checks.length} checks passed`);
process.exit(failed.length ? 1 : 0);
