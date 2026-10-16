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
  if (url === "/table.html") {
    // A page with a table, for Data › From Web.
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end("<html><body><h1>Rates</h1><table><tr><th>Name</th><th>Rate</th></tr><tr><td>Alpha &amp; Co</td><td>1.5</td></tr><tr><td>Beta</td><td>2.25</td></tr></table></body></html>");
    return;
  }
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
let lastAt = Date.now();
const ok = (id, pass, value = "") => {
  results[id] = { pass: !!pass, value: String(value) };
  const ms = Date.now() - lastAt;
  lastAt = Date.now();
  const slow = process.env.PROBE_TIMING ? `  [${ms} ms]` : "";
  console.log(`  ${pass ? "PASS" : "FAIL"} ${id}${value !== "" ? "  (" + value + ")" : ""}${slow}`);
};

const { chromium } = loadPlaywright();
const browser = await chromium.launch({
  executablePath: chromePath(),
  args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
});
const errors = [];
const page = await browser.newPage({ viewport: { width: W, height: H }, colorScheme: "light", acceptDownloads: true });
page.on("pageerror", (e) => errors.push(e.message));
page.on("crash", () => console.log("  page crashed"));
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
  // The pane's rows and title as the chrome shows them (the a11y mirror).
  const paneEls = "[...document.querySelectorAll('button,[role=button]')].filter((e) => ((e.dataset && e.dataset.a11yId) || e.id || '').startsWith('sx-pane-list-'))";
  const paneRows = () => page.evaluate(new Function("return " + paneEls + ".map((e) => (e.getAttribute('aria-label') || e.textContent || '').trim());"));
  const pressPaneRow = async (i) => {
    const r = await page.evaluate(new Function("i", "const els = " + paneEls + "; const b = els[i] && els[i].getBoundingClientRect(); return b ? { x: b.x + 4, y: b.y + b.height / 2 } : null;"), i);
    if (r) await page.mouse.click(r.x, r.y);
    return !!r;
  };

  // Checks added with the task pane and the Review/View/File/Automate panes:
  // a sheet of their own, so they can also run alone (PROBE_NEW=1).
  const newChecks = async () => {
    await run("sheet.add");
    await run("sheet.rename", "Panes");
    for (const [ref, v] of [["A1", "Item"], ["B1", "Qty"], ["A2", "Pens"], ["B2", "4"], ["A3", "Ink"], ["B3", "6"]]) await put(ref, v);
    // Insert Function: the pane lists functions, the search narrows it, a row
    // starts the formula.
    await at("D1");
    await run("formulas.insertFunction");
    const fnRows = await paneRows();
    await page.keyboard.type("VLOO");
    await page.waitForTimeout(50);
    const fnRows2 = await paneRows();
    await pressPaneRow(0);
    const s1 = await S();
    await check("formulas.insertFunction", async () => [fnRows.length > 50 && fnRows2.length === 1 && s1.editing && s1.editText === "=VLOOKUP(", fnRows.length + " → " + fnRows2.join(",") + " → " + s1.editText]);
    await page.keyboard.press("Escape");
    const cats = [["financial", "PMT"], ["logical", "IF"], ["text", "LEFT"], ["dateTime", "DATE"], ["lookupReference", "VLOOKUP"], ["mathTrig", "ROUND"], ["moreFunctions", "AVERAGE"], ["recentlyUsed", "SUM"]];
    for (const [cat, fn] of cats) {
      await at("D2");
      await run("formulas." + cat);
      const rows = await paneRows();
      const i = rows.indexOf(fn);
      if (i >= 0) await pressPaneRow(i);
      const s2 = await S();
      ok("cmd.formulas." + cat + ".list", i >= 0 && s2.editing && s2.editText === "=" + fn + "(", rows.length + " listed, " + s2.editText);
      await page.keyboard.press("Escape");
    }
    // Threaded comments, the Comments pane, notes kept open.
    await at("B2");
    await run("review.newComment", "Check the count");
    await run("review.newComment", "Looks right");
    await check("review.newComment", async () => { const t = await M("M.threadAt(1,1)"); return [t === "You: Check the count\nLooks right".replace("\nLooks", "\nYou: Looks"), t]; });
    await run("review.showComments");
    await check("review.showComments", async () => { const rows = await paneRows(); return [(await M("G.paneKind")) === "comments" && rows.some((r) => /B2: You: Looks right/.test(r)), rows.join(" | ")]; });
    await at("A3");
    await run("review.newNote", "ink is low");
    await run("review.showNote");
    await at("A1");
    await check("review.showNote", async () => [await M("M.noteShown(2,0)")]);
    await run("review.showAllNotes");
    await check("review.showAllNotes", async () => [await M("M.showAllNotes && M.noteShown(2,0)")]);
    await run("review.showAllNotes");
    await run("review.convertToComments");
    await check("review.convertToComments", async () => [await M("!M.hasNote(2,0) && M.threadAt(2,0) === 'You: ink is low'"), await M("M.threadAt(2,0)")]);
    // Allow Edit Ranges: B2:B3 stays editable under protection.
    await run("review.allowEditRanges", "B2:B3");
    await run("review.protectSheet");
    await put("B3", "7");
    await put("A2", "Pencils");
    await run("review.unprotectSheet");
    await check("review.allowEditRanges", async () => [(await cellAt("B3")) === "7" && (await cellAt("A2")) === "Pens", (await cellAt("B3")) + "," + (await cellAt("A2"))]);
    await run("review.checkAccessibility");
    await check("review.checkAccessibility", async () => { const rows = await paneRows(); return [(await M("G.paneKind")) === "accessibility" && rows.some((r) => /alt text/.test(r)) && rows.some((r) => /default sheet name/.test(r)), rows.slice(0, 3).join(" | ")]; });
    // Custom Views: zoom and a hidden column come back.
    await run("view.zoom", "150");
    await at("C1");
    await run("home.hideColumns");
    await run("view.customViews", "add:Wide");
    await run("view.zoom", "100");
    await run("home.unhideColumns");
    await run("view.customViews");
    const cvRows = await paneRows();
    await pressPaneRow(0);
    await check("view.customViews", async () => [cvRows[0] === "Wide" && (await M("M.zoom")) === 1.5 && (await M("M.isColHidden(2)")), cvRows.join(",") + " zoom " + (await M("M.zoom"))]);
    await run("view.zoom", "100");
    await at("C1");
    await run("home.unhideColumns");
    // Record Actions, then play it back from Macros.
    await run("automate.recordActions");
    await put("E1", "rec");
    await at("E1");
    await run("format.bold");
    await run("automate.recordActions", "stop:Stamp");
    await at("F1");
    await run("view.macros");
    const mRows = await paneRows();
    await at("E1");
    await run("edit.clearAll");
    await pressPaneRow(mRows.indexOf("Macro: Stamp"));
    await check("automate.recordActions", async () => [mRows.includes("Macro: Stamp") && (await cellAt("E1")) === "rec" && (await style(0, 4)).bold, await M("G.macros.get ? G.macros.get('Stamp') : ''")]);
    await check("view.macros", async () => [mRows.includes("Macro: Stamp"), mRows.join(",")]);
    await run("automate.allScripts");
    await check("automate.allScripts", async () => [(await M("G.paneKind")) === "macros"]);
    const nScripts = await M("G.book.scriptCount()");
    await run("automate.newScript", "probe");
    await check("automate.newScript", async () => [(await M("G.book.scriptCount()")) === nScripts + 1]);
    await run("script.close");
    // File: properties, options, PDF, print.
    await run("file.properties", "title=Quarterly");
    await run("file.properties", "author=Tero");
    await check("file.properties", async () => { const x = await savedXml(); const core = x["docProps/core.xml"] || ""; return [/<dc:title>Quarterly<\/dc:title>/.test(core) && /<dc:creator>Tero/.test(core), core.slice(0, 80)]; });
    await run("file.options");
    const oRows = await paneRows();
    await pressPaneRow(0);
    await check("file.options", async () => [/^Gridlines: on/.test(oRows[0]) && (await M("M.showGridlines")) === false, oRows[0]]);
    await run("file.options", "gridlines");
    await run("file.exportPdf");
    await check("file.exportPdf", async () => { const h = await M("(() => { const p = G.pdfBytes; const b = p instanceof Uint8Array ? p : new Uint8Array(p || []); return String.fromCharCode(...Array.from(b.slice(0, 5))) + ' ' + b.length; })()"); return [/^%PDF-/.test(h), h + ' ' + (await M("G.view.status"))]; });
    await page.evaluate(() => { window.__printed = 0; window.print = () => { window.__printed++; }; });
    await run("file.print");
    await check("file.print", async () => [(await page.evaluate(() => window.__printed)) === 1]);
    await run("file.info");
    await check("file.info", async () => { const rows = await paneRows(); return [rows[0] === "Title: Quarterly" && rows.includes("Export to PDF"), rows.slice(0, 2).join(" | ")]; });
    // Data: filters, What-If, outline, analysis, import.
    await run("sheet.add");
    await run("sheet.rename", "DataTools");
    const rows = [["Region", "Rep", "Sales"], ["East", "ann smith", "10"], ["East", "bob jones", "20"], ["West", "cy young", "30"], ["West", "di lee", "40"], ["West", "ed kim", "50"]];
    for (let r = 0; r < rows.length; r++) for (let c = 0; c < 3; c++) await put(String.fromCharCode(65 + c) + (r + 1), rows[r][c]);
    await at("B2");
    await run("data.filter", "A=West");
    await check("data.filter", async () => [await M("M.rowSuppressed[1] && !M.rowSuppressed[3] && !M.rowSuppressed[0]"), await M("[0,1,2,3,4,5].map((r) => M.rowSuppressed[r] ? 'h' : 's').join('')")]);
    await put("A2", "West");
    await run("data.reapply");
    await check("data.reapply", async () => [await M("!M.rowSuppressed[1] && M.rowSuppressed[2]"), await M("[0,1,2,3,4,5].map((r) => M.rowSuppressed[r] ? 'h' : 's').join('')")]);
    await run("data.filter");
    await put("A2", "East");
    for (const [ref, v] of [["E1", "Sales"], ["E2", ">25"]]) await put(ref, v);
    await at("A1");
    await run("data.advancedFilter", "criteria=E1:E2;copyTo=G1");
    await check("data.advancedFilter", async () => { const v = [await cellAt("G1"), await cellAt("I2"), await cellAt("I4"), await cellAt("I5")]; return [v.join(",") === "Region,30,50,", v.join(",")]; });
    // Flash Fill: the first name from "ann smith", capitalised.
    await put("D1", "First");
    await put("D2", "Ann");
    await at("D2");
    await run("data.flashFill");
    await check("data.flashFill", async () => { const v = [await cellAt("D3"), await cellAt("D6")]; return [v.join(",") === "Bob,Ed", v.join(",")]; });
    await put("D3", "");
    await put("D4", "");
    await at("D2");
    await run("edit.flashFill");
    await check("edit.flashFill", async () => [(await cellAt("D4")) === "Cy", await cellAt("D4")]);
    // Custom Sort: Region descending, then Sales descending.
    await at("A2");
    await run("data.customSort", "A desc, C desc");
    await check("data.sort", async () => { const v = [await cellAt("A2"), await cellAt("C2"), await cellAt("C4")]; return [v.join(",") === "West,50,30", v.join(",")]; });
    // Circle Invalid Data.
    await at("C2");
    await run("data.validation", "whole:0:45");
    await sel("C2:C6");
    await run("data.validation", "whole:0:45");
    await run("data.circleInvalid");
    await check("data.circleInvalid", async () => [(await M("M.invalidCircles.length")) === 2, await M("Array.from(M.invalidCircles).join(',')")]);
    await run("data.circleInvalid", "clear");
    // Goal Seek and Solver on K1 = K2 * 3.
    await put("K2", "1");
    await put("K1", "=K2*3");
    await at("K1");
    await run("data.goalSeek", "set=K1;to=90;by=K2");
    await check("data.goalSeek", async () => [Math.abs(Number(await cellAt("K2")) - 30) < 0.001, await cellAt("K2")]);
    await put("L2", "1");
    await put("L1", "=-(L2-7)*(L2-7)+4");
    await run("data.solver", "set=L1;max;by=L2;limits=L2<=20");
    await check("data.solver", async () => [Math.abs(Number(await cellAt("L2")) - 7) < 0.01, await cellAt("L2") + " → " + (await cellAt("L1"))]);
    // Scenarios.
    await run("data.scenarioManager", "add:Best:K2=100");
    await run("data.scenarioManager", "add:Worst:K2=5");
    await run("data.scenarioManager");
    const scRows = await paneRows();
    await pressPaneRow(1);
    await check("data.scenarioManager", async () => [scRows.join(",") === "Best,Worst" && (await cellAt("K1")) === "15", scRows.join(",") + " K1=" + (await cellAt("K1"))]);
    // Data Table: N2:N4 inputs into K2, O1 reads K1.
    for (const [ref, v] of [["N2", "1"], ["N3", "2"], ["N4", "3"], ["O1", "=K1"]]) await put(ref, v);
    await sel("N1:O4");
    await run("data.dataTable", "col=K2");
    await check("data.dataTable", async () => { const v = [await cellAt("O2"), await cellAt("O3"), await cellAt("O4"), await cellAt("K2")]; return [v.join(",") === "3,6,9,5", v.join(",")]; });
    // Consolidate two blocks by label.
    for (const [ref, v] of [["Q1", "a"], ["R1", "1"], ["Q2", "b"], ["R2", "2"], ["Q4", "b"], ["R4", "5"], ["Q5", "c"], ["R5", "7"]]) await put(ref, v);
    await at("T1");
    await run("data.consolidate", "sum:Q1:R2;Q4:R5");
    await check("data.consolidate", async () => { const v = [await cellAt("T1"), await cellAt("U1"), await cellAt("T2"), await cellAt("U2"), await cellAt("T3"), await cellAt("U3")]; return [v.join(",") === "a,1,b,7,c,7", v.join(",")]; });
    // Group, hide and show detail.
    await sel("A3:A4");
    await run("data.group");
    await at("A3");
    await run("data.hideDetail");
    const hid = await M("M.isRowHidden(2) && M.isRowHidden(3)");
    await run("data.showDetail");
    await check("data.group", async () => [(await M("M.rowLevels.get ? M.rowLevels.get(2) : M.rowLevels[2]")) === 1]);
    await check("data.hideDetail", async () => [hid]);
    await check("data.showDetail", async () => [await M("!M.isRowHidden(2) && !M.isRowHidden(3)")]);
    await sel("A3:A4");
    await run("data.ungroup");
    await check("data.ungroup", async () => [(await M("M.rowLevels.get ? M.rowLevels.get(2) : M.rowLevels[2]")) === 0]);
    // Subtotal: by Region, sum of Sales.
    await at("A1");
    await run("data.customSort", "A asc");
    await run("data.subtotal", "at=A;fn=sum;cols=C");
    await check("data.subtotal", async () => { const v = [await cellAt("A4"), await cellAt("C4"), await cellAt("A9"), await cellAt("C9")]; return [v.join(",") === "East Total,30,Grand Total,150", v.join(",") + " " + (await M("M.getFormula(8,2)"))]; });
    // Analysis ToolPak: descriptive statistics of Sales.
    await run("sheet.add");
    for (const [ref, v] of [["A1", "Score"], ["A2", "2"], ["A3", "4"], ["A4", "4"], ["A5", "6"]]) await put(ref, v);
    await sel("A1:A5");
    await run("data.analysisToolPak");
    await check("data.analysisToolPak", async () => { const v = [(await S()).sheets.slice(-1)[0], await cellAt("A1"), await cellAt("B3"), await cellAt("B6"), await cellAt("B13")]; return [v.slice(1).join(",") === "Score,4,4,4" && /Descriptive/.test(v[0]), v.join(",")]; });
    // Forecast Sheet.
    await run("sheet.add");
    for (const [ref, v] of [["A1", "Month"], ["B1", "Units"], ["A2", "1"], ["B2", "10"], ["A3", "2"], ["B3", "20"], ["A4", "3"], ["B4", "30"]]) await put(ref, v);
    await sel("A1:B4");
    const charts0 = await M("G.view.chartLayer.count()");
    await run("data.forecastSheet", "n=2");
    await check("data.forecastSheet", async () => { const v = [await cellAt("A6"), await cellAt("C6"), await M("G.view.chartLayer.count()") - charts0]; return [v.join(",") === "5,50,1" && /Forecast/.test((await S()).sheets.slice(-1)[0]), v.join(",")]; });
    // Import: CSV text, and a table loaded as a query that Refresh All rereads.
    await run("data.fromTextCsv", 'Name,City\n"Smith, J",Oslo\nLee,Rome');
    await check("data.fromTextCsv", async () => { const v = [await cellAt("A2"), await cellAt("B3")]; return [v.join("|") === "Smith, J|Rome", v.join("|")]; });
    await at("A1");
    await run("data.fromTableRange");
    const qSheet = (await S()).sheets.slice(-1)[0];
    await check("data.fromTableRange", async () => [(await cellAt("A2")) === "Smith, J" && /Query/.test(qSheet), qSheet]);
    await run("view.sheet.prev");
    await put("B3", "Paris");
    await run("data.refreshAll");
    await run("view.sheet.next");
    await check("data.refreshAll", async () => [(await cellAt("B3")) === "Paris", await cellAt("B3")]);
    await run("data.queriesConnections");
    await check("data.queriesConnections", async () => { const rows = await paneRows(); return [rows.some((r) => /^Query: .*Query/.test(r)), rows.join(" | ")]; });
    await run("data.getData");
    await check("data.getData", async () => { const rows = await paneRows(); return [rows.includes("From Text/CSV") && rows.includes("From Database (SQL)"), rows.join(" | ")]; });

    // ✕ closes the pane.
    await page.evaluate(() => { const b = [...document.querySelectorAll("button,[role=button]")].find((e) => (e.dataset && e.dataset.a11yId) === "sx-pane-head-pane-close"); window.__closeRect = b ? JSON.stringify(b.getBoundingClientRect()) : ""; });
    const cr = JSON.parse((await page.evaluate(() => window.__closeRect)) || "null");
    if (cr) await page.mouse.click(cr.x + cr.width / 2, cr.y + cr.height / 2);
    await check("pane.close", async () => [(await M("G.paneKind")) === "", await M("G.paneKind")]);
  };

  // Drawing objects, Page Layout themes, Home extras, windows, proofing:
  // a sheet of their own (PROBE_NEW2=1 runs only these).
  const PNG4x3 = "iVBORw0KGgoAAAANSUhEUgAAAAQAAAADCAIAAAA7ljmRAAAAEElEQVR4nGM4IScHRww4OQD1xwwxB+utmQAAAABJRU5ErkJggg==";
  const PNG8 = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEUlEQVR4nGN49uwXVsQwtCQAVdOxgZwiRwkAAAAASUVORK5CYII=";
  // Page coordinates of a point in the grid's own space.
  const toPage = (x, y) => page.evaluate(([x, y]) => { const A = window.sheet.app; const c = document.querySelector("canvas").getBoundingClientRect(); return { x: c.left + A.gx + x, y: c.top + A.gy + y }; }, [x, y]);
  const shapeCentre = async (i) => { const r = await M("(() => { const r = G.shapeScreenRect(" + i + "); return [r.x, r.y, r.w, r.h]; })()"); return toPage(r[0] + r[2] / 2, r[1] + r[3] / 2); };
  const cellCentre = async (r, c) => { const g = await M("(() => { const q = G.grid.cellRect(G.model, " + r + ", " + c + "); return [q.x, q.y, q.w, q.h]; })()"); return toPage(g[0] + g[2] / 2, g[1] + g[3] / 2); };
  const drag = async (pts) => { await page.mouse.move(pts[0].x, pts[0].y); await page.mouse.down(); for (const p of pts.slice(1)) await page.mouse.move(p.x, p.y, { steps: 2 }); await page.mouse.up(); await page.waitForTimeout(30); };
  const shapes = () => M("Array.from(G.model.shapes).map((s) => s.kind + ':' + s.name + ':' + s.text + ':' + s.rotation + ':' + s.groupId)");
  const newChecks2 = async () => {
    await run("sheet.add");
    await run("sheet.rename", "Shapes");
    for (const [ref, v] of [["A1", "Region"], ["B1", "Sales"], ["A2", "East"], ["B2", "10"], ["A3", "West"], ["B3", "20"], ["A4", "East"], ["B4", "30"]]) await put(ref, v);
    // Insert › Shapes: an ellipse at C2, picked by a click and moved by a drag.
    await at("C6");
    await run("insert.shape", "ellipse");
    await page.keyboard.press("Escape");
    const p0 = await shapeCentre(0);
    await page.mouse.click(p0.x, p0.y);
    const picked = await M("Array.from(G.view.selShapes).join(',')");
    const off0 = await M("G.model.shapes[0].offX");
    await drag([p0, { x: p0.x + 15, y: p0.y }, { x: p0.x + 30, y: p0.y + 10 }]);
    const off1 = await M("G.model.shapes[0].offX");
    await check("insert.shape", async () => { const k = await shapes(); return [k[0].startsWith("ellipse:") && picked === "0" && off1 - off0 >= 25, k[0] + " picked=" + picked + " moved " + (off1 - off0)]; });
    await at("F2");
    await run("insert.textBox", "Hello box");
    await run("insert.icons", "heart");
    await run("insert.wordArt", "Big title");
    await run("insert.signatureLine", "Tero T");
    await run("insert.equation", "x^2+y_1");
    await run("insert.smartArt", "Plan;Do;Check");
    const all = await shapes();
    await check("insert.textBox", async () => [all.some((k) => k.startsWith("textBox:") && k.includes(":Hello box:")), all[1]]);
    await check("insert.icons", async () => [await M("G.model.shapes[2].kind === 'icon' && G.model.shapes[2].icon === 'heart'"), all[2]]);
    await check("insert.wordArt", async () => [all[3].startsWith("wordArt:") && all[3].includes("Big title"), all[3]]);
    await check("insert.signatureLine", async () => [all[4].startsWith("signature:") && all[4].includes("Tero T"), all[4]]);
    await check("insert.equation", async () => [all[5].includes(":x²+y₁:"), all[5]]);
    await check("insert.smartArt", async () => { const sa = all.slice(6); const g = new Set(sa.map((k) => k.split(":").pop())); return [sa.filter((k) => k.startsWith("roundRect:")).length === 3 && sa.filter((k) => k.startsWith("arrow:")).length === 2 && g.size === 1 && !g.has("0"), sa.length + " shapes, groups " + [...g].join(",")]; });
    // Arrange: z-order, rotate, group, align, the selection pane.
    await run("shape.select", "0");
    await run("arrange.bringForward");
    const afterFwd = (await shapes())[1];
    await check("arrange.bringForward", async () => [afterFwd.startsWith("ellipse:") && (await M("G.view.selShapes[0]")) === 1, afterFwd]);
    await run("arrange.sendBackward");
    await check("arrange.sendBackward", async () => { const k = await shapes(); return [k[0].startsWith("ellipse:"), k[0]]; });
    await run("arrange.rotate", "right");
    await check("arrange.rotate", async () => [await M("G.model.shapes[0].rotation") === 90, await M("G.model.shapes[0].rotation")]);
    await run("shape.select", "0");
    await run("shape.select", "+1");
    await run("arrange.align", "left");
    await check("arrange.align", async () => { const xs = await M("[0, 1].map((i) => G.shapeScreenRect(i).x)"); return [xs[0] === xs[1], xs.join(",")]; });
    await run("arrange.group");
    await check("arrange.group", async () => { const g = await M("[G.model.shapes[0].groupId, G.model.shapes[1].groupId]"); await page.keyboard.press("Escape"); const p = await shapeCentre(1); await page.mouse.click(p.x, p.y); const n = await M("G.view.selShapes.length"); return [g[0] > 0 && g[0] === g[1] && n === 2, g.join(",") + " click selects " + n]; });
    await run("arrange.selectionPane");
    const selRows = await paneRows();
    const tbRow = selRows.findIndex((r) => /^Shape: TextBox/.test(r));
    await pressPaneRow(tbRow);
    await check("arrange.selectionPane", async () => [tbRow >= 0 && (await M("Array.from(G.view.selShapes).some((i) => G.model.shapes[i].kind === 'textBox')")), selRows[tbRow] + " sel " + (await M("Array.from(G.view.selShapes).join(',')"))]);
    await page.keyboard.press("Escape");
    // Draw: a pen stroke, Ink to Shape, the eraser and the lasso.
    await run("view.zoom", "100");
    const base = await cellCentre(14, 2);
    await run("draw.pen");
    await drag([base, { x: base.x + 40, y: base.y + 5 }, { x: base.x + 90, y: base.y + 12 }]);
    const ring = [];
    for (let i = 0; i <= 16; i++) { const a = (Math.PI * 2 * i) / 16; ring.push({ x: base.x + 200 + 40 * Math.cos(a), y: base.y + 30 * Math.sin(a) }); }
    await drag(ring);
    const scrib = { x: base.x, y: base.y + 80 };
    await drag([scrib, { x: scrib.x + 50, y: scrib.y + 3 }, { x: scrib.x + 90, y: scrib.y + 6 }]);
    await run("draw.pen");
    const inks = await M("G.model.shapes.filter((s) => s.kind === 'ink').length");
    await check("draw.pen", async () => [inks === 3, inks + " strokes"]);
    await run("draw.eraser");
    await page.mouse.click(scrib.x + 45, scrib.y + 3);
    await run("draw.eraser");
    await check("draw.eraser", async () => { const n = await M("G.model.shapes.filter((s) => s.kind === 'ink').length"); return [n === 2, n + " strokes left"]; });
    await run("draw.inkToShape");
    await check("draw.inkToShape", async () => { const k = (await shapes()).slice(-2).map((x) => x.split(":")[0]); return [k.join(",") === "line,ellipse", k.join(",")]; });
    await run("draw.lasso");
    const lq = await M("(() => { const r = G.shapeScreenRect(G.model.shapes.length - 1); return [r.x, r.y, r.w, r.h]; })()");
    const la = await toPage(lq[0] - 8, lq[1] - 8), lb = await toPage(lq[0] + lq[2] + 8, lq[1] + lq[3] + 8);
    await drag([la, { x: (la.x + lb.x) / 2, y: (la.y + lb.y) / 2 }, lb]);
    await run("draw.lasso");
    await check("draw.lasso", async () => { const s = await M("Array.from(G.view.selShapes).join(',')"); const last = await M("G.model.shapes.length - 1"); return [s === String(last), s]; });
    await page.keyboard.press("Escape");
    // Pictures, checkboxes, sparklines.
    await at("J2");
    const imgs0 = await M("M.images.length");
    await run("insert.picture", "data:image/png;base64," + PNG4x3);
    await check("insert.picture", async () => { const v = await M("[M.images.length, M.images[M.images.length - 1].widthPx, M.images[M.images.length - 1].heightPx].join(',')"); return [v === (imgs0 + 1) + ",4,3", v]; });
    await sel("H24:H25");
    await run("insert.checkbox");
    const cbBefore = await cellAt("H24");
    const cbPt = await cellCentre(23, 7);
    await page.mouse.click(cbPt.x, cbPt.y);
    await check("insert.checkbox", async () => { const v = [cbBefore, await M("M.getCell(23, 7)"), await M("M.isCheckbox(24, 7)")]; return [v.join(",") === "FALSE,TRUE,true", v.join(",")]; });
    await at("E2");
    await run("insert.sparkline", "column:B2:B4");
    await check("insert.sparkline", async () => { const v = await M("(() => { const s = M.sparklines[M.sparklines.length - 1]; return [s.row, s.col, s.kind, s.r1, s.r2].join(','); })()"); return [v === "1,4,column,1,3", v]; });
    // A slicer over Region: picking West hides the East rows.
    await at("A1");
    await run("insert.slicer", "Region");
    const sl = await M("G.model.shapes.length - 1");
    const sr = await M("(() => { const r = G.shapeScreenRect(" + sl + "); return [r.x, r.y, r.w, r.h]; })()");
    const items = await M("Array.from(G.model.shapes[" + sl + "].items).join(',')");
    const wi = items.split(",").indexOf("West");
    const wp = await toPage(sr[0] + 40, sr[1] + 30 + 24 * wi + 10);
    await page.mouse.click(wp.x, wp.y);
    await check("insert.slicer", async () => { const h = await M("[1, 2, 3].map((r) => M.rowSuppressed[r] ? 'h' : 's').join('')"); return [items === "East,West" && h === "hsh", items + " → " + h]; });
    await run("slicer.pick", "West");
    // Timeline: months of a date column.
    for (const [ref, v] of [["C1", "Date"], ["C2", "=DATE(2026,1,5)"], ["C3", "=DATE(2026,2,7)"], ["C4", "=DATE(2026,1,20)"]]) await put(ref, v);
    await at("A1");
    await run("insert.timeline", "Date");
    const tl = await M("G.model.shapes.length - 1");
    await run("slicer.pick", "2026-02");
    await check("insert.timeline", async () => { const v = await M("Array.from(G.model.shapes[" + tl + "].items).join(',') + ' ' + [1, 2, 3].map((r) => M.rowSuppressed[r] ? 'h' : 's').join('')"); return [v === "2026-01,2026-02 hsh", v]; });
    await run("slicer.pick", "2026-02");
    // A table's own slicer.
    for (const [ref, v] of [["L1", "Team"], ["M1", "Pts"], ["L2", "Red"], ["M2", "3"], ["L3", "Blue"], ["M3", "5"], ["L4", "Red"], ["M4", "1"]]) await put(ref, v);
    await sel("L1:M4");
    await run("insert.table");
    await at("L2");
    await run("table.insertSlicer", "Team");
    await check("table.insertSlicer", async () => { const v = await M("(() => { const s = G.model.shapes[G.model.shapes.length - 1]; return s.kind + ' ' + Array.from(s.items).join(',') + ' col ' + s.column; })()"); return [v === "slicer Red,Blue col 11", v]; });

    // Page Layout › Themes.
    await run("pageLayout.theme", "Facet");
    await check("pageLayout.theme", async () => { const v = await M("[G.themeAccent, G.book.themeName, M.getCellStyle(60, 60).fontName].join(',')"); return [v === "#90C226,Facet,Trebuchet MS", v]; });
    await run("pageLayout.themeColors", "Blue");
    await at("N8");
    await run("insert.shape", "rect");
    await check("pageLayout.themeColors", async () => { const v = await M("G.model.shapes[G.model.shapes.length - 1].fill"); return [v === "#0F6FC6", v]; });
    await page.keyboard.press("Escape");
    await run("pageLayout.themeFonts", "Georgia");
    await check("pageLayout.themeFonts", async () => { const v = await M("M.getCellStyle(60, 60).fontName"); return [v === "Georgia", v]; });
    await run("pageLayout.themeEffects", "Intense");
    await check("pageLayout.themeEffects", async () => [await M("G.view.shapeEffect") === 3, await M("G.view.shapeEffect")]);
    await run("pageLayout.theme", "Office");
    await run("pageLayout.themeEffects", "None");
    await run("pageLayout.background", "data:image/png;base64," + PNG8);
    const bg = await M("[M.backgroundSrc !== '', M.bgWidth, M.bgHeight].join(',')");
    await run("pageLayout.background", "delete");
    await check("pageLayout.background", async () => [bg === "true,8,8" && (await M("M.backgroundSrc")) === "", bg]);
    await run("pageLayout.pageSetup");
    const psRows = await paneRows();
    await pressPaneRow(0);
    await run("pageLayout.pageSetup", "size=a4;margins=wide");
    await check("pageLayout.pageSetup", async () => { const v = await M("[M.pageOrientation, M.paperSize, M.marginLeft].join(',')"); return [/^Orientation: Portrait/.test(psRows[0]) && psRows.some((r) => /^Paper size:/.test(r)) && v === "landscape,9,1", psRows.slice(0, 3).join(" | ") + " → " + v]; });
    await run("pageLayout.orientation", "portrait");
    await run("pageLayout.margins", "normal");
    await run("pageLayout.size", "letter");

    // Home: super/subscript, number formats, styles, Format Cells, cells.
    await at("A2");
    await run("home.superscript");
    const sup = await M("M.getCellStyle(1, 0).vertAlign");
    await run("home.subscript");
    await check("home.superscript", async () => [sup === "superscript", sup]);
    await check("home.subscript", async () => { const v = await M("M.getCellStyle(1, 0).vertAlign"); return [v === "subscript", v]; });
    await run("home.subscript");
    await at("B2");
    await run("format.numfmt", "0.000");
    const custom = await cellAt("B2");
    await run("format.numfmt", "longDate");
    const longD = await cellAt("B2");
    await check("home.numberFormat", async () => [custom === "10.000" && /^[A-Z][a-z]+day, January 9, 1900$/.test(longD), custom + " | " + longD]);
    await run("format.numfmt", "accounting");
    await check("home.accounting", async () => { const v = await cellAt("B2"); return [/^ \$\s*10\.00 $/.test(v), JSON.stringify(v)]; });
    await run("format.numfmt", "general");
    await at("A1");
    await run("format.bold");
    await run("format.fill", "#FFE699");
    await run("home.newCellStyle", "Banner");
    await at("H9");
    await run("home.cellStyle", "Banner");
    await check("home.newCellStyle", async () => { const s = await style(8, 7); return [s.bold && s.fill === "#FFE699", JSON.stringify(s)]; });
    await at("H10");
    await run("home.formatCells");
    const fcRows = await paneRows();
    await pressPaneRow(fcRows.findIndex((r) => /^Bold:/.test(r)));
    await run("home.formatCells", "fill=#C6EFCE;border=all;numfmt=0.0");
    await check("home.formatCells", async () => { const s = await style(9, 7); const want = ["Number:", "Alignment:", "Font:", "Border:", "Fill:", "Locked:"]; return [want.every((w) => fcRows.some((r) => r.startsWith(w))) && s.bold && s.fill === "#C6EFCE" && s.border, fcRows.length + " rows; " + JSON.stringify(s)]; });
    await put("G1", "5");
    await put("H1", "=G1*2");
    await at("G1");
    await run("home.insertCells");
    const ins = [await cellAt("G2"), await M("M.getFormula(0, 7)"), await cellAt("H1")];
    await check("home.insertCells", async () => [ins.join("|") === "5|G2*2|10", ins.join("|")]);
    await at("G1");
    await run("home.deleteCells");
    await check("home.deleteCells", async () => { const v = [await cellAt("G1"), await M("M.getFormula(0, 7)"), await cellAt("H1")]; return [v.join("|") === "5|G1*2|10", v.join("|")]; });
    const order0 = (await S()).sheets.join(",");
    await run("sheet.move", "before:Sales");
    const order1 = (await S()).sheets;
    await check("sheet.move", async () => [order1[0] === "Shapes", order0 + " → " + order1.join(",")]);
    await run("sheet.move", "end");
    await run("home.addIns");
    const aiRows = await paneRows();
    await pressPaneRow(aiRows.findIndex((r) => /^Inquire/.test(r)));
    await check("home.addIns", async () => { const r2 = await paneRows(); return [aiRows.includes("Analysis ToolPak: on") && r2.includes("Inquire: on"), r2.join(" | ")]; });

    // Review › Proofing, Show Changes; Data › Analyze Data, Relationships, From Web.
    await run("review.spelling", "words:the cat sat on a mat ten run runner runs");
    await put("A10", "teh cat sat");
    await run("review.spelling");
    const spRows = await paneRows();
    const spRow = spRows.findIndex((r) => /^A10: teh → the/.test(r));
    if (spRow >= 0) await pressPaneRow(spRow);
    await check("review.spelling", async () => { const v = await cellAt("A10"); return [spRow >= 0 && v === "the cat sat", spRows[Math.max(spRow, 0)] + " → " + v]; });
    await put("A11", "running");
    await at("A11");
    await run("review.thesaurus");
    const thRows = await paneRows();
    await pressPaneRow(thRows.indexOf("runner"));
    await check("review.thesaurus", async () => { const v = await cellAt("A11"); return [thRows.includes("runner") && v === "runner", thRows.join(",") + " → " + v]; });
    await run("review.showChanges");
    await check("review.showChanges", async () => { const r = await paneRows(); return [/Shapes!A11: "" → "running"/.test(r[0]), r[0]]; });
    await at("A2");
    await run("data.analyzeData");
    await check("data.analyzeData", async () => { const r = await paneRows(); return [r.some((x) => /^Sales: total 60, average 20/.test(x)) && r.some((x) => /highest 30 \(East\)/.test(x)), r.slice(0, 2).join(" | ")]; });
    for (const [ref, v] of [["P1", "Customer"], ["Q1", "City"], ["P2", "Ann"], ["Q2", "Oslo"], ["P3", "Bo"], ["Q3", "Rome"], ["S1", "Customer"], ["T1", "Amount"], ["S2", "Ann"], ["T2", "5"], ["S3", "Ann"], ["T3", "7"], ["S4", "Bo"], ["T4", "2"]]) await put(ref, v);
    await sel("P1:Q3");
    await run("insert.table");
    await sel("S1:T4");
    await run("insert.table");
    await run("data.relationships");
    await check("data.relationships", async () => { const r = await paneRows(); return [r.some((x) => /\[Customer\] → .*\[Customer\]/.test(x)), r.join(" | ")]; });
    await run("data.fromWeb", "http://localhost:" + PORT + "/table.html");
    await page.waitForFunction(() => window.sheet.state().sheets.some((n) => /^Web/.test(n)), null, { timeout: 10000 }).catch(() => {});
    await check("data.fromWeb", async () => { const v = [await cellAt("A1"), await cellAt("A2"), await cellAt("B3")]; return [v.join("|") === "Name|Alpha & Co|2.25", v.join("|")]; });

    // View › Window.
    await run("view.newWindow");
    await check("view.newWindow", async () => { const v = await M("[G.windowSheets.length, G.curWindow].join(',')"); return [v === "2,1", v]; });
    await run("view.sheet.prev");
    await run("view.switchWindows", "1");
    const w1 = (await S()).activeSheet;
    await run("view.switchWindows", "2");
    const w2 = (await S()).activeSheet;
    await check("view.switchWindows", async () => [w1 !== w2 && (await M("G.curWindow")) === 1, w1 + " / " + w2]);
    await run("view.arrangeAll", "vertical");
    await check("view.arrangeAll", async () => { const v = await M("[G.arranged, G.secondPane.paneX > 100, G.grid.w < G.W, G.secondSheet !== G.book.activeIndex].join(',')"); return [v === "true,true,true,true", v]; });
    const sp = await M("(() => { const q = G.secondPane; return [q.paneX + 80, q.paneY + 60]; })()");
    const spPt = await toPage(sp[0], sp[1]);
    await page.mouse.click(spPt.x, spPt.y);
    const paneSel = await M("G.secondPane.sel.active.row + ',' + G.secondPane.sel.active.col");
    await run("view.arrangeAll", "off");
    await run("view.split");
    await check("view.split", async () => { const v = await M("[G.arranged, G.splitOn, G.secondSheet === G.book.activeIndex].join(',')"); return [v === "true,true,true" && paneSel !== "0,0", v + " pane click " + paneSel]; });
    await run("view.split");
    await run("view.hideWindow");
    const hid = await M("Array.from(G.windowHidden).join(',') + ' ' + G.curWindow");
    await run("view.unhideWindow");
    const unRows = await paneRows();
    await pressPaneRow(0);
    await check("view.hideWindow", async () => [hid === "false,true 0", hid]);
    await check("view.unhideWindow", async () => { const v = await M("Array.from(G.windowHidden).join(',') + ' ' + G.curWindow"); return [unRows.length === 1 && v === "false,false 1", unRows.join(",") + " → " + v]; });
    await run("view.pageLayout");
    const rulerOn = await M("M.showRuler");
    await run("view.ruler");
    await check("view.ruler", async () => [rulerOn === true && (await M("M.showRuler")) === false, rulerOn]);
    await run("view.ruler");
    await run("view.normal");
    await run("view.switchWindows", "1");
  };
  // The UI checklist rows that need a pointer or keys: QAT, File view,
  // galleries, contextual tabs, the formula bar, the grid's gestures, sheet
  // tabs, the zoom slider and the task panes (PROBE_UI=1 runs only these).
  const axNow = () => page.evaluate(() => [...document.querySelectorAll("[role],button,input")].map((e) => ({
    id: (e.dataset && e.dataset.a11yId) || "",
    role: e.getAttribute("role") || (e.tagName === "BUTTON" ? "button" : "textbox"),
    name: (e.getAttribute("aria-label") || e.textContent || "").trim().slice(0, 80),
    r: (({ x, y, width, height }) => ({ x, y, w: width, h: height }))(e.getBoundingClientRect()),
  })));
  const byId = async (id) => (await axNow()).find((n) => n.id === id && n.r.w > 0);
  const clickNode = async (n, opts = {}) => { await page.mouse.click(n.r.x + n.r.w / 2, n.r.y + n.r.h / 2, opts); await page.waitForTimeout(80); };
  // The chrome's own rectangle for a control, in page coordinates.
  const tidRect = (tid) => page.evaluate((t) => { const r = window.sheet.app.rectOf(t); if (!r) return null; const [x, y, w, h] = r.split(",").map(Number); const c = document.querySelector("canvas").getBoundingClientRect(); return { r: { x: c.left + x, y: c.top + y, w, h } }; }, tid);
  const clickTid = async (tid, opts = {}) => { const n = await tidRect(tid); if (!n) throw new Error("no " + tid); await clickNode(n, opts); };
  const clickId = async (id) => { const n = await byId(id); if (!n) throw new Error("no " + id); await clickNode(n); };
  const uiCheck = async (id, fn) => {
    try {
      const [pass, detail] = await fn();
      ok("ui." + id, !!pass, detail === undefined ? "" : String(detail));
    } catch (e) {
      ok("ui." + id, false, String(e && e.message || e).slice(0, 160));
    }
  };
  const closePane = () => page.evaluate(async () => { window.sheet.app.grid.app.closePane(); window.sheet.app.sync(); await window.sheet.redraw(); });
  const uiChecks2 = async () => {
    console.log("== ui");
    await closePane();
    // The target workbook's own table and charts, on Sales.
    await page.evaluate(async () => { window.sheet.app.showSheet("Sales"); window.sheet.app.sync(); await window.sheet.redraw(); });
    await uiCheck("homeGroups", async () => {
      const names = (await axNow()).filter((n) => n.role === "toolbar").map((n) => n.name);
      const want = ["Clipboard", "Font", "Alignment", "Number", "Styles", "Cells", "Editing"];
      return [want.every((w) => names.includes(w)), names.join(", ")];
    });
    await uiCheck("tableFilterButton", async () => {
      const r = await M("(() => { const t = M.tableAt(0); const c = G.grid.cellRect(M, t.r0, t.c0); return [c.x, c.y, c.w, c.h, t.r0, t.c0]; })()");
      const p = await toPage(r[0] + r[2] - 8, r[1] + r[3] / 2);
      await page.mouse.click(p.x, p.y);
      await page.waitForTimeout(80);
      const rows = await paneRows();
      const kind = await M("G.paneKind");
      const north = rows.findIndex((x) => /^☑ North/.test(x));
      if (north >= 0) await pressPaneRow(north);
      const hidden = await M("(() => { const t = M.tableAt(0); let n = 0; for (let r = t.r0 + 1; r <= t.r1; r++) if (M.rowSuppressed[r]) n++; return n; })()");
      const rows2 = await paneRows();
      await pressPaneRow(2);
      const back = await M("(() => { const t = M.tableAt(0); let n = 0; for (let r = t.r0 + 1; r <= t.r1; r++) if (M.rowSuppressed[r]) n++; return n; })()");
      return [kind === "tableFilter" && north >= 0 && hidden === 1 && rows2[north] === "☐ North" && back === 0, kind + " " + rows.slice(0, 6).join(",") + " hidden " + hidden + " → " + back];
    });
    await closePane();
    await uiCheck("contextTabs", async () => {
      await at("B5");
      const tabsIn = (await axNow()).filter((n) => n.role === "tab").map((n) => n.name);
      if (tabsIn.includes("Table Design")) await clickTid("sx-ribtabs-tab-tabledesign");
      await page.evaluate(() => window.sheet.redraw());
      await page.waitForTimeout(80);
      const groups = (await axNow()).filter((n) => n.role === "toolbar").map((n) => n.name);
      await at("B30");
      const tabsOut = (await axNow()).filter((n) => n.role === "tab").map((n) => n.name);
      // A chart: a click on it brings Chart Design.
      const cr = await M("(() => { const w = G.view.chartLayer.panelAt(0).win; return [w.x, w.y, w.w, w.h]; })()");
      const cp = await toPage(cr[0] + cr[2] / 2, cr[1] + cr[3] / 2);
      await page.mouse.click(cp.x, cp.y);
      await page.waitForTimeout(80);
      await page.evaluate(() => window.sheet.redraw());
      await page.waitForTimeout(80);
      const tabsChart = (await axNow()).filter((n) => n.role === "tab").map((n) => n.name);
      await at("B30");
      return [tabsIn.includes("Table Design") && groups.includes("Table Style Options") && !tabsOut.includes("Table Design") && tabsChart.includes("Chart Design"), "in table: " + tabsIn.includes("Table Design") + ", groups " + groups.filter((g) => /Table/.test(g)).join("/") + ", outside: " + tabsOut.includes("Table Design") + ", chart: " + tabsChart.includes("Chart Design")];
    });
    await run("sheet.add");
    await run("sheet.rename", "UI");
    await uiCheck("qat", async () => {
      const ns = (await axNow()).filter((n) => n.id.startsWith("sx-qat-") && n.role === "button").map((n) => n.name);
      await put("A1", "qat");
      await clickId("sx-qat-qat-undo");
      const v = await cellAt("A1");
      return [["Save", "Undo", "Redo"].every((x) => ns.includes(x)) && v === "", ns.join(",") + " A1 after undo=" + JSON.stringify(v)];
    });
    await uiCheck("backstage", async () => {
      await clickTid("sx-ribtabs-tab-file");
      const title = await M("G.paneTitle()");
      const rows = await paneRows();
      await pressPaneRow(rows.indexOf("Info"));
      const kind = await M("G.paneKind");
      const want = ["Info", "New", "Open…", "Save As (.xlsx)", "Print", "Export to PDF"];
      return [title === "File" && want.every((x) => rows.includes(x)) && kind === "info", title + ": " + rows.join(",") + " → " + kind];
    });
    await closePane();
    await uiCheck("galleries", async () => {
      await at("C3");
      await clickId("sx-stylegallery-trigger");
      const good = await byId("sx-stylegallery-item-cs-1");
      if (good) await clickNode(good);
      const fill = (await style(2, 2)).fill;
      await at("C4");
      await clickId("sx-tablegallery-trigger");
      const items = (await axNow()).filter((n) => n.id.startsWith("sx-tablegallery-item-")).map((n) => n.name);
      await page.keyboard.press("Escape");
      await clickId("sx-cfgallery-trigger");
      const cf = (await axNow()).filter((n) => n.id.startsWith("sx-cfgallery-item-")).map((n) => n.name);
      await page.keyboard.press("Escape");
      return [fill === "#C6EFCE" && items.includes("Medium 2") && cf.some((x) => /Data Bars/.test(x)), "Good fill " + fill + "; tables " + items.length + "; cf " + cf.length];
    });
    await uiCheck("expandFormulaBar", async () => {
      await at("A2");
      await page.keyboard.type("one");
      await page.keyboard.press("Enter");
      const ex0 = await byId("app/fx-expand");
      await clickId("app/fx-expand");
      const h1 = await M("G.grid.formulaBarH");
      const st1 = JSON.stringify(ex0 && ex0.r) + " fb " + (await M("[G.view.fbFieldX, G.view.fbFieldW, G.editing].join(',')"));
      const name = (await axNow()).find((n) => n.id === "app/fx-expand").name;
      await clickId("app/fx-expand");
      const h2 = await M("G.grid.formulaBarH");
      return [h1 > 60 && h2 === 32 && /Collapse/.test(name), h1 + " → " + h2 + " (" + name + ") " + st1];
    });
    await uiCheck("autocomplete", async () => {
      await at("B2");
      await page.keyboard.type("=VLO");
      const list = await M("Array.from(G.view.acItems)");
      await page.keyboard.press("Tab");
      const buf = await M("G.editBuf");
      const tip = await M("G.view.acTip");
      await page.keyboard.type("A1,");
      const arg = await M("G.view.acTipArg");
      await page.keyboard.press("Escape");
      return [list[0] === "VLOOKUP" && buf === "=VLOOKUP(" && /^VLOOKUP\|lookup_value\|table_array/.test(tip) && arg === 1, list.join(",") + " → " + buf + " tip " + tip + " arg " + arg];
    });
    await uiCheck("pointMode", async () => {
      await put("C10", "5");
      await put("C11", "7");
      await at("D10");
      await page.keyboard.type("=");
      await page.keyboard.press("ArrowLeft");
      const one = await M("G.editBuf");
      await page.keyboard.press("Shift+ArrowDown");
      const two = await M("G.editBuf");
      await page.keyboard.press("Enter");
      const f = await M("M.getFormula(9, 3)");
      return [one === "=C10" && two === "=C10:C11" && f.replace(/^=/, "") === "C10:C11", one + " → " + two + " → " + f];
    });
    await uiCheck("multiArea", async () => {
      const a = await cellCentre(14, 1), b = await cellCentre(16, 3);
      await page.mouse.click(a.x, a.y);
      await page.keyboard.down("Control");
      await page.mouse.click(b.x, b.y);
      await page.keyboard.up("Control");
      await page.waitForTimeout(60);
      const n = await M("G.sel.rangeCount()");
      return [n === 2, n + " ranges"];
    });
    await uiCheck("fillHandle", async () => {
      await put("E20", "1");
      await put("E21", "2");
      await sel("E20:E21");
      const r = await M("(() => { const q = G.grid.cellRect(M, 20, 4); return [q.x, q.y, q.w, q.h]; })()");
      const h = await toPage(r[0] + r[2] - 1, r[1] + r[3] - 1);
      const end = await cellCentre(23, 4);
      await drag([h, { x: h.x, y: (h.y + end.y) / 2 }, { x: h.x, y: end.y }]);
      const v = [await cellAt("E22"), await cellAt("E23"), await cellAt("E24")];
      return [v[0] === "3" && v[1] === "4", v.join(",")];
    });
    await uiCheck("columnResize", async () => {
      const w0 = await M("M.colWidth(5)");
      const hx = await M("(() => { const q = G.grid.cellRect(M, 0, 5); return [q.x + q.w, G.grid.headerTop() + 6]; })()");
      const a = await toPage(hx[0], hx[1]);
      await drag([a, { x: a.x + 20, y: a.y }, { x: a.x + 40, y: a.y }]);
      const w1 = await M("M.colWidth(5)");
      await put("F2", "a much longer piece of text here");
      const b = await toPage(...(await M("(() => { const q = G.grid.cellRect(M, 0, 5); return [q.x + q.w, G.grid.headerTop() + 6]; })()")));
      await page.mouse.dblclick(b.x, b.y);
      await page.waitForTimeout(80);
      const w2 = await M("M.colWidth(5)");
      return [w1 > w0 + 20 && w2 > w1, w0 + " → " + w1 + " → " + w2];
    });
    await uiCheck("sheetTabs", async () => {
      await run("sheet.tabColor", "FF0000");
      const at0 = await M("G.book.activeIndex");
      const shown = await page.evaluate((i) => window.sheet.app.tabColourShown(i), at0);
      await clickTid("sx-tabs-tab-" + at0, { button: "right" });
      const items = (await axNow()).filter((n) => n.role === "menuitem").map((n) => n.name);
      const hide = (await axNow()).find((n) => n.role === "menuitem" && n.name === "Hide");
      if (hide) await clickNode(hide);
      const tabs1 = (await axNow()).filter((n) => n.role === "tab").map((n) => n.name);
      await run("sheet.unhide", "UI");
      const tabs2 = (await axNow()).filter((n) => n.role === "tab").map((n) => n.name);
      return [shown === "#FF0000" && items.includes("Tab Color") && !tabs1.includes("UI") && tabs2.includes("UI"), "colour " + shown + "; menu " + items.filter((x) => /Tab|Hide|Unhide/.test(x)).join(",") + "; hidden " + !tabs1.includes("UI") + "; back " + tabs2.includes("UI")];
    });
    await uiCheck("zoomSlider", async () => {
      const tr = await tidRect("sx-zoomslider-track");
      await page.mouse.click(tr.r.x + tr.r.w * 0.5, tr.r.y + tr.r.h / 2);
      await page.waitForTimeout(80);
      const z = await M("Math.round(M.zoom * 100)");
      const slider = (await axNow()).find((n) => n.role === "slider");
      await run("view.zoom", "100");
      return [!!slider && z > 150 && z < 260, "slider " + !!slider + " zoom " + z];
    });
    await uiCheck("formatCellsPane", async () => {
      await run("home.formatCells");
      const rows = await paneRows();
      const want = ["Number", "Alignment", "Font", "Border", "Fill", "Locked"];
      await closePane();
      return [want.every((w) => rows.some((r) => r.startsWith(w + ":"))), rows.map((r) => r.split(":")[0]).join(",")];
    });
    await uiCheck("taskPanes", async () => {
      const seen = [];
      for (const [cmd, kind] of [["review.showComments", "comments"], ["formulas.watchWindow", "watch"], ["arrange.selectionPane", "selection"]]) {
        await run(cmd);
        seen.push((await M("G.paneKind")) + ":" + (await page.evaluate(() => window.sheet.app.paneBuilt)));
        await closePane();
      }
      return [seen.join(",") === "comments:comments,watch:watch,selection:selection", seen.join(",")];
    });
  };
  if (process.env.PROBE_UI) {
    await uiChecks2();
    throw new Error("PROBE_UI: only the UI checks ran");
  }
  if (process.env.PROBE_NEW2) {
    await newChecks2();
    throw new Error("PROBE_NEW2: only the second batch ran");
  }
  if (process.env.PROBE_NEW) {
    await newChecks();
    throw new Error("PROBE_NEW: only the new checks ran");
  }

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
  await check("formulas.tracePrecedents", async () => { const n = await M("G.sel.selectedCount()"), arrows = await M("G.view.traceArrows.length"); return [n === 4 && (await S()).cell === "J11" && arrows >= 4, n + " cells, " + arrows / 4 + " arrows"]; });
  await run("formulas.removeArrows");
  await check("formulas.removeArrows", async () => [await M("G.view.traceArrows.length === 0")]);
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

  // Charts: Chart Design and Format, checked on the chart and on the spec it draws.
  await run("sheet.add");
  for (const [ref, v] of [["A1", "Month"], ["B1", "Sales"], ["C1", "Cost"], ["A2", "Jan"], ["B2", "10"], ["C2", "4"], ["A3", "Feb"], ["B3", "14"], ["C3", "6"], ["A4", "Mar"], ["B4", "9"], ["C4", "5"]]) await put(ref, v);
  await at("A1");
  const nCharts = await M("G.view.chartLayer.panels.length");
  await run("insert.recommendedCharts");
  const chartState = async () => { await page.waitForTimeout(120); return M(`(() => { const L = G.view.chartLayer, p = L.panels[G.activeChartPanel()], r = L.view.cached(p.chart.id); return { kind: p.chart.kind, title: p.chart.title, legend: p.chart.showLegend, labels: p.chart.dataLabels, grid: p.chart.gridlines, style: p.chart.style, t: p.chart.transposed, r0: p.chart.r0, r1: p.chart.r1, c1: p.chart.c1, sheet: p.chart.sheet, x: p.chart.x, spec: r.key || "", ok: r.ok, cats: L.view.data.cats.join(",") }; })()`); };
  await check("insert.recommendedCharts", async () => { const n = await M("G.view.chartLayer.panels.length"), c = await chartState(); return [n === nCharts + 1 && c.ok, n + " charts, kind " + c.kind]; });
  await run("chart.addElement", "title:Sales by month");
  await run("chart.addElement", "dataLabels");
  await check("chart.addElement", async () => { const c = await chartState(); return [c.ok && c.title === "Sales by month" && /"layer":\[/.test(c.spec) && /"type":"text"/.test(c.spec), c.title + " " + c.ok]; });
  await run("chart.quickLayout", "3");
  await check("chart.quickLayout", async () => { const c = await chartState(); return [c.ok && c.title === "" && c.legend && !c.labels, JSON.stringify([c.title, c.legend, c.labels])]; });
  await run("chart.changeColors", "4");
  await check("chart.changeColors", async () => { const c = await chartState(); return [c.style === 4 && /category10/.test(c.spec), c.style]; });
  await run("chart.set", "Line");
  await check("chart.set", async () => { const c = await chartState(); return [c.ok && c.kind === 6 && /"mark":"line"/.test(c.spec), c.kind]; });
  await run("chart.switchRowColumn");
  await check("chart.switchRowColumn", async () => { const c = await chartState(); return [c.ok && c.t && c.cats === "Sales,Cost", c.cats]; });
  await run("chart.switchRowColumn");
  await run("chart.selectData", "A1:B3");
  await check("chart.selectData", async () => { const c = await chartState(); return [c.ok && c.r1 === 2 && c.c1 === 1 && c.cats === "Jan,Feb", c.cats]; });
  await run("chart.shapeFill", "#C00000");
  await check("chart.shapeFill", async () => { const c = await chartState(); return [c.ok && /#C00000/.test(c.spec), c.ok]; });
  await run("chart.shapeOutline", "#000000");
  await check("chart.shapeOutline", async () => { const c = await chartState(); return [c.ok && /"stroke":"#000000"/.test(c.spec), c.ok]; });
  await run("chart.formatSelection");
  await check("chart.formatSelection", async () => [await M("G.paneKind === 'formatChart'")]);
  await run("chart.move", "300,200");
  await check("chart.move", async () => { const c = await chartState(); return [c.x === 300, c.x]; });

  // Names, auditing, evaluation, the watch window and manual calculation.
  await run("sheet.add");
  for (const [ref, v] of [["A1", "Price"], ["B1", "Qty"], ["A2", "10"], ["B2", "3"], ["A3", "20"], ["B3", "1"], ["C2", "=A2*B2"], ["C3", "=A3*B3"], ["D2", "=1/0"]]) await put(ref, v);
  await sel("A2:A3");
  await run("formulas.defineName", "Prices");
  await put("E1", "=SUM(Prices)");
  await check("formulas.defineName", async () => { const v = await cellAt("E1"), r = await M("G.book.nameRef('Prices')"); return [v === "30" && /\$A\$2:\$A\$3$/.test(r), v + " " + r]; });
  await at("E2");
  await run("formulas.useInFormula", "Prices");
  await check("formulas.useInFormula", async () => { const s2 = await S(); return [s2.editing && s2.editText === "=Prices", s2.editText]; });
  await page.keyboard.press("Escape");
  await sel("A1:B3");
  await run("formulas.createFromSelection");
  await put("E3", "=SUM(Qty)");
  await check("formulas.createFromSelection", async () => { const v = await cellAt("E3"); return [v === "4", v]; });
  await run("formulas.nameManager");
  const nmPane = await M("G.paneKind");
  await run("formulas.nameManager", "delete:Qty");
  await check("formulas.nameManager", async () => { const v = await cellAt("E3"); return [nmPane === "names" && /NAME/.test(v), nmPane + " " + v]; });
  await at("A2");
  await run("formulas.traceDependents");
  await check("formulas.traceDependents", async () => { const a = await M("Array.from(G.view.traceArrows).join(',')"); return [/^1,0,1,2/.test(a), a]; });
  await run("formulas.removeArrows");
  await at("A1");
  await run("formulas.errorChecking");
  await check("formulas.errorChecking", async () => { const s2 = await S(); return [s2.cell === "D2" && /DIV/.test(s2.status), s2.cell + " " + s2.status]; });
  await at("C2");
  await run("formulas.evaluateFormula");
  await check("formulas.evaluateFormula", async () => { const st = await M("Array.from(G.evalSteps).join(' | ')"); return [st === "=A2*B2 | =10*3 | 30", st]; });
  await run("formulas.watchWindow");
  await check("formulas.watchWindow", async () => { const w = await M("Array.from(G.watches).join(',') + ' ' + G.paneKind"); return [/!C2 watch$/.test(w), w]; });
  await run("formulas.calculationOptions", "manual");
  await put("A2", "100");
  const manualVal = await cellAt("C2");
  await run("formulas.calculationOptions", "automatic");
  await check("formulas.calculationOptions", async () => { const v = await cellAt("C2"); return [manualVal === "30" && v === "300", manualVal + "→" + v]; });

  await newChecks();
  await newChecks2();
  await uiChecks2();

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
if (!process.env.PROBE_NEW && !process.env.PROBE_NEW2 && !process.env.PROBE_UI) fs.writeFileSync(path.join(REPO, "parity", "probe.json"), JSON.stringify(out, null, 1) + "\n");
const passed = Object.values(results).filter((r) => r.pass).length;
console.log(`\n${passed}/${Object.keys(results).length} probe checks pass (a failure is a measured gap, not a broken run)`);
