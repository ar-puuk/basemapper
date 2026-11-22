// Rasterise an SVG file in headless Chromium (cross-check for resvg).
// Usage: node shot_svg.mjs in.svg out.png width height
import { chromium } from 'playwright';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const [inp, out, w, h] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: +w, height: +h } });
// Load the file itself (inlining multi-MB SVGs into the page is fragile).
await page.goto(pathToFileURL(resolve(inp)).href);
await page.screenshot({ path: out, omitBackground: true, timeout: 120000 });
await browser.close();
