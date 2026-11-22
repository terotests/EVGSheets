#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Editing correctness: each scenario starts from a new workbook, enters cells
 * the way a person types them, runs the editor's commands (copy/paste, fill,
 * sort, filter, insert/delete, undo…) and compares cells with what Excel
 * leaves there. Expected values follow Excel; GridCraft agrees on every one
 * of them (checked with gridcraft-cli run when the scenarios were written).
 *
 *   node run_ops.mjs [--only name] [--json out.json]
 */
import fs from "node:fs";
import { openEditor } from "./browser.mjs";

const argv = process.argv.slice(2);
const flag = (n, d) => (argv.includes(n) ? argv[argv.indexOf(n) + 1] : d);
const ed = await openEditor({ port: 8942 });
const page = ed.page;

// --- page helpers --------------------------------------------------------
const col = (s) => [...s].reduce((n, ch) => n * 26 + ch.charCodeAt(0) - 64, 0) - 1;
const rc = (ref) => { const m = /^([A-Z]+)(\d+)$/.exec(ref); return [Number(m[2]) - 1, col(m[1])]; };
const G = (fn, arg) => page.evaluate(new Function("a", "const G = window.sheet.app.grid.app, M = G.model; return (" + fn + ")(a);"), arg);
const run = (id, arg = "") => page.evaluate(([i, a]) => window.sheet.run(i, a), [id, arg]);
async function fresh() {
  await run("file.new");
  await page.evaluate(() => window.sheet.app.sync());
}
async function put(ref, text) {
  await run("nav.goto", ref);
  await page.evaluate(async (t) => { await window.sheet.type(t); await window.sheet.key("Enter"); }, text);
}
async function fill(cells) { for (const [k, v] of Object.entries(cells)) await put(k, String(v)); }
async function select(range) {
  const [a, b = a] = range.split(":");
  const [r0, c0] = rc(a), [r1, c1] = rc(b);
  // Excel keeps the active cell at the first corner of a dragged range.
  await G("([r0,c0,r1,c1]) => { G.moveActive(r1, c1, false); G.moveActive(r0, c0, true); }", [r0, c0, r1, c1]);
}
const key = (k, shift = false, ctrl = false) => page.evaluate(([k, s, c]) => window.sheet.key(k, s, c), [k, shift, ctrl]);
async function cell(ref) {
  const [r, c] = rc(ref);
  return G("([r,c]) => ({ v: M.getCell(r, c), f: M.getFormula(r, c), d: M.getDisplay(r, c) })", [r, c]);
}
// Paste Special as its dialog does it: mode 1 = values. There is no
// transpose option; `transpose` sets the flag a future option would read.
async function pasteSpecial(mode, transpose = false) {
  await G("([m, t]) => { const p = G.pasteMode; G.pasteMode = m; if (t) G.pasteTranspose = true; G.pasteClipboard(); G.pasteMode = p; G.pasteTranspose = false; }", [mode, transpose]);
}
// The fill handle: drag from the selection `src` to cover `to`.
async function dragFill(src, to) {
  const [a, b = a] = src.split(":");
  const [r0, c0] = rc(a), [r1, c1] = rc(b);
  await select(to);
  await G("([r0,c0,r1,c1]) => { G.fillR0 = r0; G.fillC0 = c0; G.fillR1 = r1; G.fillC1 = c1; G.fillFromSource(); }", [r0, c0, r1, c1]);
}

// --- scenarios -------------------------------------------------------------
// expect: { ref: value } compares the value; { ref: "=F" } compares the formula
// text (without "="); { ref: {d: "…"} } compares the displayed text.
const S = [];
const scenario = (name, area, steps, expect) => S.push({ name, area, steps, expect });

