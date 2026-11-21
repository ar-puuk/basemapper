# Golden-image fixtures

Offline render fixtures used by `core/tests/golden.rs`. Each case directory has:

| File | Purpose |
|---|---|
| `case.json` | bbox (EPSG:3857), width, height, zoom, optional `layers` filter, optional `fixture_from` (reuse another case's style and tiles) or `tiles_from` / `fonts_from` (reuse only its tiles / glyphs, e.g. several styles over one source), and the fidelity limits `max_bad_pixel_frac` / `max_mean_diff` (default 0 = exact parity) |
| `style.json` | The source style, with its URLs rewritten to `{{BASE}}/tiles/{z}/{x}/{y}`, `{{BASE}}/fonts/{fontstack}/{range}.pbf` and `{{BASE}}/sprites/sprite` |
| `tiles/z/x/y` | Raw tile bytes as served by the provider (MapLibre's tile zoom and basemapper's, clamped to the source maxzoom) |
| `fonts/`, `sprites/` | Glyph ranges actually needed by the visible label text, and the 1x sprite |
| `reference-maplibre-gl.png` | **The target**: the same view rendered by MapLibre GL JS with default settings. `cargo test` fails if basemapper differs by more than the case's limits |

The limits are a ratchet toward 0: when a change improves fidelity, the test
prints a `[ratchet: ...]` hint; lower the limits in `case.json` to lock the gain
in. A pixel counts as "bad" when any channel differs by more than 24/255.

## Workflows

```sh
# Add a case (downloads style + exactly the tiles needed)
cargo run -p basemapper-core --example capture_fixture -- \
  --name <case> --style <style URL or inline JSON> \
  --bbox xmin,ymin,xmax,ymax --width 800 --height 600 --zoom 14

# Regenerate the MapLibre GL JS references
cd tools/reference-render && npm install && npx playwright install chromium
node render.mjs [case ...]
# Diagnostic: MapLibre reference without labels (isolates geometry)
NO_SYMBOLS=1 node render.mjs <case>   # -> reference-maplibre-gl-nosymbols.png

# Another style over the same tiles
cargo run -p basemapper-core --example capture_fixture -- ... --tiles-from <case>
```

## Visual comparison

Every golden run writes `check/manifest.js` at the repository root
(gitignored). Open `check/index.html` in a browser for an overview grid of
all cases by style and zoom, and side-by-side, swipe, blink and diff views of
basemapper vs MapLibre, with 1–8x pixel magnification and an option to compare
against the no-labels reference.

## Cases

| Case | What it covers |
|---|---|
| `carto_{positron,dark_matter,voyager}_z{4,7,10,13,16}` | CARTO GL styles at five MapLibre zooms over Salt Lake City; z16 overzooms the source (maxzoom 15). Positron holds each zoom's tiles (`tiles_from`); all share the glyphs in `carto_positron_z4` (`fonts_from`) |
| `ugrc_litebase_slc`, `ugrc_filtered_slc` | UGRC Lite Base (ESRI VectorTileServer style), full and layer-filtered (transparent) |
| `osm_raster_slc` | OpenStreetMap raster tiles |

The test always writes `actual.png` and `diff.png` (bad pixels in red) to
`target/golden-output/<case>/`.

Keep the bbox aspect ratio equal to width/height, and choose bboxes whose
MapLibre zoom is an integer (`render.mjs` prints it). MapLibre picks tiles by
`floor(zoom)`, so a zoom of 12.9999 loads different tiles than 13.

## Data attribution

| Case | Data | License |
|---|---|---|
| `ugrc_litebase_slc`, `ugrc_filtered_slc` | Utah Geospatial Resource Center (UGRC), State of Utah, Lite Base vector tiles | Public data from the State of Utah |
| `osm_raster_slc` | © OpenStreetMap contributors, standard tile layer | Data ODbL; tiles CC BY-SA 2.0 |
| `carto_*` | © OpenStreetMap contributors, © CARTO (styles, glyphs, sprites); tiles from CARTO basemaps | Data ODbL; styles per CARTO basemap terms |

A handful of tiles is stored here for offline testing only.
