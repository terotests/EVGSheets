# Correctness tests against GridCraft

These scripts check that common features give the same results as Excel. GridCraft (`gridcraft-cli`) and LibreOffice are used as references.

Build first: `node scripts/build.mjs --evgui <EVGUI checkout>`.

| Script | What it checks |
| --- | --- |
| `run_formulas.mjs --gridcraft PATH [--no-lo]` | 333 formulas from `formulas.tsv`. Each formula is checked twice: opened from `formulas.xlsx` (built by `make_book.py`) and typed into the editor. |
| `run_ops.mjs [--only name]` | 60 editor scenarios: copy/paste, fill, sort/filter, inserting and deleting rows and columns, undo, entry and recalculation. |
| `run_numfmt.mjs --gridcraft PATH` | 81 number format codes from `numfmt_codes.txt`, each over 12 values. The displayed text is compared. |

To build `gridcraft-cli`, run `cargo build --release -p gridcraft-cli` in a checkout of storytold/gridcraft.

Rows in `formulas.tsv` are `formula<TAB>expected`. When the expected value is blank, GridCraft and LibreOffice decide; `""` means an empty string.
