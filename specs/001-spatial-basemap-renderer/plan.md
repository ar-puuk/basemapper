# Implementation Plan: Spatial Basemap Renderer

**Branch**: `feat/cpu-renderer` | **Date**: 2026-06-08, updated 2026-11-21 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-spatial-basemap-renderer/spec.md`

> **2026-11-20:** the `maplibre-rs` + `wgpu` engine of the original plan was
> replaced by an in-house CPU port of MapLibre GL JS (`core/src/vector/`).
> This plan describes the current design; [research.md](research.md) and
> [tasks.md](tasks.md) keep the original decisions as a record.

---

## Summary

`basemapper` renders MapLibre GL styled basemaps headlessly and exposes them
to R and Python. The Rust `core` crate fetches styles and tiles (`tokio` +
`reqwest`) and renders them on the CPU: raster tiles are composited; vector
tiles are drawn by a software port of MapLibre GL JS's renderer, verified
pixel by pixel against MapLibre GL JS. Output is RGBA pixels (PNG, arrays,
rasters) at any fractional zoom and pixel ratio, or true vector SVG; several
styles can be stacked. An `extendr` R package and a `PyO3`/maturin Python
wheel provide a standalone map API (`basemap()` / `Map`) and plot layers
(ggplot2, tmap, matplotlib, plotnine) that render at draw time for the plot's
extent and output resolution. CRS reprojection to EPSG:3857 happens in the
host language (`sf`, `pyproj`).

---

## Technical Context

**Language/Version**: Rust stable (edition 2021), Python ≥ 3.9 (abi3 wheel),
R ≥ 4.1 (developed on 4.6 with RTools45 on Windows)

**Primary Dependencies**:

| Crate / Package | Purpose |
|-----------------|---------|
| `tokio`, `reqwest` (rustls, gzip/deflate/brotli) | Async style, tile, glyph and sprite downloads; no OpenSSL |
| `prost`, `flate2` | MVT protobuf decoding, gzip tiles |
| `image` | Raster tile decode, PNG encode |
| `csscolorparser`, `serde_json` | Style colours and JSON |
| `pmtiles` | PMTiles archives over HTTP range requests |
| `thiserror`, `once_cell`, `log`, `lru` | Errors, runtime, logging, caching |
| `httptest`, `resvg` (dev) | Golden tests: local fixture server, SVG rasterisation |
| `pyo3`, `maturin` | Python bindings and wheel |
| `extendr-api` | R bindings |
| `pyproj`, `numpy`, `xarray`, `matplotlib` (Python) | Reprojection, arrays, plotting |
| `sf`, `terra`, `httr2`, `jsonlite`, `ggplot2`, `grid` (R) | Reprojection, rasters, HTTP, plotting |
| `playwright` + `maplibre-gl` (Node, dev) | MapLibre GL JS reference renders |
| `great-docs` (Python, dev), `roxygen2` (R, dev) | Documentation |

**Storage**: In-memory only.

**Testing**: `cargo test -p basemapper-core` (unit + 168 golden-image cases
vs MapLibre GL JS), `pytest` (end-to-end against the golden fixtures),
`testthat` (live tiles, skipped offline).

**Target Platform**: Windows, macOS, Linux; CPU only (no GPU, display server
or browser at runtime).

**Project Type**: Multi-language library (Cargo workspace + Python wheel + R package).

**Performance Goals**: City-scale views (tens of tiles) in a few seconds,
dominated by network; the golden suite (168 cases) runs in ~20 s.

**Constraints**: No system GIS libraries; no browser; TLS via rustls; 256
tiles per style per render by default.

---

## Commands

```sh
# Rust core: unit, integration and golden tests (writes check/manifest.js)
cargo test -p basemapper-core
BASEMAPPER_GOLDEN_JOBS=4 cargo test -p basemapper-core --test golden -- --nocapture

# Capture a fixture and render its MapLibre GL JS reference
cargo run -p basemapper-core --example capture_fixture -- --name <case> --style <url> \
  --bbox=<xmin,ymin,xmax,ymax> --width 800 --height 600 --zoom <z>
cd tools/reference-render && npm install && npx playwright install chromium && node render.mjs <case>

# Export a view as SVG (+ PNG) from a centre and fractional zoom
cargo run --release -p basemapper-core --example export_svg -- --style <url> [--style <url> ...] \
  --center=<lon,lat> --zoom 12.4 --output map.svg --png map.png

# Python: build and test
cd py-basemapper && maturin build --release && pip install <wheel> && pytest

