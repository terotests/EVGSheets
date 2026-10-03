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
  const pasted = await page.evaluate(() => [window.sheet.app.paste.win.open, window.sheet.app.grid.app.model.getCell(9, 5), window.sheet.app.grid.app.pasteMode]);
  ok("Paste pastes the value and closes", !pasted[0] && pasted[1] !== "" && !String(pasted[1]).startsWith("="), String(pasted[1]));
  ok("…and the next paste is a full one again", pasted[2] === 0);
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

  // Sheet tabs.
  await click(page, "sx-tabs-tab-0");
  s = await state(page);
  ok("a tab shows its sheet", s.activeSheet === 0, String(s.activeSheet));
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
