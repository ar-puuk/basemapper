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

`line-pattern`, `line-gradient`, `fill-extrusion-pattern`, `heatmap`,
`hillshade` layers, GeoJSON sources, PMTiles sources inside vector styles,
vertical
text, and right-to-left scripts. Unsupported layer types are skipped with a
warning.

### Styles with several sources

Every vector and raster source the layers use is fetched at the zoom MapLibre
covers the view with (vector: `floor(zoom)` clamped to the source's range;
raster: `round(zoom + log2(512 / tileSize))`), and each layer draws from its
own source's tiles; symbol placement uses each layer's source. `raster`
layers inside vector styles are composited like raster-only styles and
blended with raster.fragment.glsl's opacity, hue rotation, saturation,
contrast and brightness range. Multi-sprite styles (`"sprite": [{id, url}]`)
load every sheet, with images of non-default sprites named `id:name`. Golden
cases: OpenFreeMap Liberty at zoom 5 (raster relief + vector) and Versatiles
Colorful at zoom 8 (two vector sources, multi-sprite).

`fill-pattern` follows the fill_pattern shaders: the sprite image repeats in
tile space at its display size (pixels / pixelRatio) in CSS pixels at the
tile's integer zoom, phased by the tile's world pixel position so tiles line
up, sampled linearly with wrap-around; pattern fills use the translucent pass
with a patterned antialiasing outline. In SVG each image becomes a
world-aligned `<pattern>`. Golden case: ESRI World Basemap at zoom 8 (marsh
and intermittent-lake patterns, 0.15%).

`fill-extrusion` follows draw_fill_extrusion.ts at pitch 0: MapLibre's
perspective camera (cameraToCenterDistance = 1.5 view heights above the
centre, so tall buildings lean outward from the centre), heights converted at
the centre latitude, fill_extrusion.vertex.glsl's per-vertex lighting
(style `light`, default from 210° at 30°, intensity 0.5; vertical gradient on
walls) interpolated perspective-correctly, back-face culling and a depth
buffer; translucent layers draw only the nearest surface per pixel. In SVG,
faces are polygons sorted far to near in a group carrying the layer opacity.
Golden case: OpenFreeMap Liberty at zoom 16 (0.49%).

Enum layout properties (`symbol-placement`, rotation alignments,
`symbol-z-order`, `line-cap`, `line-join`) may be expressions such as
`["step", ["zoom"], "point", 11, "line"]`; they are evaluated at the tile
zoom. Line-placed labels with `text-rotation-alignment: viewport` are drawn
upright at their anchor, like point labels. Legacy `{token}` strings are resolved in plain
`text-field` strings and in the stop values of legacy functions.

`icon-text-fit` (both / width / height, with `icon-text-fit-padding`)
follows fitIconToText and getIconQuads: stretchable sprite images
(`stretchX`, `stretchY`, `content`) become a nine-slice grid whose fixed
parts keep their pixel size (golden case `synthetic_icon_text_fit`, a
generated sprite over CARTO tiles).

`text-variable-anchor` / `text-variable-anchor-offset` follow
getTextVariableAnchorOffset and the placement attempts: text is shaped about
the centre once per justification the anchors need (`text-justify: auto`),
and each anchor in turn shifts the collision box (calculateVariableLayoutShift)
until one fits (a second round allowing overlap with `text-allow-overlap`);
the chosen anchor's glyphs are drawn with the render shift, and text-fit
icons follow. Radial and explicit offsets include MapLibre's 7-unit baseline
adjustment (golden case `synthetic_variable_anchor`).

In SVG, `line-blur` is approximated by three nested strokes spread over the
blur ramp with opacities stacking to the line's alpha (MapLibre demotiles
coastline, golden case `maplibre_demotiles_z3`).

The output is always Web Mercator: a style's `projection` (e.g. globe) is
ignored, and references are rendered in Mercator too.

## Pixel ratio