# R: install (syncs the vendored core) and test; use the renv library
R CMD INSTALL -l <renv library> r-basemapper
cd r-basemapper && NOT_CRAN=true Rscript -e 'testthat::test_dir("tests/testthat")'
```

---

## Constitution Check

### I. Cargo Workspace Monorepo ⚠️ DEVIATION (documented)

- Root workspace members: `core`, `py-basemapper`. `r-basemapper` is a separate
  workspace with a vendored snapshot of `core` (`r-basemapper/src/rust/core`),
  synced by `Makevars` on in-tree builds. See Complexity Tracking.
- No binding crate depends on another binding crate.

### II. Language Boundaries ✅ PASS

- `core/` is pure Rust: no C/C++ FFI, no GDAL/GEOS/PROJ, no GPU APIs.
- Python bindings use PyO3 + maturin; R bindings use extendr.

### III. Memory Safety & Concurrency ✅ PASS

- The core owns one tokio runtime; renders are synchronous at the FFI boundary.
- Python releases the GIL (`py.allow_threads`) during renders.
- extendr: return raw vectors directly or with attributes; building a `list!`
  around a raw vector crashed R (the vector was unprotected meanwhile).

### IV. Headless Output Standard ⚠️ DEVIATION (documented)

- Core returns `RenderResult { pixels, bounds, width, height, zoom_used }` or an
  SVG string; no UI or file-system side effects.
- Python `add_basemap()` returns `xarray.DataArray` (spatial metadata in `.attrs`).
- Sizes: callers give CSS pixels and a pixel ratio; plot integrations derive
  both from the plot (extent, panel size, output dpi) at draw time.

---

## Project Structure

```text
basemapper/
├── Cargo.toml / Cargo.lock
├── core/
│   ├── src/
│   │   ├── lib.rs            # render, render_stack, render_svg(_stack), encode_png, fetch helpers
│   │   ├── bbox.rs           # RenderRequest (+ pixel_ratio), validation, zoom, bbox_from_center, fit_bounds
│   │   ├── style.rs          # StyleInput, validation, layer filter
│   │   ├── tile_fetcher.rs   # tile URLs, concurrent fetch
│   │   ├── renderer.rs       # raster tile compositing
│   │   ├── error.rs
│   │   └── vector/           # MapLibre GL JS port
│   │       ├── mvt.rs        #   MVT decoding
│   │       ├── expr.rs       #   style expressions, legacy functions, colours
│   │       ├── gl.rs         #   rasteriser, framebuffer, LineBucket, line atlas
│   │       ├── render.rs     #   layer drawing (fill/line/circle), tile placement, SVG paths
│   │       ├── symbol.rs     #   glyphs, sprites, shaping, placement, collision, SDF, SVG labels
│   │       ├── symbol_line.rs#   labels along lines
│   │       └── svg.rs        #   SVG helpers, SDF contour tracing
│   ├── examples/             # capture_fixture, export_svg, headless_render
│   └── tests/                # golden.rs (+ fixtures/golden/<case>/), bbox_unit.rs, render_integration.rs
├── py-basemapper/
│   ├── src/lib.rs            # render_basemap_raw, render_raw, render_svg_raw, encode_png, camera helpers
│   └── src/basemapper/       # map.py (Map), matplotlib_integration.py (BasemapImage, add_basemap),
│                             # plotnine_integration.py, providers.py, bbox_utils.py, _render.py
├── r-basemapper/
│   ├── R/                    # basemap.R, geom_basemap.R, tmap_integration.R, providers.R, ...
│   ├── src/rust/src/lib.rs   # extendr functions
│   └── src/rust/core/        # vendored core snapshot
├── tools/reference-render/   # render.mjs (MapLibre GL JS references), shot_svg.mjs
├── docs/rendering-engine.md
└── check/                    # gitignored visual comparison viewer
```

---

## Implementation Hazards

### H1 — Matching MapLibre exactly

Fidelity depends on reproducing MapLibre GL JS's conventions, not just its
look: pixel-centre sampling with the top-left rule, float premultiplied
blending with round-half-to-even output, style evaluated at the fractional
zoom but tiles at `floor(zoom)`, `tileRatio = 512 * overscaling / 8192` for
dashes, non-interpolatable properties (`line-dasharray`) as intervals,
placement ordered by world copy first. Check every change with the golden
suite and the `check/` viewer; never loosen a case's limits.

### H2 — Pixel ratio

Placement and collision run in CSS pixels; drawing scales by the ratio while
antialiasing stays one device pixel (`1/dpr` terms in MapLibre's shaders).
Raster tiles snap to the CSS pixel grid. Verified by the `_x2` cases.

### H3 — Layer filter scope

`filter_style_layers()` modifies only the `layers` array; sources, sprite and
glyphs stay, so remaining layers keep their assets.

### H4 — Matplotlib z-ordering

The basemap image sits at `zorder=0`; the axes patch must be transparent
(`ax.set_facecolor("none")`). Set the image extent directly in the draw-time
artist (`set_extent` would change data limits).

### H5 — R vendored core

`r-basemapper/src/rust/core` must be committed in sync with `core/` (an
install from the repo syncs it; tarball installs use the committed copy).

---

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|--------------------------------------|
| **Constitution IV**: Python `add_basemap()` returns `xarray.DataArray`, not bare `ndarray` | `numpy.ndarray` cannot carry named spatial metadata without subclassing; `xarray.DataArray` gives `.attrs` + labelled dims | `ndarray` subclass is fragile; a dict loses the array interface |
| **Constitution I**: `r-basemapper` is a separate Cargo workspace | Vendoring lets the R package build from an `install_github()` tarball without the repo-root workspace | A single root workspace breaks standalone tarball builds; submodules complicate checkout for R users |
