#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Build the EVGSheets site: one compiled engine, the page, the renderer, the
 * fonts and a sample workbook — static files any server can serve.
 *
 *   node scripts/build.mjs [--ranger DIR] [--evgui DIR] [--out DIR] [--no-minify]
 *
 * The app is Ranger source that expects to sit at gallery/evgsheets/ in a
 * Ranger checkout, beside gallery/datagrid (the core) and gallery/evgui (the
 * EVGUI controllers), and is compiled by Ranger's committed compiler
 * (dist/rgrc.js). This script puts it there:
 *
 *   Ranger    --ranger, $RANGER_DIR, the enclosing checkout when this
 *             repository sits at gallery/evgsheets, or ../Ranger.
 *   EVGUI     --evgui, $EVGUI_DIR, <ranger>/gallery/evgui when present, or
 *             ../EVGUI; cloned from GitHub when none of those exist.
 *   lib/evg   fetched by Ranger's own `scripts/deps.mjs` at the commit its
 *             ranger.json pins.
 *
 * Output (default dist/): index.html, embed.html, evgsheets.mjs (the mount
 * API), evgsheets.js (the compiled engine), sheets.css, gl/, fonts/ and
 * business-workbook.xlsx.
 */
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { sanitizeFont } from "./webfonts.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, "..");
const argv = process.argv.slice(2);
const flag = (name) => {
  const at = argv.indexOf(name);
  return at >= 0 ? argv[at + 1] : undefined;
};

const inside = fs.existsSync(path.join(REPO, "..", "..", "dist", "rgrc.js"));
const RANGER = path.resolve(
  flag("--ranger") || process.env.RANGER_DIR || (inside ? path.join(REPO, "..", "..") : path.join(REPO, "..", "Ranger")),
);
const OUT = path.resolve(flag("--out") || path.join(REPO, "dist"));
const minify = !argv.includes("--no-minify");

function die(msg) {
  console.error(msg);
  process.exit(1);
}
function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { stdio: "inherit", ...opts });
  if (r.status !== 0) die(`${cmd} ${args.join(" ")} failed`);
}
function copyTree(src, dst) {
  fs.mkdirSync(dst, { recursive: true });
  for (const e of fs.readdirSync(src, { withFileTypes: true })) {
    if (e.name === "node_modules" || e.name === ".git" || e.name === "bin" || e.name === "dist") continue;
    const s = path.join(src, e.name);
    const d = path.join(dst, e.name);
    if (e.isDirectory()) copyTree(s, d);
    else fs.copyFileSync(s, d);
  }
}
const same = (a, b) => fs.existsSync(a) && fs.existsSync(b) && fs.realpathSync(a) === fs.realpathSync(b);

if (!fs.existsSync(path.join(RANGER, "dist", "rgrc.js"))) {
  die(`not a Ranger checkout: ${RANGER} (pass --ranger <dir> or set RANGER_DIR)`);
}

// --- lib/evg ------------------------------------------------------------------
if (fs.existsSync(path.join(RANGER, "scripts", "deps.mjs"))) {
  run(process.execPath, [path.join(RANGER, "scripts", "deps.mjs")], { cwd: RANGER });
}

// --- EVGUI --------------------------------------------------------------------
const EVGUI_TARGET = path.join(RANGER, "gallery", "evgui");
let evgui = flag("--evgui") || process.env.EVGUI_DIR;
if (!evgui && !fs.existsSync(path.join(EVGUI_TARGET, "src", "UiHost.rgr"))) {
  const sibling = path.join(REPO, "..", "EVGUI");
  if (fs.existsSync(path.join(sibling, "src", "UiHost.rgr"))) evgui = sibling;
}
if (evgui) {
  evgui = path.resolve(evgui);
  if (!same(evgui, EVGUI_TARGET)) {
    copyTree(path.join(evgui, "src"), path.join(EVGUI_TARGET, "src"));
    fs.copyFileSync(path.join(evgui, "ranger.json"), path.join(EVGUI_TARGET, "ranger.json"));
  }
} else if (!fs.existsSync(path.join(EVGUI_TARGET, "src", "UiHost.rgr"))) {
  const ref = process.env.EVGUI_REF || "main";
  run("git", ["clone", "--depth", "1", "--branch", ref, "https://github.com/terotests/EVGUI", EVGUI_TARGET]);
}

// --- this repository, at gallery/evgsheets -----------------------------------
const TARGET = path.join(RANGER, "gallery", "evgsheets");
if (!same(REPO, TARGET)) {
  copyTree(path.join(REPO, "src"), path.join(TARGET, "src"));
  fs.copyFileSync(path.join(REPO, "ranger.json"), path.join(TARGET, "ranger.json"));
}

// --- compile ------------------------------------------------------------------
fs.mkdirSync(OUT, { recursive: true });
const bundle = path.join(OUT, "evgsheets.js");
fs.rmSync(bundle, { force: true });
const t0 = Date.now();
const r = spawnSync(
  process.execPath,
  ["dist/rgrc.js", "-es6", "gallery/evgsheets/src/SheetsApp.rgr", `-d=${OUT}`, "-o=evgsheets.js"],
  { cwd: RANGER, env: { ...process.env, RANGER_LIB: "./compiler/Lang.rgr:./lib/stdops.rgr" }, encoding: "utf8" },
);
const log = (r.stdout || "") + (r.stderr || "");
if (/Compilation FAILED/.test(log) || !fs.existsSync(bundle)) {
  console.error(log.split("\n").filter((l) => /FAIL|│|\^/.test(l)).slice(0, 60).join("\n"));
  die("could not compile gallery/evgsheets/src/SheetsApp.rgr");
}
console.log(`  compiled in ${((Date.now() - t0) / 1000).toFixed(1)}s`);