`RenderRequest.pixel_ratio` is MapLibre's devicePixelRatio. `width` and
`height` are CSS pixels and set the view, the zoom, tile choice and label
placement (collision runs in CSS pixels, so the same labels show); the image
is `pixel_ratio` times larger. Geometry, line widths, offsets, blur, circle
radii, translations and symbol quads scale by the ratio, while antialiasing
stays one device pixel wide, as in MapLibre's shaders (`ANTIALIASING =
1/dpr/2`, `EDGE_GAMMA = 0.105/dpr`, dash `sdfgamma / dpr`). Vector styles use
the `@2x` sprite above ratio 1; raster tiles snap to the CSS pixel grid
(MapLibre's `alignedProjMatrix`) before scaling. The `_x2` golden cases check
this against MapLibre at `pixelRatio: 2` (0.000-0.064% of pixels off).

The plotting integrations pick the ratio from the output: dpi / 96 (CSS
pixels are 1/96 in), so map symbols keep their physical size and a 300 dpi
figure gets a basemap rendered at 300 dpi. matplotlib's basemap is an artist
that renders at draw time, so `savefig(dpi=...)` re-renders natively; in R,
`geom_basemap()` renders at draw time from the device's resolution (300 dpi
for PDF / SVG devices), and `tm_basemap()` sizes its raster from the
estimated map panel (device size, margins, facets) at that resolution, so
`ggsave(dpi = 300)` and `tmap_save(dpi = 300)` both get native basemaps.

## SVG output

`render_svg` / `render_svg_stack` (Rust core) write the same view as a true
vector SVG, built from the same placed tiles, evaluated style and label
placement as the PNG:

- **Fills**: one `<path>` per polygon, clipped to its tile like MapLibre's
  stencil (crisp clip rectangles, as the stencil test is per pixel centre).
  Interiors use `shape-rendering="crispEdges"`, matching MapLibre's
  non-antialiased, pixel-centre triangles; MapLibre's antialiasing comes from
  a 1 px outline in the fill colour, emitted as a 0.8 px stroke (the average
  coverage of its aliased `gl.LINES` outline), beneath opaque layers (MapLibre
  draws it only outside the fills) and above translucent ones.
- **Lines** are stroked paths, one per geometry line (MapLibre blends each
  separately), with MapLibre's widths, caps, joins, offsets and gap widths;
  polygon rings are closed paths, and offset paths take the join's shape
  (round or bevelled) on the outside of turns, as MapLibre's join extrusions
  do. Lines thinner than 1 px are stroked at the width that matches the
  coverage of MapLibre's 1 px edge ramp ((w + 1)² / 4). Dash lengths are
  converted from MapLibre's units (pixels at the tile's integer zoom, so they
  stretch with fractional zoom); round-capped dashes are shortened by one line
  width and shifted by half of it, because SVG caps extend past the dash; odd
  dash arrays are rewritten as MapLibre's line atlas reads them (the last
  dash joins the first: `1 2 3` is `4 2` entered 3 units in). **Circles** are
  `<circle>`s.
- **Labels** are editable `<text>` by default: one element per label with
  every glyph at its MapLibre position (`x`/`y` lists, `rotate` along lines),
  so layout and collisions are MapLibre's while the shapes come from the real
  font; halos are a stroke painted beneath the fill (`paint-order`). The
  style's fonts are embedded as `@font-face` (subset to the characters used,
  or whole), found in user folders (`font_dirs`, `BASEMAPPER_FONT_DIRS`), a
  local cache, or Google Fonts for open families; fallback fonts of a stack
  are only fetched when a character needs them, and fonts that cannot be
  found (e.g. proprietary Arial) are referenced by name. The vertical offset
  between MapLibre's glyph metrics and the font baseline is measured per font
  from the glyphs themselves. Browsers use the embedded fonts; most editors
  use installed fonts of the same name, and the text stays editable. With
  `text = "paths"`, labels are instead glyph outlines traced (marching
  squares, consistently wound, `nonzero` fill) from the style's SDF glyphs at
  the shader's thresholds, one set of paths per glyph in MapLibre's draw
  order, with the soft edge approximated by two nested outlines: not
  editable, but identical everywhere and closest to MapLibre's pixels (the
  golden tests score this mode). SDF icons are traced the same way; other
  icons are embedded sprite images.
- **Opacity** is written as `fill-opacity` / `stroke-opacity` / `opacity`
  (colour alpha times the style opacity, 4 decimals).
- **Stacks** (several styles over one view, e.g. UGRC's three services) become
  one document with a group per style.

Cases with `"svg": true` measure the result: the SVG is rasterised with
resvg and scored against the same MapLibre reference, with their own ratchet
limits (`max_bad_pixel_frac_svg`, `max_mean_diff_svg`): 0.000-0.64% of
pixels off (Hillshade 0.000%, Positron 0.006%, Dark Matter 0.038%, the full
UGRC stack at zoom 12.4 0.23%), against 0-0.2% for the PNG. Chromium scores
the UGRC stack SVG at 0.39%. The rest is mostly MapLibre blending
translucent wide lines twice where their segments overlap at sharp corners,
which SVG strokes do not do, and pixel-level differences between an
antialiased SVG edge and MapLibre's aliased 1 px fill outline. Fields of
sub-pixel polygons with a contrasting `fill-outline-color` (Liberty's parks
at zoom 5: 6% of pixels) are the worst case: their aliased outline pixels
cannot be reproduced by vector strokes.

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
City, the UGRC stack at fractional zoom 12.4, pixel ratio 2, OpenFreeMap
Liberty and Versatiles Colorful (multi-source), plus OSM raster tiles and a
layer-filtered style.

See `core/tests/fixtures/golden/README.md` for the fixture workflow.
