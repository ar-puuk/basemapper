---
description: "Task list for Spatial Basemap Renderer implementation"
---

# Tasks: Spatial Basemap Renderer

**Input**: Design documents from `specs/001-spatial-basemap-renderer/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/ ✅

**Tests**: Not explicitly requested in spec — test tasks included in Phase 6 (US4 unit tests) and Phase 7 (Polish integration tests).

**Organization**: Tasks are grouped by user story to enable independent implementation
and testing. Each story can be developed, tested, and demonstrated independently once
the Foundational phase (Phase 2) is complete.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no shared dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- File paths are relative to the repository root

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Cargo workspace manifest, per-crate package configs, source stubs

- [ ] T001 Create workspace Cargo.toml at repo root declaring members = ["core", "py-basemapper", "r-basemapper"] and shared dependency version overrides
- [ ] T002 [P] Create core/Cargo.toml with package metadata and dependencies: maplibre-rs, wgpu (all backends), tokio (full), reqwest (rustls-tls), prost, image, thiserror, serde, serde_json
- [ ] T003 [P] Create py-basemapper/Cargo.toml (pyo3 + path dep on core) and py-basemapper/pyproject.toml (maturin build backend, numpy/pyproj/matplotlib dev dependencies)
- [ ] T004 [P] Create r-basemapper/Cargo.toml (extendr-api + path dep on core) and r-basemapper/DESCRIPTION (Package: basemapper, R ≥ 4.1, Imports: ggplot2, grid, sf)
- [ ] T005 Scaffold all empty source stubs so `cargo check --workspace` compiles: core/src/lib.rs, py-basemapper/src/lib.rs, r-basemapper/src/rust/src/lib.rs, py-basemapper/src/basemapper/__init__.py, r-basemapper/R/geom_basemap.R, r-basemapper/R/render_basemap_raw.R, r-basemapper/R/bbox_utils.R, r-basemapper/NAMESPACE

---

## Phase 2: Foundational (Rust Core — BLOCKS All User Stories)

**Purpose**: The complete Rust core must be implemented before any binding crate can
function. No user story work can begin until T011 is complete and `cargo test --workspace`
passes.

**⚠️ CRITICAL**: This phase must be fully completed before Phase 3.

- [ ] T006 Implement core/src/error.rs: define BasemapError enum with 8 variants (InvalidBbox, InvalidDimensions, StyleFetchFailed, StyleParseError, TileFetchFailed, TileDecodeError, RenderError, MaxTilesExceeded) using #[derive(thiserror::Error)]
- [ ] T007 [P] Implement core/src/bbox.rs: SpatialBounds struct, RenderRequest struct with all fields from data-model.md, RenderRequestBuilder, validate() method (bbox ordering, dimension ≤ 16384, zoom 0–22), zoom auto-computation formula (Web Mercator metres-per-pixel → log2), max_tiles guard that reduces zoom until tile count fits
- [ ] T008 [P] Implement core/src/style.rs: StyleInput enum with Url(String) and InlineJson(String) variants, StyleInput::from_str() constructor (starts-with-`{` heuristic), resolve(&reqwest::Client) async method that fetches URL or parses inline JSON, validate_maplibre_style() ensuring version==8 and sources key present
- [ ] T009 Implement core/src/tile_fetcher.rs: TileSource enum (XyzRaster, MapboxVectorTile, EsriVectorTile), build_tile_urls(source, bbox, zoom) → Vec<(TileCoord, String)>, fetch_all_tiles(client, urls, timeout_ms, concurrency) using tokio::task::JoinSet returning Vec<(TileCoord, TileData)> where TileData = Vec<u8>, auth_header injection for XyzRaster/ESRI and api_key query param for Mapbox
- [ ] T010 Implement core/src/renderer.rs: initialize_wgpu_headless() creating wgpu Instance + Adapter + Device + Queue targeting offscreen use, create_render_texture(device, width, height) → wgpu::Texture, setup_maplibre_map(device, queue, style_json, tiles) configuring maplibre-rs Map with ingested tile data, render_to_texture(map) → texture, read_texture_to_vec(device, queue, texture, width, height) → Vec<u8> (RGBA readback)
- [ ] T011 Implement core/src/lib.rs: pub fn render(request: RenderRequest) -> Result<RenderResult, BasemapError> orchestrating: (1) request.validate(), (2) StyleInput::resolve(), (3) fetch_all_tiles(), (4) render_to_texture(), (5) read_texture_to_vec(); construct RenderResult { pixels, bounds, width, height, zoom_used }; use std::cell::RefCell thread-local tokio Runtime for sync wrapper

**Checkpoint**: `cargo test --workspace` must pass with unit tests for bbox validation,
zoom formula, and style parsing before Phase 3 begins.

---

## Phase 3: User Story 1 — Auto-Infer Plot Extents (Priority: P1) 🎯 MVP

**Goal**: Both `add_basemap(ax)` in Python and `geom_basemap()` in R automatically
extract the bounding box and CRS from the live plot object, reproject to EPSG:3857,
and call the Rust core to return a correctly positioned basemap — with no manual bbox
argument required from the caller.

**Independent Test**: Execute Scenarios 3 (Python matplotlib) and 6 (R ggplot2) from
`quickstart.md` using a public tile endpoint. Both should produce a spatially aligned
basemap with zero `bbox` arguments passed by the user.

### Python Implementation (US1)

- [ ] T012 [P] [US1] Implement py-basemapper/src/basemapper/exceptions.py: BasemapError(RuntimeError) class with a constructor that accepts a message string; ensure the class is importable as `from basemapper import BasemapError`
- [ ] T013 [P] [US1] Implement py-basemapper/src/basemapper/bbox_utils.py: detect_crs_from_axes(ax) → pyproj.CRS (supports cartopy axes via ax.projection, GeoDataFrame-backed axes via ax._gdf_crs attribute, EPSG:4326 fallback with UserWarning); reproject_bbox_to_3857(xlim, ylim, crs) → (xmin_3857, ymin_3857, xmax_3857, ymax_3857) using pyproj.Transformer.from_crs
- [ ] T014 [US1] Implement py-basemapper/src/lib.rs: declare #[pymodule] fn basemapper(_py: Python, m: &PyModule); add #[pyfunction] render_basemap_raw(py, bbox_3857: Vec<f64>, width: u32, height: u32, style_input: &str, zoom: Option<u8>, tile_timeout_ms: Option<u32>, max_tiles: Option<u32>) -> PyResult<Py<PyBytes>>; call core::render() inside py.allow_threads(|| ...); map Err(BasemapError) to PyErr using the Python BasemapError exception class
- [ ] T015 [US1] Implement py-basemapper/src/basemapper/matplotlib_integration.py: add_basemap(ax, style_url, zoom=None, tile_timeout_ms=10_000, max_tiles=256) function that calls detect_crs_from_axes, reproject_bbox_to_3857, render_basemap_raw; converts returned bytes to numpy.ndarray shaped (H, W, 4) via np.frombuffer; attaches xmin/ymin/xmax/ymax/crs_epsg/zoom/width/height as ndarray.attrs; returns the ndarray
- [ ] T016 [US1] Update py-basemapper/src/basemapper/__init__.py: export add_basemap, render_basemap_raw (from .basemapper extension module), BasemapError; set __all__

### R Implementation (US1)

- [ ] T017 [P] [US1] Implement r-basemapper/R/bbox_utils.R: detect_crs_from_coord(coord) → integer EPSG code (extract from coord$crs via sf::st_crs, default 4326 with message); reproject_bbox_to_3857(xmin, ymin, xmax, ymax, from_epsg) → named numeric vector c(xmin, ymin, xmax, ymax) using sf::st_transform on a 2-point POINT geometry
- [ ] T018 [P] [US1] Implement r-basemapper/src/rust/src/lib.rs: #[extendr] fn render_basemap_raw(bbox_3857: Vec<f64>, width: i32, height: i32, style_input: &str, zoom: Nullable<i32>, tile_timeout_ms: i32, max_tiles: i32) -> Robj; convert args to RenderRequest; call core::render() on the package-level tokio Runtime (initialized in .onLoad via once_cell::sync::Lazy<tokio::Runtime>); on Err call rpanic! with error message; on Ok return Robj::from raw bytes Vec<u8>
- [ ] T019 [US1] Implement r-basemapper/R/render_basemap_raw.R: render_basemap_raw() R wrapper that calls the extendr Rust function; reshapes the returned raw vector to array(dim = c(4L, width, height)); transposes to c(height, width, 4) via aperm(); attaches xmin/ymin/xmax/ymax/crs_epsg/zoom attributes; converts to matrix with dim = c(height, width * 4L) for rasterGrob compatibility
- [ ] T020 [US1] Implement r-basemapper/R/geom_basemap.R: ggproto GeomBasemap <- ggproto("GeomBasemap", Geom, required_aes = character(0), draw_panel = function(data, panel_params, coord, style_url, zoom, tile_timeout, max_tiles) { detect CRS from coord; reproject x_range + y_range to EPSG:3857 via bbox_utils.R; call render_basemap_raw(); return grid::rasterGrob(m, ...) }); geom_basemap(style_url, zoom=NULL, tile_timeout=10000L, max_tiles=256L) factory function

**Checkpoint**: `add_basemap(ax)` and `geom_basemap()` both produce a correctly
positioned basemap with no manual `bbox` argument. Test with Scenario 3 and 6
from quickstart.md.

---

## Phase 4: User Story 2 — Native-Resolution Basemap Injection (Priority: P2)

**Goal**: The basemap pixel dimensions exactly match the target plot's pixel dimensions
(width × DPI for Python, device pixels for R). The basemap is injected as the bottom
layer (zorder=0 in Python, bottom grob in R) with no manual scaling required.

**Independent Test**: Execute Scenarios 2–3 (Python) and 5–6 (R) from `quickstart.md`
with explicit DPI targets (150 DPI, 6×4 inches). Verify returned ndarray/matrix
dimensions equal `width_in * dpi` × `height_in * dpi` (±2 px tolerance).

- [ ] T021 [P] [US2] Update py-basemapper/src/basemapper/bbox_utils.py: add get_axes_pixel_dims(ax) → (width_px: int, height_px: int) using ax.figure.canvas.draw_idle() to force layout, ax.get_window_extent(renderer=ax.figure.canvas.get_renderer()), multiply by ax.figure.dpi; handle un-drawn figure fallback
- [ ] T022 [US2] Update py-basemapper/src/basemapper/matplotlib_integration.py: call get_axes_pixel_dims(ax) and pass width_px, height_px to render_basemap_raw; inject result via ax.imshow(arr, extent=[xlim[0], xlim[1], ylim[0], ylim[1]], origin='upper', zorder=0, interpolation='nearest', aspect='auto'); assert arr.shape[:2] == (height_px, width_px)
- [ ] T023 [P] [US2] Update r-basemapper/R/geom_basemap.R: add get_panel_pixel_dims() helper inside draw_panel using grid::convertWidth(grid::unit(1,"npc"), "px", valueOnly=TRUE) and grid::convertHeight for height; coerce to integer; pass to render_basemap_raw
- [ ] T024 [US2] Update r-basemapper/R/geom_basemap.R draw_panel(): construct grid::rasterGrob(m, x=0.5, y=0.5, width=grid::unit(1,"npc"), height=grid::unit(1,"npc"), default.units="npc", interpolate=FALSE) ensuring the grob fills the panel viewport exactly at native pixel resolution

**Checkpoint**: Rendered basemap pixel dimensions exactly match target plot dimensions
for both Python (1200×900 at 150 DPI → 1200×900 px array) and R (rasterGrob fills
panel with zero letterboxing).

---

## Phase 5: User Story 3 — Multi-Format Tile Sources and Custom Styling (Priority: P3)

**Goal**: Callers can select from XYZ raster, Mapbox Vector Tile, or ESRI Vector Tile
sources and apply a custom MapLibre GL JSON style to control visual appearance. All
three formats render correctly; a greyscale style visibly desaturates the map.

**Independent Test**: Execute Scenarios 1 (Rust headless CLI), 4 (greyscale style),
7 (malformed style error), and 8 (network timeout error) from `quickstart.md`. All
three tile source variants must render a recognizable city-scale basemap.

- [ ] T025 [P] [US3] Complete and verify core/src/tile_fetcher.rs for all three TileSource variants: test XyzRaster URL template expansion ({z}/{x}/{y}), MapboxVectorTile with access_token query param appended, EsriVectorTile with Bearer auth_header in reqwest headers; ensure each variant's decoded bytes are passed to maplibre-rs tile ingestion separately
- [ ] T026 [P] [US3] Complete core/src/style.rs validation: after checking version==8 and sources present, validate layers array is non-empty; for each layer validate "id" and "type" fields; return StyleParseError("layer[N]: missing required field 'type'") with index and field name for debuggability
- [ ] T027 [US3] Update py-basemapper/src/lib.rs: add tile_source parameter to render_basemap_raw as an optional Python dict with keys "type" (xyz|mvt|esri), "url_template", "auth_header" (optional), "api_key" (optional); serialize to Rust TileSource variant; update docstring and BasemapError mapping
- [ ] T028 [US3] Update py-basemapper/src/basemapper/matplotlib_integration.py: add tile_source=None parameter to add_basemap(); pass through to render_basemap_raw; document usage with Mapbox and ESRI examples in docstring
- [ ] T029 [US3] Update r-basemapper/R/geom_basemap.R: add tile_source=NULL parameter to geom_basemap(); accept named list with names c("type","url_template","auth_header","api_key"); serialize to extendr rust function; add roxygen2 @param documentation with Mapbox/ESRI examples
- [ ] T030 [US3] Add core/tests/render_integration.rs: three integration tests (one per tile source type) using a mock HTTP server (httptest crate) that serves a minimal valid PNG tile, MVT protobuf, and ESRI tile; assert RenderResult pixels length == width * height * 4 and zoom_used is in expected range

**Checkpoint**: All three tile formats render correctly; custom JSON style with
`"raster-saturation": -1` produces a visually greyscale output (verified by Scenario 4).

---

## Phase 6: User Story 4 — Provider Generator Helpers (Priority: P4)

**Goal**: Data scientists can call a single helper with a bare tile URL — no MapLibre
GL JSON knowledge required — and pass the returned string directly to `add_basemap()`
or `geom_basemap()`. `VectorProvider` accepts an optional paint dictionary to control
fill and line styling of raw vector geometries.

**Independent Test**: Call `RasterProvider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")`
(Python) or `raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")` (R)
and pass the result to `add_basemap` / `geom_basemap` without any other style argument.
Map renders with a single helper call and zero MapLibre GL JSON written by the user.

### Python Implementation (US4)

- [ ] T038 [P] [US4] Create py-basemapper/src/basemapper/providers.py: implement `RasterProvider(url_template, tile_size=256)` with `__str__`/`to_style_json()` returning MapLibre GL JSON (version 8, raster source, raster layer); implement `EsriVectorProvider(base_url)` with `__str__` that strips any trailing slash from `base_url` and returns `f"{base_url.rstrip('/')}/resources/styles/root.json"` as a plain URL string (not inline JSON — the Rust core fetches it); implement `EsriRasterProvider(url_template, tile_size=256)` with `__str__`/`to_style_json()` returning MapLibre GL JSON (version 8, raster source with the provided URL and `tileSize`, raster layer); implement `VectorProvider(url_template, paint=None)` splitting paint keys into fill-family (`fill-color`, `fill-opacity`, `fill-outline-color`) and line-family (`line-color`, `line-width`, `line-opacity`) layers, emitting `warnings.warn` for unrecognised keys, using default grey fill/line when paint is absent
- [ ] T039 [US4] Update py-basemapper/src/basemapper/__init__.py: export `RasterProvider`, `VectorProvider`, `EsriVectorProvider`, `EsriRasterProvider`; add all four to `__all__`
- [ ] T040 [P] [US4] Create py-basemapper/tests/test_providers.py: unit tests covering (a) `RasterProvider` returns valid inline JSON with correct source URL, (b) `EsriVectorProvider("https://example.com/VectorTileServer")` returns `"https://example.com/VectorTileServer/resources/styles/root.json"` and `EsriVectorProvider("https://example.com/VectorTileServer/")` (trailing slash) returns the same URL without double slash, (c) `VectorProvider` with paint dict produces fill + line layers with correct paint values, (d) `VectorProvider` with no paint uses default grey values, (e) unrecognised paint key emits `UserWarning` and is absent from output JSON, (f) `EsriRasterProvider("https://example.com/MapServer/tile/{z}/{y}/{x}")` returns valid inline JSON with raster source and `tileSize: 256`, (g) provider output passable to `render_basemap_raw` without raising (mock HTTP)

### R Implementation (US4)

- [ ] T041 [P] [US4] Create r-basemapper/R/providers.R: implement `raster_provider(url_template, tile_size = 256L)` returning a JSON character string serialised via `jsonlite::toJSON(auto_unbox = TRUE)`; implement `esri_vector_provider(base_url)` returning `paste0(sub("/*$", "", base_url), "/resources/styles/root.json")` as a plain URL character string (not JSON — fetched by Rust core); implement `esri_raster_provider(url_template, tile_size = 256L)` returning MapLibre GL JSON with a raster source (tileSize = tile_size) serialised via `jsonlite::toJSON(auto_unbox = TRUE)`; implement `vector_provider(url_template, paint = list())` splitting paint list entries into fill-layer and line-layer properties, calling `warning()` for unrecognised keys, applying default grey palette when paint is empty, serialised via `jsonlite::toJSON(auto_unbox = TRUE)`
- [ ] T042 [P] [US4] Create r-basemapper/tests/testthat/test-providers.R: unit tests covering (a) `raster_provider` returns parseable JSON with correct tile URL, (b) `esri_vector_provider("https://example.com/VectorTileServer")` returns `"https://example.com/VectorTileServer/resources/styles/root.json"` and `esri_vector_provider("https://example.com/VectorTileServer/")` (trailing slash) returns identical URL without double slash, (c) `vector_provider` with named paint list produces correct layer paint entries, (d) unrecognised paint key triggers a `warning()`, (e) `vector_provider()` with empty list uses default grey values, (f) `esri_raster_provider("https://example.com/MapServer/tile/{z}/{y}/{x}")` returns parseable JSON with raster source and `tileSize: 256`

**Checkpoint**: `RasterProvider("...")` (Python) and `raster_provider("...")` (R) each
return a JSON string passable directly to `render_basemap_raw`; `VectorProvider` with
a paint dict produces visibly styled layers; all unit tests pass.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: CLI example, test suites, linting, full quickstart validation

- [ ] T031 [P] Create core/examples/headless_render.rs: clap-based CLI accepting --bbox (comma-separated f64×4), --width, --height, --style (URL or inline JSON), --output (PNG path); call core::render(); encode Vec<u8> to PNG via the image crate; validates Scenario 1 from quickstart.md
- [ ] T032 [P] Create core/tests/bbox_unit.rs: unit tests covering (a) valid RenderRequest builds successfully, (b) xmin≥xmax returns InvalidBbox, (c) width=0 returns InvalidDimensions, (d) zoom=23 returns InvalidBbox, (e) zoom auto-formula produces expected value for a known bbox+dims, (f) max_tiles guard reduces zoom correctly
- [ ] T033 [P] Create py-basemapper/tests/test_render.py: pytest tests for Scenarios 2, 4, 7, 8 from quickstart.md; use responses library or unittest.mock to mock HTTP tile endpoints for Scenarios 2 and 4; Scenario 7 passes inline JSON with version=7; Scenario 8 uses a deliberately unreachable URL with tile_timeout_ms=1000
- [ ] T034 [P] Create r-basemapper/tests/testthat/test-render.R: testthat tests for Scenarios 5–6 from quickstart.md; Scenario 5 verifies length(raw) == 400*300*4; Scenario 6 saves to a temp file and verifies file.info()$size > 50000
- [ ] T035 [P] Run rextendr::document() to regenerate r-basemapper/NAMESPACE and r-basemapper/man/*.Rd files; verify `R CMD check r-basemapper` exits with 0 errors and 0 warnings
- [ ] T036 Run `cargo clippy --workspace -- -D warnings` and resolve all lint violations; run `cargo fmt --all` for consistent formatting across all Rust source files
- [ ] T037 Execute all 8 quickstart.md validation scenarios end-to-end on a clean checkout (no pre-built binaries); document any environment-specific setup steps discovered (e.g., Mesa install on headless Linux) in r-basemapper/README.md and py-basemapper/README.md

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately; T002–T005 can run in parallel after T001
- **Foundational (Phase 2)**: Depends on Phase 1 completion — **BLOCKS all user stories**
  - T007, T008 can run in parallel after T006
  - T009 depends on T007 (bbox) and T008 (style)
  - T010 depends on T009 (needs tile data types)
  - T011 depends on T006–T010
- **User Story 1 (Phase 3)**: Depends on T011 — Python T012, T013, T017, T018 can run in parallel
- **User Story 2 (Phase 4)**: Depends on Phase 3 completion (updates existing files)
- **User Story 3 (Phase 5)**: Depends on Phase 3 completion — T025, T026 can run in parallel
- **User Story 4 (Phase 6)**: Depends on Phase 1 (package structure); T038, T040, T041 can run in parallel after T016 (Python __init__ exported); T039, T042 can run in parallel with T038
- **Polish (Phase 7)**: Depends on all user story phases; T031–T035 can run in parallel

### User Story Dependencies

- **US1 (P1)**: Can start immediately after Foundational — no inter-story dependencies
- **US2 (P2)**: Depends on US1 (updates the same add_basemap/draw_panel functions)
- **US3 (P3)**: Depends on US1 (adds tile_source param to same functions); T025–T026 (Rust core) can run in parallel with Phase 3
- **US4 (P4)**: Pure host-language code — no Rust changes. Depends on Python `__init__.py` (T016) and R package structure (T004). Can run in parallel with Phase 4–5 after Phase 3 completes.

### Within Each User Story

- Python and R implementation tasks are independent and can run in parallel within the same phase
- Within each language: exceptions/bbox_utils → core binding (lib.rs) → R/Python wrappers
- Models before services; services before integration

### Parallel Opportunities

```bash
# Phase 1 — after T001 completes:
T002, T003, T004 run in parallel

# Phase 2 — after T006:
T007, T008 run in parallel

# Phase 3 — after T011:
# Python side:
T012, T013 run in parallel
# R side (simultaneously with Python):
T017, T018 run in parallel
# T014 depends on T012+T013; T015 depends on T014
# T019 depends on T018; T020 depends on T017+T019

# Phase 5 — T025, T026, T027 can start in parallel after Phase 3 checkpoint
# Phase 6 (US4) — after T016 completes (Python) and T004 (R package structure):
T038, T040 run in parallel (Python providers + tests)
T041, T042 run in parallel (R providers + tests)
# Phase 7 — T031, T032, T033, T034, T035 all run in parallel after Phases 5+6
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001–T005)
2. Complete Phase 2: Foundational Rust Core (T006–T011) — **critical gate**
3. Complete Phase 3: User Story 1 — auto-infer bbox (T012–T020)
4. **STOP and VALIDATE**: Run Scenarios 3 and 6 from quickstart.md
5. If basemap renders with auto-inferred extents → MVP demonstrated

### Incremental Delivery

1. Phase 1 + 2 → Rust core compiles and tests pass
2. Phase 3 → `add_basemap(ax)` and `geom_basemap()` work (MVP!)
3. Phase 4 → pixel-perfect DPI matching → demo at any resolution
4. Phase 5 → Mapbox/ESRI tile support + custom styles → full feature set
5. Phase 6 → Provider helpers → basemapper works from bare tile URLs
6. Phase 7 → Polish + test coverage → release-ready

### Parallel Team Strategy

With two developers after Phase 2 completes:

- **Developer A** (Python): T012–T016 (US1 Python), T021–T022 (US2 Python), T027–T028 (US3 Python), T038–T040 (US4 Python)
- **Developer B** (R): T017–T020 (US1 R), T023–T024 (US2 R), T029 (US3 R), T041–T042 (US4 R)
- **Either**: T025–T026 (Rust core tile format completions, no language preference)

---

## Notes

- [P] tasks operate on different files with no shared mutable state — safe to parallelize
- Story label maps each task to a specific user story for traceability and independent testing
- The Rust core (Phase 2) is the single blocking dependency — prioritize it
- `cargo check --workspace` should pass after Phase 1; `cargo test --workspace` after Phase 2
- Avoid modifying the same .R or .py file from parallel tasks; per-file ownership is clear
- Stop at each Checkpoint to validate story independence before proceeding
