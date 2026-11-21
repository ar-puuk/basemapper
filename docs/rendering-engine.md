# How basemapper renders maps

basemapper turns a MapLibre style plus a bounding box into an RGBA image,
with no browser, GPU or system libraries. This page explains how, and how
the output is kept faithful to MapLibre GL JS.

## Pipeline

```
style (URL or JSON) ──► resolve style, TileJSON ──► choose zoom ──► fetch tiles
                                                                     │
          raster source ◄────────────────────────────────────────────┤
          (XYZ, WMS, PMTiles)                                        │
            renderer.rs: composite tiles                              vector source (MVT)
                                                                     │
                                         vector/ : software MapLibre ◄┘
                                                                     │
                                                RGBA pixels + bounds ◄┘
```

Everything above lives in the Rust crate `core/`. The R (`r-basemapper`) and
Python (`py-basemapper`) packages infer the bounding box and pixel size from
the plot, reproject it to EPSG:3857, and call `core::render`.

## Raster sources

`core/src/renderer.rs` fetches the XYZ, WMS or PMTiles tiles covering the
bbox and composites them onto the output canvas. This path matches MapLibre
pixel for pixel (the `osm_raster_slc` golden case differs by 0 pixels).

## Vector sources: a software port of MapLibre GL JS

`core/src/vector/` renders vector tiles by reproducing, in software, what
MapLibre GL JS does on the GPU. The goal is the same image MapLibre would
draw, not just a similar one, so behaviour is ported from MapLibre's
TypeScript source rather than reimplemented from scratch.

| Module | Responsibility | Ported from (maplibre-gl-js `src/`) |
|---|---|---|
| `mvt.rs` | Decode Mapbox Vector Tiles; group polygon rings | `vector-tile` / `classifyRings` |
| `expr.rs` | Style values: colours, legacy functions, expressions, filters | `@maplibre/maplibre-gl-style-spec` |
| `gl.rs` | Software WebGL: rasteriser, framebuffer, line tessellation, dash atlas | `data/bucket/line_bucket.ts`, `render/line_atlas.ts`, shaders |
| `render.rs` | View setup, layer loop, fills, lines, circles, background | `render/draw_*.ts`, shaders |
| `symbol.rs` | Text and icon labels: glyphs, shaping, quads, placement, drawing | `symbol/*.ts`, `webgl/draw/draw_symbol.ts` |
| `symbol_line.rs` | Labels along lines: merging, anchors, glyph placement, collision circles | `symbol/merge_lines.ts`, `get_anchors.ts`, `projection.ts` |

### The details that make it match

These are the MapLibre behaviours the port reproduces. Most were found by
measuring against MapLibre and investigating the difference.

- **View and zoom.** The style is evaluated at MapLibre's scale zoom
  (512 px tiles). Vector tiles come from `floor(zoom)`, clamped to the
  source's `maxzoom` (overzooming above it). The view is placed at the bbox
  centre minus half the canvas, as MapLibre's transform does.
- **Rasterisation.** Every primitive is sampled at pixel centres with the
  top-left rule, as a GPU does. There is no other antialiasing; edges get
  their softness from MapLibre's own shader formulas.
- **Framebuffer arithmetic.** Colours are premultiplied. Each fragment is
  blended with `src + dst × (1 − src.a)` in float32 and stored to 8 bits with
  round-half-to-even (`0.3 × 255` is exactly 76.5 in float32 and stores as
  76). Colours are premultiplied in double precision before conversion,
  like MapLibre's JavaScript.
