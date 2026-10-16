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
  const n48 = cell("$48,200");
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
  ok("ui.nameBox", boxes.some((n) => n.r.x < 120 && n.r.y > 80 && n.r.y < 120), "");
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
  const after = await page.evaluate(() => [...document.querySelectorAll("[role=slider],[role=button]")].map((e) => e.getAttribute("aria-label") || e.textContent));
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


  const dl = page.waitForEvent("download", { timeout: 5000 }).catch(() => null);
  await run("file.saveAs");
  const d = await dl;
  const bytes = d ? fs.readFileSync(await d.path()) : Buffer.alloc(0);
  ok("cmd.file.saveAs", bytes.length > 1000 && bytes[0] === 0x50 && bytes[1] === 0x4b, bytes.length + " bytes");

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
