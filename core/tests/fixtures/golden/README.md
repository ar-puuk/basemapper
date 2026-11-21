# Golden-image fixtures

Offline render fixtures used by `core/tests/golden.rs`. Each case directory has:

| File | Purpose |
|---|---|
| `case.json` | bbox (EPSG:3857), width, height, zoom, optional `layers` filter, optional `fixture_from` (reuse another case's style and tiles), and the fidelity limits `max_bad_pixel_frac` / `max_mean_diff` (default 0 = exact parity) |
| `style.json` | The source style, with its tile source rewritten to `{{BASE}}/{z}/{x}/{y}` and `sprite`/`glyphs` removed |
| `tiles/z/x/y` | Raw tile bytes as served by the provider |
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
```

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

A handful of tiles is stored here for offline testing only.
