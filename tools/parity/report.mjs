#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Writes docs/PARITY_REPORT.md from parity/*.tsv and the results of the last
 * run (parity/smoke.json from `smoke.mjs --json`, parity/probe.json from
 * probe.mjs). Nothing in the report is typed by hand.
 *
 *   node tools/parity/report.mjs
 *
 * An EVGSheets row scores only when its `test` names a check that passed in
 * the last run: `smoke:<check name>` or `probe:<check id>`. A catalog row also
 * needs its command registered in the editor (the core's command table or an
 * app menu/ribbon action, as probe.json lists them). Scoring: yes = 1,
 * partial = 0.5, no = 0. Rows that claim more than their check shows are
 * listed under "Problems".
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const P = (...p) => path.join(REPO, ...p);

function tsv(file) {
  const [head, ...lines] = fs.readFileSync(P("parity", file), "utf8").split("\n").filter(Boolean);
  const cols = head.split("\t");
  return lines.map((l) => Object.fromEntries(l.split("\t").map((v, i) => [cols[i], v ?? ""])));
}
const json = (file) => (fs.existsSync(P("parity", file)) ? JSON.parse(fs.readFileSync(P("parity", file), "utf8")) : null);

const catalog = tsv("catalog.tsv");
const ui = tsv("ui.tsv");
const metrics = tsv("metrics.tsv");
const smoke = json("smoke.json");
const probe = json("probe.json");

const value = (s) => (s === "yes" || s === "live" ? 1 : s === "partial" ? 0.5 : 0);
const fmt = (n) => (Number.isInteger(n) ? String(n) : n.toFixed(1));
const pct = (n, of) => (of ? ((100 * n) / of).toFixed(1) + "%" : "-");
const problems = [];

// Did the check a test names pass in the last run? undefined = no such check.
const smokeByName = new Map();
for (const c of smoke?.checks || []) smokeByName.set(c.name, (smokeByName.get(c.name) ?? true) && c.ok);
function passed(test) {
  if (!test || test === "-") return undefined;
  const [kind, ...rest] = test.split(":");
  const id = rest.join(":");
  if (kind === "smoke") return smoke ? smokeByName.get(id) : undefined;
  if (kind === "probe") return probe?.results?.[id]?.pass;
  return undefined;
}
const registered = new Set([...(probe?.commands || []), ...(probe?.actions || [])]);
const reachable = new Set(probe?.actions || []);

function score(row, where, needsCommand) {
  const claim = value(row.evgsheets);
  const p = passed(row.test);
  const label = `${where} ${row.id || row.command} (${row.feature || row.element})`;
  if (row.test !== "-" && p === undefined) problems.push(`${label}: check \`${row.test}\` was not found in the last run`);
  if (claim > 0 && p === false) problems.push(`${label}: claims ${row.evgsheets} but \`${row.test}\` failed`);
  if (claim === 0 && p === true) problems.push(`${label}: \`${row.test}\` passes but the row says no; update it`);
  if (needsCommand && claim > 0 && !registered.has(row.evgsheets_cmd)) {
    problems.push(`${label}: command \`${row.evgsheets_cmd}\` is not registered in the editor`);
    return 0;
  }
  return p === true ? claim : 0;
}

// --- catalog (distinct command ids, as GridCraft counts them) -------------------
const byId = new Map();
for (const r of catalog) if (!byId.has(r.command)) byId.set(r.command, r);
const ids = [...byId.values()];
const tabs = [...new Set(catalog.map((r) => r.tab))];
const tabRows = tabs.map((t) => {
  const rs = ids.filter((r) => r.tab === t);
  return {
    tab: t,
    n: rs.length,
    gc: rs.filter((r) => r.gridcraft === "live").length,
    claimed: rs.reduce((s, r) => s + value(r.evgsheets), 0),
    scored: rs.reduce((s, r) => s + score(r, "catalog", true), 0),
  };
});
const tot = tabRows.reduce((a, r) => ({ n: a.n + r.n, gc: a.gc + r.gc, claimed: a.claimed + r.claimed, scored: a.scored + r.scored }), { n: 0, gc: 0, claimed: 0, scored: 0 });
const untested = ids.filter((r) => value(r.evgsheets) > 0 && passed(r.test) !== true);
const coreOnly = ids.filter((r) => value(r.evgsheets) > 0 && registered.has(r.evgsheets_cmd) && !reachable.has(r.evgsheets_cmd));

