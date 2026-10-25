#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Write fixtures/formats/values.xlsb, a small binary workbook built record by
record from [MS-XLSB] — nothing on this machine writes .xlsb, so the test
fixture is assembled here the way GridCraft's tests assemble theirs.

Two sheets. "Data": a header row, text from the shared-string table, an RK
integer, an RK hundredth, a double, a date (number format 14), TRUE, #DIV/0!,
a formula with a cached number and one with a cached string, a merge and a
column width. "Hidden": one cell, the sheet hidden.

    python3 gallery/datagrid/tools/make_xlsb_fixture.py [out_dir]
"""
import os
import struct
import sys
import zipfile


def rec(rid, payload=b""):
    out = bytearray()
    if rid < 128:
        out.append(rid)
    else:
        out += bytes([(rid & 0x7F) | 0x80, rid >> 7])
    n = len(payload)
    while True:
        b = n & 0x7F
        n >>= 7
        out.append(b | 0x80 if n else b)
        if not n:
            break
    return bytes(out) + payload


def wide(s):
    u = s.encode("utf-16-le")
    return struct.pack("<I", len(u) // 2) + u


def row(r):
    return rec(0, struct.pack("<II", r, 0) + b"\x00" * 5 + struct.pack("<I", 0))


def cell(rid, col, value, xf=0):
    data = struct.pack("<II", col, xf) + value
    if 8 <= rid <= 11:
        data += b"\x00\x00" + struct.pack("<I", 3) + b"\x1e\x01\x00" + struct.pack("<I", 0)
    return rec(rid, data)


def rk_int(v):
    return struct.pack("<I", ((v << 2) | 2) & 0xFFFFFFFF)


def rk_hundredth(v):
    return struct.pack("<I", ((v << 2) | 3) & 0xFFFFFFFF)


strings = ["Region", "Units", "Price", "When", "Flag", "Ratio", "Total", "Label", "North", "Hämeenlinna"]


def sst():
    out = rec(159, struct.pack("<II", len(strings), len(strings)))
    for s in strings:
        out += rec(19, b"\x00" + wide(s))
    return out + rec(160)


def styles():
    out = rec(278)
    out += rec(615, struct.pack("<I", 1)) + rec(44, struct.pack("<H", 164) + wide("0.00%")) + rec(616)
    out += rec(617, struct.pack("<I", 3))
    for fmt in (0, 14, 164):
        out += rec(47, struct.pack("<HHH", 0xFFFF, fmt, 0) + b"\x00" * 10)
    out += rec(618)
    return out + rec(279)


def sheet_data():
    body = b""
    body += row(0)
    for c, s in enumerate(range(8)):
        body += cell(7, c, struct.pack("<I", s))
    body += row(1)
    body += cell(7, 0, struct.pack("<I", 8))
    body += cell(2, 1, rk_int(120))
    body += cell(2, 2, rk_hundredth(950))
    body += cell(2, 3, rk_int(45306), xf=1)
    body += cell(4, 4, b"\x01")
    body += cell(3, 5, b"\x07")
    body += cell(9, 6, struct.pack("<d", 1140.0))
    body += cell(8, 7, wide("done"))
    body += row(2)
    body += cell(7, 0, struct.pack("<I", 9))
    body += cell(5, 1, struct.pack("<d", 3.25))
    body += cell(5, 5, struct.pack("<d", 0.125), xf=2)
    out = rec(129)
    out += rec(390) + rec(60, struct.pack("<IIII", 0, 0, 20 * 256, 0) + struct.pack("<H", 2)) + rec(391)
    out += rec(145) + body + rec(146)
    out += rec(177, struct.pack("<I", 1)) + rec(176, struct.pack("<IIII", 4, 4, 0, 2)) + rec(178)
    return out + rec(130)


def sheet_hidden():
    return rec(129) + rec(145) + row(0) + cell(5, 0, struct.pack("<d", 42.0)) + rec(146) + rec(130)


def workbook():
    out = rec(131)
    out += rec(153, struct.pack("<II", 0, 0) + wide(""))
    out += rec(143)
    out += rec(156, struct.pack("<II", 0, 1) + wide("rId1") + wide("Data"))
    out += rec(156, struct.pack("<II", 1, 2) + wide("rId2") + wide("Hidden"))
    out += rec(144)
    return out + rec(132)


TYPES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="bin" ContentType="application/vnd.ms-excel.worksheet"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/xl/workbook.bin" ContentType="application/vnd.ms-excel.sheet.binary.macroEnabled.main"/><Override PartName="/xl/styles.bin" ContentType="application/vnd.ms-excel.styles"/><Override PartName="/xl/sharedStrings.bin" ContentType="application/vnd.ms-excel.sharedStrings"/></Types>"""
ROOT = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.bin"/></Relationships>"""
WBRELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.bin"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.bin"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.bin"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.bin"/></Relationships>"""

out_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "fixtures", "formats")
os.makedirs(out_dir, exist_ok=True)
with zipfile.ZipFile(os.path.join(out_dir, "values.xlsb"), "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("[Content_Types].xml", TYPES)
    z.writestr("_rels/.rels", ROOT)
    z.writestr("xl/_rels/workbook.bin.rels", WBRELS)
    z.writestr("xl/workbook.bin", workbook())
    z.writestr("xl/styles.bin", styles())
    z.writestr("xl/sharedStrings.bin", sst())
    z.writestr("xl/worksheets/sheet1.bin", sheet_data())
    z.writestr("xl/worksheets/sheet2.bin", sheet_hidden())
print("wrote", os.path.join(out_dir, "values.xlsb"))
