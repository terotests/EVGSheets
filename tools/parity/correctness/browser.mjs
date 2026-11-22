// SPDX-License-Identifier: AGPL-3.0-or-later
// The built editor (dist/) in headless Chromium, for the correctness checks.
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

export const HERE = path.dirname(fileURLToPath(import.meta.url));
export const REPO = path.resolve(HERE, "..", "..", "..");

function loadPlaywright() {
  for (const from of [REPO, process.cwd(), "/opt/node-tools"]) {
    try {
      return createRequire(path.join(from, "package.json"))("playwright-core");
    } catch (_) {
      /* next */
    }
  }
  throw new Error("playwright-core not found: npm install in this repository");
}
const chromePath = () => [process.env.CHROME_PATH, "/opt/pw-browsers/chromium"].filter(Boolean).find((p) => fs.existsSync(p));
const MIME = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".ttf": "font/ttf" };

export async function openEditor({ dist = path.join(REPO, "dist"), port = 8941 } = {}) {
  const DIST = path.resolve(dist);
  const server = http.createServer((req, res) => {
    const url = decodeURIComponent(req.url.split("?")[0]);
    const file = path.join(DIST, url.endsWith("/") ? url + "index.html" : url);
    if (!file.startsWith(DIST) || !fs.existsSync(file)) {
      res.writeHead(404);
      res.end("not found");
      return;
    }
    res.writeHead(200, { "content-type": MIME[path.extname(file)] || "application/octet-stream" });
    fs.createReadStream(file).pipe(res);
  });
  await new Promise((r) => server.listen(port, r));
  const { chromium } = loadPlaywright();
  const browser = await chromium.launch({
    executablePath: chromePath(),
    args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
  });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`http://localhost:${port}/`);
  await page.waitForFunction(() => window.__sheetsReady || window.__sheetsError, null, { timeout: 60000 });
  return {
    page,
    errors,
    async openFile(file) {
      const b64 = fs.readFileSync(file).toString("base64");
      await page.evaluate(async ([b, n]) => {
        const raw = Uint8Array.from(atob(b), (c) => c.charCodeAt(0));
        await window.sheet.open(raw.buffer, n);
      }, [b64, path.basename(file)]);
    },
    async close() {
      await browser.close();
      server.close();
    },
  };
}