// --- UI checklist ---------------------------------------------------------------
const areas = [...new Set(ui.map((r) => r.area))];
const uiScored = new Map(ui.map((r) => [r.id, score(r, "ui", false)]));
const areaRows = areas.map((a) => {
  const rs = ui.filter((r) => r.area === a);
  const w = (f) => rs.reduce((s, r) => s + Number(r.weight) * f(r), 0);
  return { area: a, n: rs.length, max: w(() => 1), gc: w((r) => value(r.gridcraft)), ev: w((r) => uiScored.get(r.id)) };
});
const uiTot = areaRows.reduce((a, r) => ({ n: a.n + r.n, max: a.max + r.max, gc: a.gc + r.gc, ev: a.ev + r.ev }), { n: 0, max: 0, gc: 0, ev: 0 });
const gaps = ui
  .map((r) => ({ ...r, lost: Number(r.weight) * (1 - uiScored.get(r.id)) }))
  .filter((r) => r.lost > 0)
  .sort((a, b) => b.lost - a.lost || value(b.gridcraft) - value(a.gridcraft) || a.id.localeCompare(b.id));

// --- functions --------------------------------------------------------------------
const ff = fs.readFileSync(P("datagrid", "src", "FormulaFunctions.rgr"), "utf8");
const known = ff.slice(ff.indexOf("sfn isKnown"), ff.indexOf("\n    }\n", ff.indexOf("sfn isKnown")));
const functions = [...new Set([...known.matchAll(/"([A-Z][A-Z0-9.]*)"/g)].map((m) => m[1]))].sort();

// --- metrics ----------------------------------------------------------------------
const wbIds = Object.keys(probe?.results || {}).filter((k) => k.startsWith("wb."));
const computed = {
  C1: `${fmt(tot.scored)} (${pct(tot.scored, tot.n)})`,
  C2: String(untested.length),
  U1: { excel: `${uiTot.max} (100%)`, gridcraft: `${fmt(uiTot.gc)} (${pct(uiTot.gc, uiTot.max)})`, evgsheets: `${fmt(uiTot.ev)} (${pct(uiTot.ev, uiTot.max)})` },
  W1: `${wbIds.filter((k) => probe.results[k].pass).length} of ${wbIds.length}`,
  F1: String(functions.length),
  T1: `${smoke ? smoke.checks.length : "?"} smoke + ${Object.keys(probe?.results || {}).length} probe`,
};
function metricValue(m, col) {
  const v = m[col];
  const c = computed[m.id];
  if (v === "computed") return typeof c === "object" ? c[col] : c;
  if (v.startsWith("probe:")) {
    const r = probe?.results?.[v.slice(6)];
    return r ? (r.value.match(/^\d+/)?.[0] ?? (r.pass ? "yes" : "no")) : "not run";
  }
  return v;
}

// --- write ------------------------------------------------------------------------
const L = [];
L.push("# EVGSheets feature parity report", "");
L.push("Generated by `tools/parity/report.mjs` from `parity/*.tsv`, `parity/smoke.json` and `parity/probe.json`. Do not edit by hand.");
L.push("Method: [PARITY.md](PARITY.md). Scoring: yes = 1, partial = 0.5. An EVGSheets row scores only when the check it names passed in the last run.");
L.push("", `Last run: ${smoke ? smoke.passed + "/" + smoke.checks.length + " smoke checks" : "smoke not run"}, ${probe ? Object.values(probe.results).filter((r) => r.pass).length + "/" + Object.keys(probe.results).length + " probe checks pass" : "probe not run"}.`);

L.push("", "## Headline", "");
L.push("| id | metric | unit | Excel | GridCraft | EVGSheets | target |", "|---|---|---|---:|---:|---:|---:|");
for (const m of metrics) L.push(`| ${m.id} | ${m.metric} | ${m.unit} | ${metricValue(m, "excel")} | ${metricValue(m, "gridcraft")} | ${metricValue(m, "evgsheets")} | ${m.target} |`);

