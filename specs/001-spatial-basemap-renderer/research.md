# Research: Spatial Basemap Renderer

**Branch**: `001-spatial-basemap-renderer` | **Date**: 2026-06-08

---

## R1 — Rendering Engine: maplibre-rs + wgpu

**Decision**: Use `maplibre-rs` (the Rust port of MapLibre GL) backed by `wgpu` for
all vector tile and raster tile rendering.

**Rationale**: `maplibre-rs` natively understands the MapLibre GL style specification
(the industry standard for vector tile styling), parses MVT and raster tile formats,
and composes rendered tile quads into a single framebuffer. It is the only mature
pure-Rust rendering library that handles the full MapLibre GL style spec. `wgpu` is
a safe, cross-platform GPU abstraction in pure Rust that targets Vulkan, Metal, and
DX12 without requiring any system library installation on the end-user's machine —
these GPU APIs are OS-level facilities (Metal on macOS, DX12 on Windows, Vulkan on
Linux with a GPU).

**Alternatives considered**:
- *tiny-skia*: Pure Rust 2D rasterizer, but has no vector tile parser or style engine.
  Would require rebuilding the entire MapLibre GL rendering pipeline.
- *vello*: GPU-first 2D renderer but no tile/style pipeline; same problem.
- *C++ MapLibre GL Native via bindgen*: Would require a C++ toolchain and violates
  the constitution's Language Boundaries principle.

**Headless rendering on GPU-less servers**:
- macOS/Windows: `wgpu` uses Metal/DX12, which are OS frameworks (zero install).
- Linux with GPU: `wgpu` uses Vulkan drivers bundled with the GPU vendor driver.
- Linux headless / CI without GPU: Requires a Vulkan software renderer such as
  `lavapipe` (part of Mesa). Mesa is a system library on Linux, but it is
  universally available on any Linux desktop and most CI images. This is documented
  as a known constraint in `quickstart.md`; it does not violate "no GIS system
  library" because Mesa is a GPU abstraction, not a GIS dependency.
- Mitigation for minimal CI: Set the `WGPU_BACKEND=gl` environment variable and
  use the OpenGL backend, which works via EGL/X11 virtual framebuffers (`Xvfb`).

---

## R2 — Tile Fetching: tokio + reqwest

**Decision**: Use `tokio` (multi-thread runtime) and `reqwest` for all concurrent
HTTP tile fetching inside `core/`.

**Rationale**: Tile rendering requires fetching dozens of tiles concurrently. `tokio`
provides structured concurrency via `tokio::task::JoinSet`; `reqwest` provides async
HTTP/1.1 and HTTP/2 with TLS. Both are pure Rust (TLS via `rustls`). The R and Python
binding crates each own one `tokio::Runtime` initialized at library load time, ensuring
zero blocking of the host language's main thread.

**Alternatives considered**:
- *ureq (sync)*: Simpler but blocking; would require `spawn_blocking` and thread
  management, losing the benefits of structured async concurrency.
- *hyper directly*: More control but more boilerplate; `reqwest` wraps `hyper`
  with sensible defaults appropriate for this use case.

---

## R3 — CRS Reprojection Strategy

**Decision**: Reprojection to EPSG:3857 (Web Mercator) is performed by the host
language layer (R uses `sf`, Python uses `pyproj`). The Rust core receives coordinates
already in EPSG:3857 and never performs CRS arithmetic.

**Rationale**: Both `sf` (R) and `pyproj` (Python) are mature, battle-tested libraries
for CRS operations. Replicating proj4 math in Rust would introduce significant
complexity and maintenance burden. Keeping reprojection in the host layer also means
the Rust core has zero dependency on any GIS math library.

**Alternatives considered**:
- *proj-sys Rust crate*: Wraps the C PROJ library via bindgen — directly violates
  the Language Boundaries principle (C FFI dependency).
- *geo-types + proj (pure Rust)*: The `proj` crate's pure-Rust mode covers a subset
  of projections. For a research tool, this would be acceptable for WGS84↔WebMercator
  but risks failing on uncommon EPSG codes. Deferred to a future enhancement.

---

## R4 — Vector Tile Format Support

**Decision**: Support all three required formats via `maplibre-rs`'s built-in tile
pipeline with `prost`-generated protobuf decoders for MVT.

