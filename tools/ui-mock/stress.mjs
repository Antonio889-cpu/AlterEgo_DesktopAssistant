// Stress test delle finestre: le apre con il finto __TAURI__ a molte dimensioni, con testi lunghi
// e liste enormi, e controlla che niente esca dalla pagina in orizzontale. Salva gli screenshot.
//   node tools/ui-mock/stress.mjs [pagina.html …]     (Playwright e Chromium già installati)
// Uscita: tools/ui-mock/out/<pagina>-<larghezza>x<altezza>[-variante].png e un riepilogo.
import { createRequire } from "module";
import fs from "fs";
import path from "path";
import http from "http";
import { fileURLToPath } from "url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");
const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../../src");
const out = path.join(here, "out");
fs.mkdirSync(out, { recursive: true });

const MOCK = fs.readFileSync(path.join(here, "tauri-mock.js"), "utf8");
const CONFIG = fs.readFileSync(path.join(here, "config.json"), "utf8");

// server statico per src/ (fetch e font funzionano come nell'app)
const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".png": "image/png", ".jpg": "image/jpeg", ".ttf": "font/ttf", ".svg": "image/svg+xml" };
const server = http.createServer((req, res) => {
  const p = path.join(root, decodeURIComponent(new URL(req.url, "http://x").pathname));
  if (!p.startsWith(root) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) { res.writeHead(404); return res.end(); }
  res.writeHead(200, { "content-type": types[path.extname(p)] || "application/octet-stream" });
  fs.createReadStream(p).pipe(res);
}).listen(0);
const base = `http://127.0.0.1:${server.address().port}/`;

const pages = process.argv.slice(2).length ? process.argv.slice(2) : ["settings.html"];
const SIZES = [[560, 480], [760, 720], [1024, 640], [1280, 720], [1600, 900], [1920, 1080], [2560, 1440], [3840, 2160]];
const VARIANTS = [["", ""], ["en", "lang=en"], ["long", "long&many"], ["zoom", "zoom"]];

const browser = await chromium.launch();
const problems = [];
let shots = 0;
for (const page of pages) {
  for (const [w, h] of SIZES) {
    for (const [vname, query] of VARIANTS) {
      if (vname === "zoom" && w < 1280) continue;
      const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 1 });
      const pg = await ctx.newPage();
      const errors = [];
      pg.on("pageerror", (e) => errors.push(String(e)));
      await pg.addInitScript(`window.__AE_CONFIG__ = ${CONFIG};\n${MOCK}`);
      await pg.goto(base + page + (query && query !== "zoom" ? "?" + query : ""));
      if (query === "zoom") await pg.evaluate(() => { document.body.style.zoom = "2"; });
      await pg.waitForTimeout(900);
      const tabs = await pg.$$eval("[data-tab]", (els) => [...new Set(els.map((e) => e.dataset.tab))]);
      for (const tab of tabs.length ? tabs : [""]) {
        if (tab) await pg.evaluate((t) => window.showTab && window.showTab(t), tab);
        await pg.waitForTimeout(450);
        const over = await pg.evaluate(() => {
          const vw = document.documentElement.clientWidth;
          const bad = [];
          if (document.documentElement.scrollWidth > vw + 1) bad.push(`pagina larga ${document.documentElement.scrollWidth}px > ${vw}px`);
          for (const el of document.querySelectorAll("body *")) {
            if (!el.offsetParent && getComputedStyle(el).position !== "fixed") continue;
            const r = el.getBoundingClientRect();
            let clipped = false;
            for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
              if (["auto", "scroll", "hidden"].includes(getComputedStyle(a).overflowX)) { clipped = a.getBoundingClientRect().right <= vw + 1; break; }
            }
            if (r.width && r.right > vw + 1 && !clipped) bad.push(`${el.tagName.toLowerCase()}${el.id ? "#" + el.id : ""}${el.className && typeof el.className === "string" ? "." + el.className.split(" ")[0] : ""} esce di ${Math.round(r.right - vw)}px`);
            if (bad.length > 6) break;
          }
          return bad;
        });
        if (over.length) problems.push(`${page} ${w}x${h} ${vname || "it"} [${tab}]: ${over.join("; ")}`);
        const file = `${path.basename(page, ".html")}-${w}x${h}${vname ? "-" + vname : ""}${tab ? "-" + tab : ""}.png`;
        if (!vname || ["home", "games", "local", "persona"].includes(tab)) { await pg.screenshot({ path: path.join(out, file) }); shots++; }
      }
      if (errors.length) problems.push(`${page} ${w}x${h} ${vname}: errori JS: ${[...new Set(errors)].slice(0, 3).join(" | ")}`);
      await ctx.close();
    }
  }
}
await browser.close();
server.close();
fs.writeFileSync(path.join(out, "report.txt"), problems.join("\n") || "nessun problema");
console.log(`${shots} screenshot in ${out}`);
console.log(problems.length ? `${problems.length} problemi:\n` + problems.slice(0, 60).join("\n") : "nessun problema");
