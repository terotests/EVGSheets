#!/usr/bin/env python3
"""Refreshes the GridCraft columns of parity/catalog.tsv from a GridCraft checkout.

    python3 tools/parity/import_gridcraft.py <path to storytold/gridcraft>

GridCraft's catalog (crates/engine/src/catalog.rs) is Excel's ribbon and menu
tree, one row per command id; docs/parity-checklist.md lists the ids GridCraft
has not implemented. Only the `gridcraft` column is replaced. The EVGSheets
columns already in the file (evgsheets_cmd, evgsheets, test, note) are kept,
matched by tab + command id, so progress is never lost on a refresh. New rows
arrive as `no`. Data source: GridCraft (MIT OR Apache-2.0), feature names only.
"""
import os, re, sys

gc = sys.argv[1] if len(sys.argv) > 1 else "../gridcraft"
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "parity", "catalog.tsv")
HEAD = ["tab", "group", "feature", "command", "gridcraft", "evgsheets_cmd", "evgsheets", "test", "note"]

def read_tsv(path):
    if not os.path.exists(path):
        return []
    lines = open(path, encoding="utf-8").read().splitlines()
    head = lines[0].split("\t")
    return [dict(zip(head, l.split("\t"))) for l in lines[1:] if l]

old = {(r["tab"], r["command"]): r for r in read_tsv(out)}

# Ids GridCraft lists under "Missing:" in its generated checklist.
missing, in_missing = set(), False
for l in open(os.path.join(gc, "docs/parity-checklist.md"), encoding="utf-8"):
    if l.startswith("Missing:"):
        in_missing = True
    elif l.startswith("Implemented:") or l.startswith("## "):
        in_missing = False
    m = re.match(r"- `([^`]+)`", l)
    if in_missing and m:
        missing.add(m.group(1))

src = open(os.path.join(gc, "crates/engine/src/catalog.rs"), encoding="utf-8").read()
body = src.split("pub const CATALOG", 1)[1].split("];", 1)[0]
rows = []
for cmd, tab, group, feat in re.findall(r'\("([^"]+)",\s*"([^"]+)",\s*"([^"]+)",\s*"([^"]+)"\)', body):
    prev = old.get((tab, cmd), {})
    rows.append({"tab": tab, "group": group, "feature": feat, "command": cmd,
                 "gridcraft": "missing" if cmd in missing else "live",
                 "evgsheets_cmd": prev.get("evgsheets_cmd", "-"), "evgsheets": prev.get("evgsheets", "no"),
                 "test": prev.get("test", "-"), "note": prev.get("note", "")})

with open(out, "w", encoding="utf-8") as f:
    f.write("\t".join(HEAD) + "\n")
    for r in rows:
        f.write("\t".join(r.get(h, "").replace("\t", " ") for h in HEAD) + "\n")
ids = {r["command"] for r in rows}
print(f"catalog.tsv: {len(rows)} rows, {len(ids)} distinct ids, {len(missing & ids)} missing in GridCraft")
