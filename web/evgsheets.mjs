// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * EVGSheets in a page: one call puts the editor into an element.
 *
 *   import { mountSheets } from "./evgsheets.mjs";
 *   const sheet = await mountSheets(document.getElementById("here"), {
 *     ui: "full",                 // "full" | "compact" | "viewer" | "grid"
 *     tools: "bold,italic,sortasc", // ribbon tools to keep ("" = all)
 *     theme: "light",             // "light" | "dark"
 *     readOnly: false,
 *     formulaBar: true, tabs: true, status: true, menubar: true, ribbon: true,
 *     xlsx: "book.xlsx",          // a URL, an ArrayBuffer, or nothing (demo sheet)
 *     name: "book.xlsx",
 *     sheet: "Summary",           // which sheet to show first
 *     base: "./",                 // where gl/, fonts/, evgsheets.js and sheets.css are
 *   });
 *   sheet.setOption("readOnly", "true");
 *   const bytes = sheet.saveBytes();  // the workbook as .xlsx
 *
 * Everything is drawn by EVG into one <canvas> through WebGL 2: the EVGUI
 * chrome (menubar, ribbon, tabs, status) and the core grid inside it. A screen
 * reader gets the app's own accessibility tree mirrored as DOM over the canvas
 * (gl/evg-a11y.js), so the canvas is not one silent picture.
 *
 * Several editors on one page are fine: each mount has its own canvas, its own
 * SheetsApp and its own keyboard. The font bytes are fetched once and shared.
 */

const FONTS_FULL = [
  ["Open Sans", "OpenSans-Regular.ttf", { family: "Open Sans", weight: "400", style: "normal" }],
  [null, "OpenSans-Bold.ttf", { family: "Open Sans", weight: "700", style: "normal" }],
  [null, "OpenSans-Italic.ttf", { family: "Open Sans", weight: "400", style: "italic" }],
  [null, "OpenSans-BoldItalic.ttf", { family: "Open Sans", weight: "700", style: "italic" }],
  ["Noto Sans", "NotoSans-Regular.ttf", { family: "Noto Sans", weight: "400", style: "normal" }],
  [null, "NotoSans-Bold.ttf", { family: "Noto Sans", weight: "700", style: "normal" }],
  [null, "NotoSans-Italic.ttf", { family: "Noto Sans", weight: "400", style: "italic" }],
  [null, "NotoSans-BoldItalic.ttf", { family: "Noto Sans", weight: "700", style: "italic" }],
  ["Helvetica", "Helvetica.ttf", { family: "Helvetica", weight: "400", style: "normal" }],
  ["Droid Serif", "DroidSerif.ttf", { family: "Droid Serif", weight: "400", style: "normal" }],
  [null, "DroidSerif-Bold.ttf", { family: "Droid Serif", weight: "700", style: "normal" }],
  ["Josefin Sans", "JosefinSans-Regular.ttf", { family: "Josefin Sans", weight: "400", style: "normal" }],
  [null, "JosefinSans-Bold.ttf", { family: "Josefin Sans", weight: "700", style: "normal" }],
  [null, "NotoEmoji-Regular.ttf", { family: "Noto Emoji", weight: "400", style: "normal" }],
  [null, "ElMessiri-Regular.ttf", { family: "El Messiri", weight: "400", style: "normal" }],
];
// A light viewer does not need every face: the default family in four cuts.
const FONTS_MINIMAL = FONTS_FULL.slice(0, 4);

const KEY_NAMES = new Set([
  "Backspace", "Enter", "Tab", "Escape", "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown",
  "Delete", "Home", "End", "PageUp", "PageDown", "F2", "F6", "F10",
]);
// Ctrl/Cmd chords the sheet answers. Also what keeps Ctrl+S from saving the
// web page and Ctrl+F from opening the browser's find bar while it has focus.
const CTRL_CHORD = /^[abcdefhiklmsuxyzABCDEFHIKLMSUXYZ ]$/;

const shared = new Map(); // base → { ready: Promise<{mod, gl, a11y, faces}> }

function asRangerBuffer(ab) {
  ab._view = new DataView(ab);
  return ab;
}

async function fetchBytes(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(url + " → " + res.status);
  return asRangerBuffer(await res.arrayBuffer());
}

function loadScript(src) {
  return new Promise((resolve, reject) => {
    const s = document.createElement("script");
    s.src = src;
    s.onload = resolve;
    s.onerror = () => reject(new Error("could not load " + src));
    document.head.appendChild(s);
  });
}

