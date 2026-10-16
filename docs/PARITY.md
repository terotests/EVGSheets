# Feature parity: how EVGSheets is measured against Excel and GridCraft

The target is Microsoft Excel (Microsoft 365). The yardstick is
[storytold/gridcraft](https://github.com/storytold/gridcraft), a clean-room
Excel in Rust that publishes its own parity lists. GridCraft's code is not
used; only its feature *lists* are (command ids and names from
`crates/engine/src/catalog.rs` and `docs/ui-parity.md`, MIT OR Apache-2.0).
The approach follows [RangerDocx](https://github.com/terotests/RangerDocx)'s
`docs/PARITY.md` against WordCraft, with one change: a row counts only when a
named check **passed in the last run**, not when a test file merely exists.

The result is [PARITY_REPORT.md](PARITY_REPORT.md).

## Running it

```sh
node scripts/build.mjs                              # → dist/
node scripts/smoke.mjs --json parity/smoke.json     # the editor's own checks
node tools/parity/probe.mjs                         # → parity/probe.json, docs/parity/evgsheets.png
node tools/parity/report.mjs                        # → docs/PARITY_REPORT.md
```

`npm run parity` runs the last three. Commit the two JSON files, the
screenshot and the report together.

## Rules

- **Checked or it does not count.** Every EVGSheets row names its check in the
  `test` column: `smoke:<check name>` (a check in `scripts/smoke.mjs`) or
  `probe:<check id>` (a check in `tools/parity/probe.mjs`). The row scores only
  if that check passed in the last run.
- A catalog row also needs its command registered in the editor: the core's
  command table or an app menu/ribbon action, both listed in `probe.json`.
- Scoring: `yes` = 1, `partial` = 0.5, `no` = 0. The report lists rows whose
  claim and check disagree under "Problems", and the claimed-but-unchecked
  rows separately (C2), so nothing is counted on trust.
- "unmeasured" is written as such. No estimate goes into a measured column.
- GridCraft's columns are GridCraft's own claims; they are refreshed with
  `python3 tools/parity/import_gridcraft.py <gridcraft checkout>`, which keeps
  the EVGSheets columns.

## What is measured

| file | rows | what |
|---|---|---|
| `parity/catalog.tsv` | 300 rows, 290 command ids | Excel's ribbon (GridCraft's catalog). `evgsheets_cmd` is the EVGSheets command that does the same thing. |
| `parity/ui.tsv` | 60 | The UI checklist: window, ribbon, formula bar, grid, sheet tabs, status bar, dialogs, accessibility, and how the target workbook is drawn. `weight` 3 = visible at first glance in the target screenshot, 2 = used daily, 1 = the rest. The report ranks the gaps by points lost. |
| `parity/metrics.tsv` | 13 | The headline numbers, Excel and GridCraft beside EVGSheets, with a target. |
| `parity/fixtures/quarterly-sales-dashboard.xlsx` | | The target workbook, written by `tools/parity/make_dashboard.py` (XlsxWriter). |

### The target workbook

The UI target is an Excel screenshot of a "Quarterly Sales Dashboard": the
Home ribbon, a formatted table with filter buttons, banded rows and a total
row, data bars, an icon set, sparklines, a column chart and a pie chart, sheet
tabs and a status bar with zoom. `make_dashboard.py` rebuilds that workbook as
a file, so the same thing can be opened in Excel, GridCraft and EVGSheets.
Screenshots of Excel are not committed (GridCraft's rule as well).

`probe.mjs` opens it at the screenshot's size (1650×1030) and checks, without
judging by eye:

- the chrome, from the accessibility mirror: ribbon tabs and controls, Name
  Box, formula bar buttons, how far the grid's columns and rows reach, sheet
  tab controls, status bar mode, statistics, zoom and view buttons;
- the workbook, from cell text and canvas pixels: values and formats, the
  table and its style, banding and filter buttons, the total row, data bars,
  icon sets and sparklines (coloured pixels where only grey text would be),
  charts, borders, number alignment;
- commands, by running them on cells below the dashboard and reading the
  state back (`cmd.<catalog id>`).

A probe failure is a measured gap, not a broken run; the probe always exits 0.

## The first measurement (2026-10-16)

See [PARITY_REPORT.md](PARITY_REPORT.md) for the numbers. In short: 54 of 290
catalog commands are proven (GridCraft 274), the weighted UI checklist is at
29 of 106 points (GridCraft claims 100.5), and 5 of the target workbook's 15
checks pass. The largest UI gaps are the ones visible at first glance: no
ribbon tabs, a grid that stops at the used range, numbers left-aligned, no
formula bar buttons, no zoom or view buttons, and the workbook's table style,
total row, data bars, icon sets, sparklines and charts not drawn.

## Second measurement: 50 % of GridCraft (2026-10-16)

The goal was half of GridCraft's own figures: C1 ≥ 137 (GridCraft 274) and
U1 ≥ 50.25 (GridCraft 100.5). Result: C1 137.5 of 290, U1 50.5 of 106,
11 of 15 target-workbook checks.

- Drawing: the grid fills the window on open, numbers and dates right-align
  under General, tables draw their style (header, bands, first/last column,
  totals) and their filter buttons, and the status bar leads with Excel's
  mode word (Ready / Enter / Edit).
- Formulas: unqualified structured references inside a table (`[Q1]`),
  SUBTOTAL, STDEV/VAR (sample and population), PMT, PV, FV, NPER, NPV, SLN.
- About 90 commands under the catalog's own ids (`home.mergeCenter`,
  `table.totalRow`, `pageLayout.printArea`, …), each with a `cmd.<id>` probe
  check and a menu entry. New menus: Page Layout, Formulas, Review.
- Files: indent, unlocked cells, double underline, gridlines/headings/show
  formulas, tab colour, sheet and workbook protection, page setup, print
  area and titles, page breaks and header/footer are written to the .xlsx;
  CSV and HTML export.

Partial (0.5) where Excel does more: Insert/Delete Cells do not re-point
references into the moved cells, Trace Precedents selects instead of drawing
arrows, the function-library commands start the formula without a dropdown,
Sort takes one key by argument, a "comment" is a note. Not done: zoom, ribbon
tabs, data bars, icon sets, sparklines and charts from the file.

## Who embeds EVGSheets

Sliqtly's spreadsheet editing is EVGSheets: `web/sheets-live.js` mounts it
with `mountSheets` (`ui: "viewer"` on slides, `ui: "full"` in the edit dialog)
from `https://terotests.github.io/EVGSheets/`, not from a pinned build. A
change on `main` reaches Sliqtly as soon as Pages deploys it.