L.push("", "## UI gaps, ranked", "");
L.push("Points lost against Excel, largest first. Weight 3 = visible at first glance in the target screenshot, 2 = used daily, 1 = the rest.", "");
L.push("| rank | id | area | element | lost | GridCraft | EVGSheets | check | note |", "|---:|---|---|---|---:|---|---|---|---|");
gaps.forEach((r, i) => {
  const p = passed(r.test);
  L.push(`| ${i + 1} | ${r.id} | ${r.area} | ${r.element} | ${fmt(r.lost)} | ${r.gridcraft} | ${r.evgsheets} | ${r.test === "-" ? "none" : `${p ? "pass" : "fail"}`} | ${r.note} |`);
});

L.push("", "## UI checklist by area (U1)", "");
L.push("| area | rows | points | GridCraft | EVGSheets |", "|---|---:|---:|---:|---:|");
for (const r of areaRows) L.push(`| ${r.area} | ${r.n} | ${r.max} | ${fmt(r.gc)} | ${fmt(r.ev)} |`);
L.push(`| **Total** | **${uiTot.n}** | **${uiTot.max}** | **${fmt(uiTot.gc)} (${pct(uiTot.gc, uiTot.max)})** | **${fmt(uiTot.ev)} (${pct(uiTot.ev, uiTot.max)})** |`);

L.push("", "## Ribbon and menu catalog (C1)", "");
L.push("GridCraft's catalog of Excel's ribbon, one row per distinct command id. *Claimed* is what the rows say; *tested* is what counts.", "");
L.push("| tab | ids | GridCraft | EVGSheets claimed | EVGSheets tested |", "|---|---:|---:|---:|---:|");
for (const r of tabRows) L.push(`| ${r.tab} | ${r.n} | ${r.gc} | ${fmt(r.claimed)} | ${fmt(r.scored)} |`);
L.push(`| **Total** | **${tot.n}** | **${tot.gc} (${pct(tot.gc, tot.n)})** | **${fmt(tot.claimed)} (${pct(tot.claimed, tot.n)})** | **${fmt(tot.scored)} (${pct(tot.scored, tot.n)})** |`);

L.push("", "### Claimed but not proven (C2)", "");
for (const r of untested) L.push(`- \`${r.command}\` ${r.feature} → \`${r.evgsheets_cmd}\` (${r.evgsheets}${r.note ? "; " + r.note : ""})`);
L.push("", "### Registered in the core but not on any menu, ribbon or context menu", "");
for (const r of coreOnly) L.push(`- \`${r.command}\` ${r.feature} → \`${r.evgsheets_cmd}\``);
if (!coreOnly.length) L.push("- none");

L.push("", "## Target workbook and probe checks", "");
L.push("`parity/fixtures/quarterly-sales-dashboard.xlsx` opened at the size of the Excel screenshot. What EVGSheets drew: [parity/evgsheets.png](parity/evgsheets.png).", "");
L.push("| check | result | measured |", "|---|---|---|");
for (const [k, r] of Object.entries(probe?.results || {})) L.push(`| ${k} | ${r.pass ? "pass" : "fail"} | ${r.value.replace(/\|/g, "\\|")} |`);

L.push("", `## Worksheet functions (F1): ${functions.length}`, "", functions.join(", "));

L.push("", "## Problems", "");
if (!smoke) problems.unshift("parity/smoke.json is missing: run `node scripts/smoke.mjs --json parity/smoke.json`");
if (!probe) problems.unshift("parity/probe.json is missing: run `node tools/parity/probe.mjs`");
L.push(...(problems.length ? [...new Set(problems)].map((p) => "- " + p) : ["- none"]));

fs.writeFileSync(P("docs", "PARITY_REPORT.md"), L.join("\n") + "\n");
console.log(`docs/PARITY_REPORT.md: C1 ${fmt(tot.scored)}/${tot.n}, U1 ${fmt(uiTot.ev)}/${uiTot.max}, ${gaps.length} UI gaps, ${problems.length} problems`);