/** The engine, the renderer, the stylesheet and the fonts — once per base. */
function loadShared(base, fontSet) {
  const key = base + "|" + fontSet;
  if (shared.has(key)) return shared.get(key);
  const p = (async () => {
    const stamp = globalThis.EVGSHEETS_BUILD ? "?v=" + globalThis.EVGSHEETS_BUILD : "";
    if (!globalThis.EVGSheetsModule) await loadScript(base + "evgsheets.js" + stamp);
    const mod = globalThis.EVGSheetsModule;
    const gl = await import(new URL(base + "gl/evg-webgl.js" + stamp, location.href).href);
    const a11y = await import(new URL(base + "gl/evg-a11y.js" + stamp, location.href).href);
    const measure = await import(new URL(base + "gl/evg-measure.js" + stamp, location.href).href);
    const css = await (await fetch(base + "sheets.css" + stamp)).text();
    const list = fontSet === "minimal" ? FONTS_MINIMAL : FONTS_FULL;
    const faces = await Promise.all(list.map(([, file]) => fetchBytes(base + "fonts/" + file)));
    // The browser draws the canvas text, so it has to know the same faces the
    // core measured with — or the caret lands beside the glyphs.
    if (typeof FontFace === "function" && document.fonts) {
      await Promise.all(list.map(async ([, file, cssFace], i) => {
        try {
          const face = new FontFace(cssFace.family, faces[i].slice(0), { weight: cssFace.weight, style: cssFace.style });
          await face.load();
          document.fonts.add(face);
        } catch (e) {
          console.warn("evgsheets: could not register " + file, e);
        }
      }));
    }
    gl.setFontFallback([...new Set(list.map(([, , c]) => c.family))]);
    await document.fonts.ready;
    // The chrome's layout measures with the browser's own text metrics.
    const measurer = measure.installCanvasMeasurer([mod]);
    measurer.refresh();
    return { mod, gl, a11y, css, list, faces };
  })();
  shared.set(key, p);
  return p;
}

function boolOpt(v) {
  if (v === undefined || v === null) return undefined;
  if (typeof v === "boolean") return v;
  return /^(1|true|yes|on)$/i.test(String(v));
}

/** Options from a query string: ?ui=viewer&tools=bold,italic&theme=dark… */
export function optionsFromQuery(search = location.search) {
  const q = new URLSearchParams(search);
  const o = {};
  for (const k of ["ui", "tools", "theme", "xlsx", "name", "sheet"]) if (q.has(k)) o[k] = q.get(k);
  for (const k of ["readOnly", "formulaBar", "tabs", "status", "menubar", "ribbon", "title", "a11y"]) {
    const v = q.get(k) ?? q.get(k.toLowerCase());
    if (v !== null) o[k] = boolOpt(v);
  }
  if (q.has("readonly")) o.readOnly = boolOpt(q.get("readonly"));
  return o;
}

