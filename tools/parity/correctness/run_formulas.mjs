#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Formula correctness: formulas.xlsx (made by make_book.py from formulas.tsv)
 * calculated by EVGSheets, GridCraft (gridcraft-cli) and LibreOffice, compared
 * with the Excel value written in formulas.tsv.
 *
 *   node run_formulas.mjs [--gridcraft PATH] [--no-lo] [--json out.json]
 */
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { spawnSync } from "node:child_process";
import { HERE, openEditor } from "./browser.mjs";

const argv = process.argv.slice(2);
const flag = (n, d) => (argv.includes(n) ? argv[argv.indexOf(n) + 1] : d);
const GC = flag("--gridcraft", process.env.GRIDCRAFT_CLI || "");
const BOOK = path.join(HERE, "formulas.xlsx");
spawnSync("python3", [path.join(HERE, "make_book.py"), BOOK], { stdio: "ignore" });

const cases = fs.readFileSync(path.join(HERE, "formulas.tsv"), "utf8").split("\n")
  .filter((l) => l && !l.startsWith("#"))
  .map((l) => { const [f, exp = ""] = l.split("\t"); return { f, exp }; });

// One CSV parser for gridcraft and LibreOffice output.
function csv(text) {
  const rows = [];
  let row = [], cell = "", q = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (q) {
      if (ch === '"' && text[i + 1] === '"') { cell += '"'; i++; } else if (ch === '"') q = false; else cell += ch;
    } else if (ch === '"') q = true;
    else if (ch === ",") { row.push(cell); cell = ""; }
    else if (ch === "\n") { row.push(cell); rows.push(row); row = []; cell = ""; }
    else if (ch !== "\r") cell += ch;
  }
  if (cell || row.length) { row.push(cell); rows.push(row); }
  return rows;
}

// --- EVGSheets
const ed = await openEditor();
await ed.openFile(BOOK);
const evg = await ed.page.evaluate((n) => {
  const M = window.sheet.app.grid.app.model;
  const out = [];
  for (let r = 0; r < n; r++) out.push(M.getCell(r, 1));
  return out;
}, cases.length);
// The same formulas typed into column E of sheet F, the way a person enters them.
// Keys go straight to the app (no redraw per formula, which is slow).
const typed = await ed.page.evaluate((fs) => {
  const A = window.sheet.app, G = A.grid.app;
  return fs.map((f, r) => {
    A.grid.run("nav.goto", "E" + (r + 1));
    for (const ch of f) A.text(ch, false, false);
    A.key("Enter", false, false);
    return G.model.getCell(r, 4);
  });
}, cases.map((c) => c.f));
await ed.close();

// --- GridCraft
let gc = null;
if (GC && fs.existsSync(GC)) {
  const r = spawnSync(GC, ["cat", BOOK, "--sheet", "F", "--range", `A1:B${cases.length}`, "--csv"], { encoding: "utf8" });
  gc = csv(r.stdout).map((row) => row[1] ?? "");
}

// --- LibreOffice (hard recalculation on load)
let lo = null;
if (!argv.includes("--no-lo") && spawnSync("which", ["soffice"]).status === 0) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "lo-"));
  spawnSync("soffice", ["--headless", "--convert-to", "csv:Text - txt - csv (StarCalc):44,34,76,1,,0,false,true,false,false,false,-1", "--outdir", dir, BOOK], { stdio: "ignore" });
  const f = fs.readdirSync(dir).find((n) => /-F\.csv$|^formulas\.csv$/.test(n));
  if (f) lo = csv(fs.readFileSync(path.join(dir, f), "utf8")).map((row) => row[1] ?? "");
}

// TRUE/FALSE and text as they are; numbers compared to a relative 1e-6
// (GridCraft and LibreOffice print the General format, about 10 digits).
function norm(v) {
  if (v === undefined || v === null) return "";
  let s = String(v);
  if (s === "true") s = "TRUE";
  if (s === "false") s = "FALSE";
  return s;
}
const isNum = (s) => /^-?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$/i.test(s.trim());
function same(a, b) {
  if (a === null || b === null) return null;
  if (isNum(a) && isNum(b)) {
    const x = Number(a), y = Number(b);
    return x === y || Math.abs(x - y) <= 1e-6 * Math.max(Math.abs(x), Math.abs(y));
  }
  return a === b;
}
const want = (e) => (e === '""' ? "" : e);
const rows = cases.map((c, i) => {
  const e = norm(want(c.exp));
  const known = c.exp !== "";
  const v = { evg: norm(evg[i]), typed: norm(typed[i]), gc: gc ? norm(gc[i]) : null, lo: lo ? norm(lo[i]) : null };
  const ref = known ? e : v.gc ?? v.lo ?? null;
  return { formula: c.f, expected: known ? e : null, ...v, evgOk: same(v.evg, ref), typedOk: same(v.typed, ref), gcOk: known ? same(v.gc, e) : null, loOk: known ? same(v.lo, e) : null };
});
const bad = rows.filter((r) => r.evgOk === false || r.typedOk === false);
for (const r of bad) console.log(`${r.formula}\n     file ${JSON.stringify(r.evg)}  typed ${JSON.stringify(r.typed)}  want ${JSON.stringify(r.expected ?? r.gc)}  gridcraft ${JSON.stringify(r.gc)}  lo ${JSON.stringify(r.lo)}`);
const count = (k) => rows.filter((r) => r[k] === true).length + "/" + rows.filter((r) => r[k] !== null).length;
console.log(`\nformulas ${rows.length}: EVGSheets opened ${count("evgOk")}, typed ${count("typedOk")}, GridCraft ${count("gcOk")}, LibreOffice ${count("loOk")}`);
const gcBad = rows.filter((r) => r.gcOk === false);
if (gcBad.length) console.log("GridCraft differs from the expected value: " + gcBad.map((r) => `${r.formula} → ${r.gc}`).join("; "));
if (flag("--json")) fs.writeFileSync(flag("--json"), JSON.stringify(rows, null, 1));
