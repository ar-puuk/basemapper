# Golden-image fixtures

Offline render fixtures used by `core/tests/golden.rs`. Each case directory has:

| File | Purpose |
|---|---|
| `case.json` | bbox (EPSG:3857), width, height, zoom, optional `layers` filter, optional `fixture_from` (reuse another case's style and tiles) or `tiles_from` / `fonts_from` (reuse only its tiles / glyphs, e.g. several styles over one source), optional `stack` (composite other cases bottom-to-top on both sides; no style of its own), and the fidelity limits `max_bad_pixel_frac` / `max_mean_diff` (default 0 = exact parity) |
| `style.json` | The source style, with its URLs rewritten to `{{BASE}}/tiles/{z}/{x}/{y}`, `{{BASE}}/fonts/{fontstack}/{range}.pbf` and `{{BASE}}/sprites/sprite` |
| `tiles/z/x/y` | Raw tile bytes as served by the provider (MapLibre's tile zoom and basemapper's, clamped to the source maxzoom) |
| `fonts/`, `sprites/` | Glyph ranges actually needed by the visible label text, and the 1x sprite |
| `reference-maplibre-gl.png` | **The target**: the same view rendered by MapLibre GL JS with default settings, its RTL text plugin loaded and CJK drawn from the style's glyphs (`localIdeographFontFamily: false`). `cargo test` fails if basemapper differs by more than the case's limits |

The limits are a ratchet toward 0: when a change improves fidelity, the test
prints a `[ratchet: ...]` hint; lower the limits in `case.json` to lock the gain
in. A pixel counts as "bad" when any channel differs by more than 24/255.

## Tolerances

Each case passes when both of its limits hold:

- `max_bad_pixel_frac`: the share of pixels where any channel (RGBA)
  differs from MapLibre's by more than 24 of 255.
- `max_mean_diff`: the mean absolute difference over all channels and
  pixels, in levels of 255.

Limits are set to the measured values plus 0.005% (bad pixels) and 0.005
(mean difference). This leaves room for floating-point noise between
machines, and nothing else. Stacked cases are compared after compositing
their members bottom to top, on both sides.

For a sense of scale: a half-pixel offset, an (N-1)/N rescale or a blur
moves every edge in the map. That puts several percent of pixels over the
threshold and the mean difference above 1. The `*_tract_*` cases (real
plot sizes at 168, 200 and 300 dpi) score at most 0.02% bad pixels and a
mean difference of 0.02.

`"svg": true` cases also rasterise the SVG output with resvg and score it
against the same reference, with the limits `max_bad_pixel_frac_svg` and
`max_mean_diff_svg`. Those are looser, because SVG is drawn by a different
rasteriser. `core/tests/svg_seams.rs` checks the SVG separately:

- It rasterises the SVG at 3x without `shape-rendering` hints, as a PDF
  viewer draws it (every clip edge antialiased).
- A tile boundary fails when its pixels are lighter than both sides, 2-3 px
  away, more often than on lines that are not tile boundaries (+0.5
  percentage points).
- The SVG at 1x must also match the PNG render: mean luminance difference
  under 2, under 1% of pixels off by more than 24.

## Pixel ratios that are not whole numbers

MapLibre sizes its drawing buffer as `floor(css size x devicePixelRatio)`
and fits the CSS view into it. basemapper lays device pixels `1 / ratio`
CSS pixel wide from the view's top-left corner. The two agree exactly when
`css size x ratio` is a whole number, so cases use CSS sizes for which it
is:

- multiples of 4 at 1.75 (168 dpi);
- multiples of 12 at 2.0833 (200 dpi);
- multiples of 8 at 3.125 (300 dpi).

A plot panel whose size is not a whole number of CSS pixels (720 px at 168
dpi is 411.43) is drawn as a slightly larger render on the same pixel grid
and cropped. The `*_tract_x175` cases are that larger render
(412 x 572 CSS px for a 720 x 1000 px panel). The core test
`a_size_that_is_not_whole_css_pixels_is_not_resampled` checks the crop
pixel for pixel. The R and Python plot tests check that a Web Mercator
panel shows the rendering's pixels unchanged: ggplot2, tmap, matplotlib and
plotnine, with a 1-pixel line centred on its column to within 0.1 px.

## Workflows

```sh
# Run the comparison (offline; writes target/golden-output/<case>/)
cargo test -p basemapper-core --release --test golden -- --nocapture
cargo test -p basemapper-core --release --test svg_seams

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

# A style with several sources (raster + vector, two vector sources):
# every source the layers use, saved to tiles/<source>/z/x/y
cargo run -p basemapper-core --example capture_fixture -- ... --all-sources
```

To run only some cases while investigating one view, set
`BASEMAPPER_GOLDEN_ONLY=case1,case2` (e.g.
`BASEMAPPER_GOLDEN_ONLY=openfreemap_liberty_z16 cargo test -p basemapper-core --test golden -- --nocapture`).

Case options in `case.json`: `pixel_ratio` (render at that devicePixelRatio),
`svg: true` (also score the SVG output, limits `max_*_svg`), `stack`
(composite other cases), `fixture_from` / `tiles_from` / `fonts_from` /
`sprites_from` (reuse another case's files), `pmtiles` (pack tile directories
of the tiles fixture into PMTiles archives, served with range requests at
`{{BASE}}/<name>`; the test also writes them to
`target/golden-output/<case>/`, where `tools/reference-render` serves them to
MapLibre through the pmtiles protocol, so run the test before rendering those
references). References are rendered in Web Mercator even
when a style asks for the globe projection.

## Visual comparison

Every golden run writes `check/manifest.js` at the repository root
(gitignored). Open `check/index.html` in a browser for an overview grid of
all cases by style and zoom, and side-by-side, swipe, blink and diff views of
basemapper vs MapLibre, with 1–8x pixel magnification and an option to compare
against the no-labels reference.

## Cases

| Case | What it covers |
|---|---|
| `carto_{positron,dark_matter,voyager}_z{1..22}` | CARTO GL styles at every MapLibre zoom from 1 to 22 over Salt Lake City; z16+ overzooms the source (maxzoom 15), z1-z2 show a wrapped world copy at the west edge. Positron holds each zoom's tiles (`tiles_from`); all share the glyphs in `carto_positron_z4` (`fonts_from`) |
| `ugrc_litebase_slc`, `ugrc_filtered_slc` | UGRC Lite Base (ESRI VectorTileServer style), full and layer-filtered (transparent) |
| `ugrc_hillshade_slc`, `ugrc_litelabels_slc` | UGRC Vector Hillshade and Lite Labels services |
| `ugrc_full_slc` | `stack` of Hillshade, Lite Base and Lite Labels (bottom to top): the full UGRC Lite basemap. Lite Base has no background; the hillshade beneath supplies it |
| `ugrc_{hillshade,litebase,litelabels,full}_z{1..22}` | The three UGRC services and their full stack at every MapLibre zoom from 1 to 22 except 13 (the `_slc` cases are zoom 13). Glyphs shared with the `_slc` cases via `fonts_from` |
| `ugrc_{hillshade,litebase,litelabels,full}_z12_4` | Fractional MapLibre zoom 12.4 (native-resolution proof of concept); also scored as SVG (`svg: true`) |
| `*_x2` | Pixel ratio 2 (MapLibre `pixelRatio: 2`, @2x sprites): the z12.4 UGRC cases, CARTO z7/z13/z14 and OSM raster, via `fixture_from` |
| `openfreemap_liberty_z5` | OpenFreeMap Liberty over Utah: a raster source (Natural Earth relief) and a vector source in one style (`--all-sources`) |
| `versatiles_colorful_z8` | VersaTiles Colorful over Salt Lake City: two vector sources and a multi-sprite (`[{id, url}]`) style; its globe projection is ignored |
| `esri_world_basemap_z8` | ESRI World Basemap over the Great Salt Lake: `fill-pattern` (marsh, intermittent lakes), also scored as SVG |
| `openfreemap_{positron,bright}_z{5,12,16}` | OpenFreeMap Positron and Bright over the Liberty views (and z12 Salt Lake City), sharing those cases' tiles, glyphs and sprite (`tiles_from` / `fonts_from` / `sprites_from`); z12 also scored as SVG |
| `openfreemap_liberty_z16` | OpenFreeMap Liberty downtown Salt Lake City: 3D buildings (`fill-extrusion`, perspective and lighting), pedestrian-area patterns, `step` symbol placement; also scored as SVG |
| `maplibre_demotiles_z3` | MapLibre demotiles over North America: `line-blur` (also in SVG), legacy function `text-field` with `{token}` stops |
| `pmtiles_*` | PMTiles sources over archives packed at test time from other fixtures' tiles (no archives are committed): `pmtiles_liberty_z5` (vector and raster sources in a vector style), `pmtiles_hillshade` (raster-dem), `pmtiles_osm_raster_slc` (raster-only style). The references equal those of the source cases pixel for pixel |
| `synthetic_*` | Hand-written styles over existing fixture tiles for features no captured style uses; `synthetic_icon_text_fit`: icon-text-fit both/width/height with a generated nine-slice sprite; `synthetic_variable_anchor`: text-variable-anchor (radial / text-offset), text-variable-anchor-offset, text-justify auto; `synthetic_line_pattern_z13` / `_x2`: line-pattern with a generated sprite (data-driven names and widths, offset, @2x); `synthetic_fill_extrusion_pattern_z13` / `_x2`: fill-extrusion-pattern over inline GeoJSON buildings (data-driven names, heights and bases, a hole, a missing image, a translucent constant pattern, viewport light, @2x); `synthetic_vertical_z10` / `_x2`: vertical text (point labels with text-writing-mode vertical, verticalized punctuation, mixed Latin, wrapped and Latin-only labels, a crowded layer falling back from horizontal to vertical, CJK line labels on vertical, upward, steep and wavy lines) with Noto Sans glyphs from OpenFreeMap; `synthetic_rtl_z10` / `_x2`: right-to-left labels (Arabic shaping with tashkeel, lam-alef, Persian, Urdu and ZWNJ; Hebrew; mixed-direction text, numbers, brackets, wrapped and two-paragraph labels, line-placed labels) with Noto Sans glyphs from OpenFreeMap; `synthetic_hillshade_*`: hillshade methods (standard, multidirectional, combined at pixel ratio 2, stacked basic/igor/standard, igor with nearest resampling) over terrarium DEM tiles stored in `synthetic_hillshade_standard/tiles/dem`; `synthetic_geojson_z13` / `_z6`: GeoJSON sources (URL and inline data: polygons with holes, multipolygons, a long simplified line, points, labels, dashes, line offset) |
| `osm_raster_slc` | OpenStreetMap raster tiles |
| `ugrc_{hillshade,litebase,litelabels,full}_tract_x175` | A real plot: a 720 x 1000 px panel (3.6 x 5 in at 200 dpi) drawn at 168 dpi (devicePixelRatio 1.75) at the fractional zoom 13.141 over a Utah census tract. This is the 412 x 572 CSS px render that the panel is cropped from |
| `ugrc_*_tract_x2083` | 200 dpi (devicePixelRatio 25/12) at zoom 12.1726, 348 x 480 CSS px |
| `ugrc_*_tract_x3125` | 300 dpi (devicePixelRatio 3.125) at zoom 13.141, 232 x 320 CSS px, over the x175 cases' tiles (`fixture_from`) |
| `openfreemap_liberty_tract_x175` | OpenFreeMap Liberty over the x175 tract view (`--all-sources`) |

The test always writes `actual.png` and `diff.png` (bad pixels in red) to
`target/golden-output/<case>/`.

Keep the bbox aspect ratio equal to width/height, and choose bboxes whose
MapLibre zoom is an integer (`render.mjs` prints it). MapLibre picks tiles by
`floor(zoom)`, so a zoom of 12.9999 loads different tiles than 13.

## Data attribution

| Case | Data | License |
|---|---|---|
| `ugrc_*` | Utah Geospatial Resource Center (UGRC), State of Utah: Lite Base, Vector Hillshade and Lite Labels vector tiles | Public data from the State of Utah |
| `osm_raster_slc` | © OpenStreetMap contributors, standard tile layer | Data ODbL; tiles CC BY-SA 2.0 |
| `carto_*` | © OpenStreetMap contributors, © CARTO (styles, glyphs, sprites); tiles from CARTO basemaps | Data ODbL; styles per CARTO basemap terms |
| `openfreemap_*` | © OpenStreetMap contributors, © OpenMapTiles, Natural Earth; style, tiles, glyphs and sprites from OpenFreeMap | Data ODbL; OpenMapTiles schema CC-BY 4.0; Natural Earth public domain |
| `esri_*` | Esri, HERE, Garmin, © OpenStreetMap contributors; ESRI World Basemap style, tiles, glyphs and sprites | Per Esri terms of use |
| `maplibre_demotiles_*` | MapLibre demotiles (Natural Earth data) | Natural Earth public domain; style BSD-3-Clause (MapLibre) |
| `versatiles_*` | © OpenStreetMap contributors; style, tiles, glyphs and sprites from VersaTiles | Data ODbL; styles per VersaTiles terms |

A handful of tiles is stored here for offline testing only.
