#!/usr/bin/env python3
"""Writes parity/fixtures/quarterly-sales-dashboard.xlsx, the UI benchmark's
target workbook: a "Quarterly Sales Dashboard" Excel screenshot (formatted
table with filters and banding, data bars, icon sets, sparklines, a column
and a pie chart) rebuilt as a file, so the same workbook can be opened in
Excel, GridCraft and EVGSheets and compared. Screenshots of Excel are not
committed.

    python3 tools/parity/make_dashboard.py

Needs XlsxWriter (it writes tables, data bars, icon sets and sparklines,
which openpyxl does not). The output is deterministic for a given XlsxWriter
version; it is committed so the benchmark runs without Python.
"""
import datetime, os, sys
import xlsxwriter

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "parity", "fixtures",
                   "quarterly-sales-dashboard.xlsx")

ROWS = [
    ("North",   48200, 51340, 56010, 61870),
    ("South",   39150, 37980, 42400, 45120),
    ("East",    55300, 58910, 57230, 63480),
    ("West",    61020, 64400, 70150, 76390),
    ("Central", 28740, 31260, 33980, 36510),
    ("Online",  72410, 81630, 95220, 112840),
]

wb = xlsxwriter.Workbook(OUT)
wb.set_properties({"title": "Quarterly Sales Dashboard", "created": datetime.datetime(2026, 1, 1)})
ws = wb.add_worksheet("Sales")

title = wb.add_format({"font_size": 18, "font_color": "#1F3864"})
sub = wb.add_format({"italic": True, "font_color": "#7F7F7F", "font_size": 10})
money = wb.add_format({"num_format": "$#,##0"})
pct = wb.add_format({"num_format": "0.0%"})
def box(row, right, fmt=None):
    """A cell of the B14:C17 summary box, which has a thin outline."""
    p = {"top": 1 if row == 14 else 0, "bottom": 1 if row == 17 else 0}
    p.update({"right": 1} if right else {"left": 1, "bold": True})
    if fmt:
        p["num_format"] = fmt
    return wb.add_format(p)

ws.set_column("A:A", 2.5)
ws.set_column("B:B", 13)
ws.set_column("C:G", 11)
ws.set_column("H:I", 9)
ws.write("B2", "Quarterly Sales Dashboard", title)
ws.write("B3", "Fiscal year 2026 · all figures in USD", sub)

# B4:I10 formatted table with filter buttons, banded rows, a total row (11).
data = []
for i, (region, *q) in enumerate(ROWS):
    r = 5 + i
    data.append([region, *q, None, None, None])
ws.add_table("B4:I11", {
    "name": "SalesTable",
    "style": "Table Style Medium 2",
    "total_row": True,
    "data": data,
    "columns": [
        {"header": "Region", "total_string": "Total"},
        {"header": "Q1", "format": money, "total_function": "sum"},
        {"header": "Q2", "format": money, "total_function": "sum"},
        {"header": "Q3", "format": money, "total_function": "sum"},
        {"header": "Q4", "format": money, "total_function": "sum"},
        {"header": "Total", "format": money, "formula": "=SUM(SalesTable[@[Q1]:[Q4]])", "total_function": "sum"},
        {"header": "Growth", "format": pct, "formula": "=SalesTable[@Q4]/SalesTable[@Q1]-1",
         "total_function": "=SUBTOTAL(109,[Q4])/SUBTOTAL(109,[Q1])-1", "total_value": 0.2997},
        {"header": "Trend"},
    ],
})

# Data bars on Total, an icon set on Growth, sparklines in Trend.
ws.conditional_format("G5:G10", {"type": "data_bar", "bar_color": "#5B9BD5", "bar_solid": False})
ws.conditional_format("H5:H10", {"type": "icon_set", "icon_style": "3_arrows"})
for i in range(len(ROWS)):
    r = 5 + i
    ws.add_sparkline(f"I{r}", {"range": f"Sales!C{r}:F{r}", "type": "line", "series_color": "#1F4E79"})

# Summary box B14:C17.
best = max(ROWS, key=lambda x: sum(x[1:]))[0]
total = sum(sum(x[1:]) for x in ROWS)
online = sum(ROWS[-1][1:])
ws.write("B14", "Best region", box(14, False))
ws.write_formula("C14", "=INDEX(SalesTable[Region],MATCH(MAX(SalesTable[Total]),SalesTable[Total],0))",
                 box(14, True), best)
ws.write("B15", "Average quarter", box(15, False))
ws.write_formula("C15", "=AVERAGE(SalesTable[[Q1]:[Q4]])", box(15, True, "$#,##0"), total / 24)
ws.write("B16", "Online share", box(16, False))
ws.write_formula("C16", "=G10/G11", box(16, True, "0.0%"), online / total)
ws.write("B17", "Year-over-year", box(17, False))
ws.write("C17", 0.30, box(17, True, "0.0%"))

# Charts: clustered column by region and quarter, pie of the annual total.
col = wb.add_chart({"type": "column"})
colors = ["#255E91", "#ED7D31", "#2E7D32", "#4AA3DF"]
for k, q in enumerate(("Q1", "Q2", "Q3", "Q4")):
    c = "CDEF"[k]
    col.add_series({"name": q, "categories": "=Sales!$B$5:$B$10", "values": f"=Sales!${c}$5:${c}$10",
                    "fill": {"color": colors[k]}})
col.set_title({"name": "Revenue by Region"})
col.set_legend({"position": "bottom"})
col.set_y_axis({"num_format": "$#,##0", "major_gridlines": {"visible": True}})
ws.insert_chart("K4", col, {"x_scale": 1.15, "y_scale": 1.12})

pie = wb.add_chart({"type": "pie"})
pie.add_series({"name": "Share of Annual Total", "categories": "=Sales!$B$5:$B$10", "values": "=Sales!$G$5:$G$10"})
pie.set_title({"name": "Share of Annual Total"})
pie.set_legend({"position": "right"})
ws.insert_chart("K20", pie, {"x_scale": 1.15, "y_scale": 1.2})

ws.activate()
ws.set_selection("A1")
wb.close()
print("wrote", os.path.relpath(OUT))