| Format | Mechanism | Rust crate(s) |
|--------|-----------|---------------|
| Raster XYZ | HTTP GET `{z}/{x}/{y}.png|jpg` → decode → composite | `reqwest` + `image` |
| Mapbox Vector Tiles (MVT) | HTTP GET protobuf → decode → render via style | `prost` + `maplibre-rs` |
| ESRI Vector Tiles | Same protobuf wire format as MVT; style via MapLibre GL JSON | `prost` + `maplibre-rs` |

**Rationale**: The ESRI Vector Tile format uses the same `.pbf` (protobuf) wire format
as Mapbox MVT. Differences are in the tile index URL and the style JSON structure.
`maplibre-rs` handles the rendering uniformly once the tiles are decoded; ESRI-specific
style JSON fields that have no MapLibre GL equivalent are ignored gracefully.

---

## R5 — Style Definition Format

**Decision**: Use the MapLibre GL Style Specification as the canonical style JSON
schema. Callers pass either a URL pointing to a remote style JSON or an inline JSON
string.

**Rationale**: MapLibre GL Style Spec is the industry standard supported natively by
`maplibre-rs`. All major tile providers (Mapbox, ESRI, OpenFreeMap, etc.) publish
styles in this format or a direct superset. Supporting a custom schema would add
unnecessary translation complexity.

**`style_input` dual-mode parsing rule** (applied in `core/`):
1. If the string starts with `{`, treat as inline JSON and parse directly.
2. Otherwise, treat as a URL, fetch via `reqwest`, parse the response body as JSON.

---

## R6 — Zoom Level Selection

**Decision**: Automatically compute the appropriate zoom level from the bounding box
and target pixel dimensions using the standard Web Mercator zoom formula. Callers may
pass an explicit `zoom` override.

**Formula** (for EPSG:3857 bbox):
```
bbox_width_m = xmax_3857 - xmin_3857
meters_per_pixel = bbox_width_m / width_px
zoom = log2(EARTH_CIRCUMFERENCE_M / (256 * meters_per_pixel))
zoom = clamp(floor(zoom), 0, 22)
```

**Max-tile guard**: If the computed zoom would require more than `max_tiles` (default
256) tile downloads, the zoom is reduced until the tile count fits. This prevents
accidental over-fetching for very large extents.

---

## R7 — Python and R Binding Architecture

**Python**:
- `maturin develop` / `maturin build --release` compiles the `py-basemapper` crate
  into a native `.pyd`/`.so` extension module.
- The PyO3 `#[pymodule]` exposes `render_basemap_raw(bbox_3857, width, height, style_input, zoom) -> bytes`.
- A thin `basemapper/matplotlib_integration.py` wraps this:
  - Extracts limits from `ax` and calls `pyproj.Transformer` to reproject to EPSG:3857.
  - Calls `render_basemap_raw`, converts to `numpy.ndarray` shaped `(H, W, 4)`.
  - Attaches spatial bounds as a named attribute.
  - Places the array under existing `ax` content via `ax.imshow(X, extent=..., zorder=0)`.

**R**:
- `rextendr::document()` / `cargo build` compiles the `r-basemapper` crate.
- The extendr `#[extendr]` macro exposes `render_basemap_raw(bbox_3857, width, height, style_input, zoom_int)` returning a raw integer vector.
- A `geom_basemap.R` file defines the `ggproto` subclass `GeomBasemap`:
  - `geom_basemap()` registers the geom; does NOT fetch tiles.
  - `draw_panel()` extracts limits from `panel_params`, uses `sf::st_transform()`
    to reproject to EPSG:3857, calls `render_basemap_raw`, reshapes to `matrix`,
    attaches the `SpatialBounds` attribute, and returns a `grid::rasterGrob`.

---

## R8 — Cross-Platform Build Matrix

| Platform | GPU Backend | Binaries |
|----------|-------------|---------|
| macOS arm64/x86_64 | Metal (OS framework) | Python wheel, R package |
| Windows x86_64 | DX12 (OS) | Python wheel, R package |
| Linux x86_64 (with GPU) | Vulkan | Python wheel, R package |
| Linux x86_64 (headless) | Vulkan (lavapipe) or GL (ANGLE) | Python wheel, R package |

**CI strategy**: GitHub Actions matrix across `ubuntu-latest`, `macos-latest`,
`windows-latest`. Linux headless tests run with `WGPU_BACKEND=gl` and a virtual
framebuffer. Python wheels distributed via `maturin publish`; R package via CRAN or
r-universe.