- **Tile clipping.** Each tile's fills and lines are clipped to the tile
  square (MapLibre's stencil test), so geometry repeated in tile buffers is
  never drawn twice.
- **Fills.** Interiors by pixel centre, plus 1 px `gl.LINES` outlines with the
  `fill_outline` shader. Opaque fills keep their default outline outside the
  fill only (MapLibre's depth sublayers).
- **Lines.** MapLibre's line tessellation (joins, caps, sharp-corner offsets,
  extrusion vectors rounded to 1/63) and its line shader (offset, gap width,
  blur). Dashes use a port of MapLibre's signed-distance dash atlas.
- **Labels.** Glyph PBFs laid out as in MapLibre's atlas; MapLibre's line
  breaking and justification; glyph corners rounded to 1/32 em; anchors at
  points, line starts and polygon poles of inaccessibility; placement top
  layer first, tiles in (x, y) order, `symbol-sort-key` across tiles; boxes
  and circles in a collision grid padded 100 px beyond the view; the
  `symbol_sdf` shader with one- or two-pass halos.
- **Icons.** Sprite images premultiplied as in MapLibre's icon atlas, sampled
  `NEAREST` or `LINEAR` under the same conditions; SDF icons through the text
  shader.

### Not supported yet

`fill-pattern`, `line-pattern`, `line-gradient`, `fill-extrusion`, `heatmap`,
`hillshade` and `raster` layers inside vector styles, variable label anchors
(`text-variable-anchor`), `icon-text-fit`, vertical text, and right-to-left
scripts. Unsupported layer types are skipped with a warning.

## SVG output

`render_svg` / `render_svg_stack` (Rust core) write the same view as a true
vector SVG, built from the same placed tiles, evaluated style and label
placement as the PNG:

- **Fills** are `<path>`s (even-odd), clipped to their tile like MapLibre's
  stencil clipping. **Lines** are stroked paths with MapLibre's widths, caps,
  joins, offsets and gap widths; dash lengths are converted from MapLibre's
  units (pixels at the tile's integer zoom, so they stretch with fractional
  zoom), and round-capped dashes are shortened by one line width because SVG
  caps extend past the dash. **Circles** are `<circle>`s.
- **Labels** are glyph outlines traced (marching squares) from the style's
  SDF glyphs at the shader's thresholds: the halo beneath the fill, each
  glyph positioned exactly as placed for the PNG. No fonts are needed to view
  the file. SDF icons are traced the same way; other icons are embedded
  sprite images.
- **Stacks** (several styles over one view, e.g. UGRC's three services) become
  one document with a group per style.

SVG cannot express MapLibre's antialiasing (a smoothstep over the SDF, a
one-pixel ramp on line edges), so edges differ slightly from MapLibre's
pixels. Cases with `"svg": true` measure this: the SVG is rasterised with
resvg and scored against the same MapLibre reference, with its own ratchet
limits (`max_bad_pixel_frac_svg`, `max_mean_diff_svg`). Typical results are
0.04–1.2% of pixels off, against 0–0.2% for the PNG; rasterising in
Chromium gives the same score as resvg (0.538% vs 0.541% on the full UGRC
stack at zoom 12.4), so the result does not depend on the viewer.

Export a view from a centre and a (fractional) MapLibre zoom with:

```sh
cargo run --release -p basemapper-core --example export_svg --   --style <hillshade style URL> --style <lite base style URL> --style <lite labels style URL>   --center=-111.8887,40.7556 --zoom 12.4 --width 800 --height 600   --output ugrc.svg --png ugrc.png
```

`tools/reference-render/shot_svg.mjs` rasterises an SVG in headless Chromium
for cross-checking.

## How fidelity is verified

`core/tests/golden.rs` renders offline fixtures and compares each image with
MapLibre GL JS's render of the same view.

- **Fixtures** (`core/tests/fixtures/golden/<case>/`) hold a style, exactly
  the tiles, glyphs and sprite the view needs, and `case.json` (bbox, size,
  zoom, limits). They are captured from live services with
  `cargo run -p basemapper-core --example capture_fixture`.
- **References** (`reference-maplibre-gl.png`) are rendered by
  `tools/reference-render/render.mjs`: MapLibre GL JS with default settings in
  headless Chromium, serving the same fixture files.
- **Score.** A pixel is "bad" if any channel differs by more than 24/255. Each
  case has limits on the bad-pixel fraction and mean difference that only
  ever go down (a ratchet); the goal is 0.
- **Stacks.** A case with `"stack"` composites other cases bottom to top on
  both sides, e.g. the full UGRC basemap (Hillshade, Lite Base, Lite Labels).
- **Viewer.** Each test run writes `check/manifest.js`; open
  `check/index.html` (gitignored) for an overview grid and side-by-side,
  swipe, blink and diff views.

Coverage: CARTO Positron, Dark Matter and Voyager, and the UGRC Lite basemap
(each service and the full stack), at MapLibre zooms 1 to 22 over Salt Lake
City, the UGRC stack at fractional zoom 12.4, plus OSM raster tiles and a
layer-filtered style.

See `core/tests/fixtures/golden/README.md` for the fixture workflow.
