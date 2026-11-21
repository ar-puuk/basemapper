# Golden-image fixtures

Offline render fixtures used by `core/tests/golden.rs`. Each case directory has:

| File | Purpose |
|---|---|
| `case.json` | bbox (EPSG:3857), width, height, zoom, optional `layers` filter, optional `fixture_from` (reuse another case's style and tiles), optional `max_mean_diff` / `max_bad_pixel_frac` tolerances |
| `style.json` | The source style, with its tile source rewritten to `{{BASE}}/{z}/{x}/{y}` and `sprite`/`glyphs` removed |
| `tiles/z/x/y` | Raw tile bytes as served by the provider |
| `expected.png` | **Regression snapshot** from basemapper's current renderer. `cargo test` fails if output drifts beyond tolerance |
| `reference-maplibre-gl.png` | **Fidelity target**: the same view rendered by MapLibre GL JS. Reported as a score by the test, not pass/fail (yet) |

## Workflows

```sh
# Add a case (downloads style + exactly the tiles needed)
cargo run -p basemapper-core --example capture_fixture -- \
  --name <case> --style <style URL or inline JSON> \
  --bbox xmin,ymin,xmax,ymax --width 800 --height 600 --zoom 14

# Regenerate expected.png after an intentional renderer change
BASEMAPPER_BLESS=1 cargo test -p basemapper-core --test golden

# Regenerate the MapLibre GL JS references
cd tools/reference-render && npm install && npx playwright install chromium
node render.mjs [case ...]

# Skip vector cases on a machine with no GPU adapter
BASEMAPPER_NO_GPU=1 cargo test -p basemapper-core --test golden
```

On a mismatch the test writes `actual.png` and `diff.png` (bad pixels in red)
to `target/golden-output/<case>/`.

Keep the bbox aspect ratio equal to width/height, and choose bboxes whose
MapLibre zoom is an integer (`render.mjs` prints it). MapLibre picks tiles by
`floor(zoom)`, so a zoom of 12.9999 loads different tiles than 13.

## Data attribution

| Case | Data | License |
|---|---|---|
| `ugrc_litebase_slc`, `ugrc_filtered_slc` | Utah Geospatial Resource Center (UGRC), State of Utah, Lite Base vector tiles | Public data from the State of Utah |
| `osm_raster_slc` | © OpenStreetMap contributors, standard tile layer | Data ODbL; tiles CC BY-SA 2.0 |

A handful of tiles is stored here for offline testing only.