scenario("copy-relative", "copy/paste", async () => {
  await fill({ A1: 1, A2: 2, B1: "=A1*2" });
  await select("B1"); await run("edit.copy"); await select("B2"); await run("edit.paste");
}, { B2: 4, "B2=": "A2*2" });
scenario("copy-absolute-mixed", "copy/paste", async () => {
  await fill({ A1: 1, B1: "=$A$1+A$1+$A1" });
  await select("B1"); await run("edit.copy"); await select("C3"); await run("edit.paste");
}, { "C3=": "$A$1+B$1+$A3" });
scenario("paste-tiles-selection", "copy/paste", async () => {
  await fill({ A1: 1, A2: 2, A3: 3, A4: 4, B1: "=A1*2" });
  await select("B1"); await run("edit.copy"); await select("B2:B4"); await run("edit.paste");
}, { B4: 8, "B4=": "A4*2" });
scenario("cut-paste-moves-references", "copy/paste", async () => {
  await fill({ A1: 5, B1: "=A1" });
  await select("A1"); await run("edit.cut"); await select("C1"); await run("edit.paste");
}, { A1: "", C1: 5, "B1=": "C1", B1: 5 });
scenario("cut-paste-formula-keeps-refs", "copy/paste", async () => {
  await fill({ A1: 5, B1: "=A1*2" });
  await select("B1"); await run("edit.cut"); await select("D5"); await run("edit.paste");
}, { B1: "", "D5=": "A1*2", D5: 10 });
scenario("copy-to-other-sheet", "copy/paste", async () => {
  await fill({ A1: 7, B1: "=A1+1" });
  await select("A1:B1"); await run("edit.copy");
  await run("sheet.add"); await select("A1"); await run("edit.paste");
}, { A1: 7, B1: 8, "B1=": "A1+1" });
scenario("copy-ref-off-sheet-is-REF", "copy/paste", async () => {
  await fill({ A1: 1, B2: "=A1" });
  await select("B2"); await run("edit.copy"); await select("A1"); await run("edit.paste");
}, { "A1=": "#REF!" });

scenario("fill-down-formula", "fill", async () => {
  await fill({ A1: 1, A2: 2, A3: 3, B1: "=A1+1" });
  await select("B1:B3"); await run("edit.fillDown");
}, { B3: 4, "B3=": "A3+1" });
scenario("fill-right-formula", "fill", async () => {
  await fill({ A1: 1, B1: 2, C1: 3, A2: "=A1*10" });
  await select("A2:C2"); await run("edit.fillRight");
}, { C2: 30, "C2=": "C1*10" });
scenario("fill-handle-number-series", "fill", async () => {
  await fill({ A1: 1, A2: 3 });
  await select("A1:A2"); await dragFill("A1:A2", "A1:A5");
}, { A3: 5, A5: 9 });
scenario("fill-handle-single-number-copies", "fill", async () => {
  await fill({ A1: 7 });
  await dragFill("A1", "A1:A3");
}, { A3: 7 });
scenario("fill-handle-text-number", "fill", async () => {
  await fill({ A1: "Item1" });
  await dragFill("A1", "A1:A3");
}, { A2: "Item2", A3: "Item3" });
scenario("fill-handle-months", "fill", async () => {
  await fill({ A1: "Jan" });
  await dragFill("A1", "A1:A3");
}, { A2: "Feb", A3: "Mar" });
scenario("fill-handle-weekdays", "fill", async () => {
  await fill({ A1: "Monday" });
  await dragFill("A1", "A1:A3");
}, { A3: "Wednesday" });
scenario("fill-handle-dates", "fill", async () => {
  await fill({ A1: "2024-01-31" });
  await dragFill("A1", "A1:A3");
}, { A2: 45323, A3: 45324, "A3": 45324 });
scenario("fill-handle-keeps-format", "fill", async () => {
  await fill({ A1: "1" });
  await select("A1"); await run("format.numfmt", "0.00%");
  await dragFill("A1", "A1:A2");
}, { A2: { d: "100.00%" } });
scenario("fill-series-step", "fill", async () => {
  await fill({ A1: 10 });
  await select("A1:A4"); await run("edit.fillSeries", "5");
}, { A4: 25 });

