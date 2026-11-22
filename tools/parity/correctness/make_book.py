#!/usr/bin/env python3
"""Build formulas.xlsx from formulas.tsv: sheet "D" holds the data the formulas
read, sheet "F" holds one formula per row in column B (column A is its text).
No cached values are written, so every reader has to calculate.

  python3 make_book.py [out.xlsx]
"""
import sys, os, datetime
from openpyxl import Workbook
from openpyxl.workbook.defined_name import DefinedName
from openpyxl.worksheet.table import Table

HERE = os.path.dirname(os.path.abspath(__file__))
out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "formulas.xlsx")

def rows():
    for line in open(os.path.join(HERE, "formulas.tsv"), encoding="utf-8"):
        line = line.rstrip("\n")
        if not line or line.startswith("#"):
            continue
        f, _, exp = line.partition("\t")
        yield f, exp

wb = Workbook()
F = wb.active
F.title = "F"
D = wb.create_sheet("D")
for i, v in enumerate([1, 2, 3, 4, 5]): D.cell(i + 1, 1, v)
for i, v in enumerate([10, "text", True, None, -2.5]):
    if v is not None: D.cell(i + 1, 2, v)
for i, v in enumerate(["apple", "Banana", "cherry", "apple", "date"]): D.cell(i + 1, 3, v)
for i, v in enumerate([5, 3, 8, 3, 1]): D.cell(i + 1, 4, v)
for i, v in enumerate([datetime.date(2024, 1, 15), datetime.date(2024, 2, 29), datetime.date(2023, 12, 31)]):
    c = D.cell(i + 1, 5, v); c.number_format = "yyyy-mm-dd"
for i, v in enumerate(["  Hello World  ", "a,b,c", "ABC"]): D.cell(i + 1, 6, v)
D["G1"] = "=1/0"
for i, (k, v) in enumerate(zip("abcde", [100, 200, 300, 400, 500])):
    D.cell(i + 1, 9, k); D.cell(i + 1, 10, v)
for i, (k, v) in enumerate(zip([1, 3, 5, 7, 9], [10, 20, 30, 40, 50])):
    D.cell(i + 1, 12, k); D.cell(i + 1, 13, v)
for i, (k, v) in enumerate(zip("abc", [11, 22, 33])):
    D.cell(1, 15 + i, k); D.cell(2, 15 + i, v)
# A table "Sales" at S1:T4 and two names.
D["S1"], D["T1"] = "Item", "Amount"
for i, (k, v) in enumerate(zip(["x", "y", "z"], [10, 20, 30])):
    D.cell(i + 2, 19, k); D.cell(i + 2, 20, v)
D.add_table(Table(displayName="Sales", ref="S1:T4"))
wb.defined_names["myRange"] = DefinedName("myRange", attr_text="D!$A$1:$A$5")
wb.defined_names["taxRate"] = DefinedName("taxRate", attr_text="0.24")

DYN = ("SEQUENCE(", "SORT(", "SORTBY(", "UNIQUE(", "FILTER(", "LET(", "TEXTSPLIT(", "CHOOSECOLS(",
       "VSTACK(", "LAMBDA(", "REDUCE(", "XLOOKUP(", "XMATCH(", "IFS(", "SWITCH(", "MAXIFS(", "MINIFS(",
       "CONCAT(", "TEXTJOIN(", "TEXTBEFORE(", "TEXTAFTER(", "XOR(", "IFNA(", "DAYS(", "ISOWEEKNUM(",
       "CEILING.MATH(", "FLOOR.MATH(", "STDEV.P(", "VAR.P(", "RANK.EQ(", "AGGREGATE(", "NUMBERVALUE(",
       "UNICHAR(", "ISFORMULA(", "TRANSPOSE(")
FUT = {"SEQUENCE", "SORT", "SORTBY", "UNIQUE", "FILTER", "LET", "TEXTSPLIT", "CHOOSECOLS", "VSTACK",
       "LAMBDA", "REDUCE", "XLOOKUP", "XMATCH", "IFS", "SWITCH", "MAXIFS", "MINIFS", "CONCAT", "TEXTJOIN",
       "TEXTBEFORE", "TEXTAFTER", "XOR", "IFNA", "DAYS", "ISOWEEKNUM", "CEILING.MATH", "FLOOR.MATH",
       "STDEV.P", "VAR.P", "RANK.EQ", "AGGREGATE", "NUMBERVALUE", "UNICHAR", "ISFORMULA"}
import re
def xl(f):
    # Newer functions carry the _xlfn. prefix in the file, as Excel writes them.
    def sub(m):
        n = m.group(1)
        pre = "_xlfn._xlws." if n in ("SORT", "FILTER") else "_xlfn."
        return (pre + n + "(") if n in FUT else m.group(0)
    return re.sub(r"(?<![A-Za-z_.])([A-Z][A-Z0-9.]*)\(", sub, f)

for r, (f, exp) in enumerate(rows(), start=1):
    F.cell(r, 1, f[1:])  # the text without its "="
    F.cell(r, 2, xl(f))
wb.save(out)
print(out)
