#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Rebuild fixtures/formats/: this repository's .xlsx fixtures as LibreOffice
# saves them in .xls and .ods, a hand-assembled .xlsb, a macro-enabled .xlsm
# and template .xltx, and two text files. FormatsTest compares every one with
# the .xlsx it came from.
#
#   bash gallery/datagrid/tools/make_format_fixtures.sh
set -eu
here=$(cd "$(dirname "$0")" && pwd)
fx="$here/../fixtures"
out="$fx/formats"
mkdir -p "$out"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
for n in formulas styles-showcase merged business-workbook accented-text sales; do
  cp "$fx/$n.xlsx" "$tmp/"
  soffice --headless --convert-to xls --outdir "$out" "$tmp/$n.xlsx" >/dev/null 2>&1
  soffice --headless --convert-to ods --outdir "$out" "$tmp/$n.xlsx" >/dev/null 2>&1
done
python3 "$here/make_xlsb_fixture.py" "$out" >/dev/null
python3 - "$fx/sales.xlsx" "$out" <<'PY'
import sys, zipfile, os
src, out = sys.argv[1], sys.argv[2]
# A VBA project is opaque bytes to everything but Excel; these stand in for
# one, and the test checks they come back byte for byte.
vba = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1" + bytes(range(256)) * 4
def rewrite(dest, main_type, with_vba):
    with zipfile.ZipFile(src) as zin, zipfile.ZipFile(dest, "w", zipfile.ZIP_DEFLATED) as zout:
        for item in zin.infolist():
            data = zin.read(item.filename)
            if item.filename == "[Content_Types].xml":
                t = data.decode()
                t = t.replace("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml", main_type)
                if with_vba:
                    t = t.replace("</Types>", '<Default Extension="bin" ContentType="application/vnd.ms-office.vbaProject"/></Types>')
                data = t.encode()
            if item.filename == "xl/_rels/workbook.xml.rels" and with_vba:
                t = data.decode()
                t = t.replace("</Relationships>", '<Relationship Id="rIdVba" Type="http://schemas.microsoft.com/office/2006/relationships/vbaProject" Target="vbaProject.bin"/></Relationships>')
                data = t.encode()
            zout.writestr(item, data)
        if with_vba:
            zout.writestr("xl/vbaProject.bin", vba)
rewrite(os.path.join(out, "sales.xlsm"), "application/vnd.ms-excel.sheet.macroEnabled.main+xml", True)
rewrite(os.path.join(out, "sales.xltx"), "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml", False)
PY
printf 'Name;Amount;When\r\n"Smith; John";12,5;2024-03-15\r\n"He said ""hi""\nthen left";7;15.3.2024\r\n' > "$out/semicolons.csv"
printf '\xef\xbb\xbfRegion\tUnits\nH\xc3\xa4meenlinna\t120\nNorth\t80\n' > "$out/units.tsv"
printf 'Caf\xe9\t3\n' > "$out/latin1.txt"
echo "fixtures in $out"
