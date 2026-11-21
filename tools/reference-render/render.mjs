// Render golden fixtures with MapLibre GL JS (headless Chromium) to produce an
// independent reference image for each case: `reference-maplibre-gl.png`.
//
// Usage (from this directory):
//   npm install && npx playwright install chromium
//   node render.mjs              # all cases
//   node render.mjs osm_raster_slc ugrc_litebase_slc
//
// Tiles are served from the fixture directory by intercepting requests to
// http://fixture.local/{z}/{x}/{y}, so no network access is needed. The camera
// is derived from case.json so the output covers exactly the same EPSG:3857
// bbox at the same pixel size as basemapper's render.

import { chromium } from 'playwright';
import { readFileSync, writeFileSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
const goldenRoot = join(here, '../../core/tests/fixtures/golden');
const require = createRequire(import.meta.url);
const mlDist = dirname(require.resolve('maplibre-gl/dist/maplibre-gl.js'));

const PAGE = `<!doctype html><html><head>
<link rel="stylesheet" href="http://assets.local/maplibre-gl.css">
<script src="http://assets.local/maplibre-gl.js"></script>
<style>html,body,#map{margin:0;width:100%;height:100%;background:transparent}</style>
</head><body><div id="map"></div></body></html>`;

const R = 6378137;
const WORLD = 2 * Math.PI * R;

// EPSG:3857 bbox + pixel size -> GL JS center/zoom (GL zoom uses 512 px tiles).
function camera([xmin, ymin, xmax, ymax], width, height) {
  const mpp = (xmax - xmin) / width;
  const mppY = (ymax - ymin) / height;
  if (Math.abs(mpp - mppY) / mpp > 0.01) {
    console.warn(`  warning: bbox aspect != pixel aspect (${mpp} vs ${mppY} m/px)`);
  }
  const cx = (xmin + xmax) / 2;
  const cy = (ymin + ymax) / 2;
  const lng = (cx / R) * (180 / Math.PI);
  const lat = (2 * Math.atan(Math.exp(cy / R)) - Math.PI / 2) * (180 / Math.PI);
  let zoom = Math.log2(WORLD / (512 * mpp));
  // GL picks tiles by floor(zoom): 12.9999 would load z12 tiles. Snap values
  // within 0.001 of an integer (a <0.07% scale change, sub-pixel at 800 px).
  if (Math.abs(zoom - Math.round(zoom)) < 1e-3) zoom = Math.round(zoom);
  return { center: [lng, lat], zoom };
}

function filterLayers(style, layers) {
  if (!layers || layers.length === 0) return style;
  const exclude = layers[0].startsWith('-');
  const ids = new Set(layers.map((l) => (exclude ? l.slice(1) : l)));
  style.layers = style.layers.filter((l) => (exclude ? !ids.has(l.id) : ids.has(l.id)));
  return style;
}

async function renderCase(browser, name) {
  const dir = join(goldenRoot, name);
  const kase = JSON.parse(readFileSync(join(dir, 'case.json'), 'utf8'));
  const fixtureDir = kase.fixture_from ? join(goldenRoot, kase.fixture_from) : dir;
  const styleText = readFileSync(join(fixtureDir, 'style.json'), 'utf8').replaceAll(
    '{{BASE}}',
    'http://fixture.local',
  );
  const style = filterLayers(JSON.parse(styleText), kase.layers);
  const { center, zoom } = camera(kase.bbox, kase.width, kase.height);

  const page = await browser.newPage({
    viewport: { width: kase.width, height: kase.height },
    deviceScaleFactor: 1,
  });
  page.on('pageerror', (e) => console.error(`  page error: ${e.message}`));
  page.on('console', (m) => {
    if (m.type() === 'error' || m.type() === 'warning' || m.text().startsWith('stats')) console.error(`  console: ${m.text()}`);
  });

  let missing = 0;
  await page.route('http://fixture.local/**', (route) => {
    const p = new URL(route.request().url()).pathname.split('/').filter(Boolean);
    const file = join(fixtureDir, 'tiles', ...p);
    if (process.env.DEBUG_TILES) console.error(`  request: ${p.join("/")} exists=${existsSync(file)}`);
    if (existsSync(file)) {
      route.fulfill({ status: 200, body: readFileSync(file) });
    } else {
      missing++;
      console.error(`  missing tile: ${p.join("/")}`);
      route.fulfill({ status: 404, body: '' });
    }
  });
  await page.route('http://assets.local/**', (route) => {
    const f = new URL(route.request().url()).pathname.slice(1);
    if (f === 'index.html') {
      route.fulfill({ status: 200, contentType: 'text/html', body: PAGE });
    } else {
      route.fulfill({ status: 200, body: readFileSync(join(mlDist, f)) });
    }
  });
  await page.goto('http://assets.local/index.html');

  const dataUrl = await page.evaluate(
    ({ style, center, zoom }) =>
      new Promise((resolve, reject) => {
        const map = new maplibregl.Map({
          container: 'map',
          style,
          center,
          zoom,
          interactive: false,
          attributionControl: false,
          fadeDuration: 0,
          canvasContextAttributes: { preserveDrawingBuffer: true, antialias: true },
        });
        map.on('error', (e) => console.error(String(e.error?.message ?? e.error)));
        map.once('idle', () => {
          // A rendered-feature count of 0 on a vector case usually means the
          // fixture lacks the tiles MapLibre requested (see "missing tile").
          const rendered = map.queryRenderedFeatures().length;
          const layers = map.getStyle().layers.length;
          console.log(`stats: zoom=${map.getZoom()} layers=${layers} renderedFeatures=${rendered}`);
          resolve(map.getCanvas().toDataURL('image/png'));
        });
        setTimeout(() => reject(new Error('timed out waiting for idle')), 60000);
      }),
    { style, center, zoom },
  );
  await page.close();

  const out = join(dir, 'reference-maplibre-gl.png');
  writeFileSync(out, Buffer.from(dataUrl.split(',')[1], 'base64'));
  console.log(`ok   ${name}: zoom ${zoom.toFixed(3)}, ${missing} tile requests outside fixture -> ${out}`);
}

const names = process.argv.slice(2).length
  ? process.argv.slice(2)
  : readdirSync(goldenRoot).filter((d) => existsSync(join(goldenRoot, d, 'case.json')));

const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
try {
  for (const n of names) await renderCase(browser, n);
} finally {
  await browser.close();
}
