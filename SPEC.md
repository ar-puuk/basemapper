# Technical Specification: basemapper
**A headless MapLibre GL renderer for R and Python**

> Updated 2026-11-21. The original specification (June 2026) built on
> `maplibre-rs` + `wgpu`; that engine was replaced on 2026-11-20 by an
> in-house CPU port of MapLibre GL JS (see
> [docs/rendering-engine.md](docs/rendering-engine.md)).

## 1. Goal

Render basemaps at **native resolution** for static maps in R and Python
(`ggplot2`, `tmap`, `matplotlib`, `plotnine`, or standalone): for the exact
view, at any fractional zoom and output resolution, instead of resampled
pre-rendered tiles. Input is a MapLibre / Mapbox GL style (vector tiles, XYZ
raster, WMS, WMTS, PMTiles, ESRI vector tiles); output is RGBA pixels, PNG,
`terra`/`xarray`-ready arrays, or true vector SVG. No browser, GPU, display
server or GDAL.

**Fidelity requirement:** vector styles must match MapLibre GL JS pixel for
pixel. Every change is gated by golden-image tests against MapLibre GL JS
renders of the same views (§6).

**Deferred rendering:** plotting integrations render when the plot is drawn,
from the plot's own extent, size and output resolution; users never pass
coordinates or pixel sizes.

## 2. Repository layout

```text
basemapper/
├── Cargo.toml                 # workspace: core, py-basemapper
├── core/                      # Rust: style, tiles, rendering (no bindings)
│   ├── src/lib.rs             # render, render_stack, render_svg(_stack), encode_png
│   ├── src/bbox.rs            # RenderRequest, zoom maths, bbox_from_center, fit_bounds
│   ├── src/style.rs           # style resolution, validation, layer filter
│   ├── src/tile_fetcher.rs    # concurrent tile download (tokio + reqwest)
│   ├── src/renderer.rs        # raster-tile compositing
│   └── src/vector/            # MapLibre GL JS port (see docs/rendering-engine.md)
├── py-basemapper/             # PyO3 + maturin wheel (abi3)
├── r-basemapper/              # extendr package; separate Cargo workspace with
│                              # a vendored core snapshot (src/rust/core)
├── tools/reference-render/    # MapLibre GL JS references (Playwright)
└── docs/rendering-engine.md   # how the engine works and is verified
```

## 3. Rust core API

```rust
pub struct RenderRequest {
    pub bbox: [f64; 4],            // EPSG:3857; xmin > xmax = anti-meridian
    pub width: u32, pub height: u32, // CSS pixels: set view and zoom
    pub pixel_ratio: f64,          // devicePixelRatio: image is width*ratio x height*ratio
    pub style_input: StyleInput,   // URL or inline JSON
    pub zoom: Option<u8>,          // None = MapLibre's choice
    pub layers: Option<Vec<String>>, // keep IDs, or drop "-ID"s
    pub auth_token: Option<String>,
    pub tile_timeout_ms: u32, pub max_tiles: u32, pub tile_concurrency: u32,
    pub fail_on_tile_error: bool,
}

pub fn render(req: RenderRequest) -> Result<RenderResult, BasemapError>;        // RGBA
pub fn render_stack(reqs: Vec<RenderRequest>) -> Result<RenderResult, BasemapError>; // styles bottom to top
pub fn render_svg(req: RenderRequest) -> Result<String, BasemapError>;
pub fn render_svg_stack(reqs: Vec<RenderRequest>) -> Result<String, BasemapError>;
pub fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, BasemapError>;
pub fn bbox_from_center(lon: f64, lat: f64, zoom: f64, width: u32, height: u32) -> [f64; 4];
pub fn fit_bounds(bounds_3857: [f64; 4], width: u32, height: u32) -> (f64, f64, f64);
```

The core only takes EPSG:3857. Host languages reproject (`sf` in R, `pyproj`
in Python). Renders are synchronous at the FFI boundary; tile fetching runs on
an internal tokio runtime.

## 4. Rendering

- **Vector styles:** a software port of MapLibre GL JS's WebGL pipeline: MVT
  decoding, style expressions, fill/line/circle/symbol buckets, line
  tessellation and dash atlas, glyph shaping, symbol placement and collision,
  SDF text and icons, stencil tile clipping, float premultiplied blending.
  Style evaluated at the true fractional zoom; tiles at `floor(zoom)` clamped
  to the source maxzoom; world copies across ±180°.
- **Raster sources:** tiles at MapLibre's rounded zoom, composited at the
  output size, snapped to the CSS pixel grid.
- **Pixel ratio:** MapLibre's devicePixelRatio. Layout and labels in CSS
  pixels, drawing at device pixels; `@2x` sprites above ratio 1.
- **SVG:** the same view and placement as true vector output: fills, lines,
  circles as paths; labels as glyph outlines traced from the SDF glyphs; icons
  embedded. Edges emulate MapLibre's (crisp interiors plus its 1 px
  antialiasing outline) so a rasterised SVG stays close to MapLibre's pixels.

Not supported yet: `fill-pattern`, `line-pattern`, `line-gradient`,
`fill-extrusion`, `heatmap`, `hillshade`/`raster` layers inside vector
styles, variable anchors, `icon-text-fit`, vertical and RTL text.

## 5. Language APIs

| | R | Python |
|---|---|---|
| Map view | `basemap(style, width, height, center, zoom, bounds, crs, ratio)`, `set_view()`, `set_bounds()`, `basemap_bounds()` | `Map(style, width, height, ratio, center=, zoom=, bounds=, crs=)` |
| Output | `render_png()`, `render_svg()`, `render_array()`, `render_raster()` | `.render_png()`, `.render_svg()`, `.render_array()`, `.render_buffer()` |
| Plot layers | `geom_basemap()`, `tm_basemap()` | `add_basemap()`, plotnine `geom_basemap()` |
| Low level | `render_basemap_raw()` | `render_basemap_raw()`, `render_styles_raw()` |
| Styles | providers: `basemap_provider()`, `raster_provider()`, `vector_provider()`, `esri_*_provider()`, `wms_provider()`, `wmts_provider()`, `pmtiles_provider()` | same (classes / functions) |

A style argument may be a list of styles, stacked bottom to top.

Plot layers choose the pixel ratio from the output: `dpi / 96` (CSS pixels
are 1/96 in), so symbols keep their physical size and a 300 dpi figure gets a
basemap rendered at 300 dpi. matplotlib uses an artist that renders at draw
time (`savefig(dpi=...)` re-renders); `geom_basemap()` renders in grid's
`makeContent` from the device resolution; `tm_basemap()` sizes its raster
from the estimated map panel. PDF/SVG devices use 300 dpi.

## 6. Verification

- `cargo test -p basemapper-core`: unit tests plus the golden suite: 168
  offline fixtures (CARTO Positron / Dark Matter / Voyager and UGRC at zooms
  1–22, fractional zoom 12.4, pixel ratio 2, OSM raster) compared with MapLibre
  GL JS references; a pixel is bad if any channel differs by more than 24;
  per-case limits only ever tighten. `"svg": true` cases also score the SVG
  rasterised with resvg.
- `pytest` in `py-basemapper/` (end-to-end against the fixtures), `testthat`
  in `r-basemapper/` (live tiles, skipped offline).
- `check/index.html` (gitignored) visual comparison viewer.