export async function mountSheets(container, options = {}) {
  const base = options.base || "./";
  const fontSet = options.fonts || (options.ui === "viewer" || options.ui === "grid" ? "minimal" : "full");
  const { mod, gl: glMod, a11y: a11yMod, css, list, faces } = await loadShared(base, fontSet);

  container.classList.add("evgsheets-host");
  const wrap = document.createElement("div");
  wrap.className = "evgsheets-wrap";
  wrap.style.cssText = "position:relative;width:100%;height:100%;overflow:hidden;outline:none";
  const canvas = document.createElement("canvas");
  canvas.tabIndex = 0;
  canvas.setAttribute("aria-label", options.label || "Spreadsheet");
  canvas.style.cssText = "display:block;width:100%;height:100%;outline:none;touch-action:none";
  wrap.appendChild(canvas);
  container.appendChild(wrap);

  const gl = canvas.getContext("webgl2", { antialias: true, premultipliedAlpha: false, stencil: true });
  if (!gl) throw new Error("EVGSheets needs WebGL 2");

  const sizeOf = () => {
    const r = wrap.getBoundingClientRect();
    return { w: Math.max(240, Math.floor(r.width)), h: Math.max(160, Math.floor(r.height)) };
  };
  let { w, h } = sizeOf();
  const app = new mod.SheetsApp();
  app.start(w, h, css);
  list.forEach(([family], i) => {
    if (family) app.addFont(family, faces[i]);
    else app.addFace(faces[i]);
  });
  if (options.ui) app.setPreset(options.ui);
  const opt = (name, v) => {
    if (v === undefined || v === null) return;
    app.setOption(name, typeof v === "boolean" ? String(v) : String(v));
  };
  opt("menubar", options.menubar);
  opt("ribbon", options.ribbon);
  opt("formulaBar", options.formulaBar);
  opt("tabs", options.tabs);
  opt("status", options.status);
  opt("title", options.title);
  opt("readOnly", options.readOnly);
  if (options.tools !== undefined) opt("tools", options.tools);
  if (options.name) opt("name", options.name);
  if (options.theme) app.setTheme(options.theme);
  else if (options.theme !== false && window.matchMedia?.("(prefers-color-scheme: dark)").matches) app.setTheme("dark");

  // --- the workbook ---------------------------------------------------------
  let note = "";
  async function openBytes(raw, name) {
    const ok = app.openWorkbook(asRangerBuffer(raw), name || "workbook.xlsx");
    note = ok ? "" : app.grid.note;
    clearImages();
    await draw(true);
    return ok;
  }
  if (options.xlsx instanceof ArrayBuffer) {
    // Opened here, before the drawing below exists; openBytes is for later.
    if (!app.openWorkbook(asRangerBuffer(options.xlsx.slice(0)), options.name || "workbook.xlsx")) {
      note = app.grid.note;
      app.useDemo();
    }
  } else if (typeof options.xlsx === "string" && options.xlsx) {
    try {
      const raw = await (await fetch(options.xlsx)).arrayBuffer();
      if (!app.openWorkbook(asRangerBuffer(raw), options.name || options.xlsx.split("/").pop())) app.useDemo();
    } catch (e) {
      console.warn("evgsheets: could not open " + options.xlsx, e);
      app.useDemo();
    }
  } else {
    app.useDemo();
  }
  if (options.sheet) app.showSheet(options.sheet);

  // --- pictures in the sheet ------------------------------------------------
  const imageCache = new Map();
  const blobUrls = new Map();
  function clearImages() {
    imageCache.clear();
    for (const url of blobUrls.values()) if (url) URL.revokeObjectURL(url);
    blobUrls.clear();
  }
  function mediaUrl(part) {
    if (blobUrls.has(part)) return blobUrls.get(part);
    const book = app.grid.app.book;
    for (let s = 0; s < book.sheetCount(); s += 1) {
      const sheet = book.sheetAt(s);
      for (let i = 0; i < sheet.imageCount(); i += 1) {
        const img = sheet.imageAt(i);
        if (img.src !== part || !img.bytes) continue;
        const view = img.bytes instanceof ArrayBuffer ? new Uint8Array(img.bytes) : img.bytes;
        const url = URL.createObjectURL(new Blob([view], { type: part.endsWith(".png") ? "image/png" : "image/jpeg" }));
        blobUrls.set(part, url);
        return url;
      }
    }
    blobUrls.set(part, "");
    return "";
  }
  async function imagesFor(doc) {
    const wanted = new Set(doc.list.cmds.filter((c) => c.k === 2 && c.src).map((c) => c.src));
    for (const src of wanted) {
      if (imageCache.has(src)) continue;
      const url = mediaUrl(src);
      if (!url) { imageCache.set(src, null); continue; }
      const fetched = await glMod.loadImages({ list: { cmds: [{ k: 2, src: url }] } }, { base: "" });
      imageCache.set(src, fetched.get(url) || null);
    }
    const out = new Map();
    for (const src of wanted) out.set(src, imageCache.get(src) || null);
    return out;
  }

  // --- the accessibility mirror ---------------------------------------------
  const mirror = options.a11y === false ? null : a11yMod.createA11yMirror(wrap, {
    canvas,
    label: options.label || "Spreadsheet",
    onActivate: async (node) => {
      a11yMod.pressAtCentre(node, (x, y) => {
        app.pointer(x, y, true, false, false);
        app.pointer(x, y, false, false, false);
      });
      await after();
    },
  });

  // --- drawing --------------------------------------------------------------
  let lastScene = "";
  let destroyed = false;
  let frames = 0;
  async function draw(force) {
    if (destroyed) return;
    const text = app.scene();
    if (!force && text === lastScene) return;
    lastScene = text;
    const doc = JSON.parse(text);
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    const bw = Math.round(doc.width * dpr);
    const bh = Math.round(doc.height * dpr);
    if (canvas.width !== bw || canvas.height !== bh) {
      canvas.width = bw;
      canvas.height = bh;
    }
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clearColor(1, 1, 1, 1);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT | gl.STENCIL_BUFFER_BIT);
    glMod.renderDisplayList(gl, doc, { dpr, images: await imagesFor(doc) });
    frames += 1;
    if (mirror) {
      try {
        mirror.update(JSON.parse(app.a11y()));
      } catch (e) {
        console.warn("evgsheets a11y:", e);
      }
    }
    controller.lastDoc = doc;
    // A style change starts transitions (hover, a theme switch); they only
    // move while something ticks the clock.
    if (app.tick(0)) animate();
  }

  // --- files ------------------------------------------------------------------
  const fileInput = document.createElement("input");
  fileInput.type = "file";
  fileInput.accept = ".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
  fileInput.style.display = "none";
  wrap.appendChild(fileInput);
  fileInput.addEventListener("change", async () => {
    const f = fileInput.files && fileInput.files[0];
    if (!f) return;
    await openBytes(await f.arrayBuffer(), f.name);
    fileInput.value = "";
  });

  function saveBlob(blob, name) {
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    a.style.display = "none";
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  function serveFileRequest() {
    const want = app.takeFileRequest();
    if (!want) return;
    if (want === "open") {
      if (options.onOpenRequest) options.onOpenRequest(controller);
      else fileInput.click();
      return;
    }
    if (want === "saveAs") {
      const raw = app.grid.saveWorkbookBytes();
      const name = app.grid.suggestSaveName() || "workbook.xlsx";
      if (options.onSave) options.onSave(raw, name, controller);
      else saveBlob(new Blob([raw], { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" }), name);
      app.grid.markSavedAs(name);
    }
  }

  // --- the clipboard ----------------------------------------------------------
  let lastCopy = app.copySeq() | 0;
  async function maybeCopy() {
    const seq = app.copySeq() | 0;
    if (seq === lastCopy) return;
    lastCopy = seq;
    const g = app.grid.app;
    const text = g.clipboardTsv || "";
    const html = g.clipboardHtml || "";
    try {
      if (html && typeof ClipboardItem === "function" && navigator.clipboard?.write) {
        await navigator.clipboard.write([new ClipboardItem({
          "text/plain": new Blob([text], { type: "text/plain" }),
          "text/html": new Blob([html], { type: "text/html" }),
        })]);
      } else {
        await navigator.clipboard.writeText(text);
      }
    } catch (_) {
      /* no permission: the sheet's own clipboard still works */
    }
  }

  let changeTimer = 0;
  async function after() {
    serveFileRequest();
    await draw();
    await maybeCopy();
    if (options.onChange) {
      clearTimeout(changeTimer);
      changeTimer = setTimeout(() => options.onChange(controller), 120);
    }
    animate();
  }

  // Submenus open on a delay and transitions run on a clock: keep ticking
  // while the controllers say something is still moving.
  let ticking = false;
  function animate() {
    if (ticking) return;
    ticking = true;
    let t0 = performance.now();
    const step = async (now) => {
      const busy = app.tick(now - t0);
      t0 = now;
      if (app.coasting()) app.idle();
      await draw();
      if ((busy || app.coasting()) && !destroyed) requestAnimationFrame(step);
      else ticking = false;
    };
    requestAnimationFrame(step);
  }

  // --- input --------------------------------------------------------------------
  const at = (ev) => {
    const r = canvas.getBoundingClientRect();
    return { x: Math.floor(ev.clientX - r.left), y: Math.floor(ev.clientY - r.top) };
  };
  let down = false;
  let lastCursor = "";
  canvas.addEventListener("pointerdown", async (ev) => {
    if (ev.button === 2) return;
    canvas.setPointerCapture(ev.pointerId);
    if (!mirror) canvas.focus();
    down = true;
    const { x, y } = at(ev);
    app.pointer(x, y, true, ev.shiftKey, ev.ctrlKey || ev.metaKey);
    await after();
  });
  canvas.addEventListener("pointermove", async (ev) => {
    const { x, y } = at(ev);
    const want = app.cursorAt(x, y);
    if (want !== lastCursor) {
      canvas.style.cursor = want;
      lastCursor = want;
    }
    app.pointer(x, y, down, ev.shiftKey, ev.ctrlKey || ev.metaKey);
    await after();
  });
  const release = async (ev) => {
    if (!down) return;
    down = false;
    const { x, y } = at(ev);
    app.pointer(x, y, false, ev.shiftKey, ev.ctrlKey || ev.metaKey);
    await after();
  };
  canvas.addEventListener("pointerup", release);
  canvas.addEventListener("pointercancel", release);
  canvas.addEventListener("contextmenu", async (ev) => {
    ev.preventDefault();
    const { x, y } = at(ev);
    app.rightClick(x, y);
    await after();
  });
  canvas.addEventListener("dblclick", async (ev) => {
    const { x, y } = at(ev);
    app.doubleClick(x, y);
    await after();
  });
  canvas.addEventListener("wheel", async (ev) => {
    ev.preventDefault();
    const { x, y } = at(ev);
    const horizontal = ev.shiftKey || Math.abs(ev.deltaX) > Math.abs(ev.deltaY);
    const raw = horizontal ? (ev.deltaX || ev.deltaY) : ev.deltaY;
    if (horizontal) app.wheelX(x, y, raw < 0 ? 1 : -1);
    else app.wheel(x, y, raw < 0 ? 1 : -1);
    await after();
  }, { passive: false });

  // Keys are taken on the wrapper: with the mirror up, focus is on one of its
  // DOM nodes and the canvas never sees the event.
  wrap.addEventListener("keydown", async (ev) => {
    if (options.onKey && options.onKey(ev, controller) === false) return;
    if (KEY_NAMES.has(ev.key)) {
      // Tab inside the sheet moves to the next cell; it leaves the editor only
      // from the chrome, where F6 is the way between regions.
      ev.preventDefault();
      app.key(ev.key, ev.shiftKey, ev.ctrlKey || ev.metaKey);
      await after();
      return;
    }
    if (ev.ctrlKey || ev.metaKey) {
      if (ev.key === "v" || ev.key === "V") {
        // Served by the paste event, which does not say whether Shift was down.
        pasteValues = ev.shiftKey;
        return;
      }
      if (CTRL_CHORD.test(ev.key)) {
        ev.preventDefault();
        app.text(ev.key, ev.shiftKey, true);
        await after();
      }
      return;
    }
    if (ev.key.length === 1) {
      ev.preventDefault();
      if (ev.key === " " && !app.gridFocused) app.key(" ", false, false);
      else app.text(ev.key, ev.shiftKey, false);
      await after();
    }
  });
  let pasteValues = false;
  const onPaste = async (ev) => {
    if (!wrap.contains(document.activeElement)) return;
    ev.preventDefault();
    const text = ev.clipboardData ? ev.clipboardData.getData("text/plain") : "";
    if (pasteValues) app.pasteTextValues(text);
    else app.pasteText(text);
    pasteValues = false;
    await after();
  };
  window.addEventListener("paste", onPaste);

  const ro = new ResizeObserver(async () => {
    const s = sizeOf();
    if (s.w === w && s.h === h) return;
    w = s.w;
    h = s.h;
    app.resize(w, h);
    await draw(true);
  });
  ro.observe(wrap);
  const blink = setInterval(() => draw(), 1000);

  const controller = {
    app,
    canvas,
    element: wrap,
    lastDoc: null,
    get note() { return note; },
    open: openBytes,
    async openUrl(url, name) {
      const raw = await (await fetch(url)).arrayBuffer();
      return openBytes(raw, name || url.split("/").pop());
    },
    saveBytes: () => app.grid.saveWorkbookBytes(),
    download() {
      app.grid.run("file.saveAs", "");
      serveFileRequest();
    },
    state: () => JSON.parse(app.state()),
    async setPreset(name) { app.setPreset(name); await draw(true); },
    async setOption(name, value) { app.setOption(name, String(value)); await draw(true); },
    async setTheme(name) { app.setTheme(name); await draw(true); },
    async run(id, arg = "") { const ok = app.grid.run(id, arg); app.sync(); await after(); return ok; },
    async press(tid) { const ok = app.pressId(tid); await after(); return ok; },
    async key(name, shift = false, ctrl = false) { app.key(name, shift, ctrl); await after(); },
    async type(text) { for (const ch of text) app.text(ch, false, false); await after(); },
    focus() { canvas.focus(); },
    redraw: () => draw(true),
    frames: () => frames,
    destroy() {
      destroyed = true;
      clearInterval(blink);
      ro.disconnect();
      window.removeEventListener("paste", onPaste);
      clearImages();
      wrap.remove();
    },
  };
  await draw(true);
  return controller;
}