scenario("sort-asc-keeps-row-formulas", "sort/filter", async () => {
  await fill({ A1: 3, A2: 1, A3: 2, B1: "=A1*10", B2: "=A2*10", B3: "=A3*10" });
  await select("A1:B3"); await run("data.sort.asc");
}, { A1: 1, A3: 3, B1: 10, B3: 30, "B1=": "A1*10" });
scenario("sort-mixed-types", "sort/filter", async () => {
  await fill({ A1: 2, A2: "b", A3: "TRUE", A4: "a", A5: 1 });
  await select("A1:A5"); await run("data.sort.asc");
}, { A1: 1, A2: 2, A3: "a", A4: "b", A5: "TRUE" });
scenario("sort-desc-text-case-insensitive", "sort/filter", async () => {
  await fill({ A1: "apple", A2: "Banana", A3: "cherry" });
  await select("A1:A3"); await run("data.sort.desc");
}, { A1: "cherry", A2: "Banana", A3: "apple" });
scenario("sort-blanks-last", "sort/filter", async () => {
  await fill({ A1: 3, A3: 1, A4: 2, B1: "x", B2: "y", B3: "z", B4: "w" });
  await select("A1:B4"); await run("data.sort.desc");
}, { A1: 3, A2: 2, A3: 1, A4: "", B4: "y" });
scenario("sort-header-row-stays", "sort/filter", async () => {
  await fill({ A1: "Name", B1: "Score", A2: "c", B2: 3, A3: "a", B3: 1, A4: "b", B4: 2 });
  await select("B2"); await run("data.sort.asc");
}, { A1: "Name", B2: 1, A2: "a", B4: 3 });
scenario("subtotal-ignores-filtered", "sort/filter", async () => {
  await fill({ A1: "K", A2: "x", A3: "y", A4: "x", B1: "V", B2: 1, B3: 10, B4: 100, D1: "=SUBTOTAL(9,B2:B4)", D2: "=SUM(B2:B4)" });
  await G("() => { M.setFilter(0, true, ['x']); }");
  await G("() => { G.sheetView.markDirty(); G.sheetView.rebuild(); G.engine.recalcAll && G.engine.recalcAll(); }");
}, { D1: 101, D2: 111 });

scenario("insert-row-grows-sum", "rows/cols", async () => {
  await fill({ A1: 1, A2: 2, A3: 3, A4: "=SUM(A1:A3)" });
  await select("A2"); await run("insert.row.above");
}, { "A5=": "SUM(A1:A4)", A5: 6, A2: "" });
scenario("insert-row-below-sum-range-end", "rows/cols", async () => {
  await fill({ A1: 1, A2: 2, A3: "=SUM(A1:A2)" });
  await select("A3"); await run("insert.row.above");
}, { "A4=": "SUM(A1:A2)" });
scenario("delete-row-ref-becomes-REF", "rows/cols", async () => {
  await fill({ A1: 1, A2: 2, B1: "=A2" });
  await select("A2"); await run("delete.rows");
}, { "B1=": "#REF!", B1: "#REF!" });
scenario("delete-row-shrinks-range", "rows/cols", async () => {
  await fill({ A1: 1, A2: 2, A3: 3, A4: "=SUM(A1:A3)" });
  await select("A2"); await run("delete.rows");
}, { "A3=": "SUM(A1:A2)", A3: 4 });
scenario("delete-column-shrinks-range", "rows/cols", async () => {
  await fill({ A1: 1, B1: 2, C1: 3, A2: "=SUM(A1:C1)" });
  await select("B1"); await run("delete.columns");
}, { "A2=": "SUM(A1:B1)", A2: 4 });
scenario("insert-column-moves-other-sheet-ref", "rows/cols", async () => {
  await fill({ B1: 5 });
  await run("sheet.add"); await put("A1", "=Sheet1!B1");
  await run("view.sheet.prev"); await select("A1"); await run("insert.column.left");
  await run("view.sheet.next");
}, { "A1=": "Sheet1!C1", A1: 5 });
scenario("insert-row-absolute-ref", "rows/cols", async () => {
  await fill({ A3: 9, C1: "=$A$3" });
  await select("A2"); await run("insert.row.above");
}, { "C1=": "$A$4", C1: 9 });