// One global, so the page and any other script on it do not collide. The
// measurer classes go out with the app: the canvas text measurer is installed
// into THIS bundle's copy of them.
let src = fs.readFileSync(bundle, "utf8");
src =
  "// EVGSheets engine — compiled from Ranger source (github.com/terotests/EVGSheets).\n" +
  "(function () {\n" + src +
  "\n;globalThis.EVGSheetsModule = { SheetsApp: SheetsApp, EVGHostTextMeasurer: EVGHostTextMeasurer, EVGDefaultMeasurer: EVGDefaultMeasurer };\n})();\n";
fs.writeFileSync(bundle, src);

// Loadable by a browser? Nothing may reach for the file system while loading.
{
  const probe = new Function("globalThis", "require", src + "\n;return typeof globalThis.EVGSheetsModule.SheetsApp;");
  const g = {};
  if (probe(g, undefined) !== "function") die("evgsheets.js does not publish EVGSheetsModule.SheetsApp");
}

if (minify) {
  let esbuild = null;
  for (const from of [REPO, RANGER, path.join(RANGER, "gallery", "ui", "conformance", "dom")]) {
    try {
      esbuild = createRequire(path.join(from, "package.json"))("esbuild");
      break;
    } catch (_) {
      /* next */
    }
  }
  if (esbuild) {
    const before = fs.statSync(bundle).size;
    const out = esbuild.transformSync(fs.readFileSync(bundle, "utf8"), { minify: true, target: "es2020" });
    fs.writeFileSync(bundle, out.code);
    console.log(`  minified ${(before / 1e6).toFixed(2)} MB → ${(out.code.length / 1e6).toFixed(2)} MB`);
  } else {
    console.log("  (no esbuild — shipping the unminified engine; npm install in this repository adds it)");
  }
}

// --- the page and its assets ------------------------------------------------
for (const f of ["index.html", "embed.html", "evgsheets.mjs"]) fs.copyFileSync(path.join(REPO, "web", f), path.join(OUT, f));
fs.copyFileSync(path.join(REPO, "src", "sheets.css"), path.join(OUT, "sheets.css"));
fs.mkdirSync(path.join(OUT, "gl"), { recursive: true });
for (const f of ["evg-webgl.js", "evg-a11y.js", "evg-measure.js"]) {
  fs.copyFileSync(path.join(RANGER, "lib", "evg", "gl", f), path.join(OUT, "gl", f));
}
const FONT_SRC = path.join(RANGER, "gallery", "pdf_writer", "assets", "fonts");
const FONTS = [
  "Open_Sans/OpenSans-Regular.ttf", "Open_Sans/OpenSans-Bold.ttf", "Open_Sans/OpenSans-Italic.ttf", "Open_Sans/OpenSans-BoldItalic.ttf",
  "Noto_Sans/NotoSans-Regular.ttf", "Noto_Sans/NotoSans-Bold.ttf", "Noto_Sans/NotoSans-Italic.ttf", "Noto_Sans/NotoSans-BoldItalic.ttf",
  "Helvetica/Helvetica.ttf", "Droid_Serif/DroidSerif.ttf", "Droid_Serif/DroidSerif-Bold.ttf",
  "Josefin_Sans/JosefinSans-Regular.ttf", "Josefin_Sans/JosefinSans-Bold.ttf",
  "Noto_Emoji/NotoEmoji-Regular.ttf", "El_Messiri/ElMessiri-Regular.ttf",
];
fs.mkdirSync(path.join(OUT, "fonts"), { recursive: true });
// Cleaned so the browser's font sanitizer has nothing to report (webfonts.mjs).
for (const f of FONTS) {
  fs.writeFileSync(path.join(OUT, "fonts", path.basename(f)), sanitizeFont(fs.readFileSync(path.join(FONT_SRC, f))));
}
fs.copyFileSync(path.join(RANGER, "gallery", "datagrid", "fixtures", "business-workbook.xlsx"), path.join(OUT, "business-workbook.xlsx"));
fs.writeFileSync(path.join(OUT, ".nojekyll"), "");

// --- the build stamp --------------------------------------------------------
// Every URL the page loads carries ?v=<hash>, so a rebuilt page is fetched
// rather than taken from a cache that still holds the last one.
const h = crypto.createHash("sha1");
for (const f of ["evgsheets.js", "evgsheets.mjs", "sheets.css", "gl/evg-webgl.js", "gl/evg-a11y.js", "gl/evg-measure.js"]) {
  h.update(fs.readFileSync(path.join(OUT, f)));
}
const STAMP = h.digest("hex").slice(0, 10);
for (const f of ["index.html", "embed.html"]) {
  const p = path.join(OUT, f);
  fs.writeFileSync(p, fs.readFileSync(p, "utf8").split("__BUILD__").join(STAMP));
}
console.log(`  ${OUT}`);
console.log(`build ${STAMP}`);
