#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * The UI half of the parity benchmark: opens the built editor in Chromium
 * with the target workbook (parity/fixtures/quarterly-sales-dashboard.xlsx,
 * a "Quarterly Sales Dashboard" Excel screenshot rebuilt as a file by
 * make_dashboard.py) at the screenshot's size, and records what is there.
 * Screenshots of Excel itself are not committed.
 *
 *   node tools/parity/probe.mjs [--dist dist] [--port 8932]
 *
 * Writes parity/probe.json (one pass/fail per check id, read by report.mjs,
 * plus the registered command ids) and docs/parity/evgsheets.png. Checks read
 * the app's own state, the accessibility mirror, and pixels of the rendered
 * canvas; none of them is a judgement by eye. The ids are the `probe:` tests
 * named in parity/ui.tsv.
 */
import http from "node:http";
import fs from "node:fs";
import zlib from "node:zlib";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, "..", "..");
const argv = process.argv.slice(2);
const flag = (n, d) => {
  const i = argv.indexOf(n);
  return i >= 0 ? argv[i + 1] : d;
};
const DIST = path.resolve(flag("--dist", path.join(REPO, "dist")));
const PORT = parseInt(flag("--port", "8932"), 10);
const FIXTURE = path.join(REPO, "parity", "fixtures", "quarterly-sales-dashboard.xlsx");
const W = 1650, H = 1030; // the size of the Excel screenshot

function loadPlaywright() {
  for (const from of [REPO, process.cwd(), "/opt/node-tools"]) {
    try {
      return createRequire(path.join(from, "package.json"))("playwright-core");
    } catch (_) {
      /* next */
    }
  }
  throw new Error("playwright-core not found: npm install in this repository");
}
const chromePath = () => [process.env.CHROME_PATH, "/opt/pw-browsers/chromium"].filter(Boolean).find((p) => fs.existsSync(p));

const MIME = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".ttf": "font/ttf" };
const server = http.createServer((req, res) => {
  const url = decodeURIComponent(req.url.split("?")[0]);
  const file = url === "/target.xlsx" ? FIXTURE : path.join(DIST, url.endsWith("/") ? url + "index.html" : url);
  if ((file !== FIXTURE && !file.startsWith(DIST)) || !fs.existsSync(file)) {
    res.writeHead(404);
    res.end("not found");
    return;
  }
  res.writeHead(200, { "content-type": MIME[path.extname(file)] || "application/octet-stream" });
  fs.createReadStream(file).pipe(res);
});
await new Promise((r) => server.listen(PORT, r));

const results = {};
const ok = (id, pass, value = "") => {
  results[id] = { pass: !!pass, value: String(value) };
  console.log(`  ${pass ? "PASS" : "FAIL"} ${id}${value !== "" ? "  (" + value + ")" : ""}`);
};