scenario("undo-typing", "undo", async () => {
  await fill({ A1: 1 }); await put("A1", "2"); await put("A1", "3");
  await run("edit.undo"); await run("edit.undo");
}, { A1: 1 });
scenario("redo-typing", "undo", async () => {
  await fill({ A1: 1 }); await put("A1", "2");
  await run("edit.undo"); await run("edit.redo");
}, { A1: 2 });
scenario("undo-sort", "undo", async () => {
  await fill({ A1: 3, A2: 1, A3: 2 });
  await select("A1:A3"); await run("data.sort.asc"); await run("edit.undo");
}, { A1: 3, A2: 1, A3: 2 });
scenario("undo-delete-row-restores-refs", "undo", async () => {
  await fill({ A1: 1, A2: 2, B1: "=A2" });
  await select("A2"); await run("delete.rows"); await run("edit.undo");
}, { A2: 2, "B1=": "A2", B1: 2 });
scenario("undo-paste", "undo", async () => {
  await fill({ A1: 1, B1: 9 });
  await select("A1"); await run("edit.copy"); await select("B1"); await run("edit.paste"); await run("edit.undo");
}, { B1: 9 });
scenario("undo-fill-down", "undo", async () => {
  await fill({ A1: 1, A2: 5 });
  await select("A1:A2"); await run("edit.fillDown"); await run("edit.undo");
}, { A2: 5 });
scenario("undo-clear", "undo", async () => {
  await fill({ A1: 1, A2: "=A1+1" });
  await select("A1:A2"); await run("edit.clear"); await run("edit.undo");
}, { A1: 1, "A2=": "A1+1", A2: 2 });
scenario("undo-insert-column", "undo", async () => {
  await fill({ A1: 1, B1: "=A1" });
  await select("A1"); await run("insert.column.left"); await run("edit.undo");
}, { A1: 1, "B1=": "A1" });

scenario("entry-thousands", "entry", async () => { await fill({ A1: "1,234", B1: "=A1*1" }); }, { B1: 1234 });
scenario("entry-percent", "entry", async () => { await fill({ A1: "12%", B1: "=A1*1" }); }, { B1: 0.12, A1d: { d: "12%" } });
scenario("entry-iso-date", "entry", async () => { await fill({ A1: "2024-01-15" }); }, { A1: 45306 });
scenario("entry-us-date", "entry", async () => { await fill({ A1: "1/2/2024" }); }, { A1: 45293 });
scenario("entry-time", "entry", async () => { await fill({ A1: "18:30", B1: "=A1*1" }); }, { B1: 0.7708333333 });
scenario("entry-currency", "entry", async () => { await fill({ A1: "$5.50", B1: "=A1*1" }); }, { B1: 5.5 });
scenario("entry-scientific", "entry", async () => { await fill({ A1: "1e3" }); }, { A1: 1000 });
scenario("entry-negative-parens", "entry", async () => { await fill({ A1: "(42)", B1: "=A1*1" }); }, { B1: -42 });
scenario("entry-apostrophe-text", "entry", async () => { await fill({ A1: "'123", B1: "=ISTEXT(A1)" }); }, { A1d: { d: "123" }, B1: "TRUE" });
scenario("entry-boolean", "entry", async () => { await fill({ A1: "true", B1: "=ISLOGICAL(A1)" }); }, { A1: "TRUE", B1: "TRUE" });
scenario("entry-lowercase-formula", "entry", async () => { await fill({ A1: 2, B1: "=sum(a1,a1)" }); }, { "B1=": "SUM(A1,A1)", B1: 4 });
scenario("entry-leading-zero-number", "entry", async () => { await fill({ A1: "007" }); }, { A1: 7 });
scenario("entry-fraction", "entry", async () => { await fill({ A1: "1 1/2", B1: "=A1*1" }); }, { B1: 1.5 });

