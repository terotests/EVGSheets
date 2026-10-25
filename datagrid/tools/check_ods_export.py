#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""The independent half of FormatsTest's .ods round trip.

FormatsTest writes each .xlsx fixture out as .ods and reads it back with the
editor's own reader, which proves the two agree. This asks LibreOffice: it
opens every .ods the test wrote, RECALCULATES it (so each formula result is
LibreOffice's, computed from the OpenFormula text the writer produced), and
saves it as .xlsx; openpyxl then compares every cell with the original
fixture's cached values.

    python3 gallery/datagrid/tools/check_ods_export.py [out_dir] [fixtures_dir]
"""
import math
import os
import shutil
import subprocess
import sys
import re
import tempfile
import zipfile

from openpyxl import load_workbook

out_dir = sys.argv[1] if len(sys.argv) > 1 else "gallery/datagrid/harness/out/export"
fix_dir = sys.argv[2] if len(sys.argv) > 2 else "gallery/datagrid/fixtures"


def same(a, b):
    if a is None and b in ("", None):
        return True
    if b is None and a in ("", None):
        return True
    if isinstance(a, bool) or isinstance(b, bool):
        return a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return math.isclose(a, b, rel_tol=1e-9, abs_tol=1e-9)
    if hasattr(a, "toordinal") and hasattr(b, "toordinal"):
        return a == b
    return str(a) == str(b)


failed = 0
checked = 0
tmp = tempfile.mkdtemp()
try:
    for name in sorted(os.listdir(out_dir)):
        if not name.endswith(".ods"):
            continue
        src = os.path.join(fix_dir, name[:-4] + ".xlsx")
        if not os.path.exists(src):
            continue
        # Formula results are taken out first, so every value LibreOffice
        # reports for a formula cell is one it computed from the formula text.
        bare = os.path.join(tmp, name)
        with zipfile.ZipFile(os.path.join(out_dir, name)) as zin, zipfile.ZipFile(bare, "w") as zout:
            for item in zin.infolist():
                data = zin.read(item.filename)
                if item.filename == "content.xml":
                    text = data.decode("utf-8")
                    text = re.sub(r'(<table:table-cell[^>]*table:formula="[^"]*")[^>]*?(/?>)',
                                  lambda m: re.sub(r' office:[a-z-]+="[^"]*"', "", m.group(0)), text)
                    text = re.sub(r'(<table:table-cell[^>]*table:formula="[^"]*"[^>/]*>)<text:p>.*?</text:p>(</table:table-cell>)',
                                  r"\1\2", text)
                    data = text.encode("utf-8")
                zout.writestr(item, data, compress_type=zipfile.ZIP_STORED if item.filename == "mimetype" else zipfile.ZIP_DEFLATED)
        r = subprocess.run(["soffice", "--headless", "--convert-to", "xlsx", "--outdir", tmp, bare],
                           capture_output=True, text=True)
        got_path = os.path.join(tmp, name[:-4] + ".xlsx")
        if not os.path.exists(got_path):
            print(f"  FAIL {name}: LibreOffice could not open it ({r.stderr.strip()})")
            failed += 1
            continue
        # The reference is the fixture as LibreOffice computes it, so a
        # fixture saved without cached results still has something to say.
        subprocess.run(["soffice", "--headless", "--convert-to", "xlsx:Calc MS Excel 2007 XML", "--outdir", os.path.join(tmp, "ref"), src],
                       capture_output=True, text=True)
        ref_path = os.path.join(tmp, "ref", name[:-4] + ".xlsx")
        want = load_workbook(ref_path if os.path.exists(ref_path) else src, data_only=True)
        got = load_workbook(got_path, data_only=True)
        if want.sheetnames != got.sheetnames:
            print(f"  FAIL {name}: sheets {got.sheetnames} want {want.sheetnames}")
            failed += 1
            continue
        bad = []
        for ws in want.worksheets:
            gs = got[ws.title]
            for row in ws.iter_rows():
                for cell in row:
                    g = gs[cell.coordinate].value
                    checked += 1
                    if not same(cell.value, g):
                        bad.append(f"{ws.title}!{cell.coordinate}: {g!r} want {cell.value!r}")
            want_merges = sorted(str(m) for m in ws.merged_cells.ranges)
            got_merges = sorted(str(m) for m in gs.merged_cells.ranges)
            if want_merges != got_merges:
                bad.append(f"{ws.title} merges {got_merges} want {want_merges}")
        if bad:
            failed += 1
            print(f"  FAIL {name}: {len(bad)} difference(s), first: {bad[0]}")
        else:
            print(f"  PASS {name}: LibreOffice reads and recalculates it to the fixture's values")
finally:
    shutil.rmtree(tmp, ignore_errors=True)

print(f"checked {checked} cells, {failed} file(s) failed")
sys.exit(1 if failed else 0)
