#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Number-format correctness: every code in numfmt_codes.txt applied to a set
 * of values (numfmt.xlsx, written by openpyxl), the displayed text read from
 * EVGSheets and from GridCraft (gridcraft-cli cat). GridCraft's formatter is
 * checked against Apache POI and Excel; a difference is listed, LibreOffice
 * (when installed) breaks ties.
 *
 *   node run_numfmt.mjs --gridcraft PATH [--json out.json]
 */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { HERE, openEditor } from "./browser.mjs";

const argv = process.argv.slice(2);
const flag = (n, d) => (argv.includes(n) ? argv[argv.indexOf(n) + 1] : d);
const GC = flag("--gridcraft", process.env.GRIDCRAFT_CLI || "");
const codes = fs.readFileSync(path.join(HERE, "numfmt_codes.txt"), "utf8").split("\n").filter(Boolean);
const values = [0, 1, -1, 0.5, 1234.5678, -1234.5678, 0.001234, 12345678, 45306.75, 0.3333333, 100, "text"];
const BOOK = path.join(os.tmpdir(), "evgsheets-numfmt.xlsx");
const py = `
import json, sys
from openpyxl import Workbook
codes, values = json.loads(sys.argv[1]), json.loads(sys.argv[2])
wb = Workbook(); ws = wb.active; ws.title = "N"
for r, code in enumerate(codes, start=1):
    for c, v in enumerate(values, start=1):
        cell = ws.cell(r, c, v); cell.number_format = code
for c in range(1, len(values) + 1):
    ws.column_dimensions[chr(64 + c)].width = 40
wb.save(sys.argv[3])`;
const mk = spawnSync("python3", ["-c", py, JSON.stringify(codes), JSON.stringify(values), BOOK], { encoding: "utf8" });
if (mk.status !== 0) throw new Error(mk.stderr);

function csv(text) {
  const rows = []; let row = [], cell = "", q = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (q) { if (ch === '"' && text[i + 1] === '"') { cell += '"'; i++; } else if (ch === '"') q = false; else cell += ch; }
    else if (ch === '"') q = true; else if (ch === ",") { row.push(cell); cell = ""; }
    else if (ch === "\n") { row.push(cell); rows.push(row); row = []; cell = ""; } else if (ch !== "\r") cell += ch;
  }
  if (cell || row.length) { row.push(cell); rows.push(row); }
  return rows;
}

const ed = await openEditor({ port: 8943 });
await ed.openFile(BOOK);
const evg = await ed.page.evaluate(([nr, nc]) => {
  const M = window.sheet.app.grid.app.model;
  const out = [];
  for (let r = 0; r < nr; r++) { const row = []; for (let c = 0; c < nc; c++) row.push(M.getDisplay(r, c)); out.push(row); }
  return out;
}, [codes.length, values.length]);
await ed.close();

const last = String.fromCharCode(64 + values.length);
const gc = csv(spawnSync(GC, ["cat", BOOK, "--range", `A1:${last}${codes.length}`, "--csv"], { encoding: "utf8" }).stdout);
let lo = null;
if (spawnSync("which", ["soffice"]).status === 0) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "lo-"));
  spawnSync("soffice", ["--headless", "--convert-to", "csv:Text - txt - csv (StarCalc):44,34,76,1,,1033,false,true,true", "--outdir", dir, BOOK], { stdio: "ignore" });
  const f = fs.readdirSync(dir).find((n) => n.endsWith(".csv"));
  if (f) lo = csv(fs.readFileSync(path.join(dir, f), "utf8"));
}
// Accounting/fill formats pad to the column; compare with runs of spaces squeezed.
const sq = (s) => String(s ?? "").replace(/\s+/g, " ").trim();
const rows = [];
let same = 0, total = 0;
const perCode = [];
codes.forEach((code, r) => {
  const bad = [];
  values.forEach((v, c) => {
    total++;
    const e = evg[r][c], g = (gc[r] || [])[c] ?? "", l = lo ? (lo[r] || [])[c] ?? "" : null;
    const ok = sq(e) === sq(g);
    if (ok) same++; else bad.push({ v, evg: e, gridcraft: g, lo: l });
    rows.push({ code, v, evg: e, gridcraft: g, lo: l, ok });
  });
  if (bad.length) perCode.push({ code, bad });
});
for (const p of perCode) {
  console.log(`${p.code}`);
  for (const b of p.bad.slice(0, 4)) console.log(`   ${JSON.stringify(b.v)}: evg ${JSON.stringify(b.evg)}  gridcraft ${JSON.stringify(b.gridcraft)}  lo ${JSON.stringify(b.lo)}`);
  if (p.bad.length > 4) console.log(`   … ${p.bad.length - 4} more`);
}
console.log(`\nnumber formats: ${codes.length - perCode.length}/${codes.length} codes fully agree; ${same}/${total} cells`);
if (flag("--json")) fs.writeFileSync(flag("--json"), JSON.stringify(rows, null, 1));