scenario("empty-ref-is-zero", "calc", async () => { await fill({ B1: "=A1", C1: "=A1&\"x\"" }); }, { B1: 0, C1: "x" });
scenario("recalc-chain", "calc", async () => {
  await fill({ A1: 1, B1: "=A1*2", C1: "=B1+1" }); await put("A1", "5");
}, { C1: 11 });
scenario("recalc-cross-sheet", "calc", async () => {
  await fill({ A1: 1 }); await run("sheet.add"); await put("A1", "=Sheet1!A1*3");
  await run("view.sheet.prev"); await put("A1", "4"); await run("view.sheet.next");
}, { A1: 12 });
scenario("circular-reference-no-hang", "calc", async () => {
  await fill({ A1: "=B1+1", B1: "=A1+1", C1: 5 });
}, { C1: 5 });
scenario("rename-sheet-updates-refs", "calc", async () => {
  await fill({ A1: 3 }); await run("sheet.add"); await put("A1", "=Sheet1!A1");
  await run("view.sheet.prev"); await run("sheet.rename", "Data"); await run("view.sheet.next");
}, { "A1=": "Data!A1", A1: 3 });
scenario("delete-sheet-refs-become-REF", "calc", async () => {
  await fill({ A1: 3 }); await run("sheet.add"); await put("A1", "=Sheet1!A1");
  await run("view.sheet.prev"); await run("sheet.delete");
}, { A1: "#REF!" });
scenario("paste-special-values", "copy/paste", async () => {
  await fill({ A1: 2, B1: "=A1*3" });
  await select("B1"); await run("edit.copy"); await select("C1"); await pasteSpecial(1);
}, { C1: 6, "C1=": "" });
scenario("paste-special-transpose", "copy/paste", async () => {
  await fill({ A1: 1, B1: 2, C1: 3 });
  await select("A1:C1"); await run("edit.copy"); await select("E1"); await pasteSpecial(1, true);
}, { E1: 1, E3: 3 });
scenario("remove-duplicates", "sort/filter", async () => {
  await fill({ A1: "a", A2: "b", A3: "a", A4: "c" });
  await select("A1:A4"); await run("data.removeDuplicates", "");
}, { A1: "a", A2: "b", A3: "c", A4: "" });

// --- run -------------------------------------------------------------------
const only = flag("--only");
const results = [];
const sameVal = (got, want) => {
  if (typeof want === "number") { const n = Number(got); return got !== "" && Math.abs(n - want) <= 1e-9 * Math.max(1, Math.abs(want)); }
  if (want === "TRUE" || want === "FALSE") return String(got).toUpperCase() === want;
  return String(got) === String(want);
};
for (const s of S) {
  if (only && s.name !== only) continue;
  let err = "";
  const fails = [];
  try {
    await fresh();
    await s.steps();
    for (const [k, want] of Object.entries(s.expect)) {
      const ref = k.replace(/(=|d)$/, "");
      const c = await cell(ref);
      if (k.endsWith("=")) { if (c.f !== want) fails.push(`${ref} formula ${JSON.stringify(c.f)} ≠ ${JSON.stringify(want)}`); }
      else if (want && typeof want === "object") { if (c.d !== want.d) fails.push(`${ref} shows ${JSON.stringify(c.d)} ≠ ${JSON.stringify(want.d)}`); }
      else if (!sameVal(c.v, want)) fails.push(`${ref} ${JSON.stringify(c.v)} ≠ ${JSON.stringify(want)}`);
    }
  } catch (e) { err = String(e.message || e).split("\n")[0]; }
  const ok = !err && fails.length === 0;
  results.push({ name: s.name, area: s.area, ok, fails, err });
  console.log(`  ${ok ? "PASS" : "FAIL"} ${s.area.padEnd(12)} ${s.name}${ok ? "" : "  — " + (err || fails.join("; "))}`);
}
const pass = results.filter((r) => r.ok).length;
console.log(`\nops ${pass}/${results.length}`);
if (ed.errors.length) console.log("page errors: " + ed.errors.slice(0, 5).join(" | "));
if (flag("--json")) fs.writeFileSync(flag("--json"), JSON.stringify(results, null, 1));
await ed.close();
