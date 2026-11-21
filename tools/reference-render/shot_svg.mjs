// Rasterise an SVG file in headless Chromium (cross-check for resvg).
// Usage: node shot_svg.mjs in.svg out.png width height
import { chromium } from 'playwright';
import { readFileSync } from 'node:fs';
const [inp, out, w, h] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: +w, height: +h } });
await page.setContent(`<html><body style="margin:0">${readFileSync(inp, 'utf8')}</body></html>`);
await page.screenshot({ path: out, omitBackground: true });
await browser.close();