const { chromium } = loadPlaywright();
const browser = await chromium.launch({
  executablePath: chromePath(),
  args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
});
const errors = [];
const page = await browser.newPage({ viewport: { width: W, height: H }, colorScheme: "light", acceptDownloads: true });
page.on("pageerror", (e) => errors.push(e.message));
let commands = [];
let actions = [];
try {
  await page.goto(`http://localhost:${PORT}/?xlsx=target.xlsx&name=quarterly-sales-dashboard.xlsx&theme=light`);
  await page.waitForFunction(() => window.__sheetsReady || window.__sheetsError, null, { timeout: 60000 });
  await page.waitForTimeout(400);

  // What the editor can be asked to do: the core's command table and the
  // menu/ribbon/context actions of the app. report.mjs matches the catalog's
  // EVGSheets command ids against these.
  commands = await page.evaluate(() => window.sheet.app.grid.app.commands.commands.map((c) => c.id));
  actions = await page.evaluate(() => Object.values(window.sheet.app.actions).map((a) => a.command));

  // The accessibility mirror: every control the chrome draws, by role and name.
  const ax = await page.evaluate(() =>
    [...document.querySelectorAll("[role],button,input")].map((e) => ({
      role: e.getAttribute("role") || (e.tagName === "BUTTON" ? "button" : "textbox"),
      name: (e.getAttribute("aria-label") || e.textContent || "").trim().slice(0, 60),
      r: (({ x, y, width, height }) => ({ x, y, w: width, h: height }))(e.getBoundingClientRect()),
    })),
  );
  const named = (role, re) => ax.filter((n) => n.role === role && re.test(n.name));
  const cell = (text) => ax.find((n) => n.role === "gridcell" && n.name === text);

  // Pixels of the page, read back through a 2-D canvas (no PNG decoder needed).
  const png = await page.screenshot({ fullPage: false });
  fs.mkdirSync(path.join(REPO, "docs", "parity"), { recursive: true });
  fs.writeFileSync(path.join(REPO, "docs", "parity", "evgsheets.png"), png);
  await page.evaluate(async (b64) => {
    const img = new Image();
    img.src = "data:image/png;base64," + b64;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = img.width;
    c.height = img.height;
    c.getContext("2d").drawImage(img, 0, 0);
    window.__px = c.getContext("2d");
  }, png.toString("base64"));
  const px = (x, y) => page.evaluate(([x, y]) => [...window.__px.getImageData(Math.round(x), Math.round(y), 1, 1).data].slice(0, 3), [x, y]);
  const isBlank = (p) => p[0] > 245 && p[1] > 245 && p[2] > 245;
  // Text is drawn in greys; bars, icons, sparklines and table styles in colour.
  const isColour = (p) => Math.max(...p) - Math.min(...p) > 50;
  // Any pixel in the cell's strip that is not paper (white), scanning a row.
  const scan = async (n, fx0, fx1, fy, test) => {
    if (!n) return false;
    for (let f = fx0; f <= fx1 + 1e-9; f += 0.025) {
      if (test(await px(n.r.x + n.r.w * f, n.r.y + n.r.h * fy))) return true;
    }
    return false;
  };
  const inked = (n, a, b, fy) => scan(n, a, b, fy, (p) => !isBlank(p));
  const coloured = (n, a, b, fy) => scan(n, a, b, fy, isColour);

  const st = await page.evaluate(() => window.sheet.state());
  const m = await page.evaluate(() => {
    const g = window.sheet.app.grid.app;
    const s = g.book.activeSheet();
    return {
      tables: s.tables.length,
      cf: s.cfRules.map((r) => r.kind),
      charts: g.view.chartLayer.panels.length,
      images: s.images.length,
    };
  });

  // --- the workbook ------------------------------------------------------------
  console.log("== target workbook");
  ok("wb.opens", st.sheets.length === 1 && st.sheets[0] === "Sales" && !errors.length, st.sheets.join(",") + (errors[0] || ""));
  ok("wb.values", !!cell("$217,420") && !!cell("$362,100"), cell("$217,420") ? "G5 $217,420" : "G5 missing");
  ok("wb.percent", !!cell("28.4%") && !!cell("55.8%"));
  ok("wb.totalRow", !!cell("$304,820") && !!cell("$1,381,540") && !!cell("26.2%"), cell("$304,820") ? "" : "total row does not compute (SUBTOTAL over table columns), so C16 =G10/G11 is " + (cell("#DIV/0!") ? "#DIV/0!" : "wrong"));
  ok("wb.structuredRefs", !!cell("$57,564") && !!cell("Online"), "C15 =AVERAGE(SalesTable[[Q1]:[Q4]]) " + (cell("$57,564") ? "$57,564" : "wrong"));
  ok("wb.table", m.tables === 1, m.tables + " table(s) in the model");
  const hdr = cell("Region");
  const band5 = cell("North"), band6 = cell("South");
  ok("wb.tableStyle", hdr && isColour(await px(hdr.r.x + 3, hdr.r.y + 3)), hdr ? (await px(hdr.r.x + 3, hdr.r.y + 3)).join(",") : "");
  ok("wb.banding", band5 && band6 && (await px(band5.r.x + 3, band5.r.y + 3)).join() !== (await px(band6.r.x + 3, band6.r.y + 3)).join());
  // Filter buttons: the right end of each table header cell carries one.
  const q1 = cell("Q1");
  ok("wb.tableFilters", q1 && (await inked(q1, 0.75, 0.97, 0.5)), "");
  // Numbers sit at the right of their cell unless told otherwise.
  const n48 = cell("$39,150"); // an unbanded row
  ok("wb.numbersRight", n48 && !(await inked(n48, 0.08, 0.3, 0.5)) && (await inked(n48, 0.7, 0.95, 0.5)), "");
  ok("wb.dataBars", await coloured(cell("$271,960"), 0.05, 0.6, 0.8), "cf kinds read: " + JSON.stringify(m.cf));
  ok("wb.iconSets", await coloured(cell("28.4%"), 0.02, 0.4, 0.5));
  const trend = ax.filter((n) => n.role === "columnheader" && n.name === "I")[0];
  const i5 = band5 && trend ? { r: { x: trend.r.x, y: band5.r.y, w: trend.r.w, h: band5.r.h } } : null;
  ok("wb.sparklines", await coloured(i5, 0.05, 0.95, 0.5) || (await coloured(i5, 0.05, 0.95, 0.3)) || (await coloured(i5, 0.05, 0.95, 0.7)));
  ok("wb.charts", m.charts >= 2, m.charts + " chart(s)");
  ok("wb.border", !!cell("Best region"));

  // --- the chrome --------------------------------------------------------------
  console.log("== chrome");
  const tabs = named("tab", /^(Home|Insert|Draw|Page Layout|Formulas|Data|Review|View|Automate)$/);
  ok("ui.ribbonTabs", tabs.length >= 9, tabs.length + " of 9 Excel ribbon tabs; menubar menus: " + named("menuitem", /./).length);
  const groups = ax.filter((n) => n.role === "toolbar");
  // Controls inside the ribbon's toolbars (closed dialogs' buttons are not counted).
  const inside = (n, g) => n.r.w > 0 && n.r.x >= g.r.x && n.r.y >= g.r.y && n.r.x + n.r.w <= g.r.x + g.r.w + 1 && n.r.y + n.r.h <= g.r.y + g.r.h + 1;
  const controls = ax.filter((n) => (n.role === "button" || n.role === "combobox") && groups.some((g) => inside(n, g)));
  ok("ui.ribbonGroups", groups.length >= 7, groups.length + " toolbar groups");
  ok("ui.homeControls", controls.length >= 60, controls.length + " ribbon controls (Excel's Home tab: ~60)");
  const boxes = ax.filter((n) => n.role === "textbox" || n.role === "combobox");
  // Left of the formula, just above the column headers (the ribbon's height varies).
  const headTop = Math.min(...ax.filter((n) => n.role === "columnheader").map((n) => n.r.y));
  ok("ui.nameBox", boxes.some((n) => n.r.x < 120 && n.r.y < headTop && n.r.y > headTop - 60), "");
  ok("ui.fxButtons", named("button", /insert function|^fx$|^cancel$|^enter$/i).length >= 1, "");
  const cols = ax.filter((n) => n.role === "columnheader" && /^[A-Z]+$/.test(n.name));
  const lastRight = Math.max(...cols.map((n) => n.r.x + n.r.w));
  ok("ui.gridFillsWindow", lastRight >= W - 80, cols.length + " column headers drawn, last ends at x=" + Math.round(lastRight) + " of " + W);
  const rows = ax.filter((n) => n.role === "rowheader");
  ok("ui.rowsFillWindow", rows.length >= 35, rows.length + " row headers");
  ok("ui.sheetTabs", ax.some((n) => n.role === "tab" && n.name === "Sales"));
  ok("ui.addSheet", named("button", /add sheet/i).some((n) => n.r.w > 0));
  ok("ui.tabScroll", named("button", /previous sheet|next sheet|scroll/i).length >= 2);
  ok("ui.statusReady", ax.some((n) => n.role === "status" && /^Ready\b/.test(n.name)), ax.filter((n) => n.role === "status").map((n) => n.name).join(" | "));
  // Select C5:F10 and read the status bar statistics.
  const c5 = cell("$48,200"), f10 = cell("$112,840");
  await page.mouse.click(c5.r.x + 5, c5.r.y + 5);
  await page.keyboard.down("Shift");
  await page.mouse.click(f10.r.x + 5, f10.r.y + 5);
  await page.keyboard.up("Shift");
  await page.waitForTimeout(150);
  const status = await page.evaluate(() => window.sheet.app.statsText() + " [" + window.sheet.state().selCells + " cells]");
  ok("ui.statusStats", /Sum/.test(status) && /Average/.test(status) && /Count/.test(status), status);
  const after = await page.evaluate(() => [...document.querySelectorAll("[role=slider],[role=button],button")].map((e) => e.getAttribute("aria-label") || e.textContent));
  ok("ui.zoom", ax.some((n) => n.role === "slider") || after.some((t) => /zoom/i.test(t || "")));
  ok("ui.viewButtons", after.filter((t) => /normal|page layout|page break/i.test(t || "")).length >= 3);
  ok("ui.titleBar", ax.some((n) => /quarterly-sales-dashboard/.test(n.name)) || (await page.evaluate(() => window.sheet.state().source)) !== "");
  ok("ui.menubar", ax.some((n) => n.role === "menubar"));

  // --- commands ----------------------------------------------------------------
  // One check per catalog command whose effect the app's state can show. Each
  // runs the command the menu or ribbon runs, below the dashboard (rows 30+).
  console.log("== commands");
  const run = (id, arg = "") => page.evaluate(([i, a]) => window.sheet.run(i, a), [id, arg]);
  const S = () => page.evaluate(() => window.sheet.state());
  const at = (ref) => run("nav.goto", ref);
  const put = async (ref, text) => {
    await at(ref);
    await page.keyboard.type(text);
    await page.keyboard.press("Enter");
    await page.waitForTimeout(30);
  };
  const get = (r, c) => page.evaluate(([r, c]) => window.sheet.app.grid.app.model.getCell(r, c), [r, c]);
  await page.mouse.click(c5.r.x + 5, c5.r.y + 5);
  const flagCmd = async (id, cmd, arg, key, want) => {
    await at("D30");
    await run(cmd, arg);
    const v = (await S())[key];
    ok("cmd." + id, v === want, key + "=" + v);
  };
  await flagCmd("home.italic", "format.italic", "", "italic", true);
  await flagCmd("home.underline", "format.underline", "", "underline", true);
  await flagCmd("home.strikethrough", "format.strike", "", "strike", true);
  await flagCmd("home.wrapText", "format.wrap", "", "wrap", true);
  await flagCmd("home.alignLeft", "format.align", "left", "align", "left");
  await flagCmd("home.alignCenter", "format.align", "center", "align", "center");
  await flagCmd("home.alignRight", "format.align", "right", "align", "right");
  await flagCmd("home.alignTop", "format.valign", "top", "valign", "top");
  await flagCmd("home.alignMiddle", "format.valign", "center", "valign", "center");
  await flagCmd("home.alignBottom", "format.valign", "bottom", "valign", "bottom");
  await flagCmd("home.fontName", "format.font", "Noto Sans", "font", "Noto Sans");
  await at("D30");
  const size0 = (await S()).size;
  await run("format.size.up");
  const size1 = (await S()).size;
  ok("cmd.home.increaseFontSize", size1 > size0, size0 + "→" + size1);
  await run("format.size.down");
  ok("cmd.home.decreaseFontSize", (await S()).size < size1, size1 + "→" + (await S()).size);
  await run("format.clear");
  ok("cmd.edit.clearFormats", !(await S()).italic && !(await S()).underline);

  const fmt = async (id, cmd, arg, re) => {
    await put("E30", "1234.5");
    await at("E30");
    await run(cmd, arg);
    const s = await S();
    ok("cmd." + id, re.test(s.display), s.numFmt + " → " + s.display);
  };
  await fmt("home.percent", "format.numfmt", "percent", /%$/);
  await fmt("home.comma", "format.numfmt", "comma", /^1,23\d/);
  await fmt("home.accounting", "format.numfmt", "currency", /\$/);
  await put("E30", "1234.5");
  await at("E30");
  await run("format.numfmt", "comma");
  const d0 = (await S()).display;
  await run("format.decimals", "more");
  const d1 = (await S()).display;
  ok("cmd.home.increaseDecimal", d1.length === d0.length + 1, d0 + " → " + d1);
  await run("format.decimals", "less");
  ok("cmd.home.decreaseDecimal", (await S()).display.length === d1.length - 1, d1 + " → " + (await S()).display);

  await at("D32");
  await page.keyboard.press("Shift+ArrowRight");
  await run("format.merge");
  ok("cmd.home.mergeCells", (await S()).merged === true);
  await at("D32");
  await run("format.merge");
  ok("cmd.home.unmergeCells", (await S()).merged === false);

  await put("D34", "x");
  await put("D35", "y");
  await at("D34");
  await run("delete.rows");
  ok("cmd.home.deleteRows", (await get(33, 3)) === "y", "D34=" + (await get(33, 3)));
  await put("D34", "x");
  await put("E34", "y");
  await at("D34");
  await run("delete.columns");
  ok("cmd.home.deleteColumns", (await get(33, 3)) === "y", "D34=" + (await get(33, 3)));
  await at("D34");
  await run("insert.row.above");
  ok("cmd.home.insertRows", (await get(34, 3)) === "y" && (await get(33, 3)) === "");
  await run("insert.column.left");
  ok("cmd.home.insertColumns", (await get(34, 4)) === "y");

  await put("K36", "a long piece of text to fit");
  const w0 = await page.evaluate(() => window.sheet.app.grid.app.model.colWidths[10] || 0);
  await at("K36");
  await run("format.autofit.column");
  const w1 = await page.evaluate(() => window.sheet.app.grid.app.model.colWidths[10] || 0);
  ok("cmd.home.autofitColumnWidth", w1 > w0, w0 + "→" + w1);

  await at("C5");
  await run("view.freeze.here");
  let s2 = await S();
  ok("cmd.view.freezePanes", s2.freezeRows === 4 && s2.freezeCols === 2, s2.freezeRows + "," + s2.freezeCols);
  await run("view.freeze.none");
  s2 = await S();
  ok("cmd.view.unfreezePanes", s2.freezeRows === 0 && s2.freezeCols === 0);

  const n0 = (await S()).sheets.length;
  await run("sheet.add");
  ok("cmd.home.insertSheet", (await S()).sheets.length === n0 + 1);
  // Sorting on the new, empty sheet: the dashboard's table would join the region.
  await put("A1", "3");
  await put("A2", "1");
  await put("A3", "2");
  await at("A1");
  await run("data.sort.asc");
  ok("cmd.data.sortAscending", (await get(0, 0)) === "1" && (await get(2, 0)) === "3", [await get(0, 0), await get(1, 0), await get(2, 0)].join(","));
  await at("A1");
  await run("data.sort.desc");
  ok("cmd.data.sortDescending", (await get(0, 0)) === "3", [await get(0, 0), await get(1, 0), await get(2, 0)].join(","));


  // --- Excel ribbon commands by their catalog ids ------------------------------
  // On a fresh sheet, so nothing here leans on the dashboard.
  const J = (fn, arg) => page.evaluate(fn, arg);
  const M = (expr) => page.evaluate(new Function("const G = window.sheet.app.grid.app, M = G.model; return (" + expr + ");"));
  const check = async (id, fn) => {
    try {
      const [pass, detail] = await fn();
      ok("cmd." + id, !!pass, detail === undefined ? "" : String(detail));
    } catch (e) {
      ok("cmd." + id, false, String(e && e.message || e).slice(0, 120));
    }
  };
  const sel = async (ref) => {
    const [a, b] = ref.split(":");
    await at(a);
    if (b) await page.evaluate(([b]) => {
      const G = window.sheet.app.grid.app;
      const m = /^([A-Z]+)(\d+)$/.exec(b);
      let c = 0;
      for (const ch of m[1]) c = c * 26 + ch.charCodeAt(0) - 64;
      G.sel.active.row = +m[2] - 1;
      G.sel.active.col = c - 1;
    }, [b]);
  };
  const cellAt = (ref) => page.evaluate(([ref]) => {
    const G = window.sheet.app.grid.app;
    const m = /^([A-Z]+)(\d+)$/.exec(ref);
    let c = 0;
    for (const ch of m[1]) c = c * 26 + ch.charCodeAt(0) - 64;
    return G.model.getDisplay(+m[2] - 1, c - 1);
  }, [ref]);
  const style = (r, c) => page.evaluate(([r, c]) => {
    const s = window.sheet.app.grid.app.model.getCellStyle(r, c);
    return { indent: s.indent, locked: s.locked, dbl: s.doubleUnderline, rot: s.textRotation, fill: s.fillRgb, text: s.textRgb, bold: s.bold, align: s.align, border: s.hasBorder };
  }, [r, c]);
  const unzip = (bytes) => {
    const buf = Buffer.from(bytes);
    const out = {};
    let eocd = buf.length - 22;
    while (eocd >= 0 && buf.readUInt32LE(eocd) !== 0x06054b50) eocd--;
    let p = buf.readUInt32LE(eocd + 16);
    const n = buf.readUInt16LE(eocd + 10);
    for (let i = 0; i < n; i++) {
      const method = buf.readUInt16LE(p + 10), size = buf.readUInt32LE(p + 20);
      const nl = buf.readUInt16LE(p + 28), xl = buf.readUInt16LE(p + 30), cl = buf.readUInt16LE(p + 32);
      const local = buf.readUInt32LE(p + 42);
      const name = buf.toString("utf8", p + 46, p + 46 + nl);
      const lnl = buf.readUInt16LE(local + 26), lxl = buf.readUInt16LE(local + 28);
      const data = buf.subarray(local + 30 + lnl + lxl, local + 30 + lnl + lxl + size);
      out[name] = (method === 8 ? zlib.inflateRawSync(data) : data).toString("utf8");
      p += 46 + nl + xl + cl;
    }
    return out;
  };
  const savedXml = async () => unzip(await page.evaluate(() => Array.from(new Uint8Array(window.sheet.saveBytes()))));

  await run("sheet.add");
  const sheetIdx = (await S()).activeSheet;

  // Home: alignment and font
  await put("A1", "x");
  await sel("A1:C1");
  await run("home.mergeCenter");
  await check("home.mergeCenter", async () => { await at("A1"); const s = await S(); return [s.merged && s.align === "center", s.merged + "," + s.align]; });
  await sel("A3:C4");
  await run("home.mergeAcross");
  await check("home.mergeAcross", async () => [await M("M.mergeAt(2,0) !== '' && M.mergeAt(3,0) !== '' && M.mergeAt(2,0) !== M.mergeAt(3,0)"), await M("M.mergeAt(2,0) + '|' + M.mergeAt(3,0)")]);
  await sel("A3:C4");
  await run("home.unmergeCells");
  await check("home.unmergeCells", async () => [await M("M.mergeAt(2,0) === '' && M.mergeAt(3,0) === ''")]);
  await at("B6");
  await run("home.orientation", "counterclockwise");
  await check("home.orientation", async () => { const s = await style(5, 1); return [s.rot === 45, s.rot]; });
  await run("home.increaseIndent");
  await run("home.increaseIndent");
  await check("home.increaseIndent", async () => { const s = await style(5, 1); return [s.indent === 2, s.indent]; });
  await run("home.decreaseIndent");
  await check("home.decreaseIndent", async () => { const s = await style(5, 1); return [s.indent === 1, s.indent]; });
  await run("home.doubleUnderline");
  await check("home.doubleUnderline", async () => { const s = await style(5, 1); return [s.dbl === true, s.dbl]; });
  await run("home.drawBorder", "all");
  await check("home.drawBorder", async () => { const s = await style(5, 1); return [s.border === true]; });
  await at("B7");
  await run("home.cellStyle", "Bad");
  await check("home.cellStyle", async () => { const s = await style(6, 1); return [/FFC7CE/i.test(s.fill) && /9C0006/i.test(s.text), s.fill + " " + s.text]; });

  // Home: cells
  await sel("A8:A9");
  await run("home.rowHeight", "30");
  await check("home.rowHeight", async () => { const h = await M("M.rowHeight(7)"); return [h === 40, h]; });
  await sel("D1:E1");
  await run("home.columnWidth", "20");
  await check("home.columnWidth", async () => { const w = await M("M.colWidth(3)"); return [w === 145, w]; });
  await run("home.defaultWidth", "10");
  await check("home.defaultWidth", async () => { const w = await M("M.defaultColW"); return [w === 75, w]; });
  await sel("A10:A11");
  await run("home.hideRows");
  await check("home.hideRows", async () => [await M("M.isRowHidden(9) && M.isRowHidden(10)")]);
  await sel("A9:A12");
  await run("home.unhideRows");
  await check("home.unhideRows", async () => [await M("!M.isRowHidden(9) && !M.isRowHidden(10)")]);
  await sel("F1:G1");
  await run("home.hideColumns");
  await check("home.hideColumns", async () => [await M("M.isColHidden(5) && M.isColHidden(6)")]);
  await sel("E1:H1");
  await run("home.unhideColumns");
  await check("home.unhideColumns", async () => [await M("!M.isColHidden(5) && !M.isColHidden(6)")]);
  await put("H1", "1");
  await put("H2", "2");
  await at("H1");
  await run("home.insertCells", "down");
  await check("home.insertCells", async () => [(await cellAt("H1")) === "" && (await cellAt("H2")) === "1" && (await cellAt("H3")) === "2", [await cellAt("H1"), await cellAt("H2"), await cellAt("H3")].join(",")]);
  await at("H1");
  await run("home.deleteCells", "up");
  await check("home.deleteCells", async () => [(await cellAt("H1")) === "1" && (await cellAt("H2")) === "2", [await cellAt("H1"), await cellAt("H2")].join(",")]);
  await sel("H1:H2");
  const cf0 = await M("M.cfRuleCount()");
  await run("home.conditionalFormat", "colorScale");
  await check("home.conditionalFormat", async () => { const n = await M("M.cfRuleCount()"); return [n === cf0 + 1, cf0 + "→" + n]; });
  await run("home.clearRules");
  await check("home.clearRules", async () => { const n = await M("M.cfRuleCount()"); return [n === cf0, n]; });

  // Editing
  await put("J1", "5");
  await put("J2", "7");
  await put("K1", "=J1*2");
  await sel("K1:K3");
  await run("edit.fillDown");
  await check("edit.fillDown", async () => [(await M("M.getFormula(1,10)")) === "J2*2" && (await cellAt("K2")) === "14", (await M("M.getFormula(1,10)")) + " = " + (await cellAt("K2"))]);
  await put("M3", "=J3+1");
  await sel("M1:M3");
  await run("edit.fillUp");
  await check("edit.fillUp", async () => [(await M("M.getFormula(0,12)")) === "J1+1" && (await cellAt("M1")) === "6", (await M("M.getFormula(0,12)")) + " = " + (await cellAt("M1"))]);
  await put("J5", "a");
  await sel("J5:L5");
  await run("edit.fillRight");
  await check("edit.fillRight", async () => [(await cellAt("L5")) === "a"]);
  await put("L6", "b");
  await sel("J6:L6");
  await run("edit.fillLeft");
  await check("edit.fillLeft", async () => [(await cellAt("J6")) === "b"]);
  await put("J8", "10");
  await sel("J8:J11");
  await run("edit.fillSeries", "5");
  await check("edit.fillSeries", async () => [(await cellAt("J11")) === "25", await cellAt("J11")]);
  await sel("J8:K11");
  await run("edit.goToSpecial", "blanks");
  await check("edit.goToSpecial", async () => { const n = await M("G.sel.selectedCount()"); return [n === 4, n]; });
  await run("edit.goTo", "Z40");
  await check("edit.goTo", async () => [(await S()).cell === "Z40", (await S()).cell]);
  await at("J12");
  await run("formulas.autoSum");
  await check("formulas.autoSum", async () => [(await M("M.getFormula(11,9)")) === "SUM(J8:J11)" && (await cellAt("J12")) === "70", (await M("M.getFormula(11,9)")) + " = " + (await cellAt("J12"))]);
  await at("J8");
  await run("review.newNote", "check this");
  await check("review.newNote", async () => [(await M("M.noteAt(7,9)")) === "check this"]);
  await at("J10");
  await run("review.newComment", "and this");
  await check("review.newComment", async () => [(await M("M.noteAt(9,9)")) === "and this"]);
  await at("A1");
  await run("review.nextComment");
  await check("review.nextComment", async () => [(await S()).cell === "J8", (await S()).cell]);
  await run("review.nextComment");
  await run("review.previousComment");
  await check("review.previousComment", async () => [(await S()).cell === "J8", (await S()).cell]);
  await at("J10");
  await run("review.deleteComment");
  await check("review.deleteComment", async () => [!(await M("M.hasNote(9,9)"))]);
  await at("J8");
  await run("edit.clearComments");
  await check("edit.clearComments", async () => [!(await M("M.hasNote(7,9)"))]);
  await page.evaluate(() => window.sheet.app.grid.app.model.setHyperlink(8, 9, "https://example.com"));
  await at("J9");
  await run("edit.clearHyperlinks");
  await check("edit.clearHyperlinks", async () => [!(await M("M.hasHyperlink(8,9)")) && (await cellAt("J9")) === "15", await cellAt("J9")]);
  await at("B7");
  await run("edit.clearAll");
  await check("edit.clearAll", async () => { const s = await style(6, 1); return [s.fill === "" && (await cellAt("B7")) === "", s.fill]; });
  await run("review.workbookStatistics");
  await check("review.workbookStatistics", async () => { const s = (await S()).status; return [/formulas \d+/.test(s) && /Sheets \d+/.test(s), s]; });

  // Formulas
  await at("K1");
  await run("formulas.showFormulas");
  await check("formulas.showFormulas", async () => [await M("M.showFormulas === true")]);
  await run("formulas.showFormulas");
  await page.evaluate(() => window.sheet.app.grid.app.model.setCellRaw(0, 9, "50"));
  await run("formulas.calculateNow");
  await check("formulas.calculateNow", async () => [(await cellAt("K1")) === "100", await cellAt("K1")]);
  await page.evaluate(() => window.sheet.app.grid.app.model.setCellRaw(0, 9, "60"));
  await run("formulas.calculateSheet");
  await check("formulas.calculateSheet", async () => [(await cellAt("K1")) === "120", await cellAt("K1")]);
  await at("J12");
  await run("formulas.tracePrecedents");
  await check("formulas.tracePrecedents", async () => { const n = await M("G.sel.selectedCount()"); return [n === 4 && (await S()).cell === "J11", n + " " + (await S()).cell]; });
  const fnCats = [["financial", "PMT"], ["logical", "IF"], ["text", "LEFT"], ["dateTime", "DATE"], ["lookupReference", "VLOOKUP"], ["mathTrig", "ROUND"], ["moreFunctions", "AVERAGE"], ["recentlyUsed", "SUM"], ["insertFunction", "SUM"]];
  for (const [cat, fn] of fnCats) {
    await at("N20");
    await run("formulas." + cat, fn);
    const s = await S();
    ok("cmd.formulas." + cat, s.editing && s.editText === "=" + fn + "(", s.editText);
    await page.keyboard.press("Escape");
  }

  // Data
  for (const [ref, v] of [["P1", "a"], ["P2", "b"], ["P3", "a"], ["P4", "c"], ["P5", "b"]]) await put(ref, v);
  await sel("P1:P5");
  await run("data.removeDuplicates");
  await check("data.removeDuplicates", async () => { const v = []; for (let i = 1; i <= 5; i++) v.push(await cellAt("P" + i)); return [v.join(",") === "a,b,c,,", v.join(",")]; });
  await put("R1", "x;y;z");
  await at("R1");
  await run("data.textToColumns", "semicolon");
  await check("data.textToColumns", async () => [(await cellAt("R1")) === "x" && (await cellAt("T1")) === "z", [await cellAt("R1"), await cellAt("S1"), await cellAt("T1")].join(",")]);

  // View
  await run("view.gridlines");
  await check("view.gridlines", async () => [await M("M.showGridlines === false")]);
  await run("view.gridlines");
  const hw0 = await M("G.grid.rowHeaderW");
  await run("view.headings");
  await check("view.headings", async () => { const w = await M("G.grid.rowHeaderW"); return [w === 0 && hw0 > 0, hw0 + "→" + w]; });
  await run("view.headings");
  // Zoom: the grid's own geometry scales, the stored widths do not.
  const cw0 = await M("M.colWidth(2)");
  await run("view.zoom", "150");
  await check("view.zoom", async () => { const cw = await M("M.colWidth(2)"), raw = await M("M.rawColWidth(2)"); return [Math.abs(cw - raw * 1.5) <= 1 && raw === cw0, cw0 + "→" + cw + " (raw " + raw + ")"]; });
  await run("view.zoomIn");
  await check("view.zoomIn", async () => { const z = await M("M.zoom"); return [Math.abs(z - 1.6) < 0.01, z]; });
  await run("view.zoomOut");
  await check("view.zoomOut", async () => { const z = await M("M.zoom"); return [Math.abs(z - 1.5) < 0.01, z]; });
  await run("view.zoom100");
  await check("view.zoom100", async () => [await M("M.zoom === 1 && M.colWidth(2) === " + cw0)]);
  await sel("B4:C5");
  await run("view.zoomToSelection");
  await check("view.zoomToSelection", async () => { const z = await M("M.zoom"); return [z > 2, z]; });
  await run("view.zoom100");
  await run("view.pageBreakPreview");
  await check("view.pageBreakPreview", async () => { const b = await M("M.pageBreaks(true).length + '/' + M.viewMode"); return [/pageBreak$/.test(b), b]; });
  await run("view.pageLayout");
  await check("view.pageLayout", async () => [await M("M.viewMode === 'pageLayout'")]);
  await run("view.normal");
  await check("view.normal", async () => [await M("M.viewMode === 'normal'")]);
  await run("view.freezeTopRow");
  await check("view.freezeTopRow", async () => { const s = await S(); return [s.freezeRows === 1 && s.freezeCols === 0]; });
  await run("view.freezeFirstColumn");
  await check("view.freezeFirstColumn", async () => { const s = await S(); return [s.freezeRows === 0 && s.freezeCols === 1]; });
  await run("view.freeze.none");

  // Tables, on a block of their own
  for (const [ref, v] of [["A20", "Name"], ["B20", "Qty"], ["A21", "a"], ["B21", "1"], ["A22", "b"], ["B22", "2"], ["A23", "a"], ["B23", "1"]]) await put(ref, v);
  await at("A20");
  await run("home.formatAsTable", "TableStyleLight9");
  await check("home.formatAsTable", async () => [(await M("M.tableIndexAt(22,1)")) >= 0 && (await M("M.tableAt(M.tableIndexAt(22,1)).styleName")) === "TableStyleLight9", await M("M.tableCount()")]);
  await at("A21");
  await run("table.rename", "Stock");
  await check("table.rename", async () => [(await M("M.tableAt(M.tableIndexAt(20,0)).name")) === "Stock"]);
  await run("table.removeDuplicates");
  await check("table.removeDuplicates", async () => [(await cellAt("A23")) === "" && (await cellAt("A22")) === "b", [await cellAt("A21"), await cellAt("A22"), await cellAt("A23")].join(",")]);
  await run("table.resize", "A20:B23");
  await check("table.resize", async () => [(await M("M.tableAt(M.tableIndexAt(20,0)).r1")) === 22]);
  await run("table.resize", "A20:B22");
  await run("table.totalRow");
  await check("table.totalRow", async () => [(await cellAt("A23")) === "Total" && (await cellAt("B23")) === "3", [await cellAt("A23"), await cellAt("B23"), await M("M.getFormula(22,1)")].join(" ")]);
  for (const [cmd, field] of [["table.bandedRows", "showRowStripes"], ["table.bandedColumns", "showColumnStripes"], ["table.firstColumn", "showFirstColumn"], ["table.lastColumn", "showLastColumn"]]) {
    await at("A21");
    const before = await M("M.tableAt(M.tableIndexAt(20,0))." + field);
    await run(cmd);
    const after = await M("M.tableAt(M.tableIndexAt(20,0))." + field);
    ok("cmd." + cmd, before !== after, before + "→" + after);
  }
  await at("A21");
  await run("table.style", "TableStyleDark2");
  await check("table.style", async () => [(await M("M.tableAt(M.tableIndexAt(20,0)).styleName")) === "TableStyleDark2"]);
  await run("table.headerRow");
  await check("table.headerRow", async () => [(await M("M.tableAt(M.tableIndexAt(21,0)).r0")) === 20 && (await M("M.tableAt(M.tableIndexAt(21,0)).headerRowCount")) === 0]);
  await run("table.headerRow");
  await at("A21");
  await run("table.convertToRange");
  await check("table.convertToRange", async () => { const s = await style(19, 0); return [(await M("M.tableIndexAt(20,0)")) < 0 && s.fill !== "", s.fill]; });
  for (const [ref, v] of [["D20", "k"], ["D21", "1"]]) await put(ref, v);
  await sel("D20:D21");
  await run("insert.table");
  await check("insert.table", async () => [(await M("M.tableIndexAt(20,3)")) >= 0]);
  await at("F20");
  await run("insert.symbol", "€");
  await check("insert.symbol", async () => { const s = await S(); return [s.editing && s.editText.endsWith("€"), s.editText]; });
  await page.keyboard.press("Escape");

  // Page Layout, read back from the saved file
  await sel("A20:B23");
  await run("pageLayout.printArea");
  await run("pageLayout.printTitles", "20:20");
  await run("pageLayout.orientation", "landscape");
  await run("pageLayout.size", "a4");
  await run("pageLayout.margins", "narrow");
  await run("pageLayout.printGridlines");
  await run("pageLayout.printHeadings");
  await run("pageLayout.scaleToFit", "1,0");
  await at("A22");
  await run("pageLayout.breaks", "insert");
  await run("pageLayout.headerFooter", "Quarterly|Page &P");
  await run("sheet.tabColor", "#FF0000");
  const parts = await savedXml();
  const sx = Object.keys(parts).filter((k) => /^xl\/worksheets\/[^/]+\.xml$/.test(k)).map((k) => parts[k]).join("\n");
  const wbx = parts["xl/workbook.xml"] || "";
  ok("cmd.pageLayout.printArea", /_xlnm\.Print_Area[^>]*>[^<]*\$A\$20:\$B\$23</.test(wbx), (wbx.match(/<definedNames>.*<\/definedNames>/) || [""])[0].slice(0, 120));
  ok("cmd.pageLayout.printTitles", /_xlnm\.Print_Titles[^>]*>[^<]*\$20:\$20</.test(wbx));
  ok("cmd.pageLayout.orientation", /<pageSetup[^>]*orientation="landscape"/.test(sx), (sx.match(/<pageSetup[^>]*>/) || [""])[0]);
  ok("cmd.pageLayout.size", /<pageSetup[^>]*paperSize="9"/.test(sx));
  ok("cmd.pageLayout.margins", /<pageMargins left="0.25"/.test(sx), (sx.match(/<pageMargins[^>]*>/) || [""])[0]);
  ok("cmd.pageLayout.printGridlines", /<printOptions[^>]*gridLines="1"/.test(sx));
  ok("cmd.pageLayout.printHeadings", /<printOptions[^>]*headings="1"/.test(sx));
  ok("cmd.pageLayout.scaleToFit", /fitToPage="1"/.test(sx) && /fitToWidth="1"/.test(sx), (sx.match(/<sheetPr>.*?<\/sheetPr>/) || [""])[0]);
  ok("cmd.pageLayout.breaks", /<rowBreaks[^>]*><brk id="21"/.test(sx));
  ok("cmd.pageLayout.headerFooter", /<oddHeader>&amp;CQuarterly<\/oddHeader>/.test(sx));
  ok("cmd.sheet.tabColor", /<tabColor rgb="FFFF0000"\/>/.test(sx));

  // Protection
  await at("H5");
  await run("home.lockCell");
  await check("home.lockCell", async () => { const s = await style(4, 7); return [s.locked === false]; });
  await run("review.protectSheet");
  await put("H5", "open");
  await put("H6", "shut");
  await check("review.protectSheet", async () => [(await cellAt("H5")) === "open" && (await cellAt("H6")) === "" && /<sheetProtection /.test(Object.values(await savedXml()).join("")), [await cellAt("H5"), await cellAt("H6")].join(",")]);
  await run("review.unprotectSheet");
  await put("H6", "shut");
  await check("review.unprotectSheet", async () => [(await cellAt("H6")) === "shut"]);
  await run("review.protectWorkbook");
  const nP = (await S()).sheets.length;
  await run("sheet.add");
  await check("review.protectWorkbook", async () => [(await S()).sheets.length === nP && /workbookProtection/.test((await savedXml())["xl/workbook.xml"]), (await S()).sheets.length]);
  await run("review.protectWorkbook");

  // Sheets
  await run("sheet.add");
  const nH = (await S()).sheets.length;
  const hiddenName = (await S()).sheets[(await S()).activeSheet];
  await run("sheet.hide");
  await check("sheet.hide", async () => [await M("G.book.sheetAt(" + (nH - 1) + ").sheetHidden === true") && (await S()).activeSheet !== nH - 1]);
  await run("sheet.unhide", hiddenName);
  await check("sheet.unhide", async () => [await M("G.book.sheetAt(" + (nH - 1) + ").sheetHidden === false") && (await S()).activeSheet === nH - 1]);

  // Exports
  await run("view.sheet.prev");
  for (const [cmd, re] of [["file.exportCsv", /Name,Qty|k\r?\n/], ["file.exportHtml", /<table[\s\S]*<td[^>]*>Total<\/td>/]]) {
    const dlx = page.waitForEvent("download", { timeout: 5000 }).catch(() => null);
    await run(cmd);
    const got = await dlx;
    const text = got ? fs.readFileSync(await got.path(), "utf8") : "";
    ok("cmd." + cmd, re.test(text), (got ? got.suggestedFilename() : "no download") + " " + text.length + " chars");
  }

  // Sorting on a sheet of its own: a sort takes the whole block it is in.
  await run("sheet.add");
  for (const [ref, v] of [["V1", "1"], ["W1", "3"], ["V2", "2"], ["W2", "1"], ["V3", "3"], ["W3", "2"]]) await put(ref, v);
  await sel("V1:W3");
  await run("data.sort", "W desc");
  await check("data.sort", async () => [(await cellAt("W1")) === "3" && (await cellAt("V1")) === "1", [await cellAt("V1"), await cellAt("W1")].join(",")]);


  // PivotTables, from a small block on a sheet called Data.
  await run("sheet.add");
  await run("sheet.rename", "Data");
  for (const [ref, v] of [["A1", "Region"], ["B1", "Product"], ["C1", "Sales"], ["D1", "Units"], ["A2", "North"], ["B2", "A"], ["C2", "10"], ["D2", "1"], ["A3", "North"], ["B3", "B"], ["C3", "20"], ["D3", "2"], ["A4", "South"], ["B4", "A"], ["C4", "30"], ["D4", "3"], ["A5", "South"], ["B5", "B"], ["C5", "40"], ["D5", "4"], ["A6", "North"], ["B6", "A"], ["C6", "5"], ["D6", "1"]]) await put(ref, v);
  await at("B3");
  await run("insert.pivotTable");
  const pv = async (refs) => { const o = []; for (const r of refs) o.push(await cellAt(r)); return o.join(","); };
  await check("insert.pivotTable", async () => { const v = await pv(["A3", "B3", "C3", "A4", "B4", "A6", "B6", "C6"]); return [v === "Region,Sum of Sales,Sum of Units,North,35,Grand Total,105,11", v]; });
  await run("pivot.fieldList");
  const pane1 = await M("G.paneKind");
  await run("pivot.fieldList");
  await check("pivot.fieldList", async () => { const p2 = await M("G.paneKind"); return [pane1 === "" && p2 === "pivotFields", pane1 + "→" + p2]; });
  await at("A4");
  await run("pivot.valueSettings", "Sales:average");
  await check("pivot.valueSettings", async () => { const v = await pv(["B3", "B4"]); return [/^Average of Sales,11\.6/.test(v), v]; });
  await run("pivot.valueSettings", "Sales:sum");
  await run("pivot.sort", "desc");
  await check("pivot.sort", async () => { const v = await pv(["A4", "B4"]); return [v === "South,70", v]; });
  await run("pivot.sort", "none");
  await run("pivot.filter", "Product=A");
  await check("pivot.filter", async () => { const v = await pv(["B4", "B5", "B6"]); return [v === "15,30,45", v]; });
  await run("pivot.filter", "");
  await run("pivot.addField", "Product:rows");
  await check("pivot.addField", async () => { const v = await pv(["A3", "A4", "B4", "A5", "B5"]); return [v.replace(/ +/g, " ") === "Row Labels,North,35, A,15", v]; });
  await run("pivot.subtotals", "off");
  await check("pivot.subtotals", async () => { const v = await pv(["A4", "B4", "B5"]); return [v.replace(/ +/g, " ") === "North,,15", v]; });
  await run("pivot.subtotals", "on");
  await run("pivot.collapse", "collapse");
  await check("pivot.collapse", async () => { const v = await pv(["A4", "A5", "B5"]); return [v === "North,South,70", v]; });
  await run("pivot.collapse", "expand");
  await run("pivot.reportLayout", "tabular");
  await check("pivot.reportLayout", async () => { const v = await pv(["A3", "B3"]); return [v === "Region,Product", v]; });
  await run("pivot.reportLayout", "compact");
  await run("pivot.removeField", "Units");
  await check("pivot.removeField", async () => { const v = await pv(["B3", "C3"]); return [v === "Sum of Sales,", v]; });
  await run("pivot.moveField", "Product:columns");
  await check("pivot.moveField", async () => { const v = await pv(["A3", "B3", "C3", "D3", "B4"]); return [v === "Region,A,B,Grand Total,15", v]; });
  await run("pivot.removeField", "Product");
  await run("pivot.calculatedField", "Double=Sales*2");
  await check("pivot.calculatedField", async () => { const v = await pv(["C3", "C4"]); return [v === "Double,70", v]; });
  await run("pivot.grandTotals", "off");
  await check("pivot.grandTotals", async () => { const v = await pv(["A5", "A6"]); return [v === "South,", v]; });
  await run("pivot.grandTotals", "on");
  await run("pivot.style", "PivotStyleMedium2");
  await check("pivot.style", async () => { const s = await style(2, 0); return [/B4C6E7/i.test(s.fill) && s.bold, JSON.stringify(s)]; });
  await run("pivot.changeSource", "Data!A1:D5");
  await check("pivot.changeSource", async () => { const v = await pv(["B4"]); return [v === "30", v]; });
  const pivotSheet = (await S()).activeSheet;
  await run("view.sheet.prev");
  await put("C2", "100");
  await run("view.sheet.next");
  await at("A4");
  await run("pivot.refresh");
  await check("pivot.refresh", async () => { const v = await pv(["B4"]); return [v === "120", v]; });
  await run("pivot.delete");
  await check("pivot.delete", async () => { const v = await pv(["A3", "A4", "B4"]); return [v === ",,", v]; });
  await run("view.sheet.prev");
  await at("A2");
  await run("insert.recommendedPivotTables", "Rows=Units;Values=Sales");
  await check("insert.recommendedPivotTables", async () => { const v = await pv(["F1", "G1", "F2"]); return [v === "Units,Sum of Sales,1", v]; });
  await at("F2");
  await run("pivot.group", "bin:2");
  await check("pivot.group", async () => { const v = await pv(["F2", "G2", "F3", "G3"]); return [v === "0-1,105,2-3,50", v]; });
  const charts0 = await M("G.view.chartLayer.panels.length");
  await at("A2");
  await run("insert.pivotChart");
  await check("insert.pivotChart", async () => { const n = await M("G.view.chartLayer.panels.length"), v = await pv(["A4"]); return [n === charts0 + 1 && v === "North", n + " charts, " + v]; });
  await run("view.sheet.prev");
  await at("A1");
  await run("insert.table");
  await run("table.summarizePivot");
  await check("table.summarizePivot", async () => { const v = await pv(["A3", "B4"]); return [v === "Region,125", v]; });

  const dl = page.waitForEvent("download", { timeout: 5000 }).catch(() => null);
  await run("file.saveAs");
  const d = await dl;
  const bytes = d ? fs.readFileSync(await d.path()) : Buffer.alloc(0);
  ok("cmd.file.saveAs", bytes.length > 1000 && bytes[0] === 0x50 && bytes[1] === 0x4b, bytes.length + " bytes");

  await put("A5", "45000");
  await at("A5");
  await run("format.numfmt", "date");
  const nf = await S();
  ok("cmd.home.numberFormat", /20\d\d|\d+\/\d+\/\d+/.test(nf.display) && nf.numFmt !== "General", nf.numFmt + " → " + nf.display);
  await run("file.close");
  const closed = await S();
  ok("cmd.file.close", closed.sheets.length === 1 && closed.source === "" && (await get(0, 0)) === "", closed.sheets.length + " sheet(s), source '" + closed.source + "'");

  ok("ui.noErrors", errors.length === 0, errors.slice(0, 2).join(" | "));
} catch (e) {
  ok("probe.finished", false, e && e.stack ? e.stack.split("\n").slice(0, 2).join(" ") : String(e));
} finally {
  await browser.close();
  server.close();
}

const out = {
  fixture: "parity/fixtures/quarterly-sales-dashboard.xlsx",
  viewport: `${W}x${H}`,
  results,
  commands: [...new Set(commands)].sort(),
  actions: [...new Set(actions)].sort(),
};
fs.writeFileSync(path.join(REPO, "parity", "probe.json"), JSON.stringify(out, null, 1) + "\n");
const passed = Object.values(results).filter((r) => r.pass).length;
console.log(`\n${passed}/${Object.keys(results).length} probe checks pass (a failure is a measured gap, not a broken run)`);
