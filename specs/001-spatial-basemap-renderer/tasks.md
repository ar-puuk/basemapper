---
description: "Task list for Spatial Basemap Renderer implementation"
---

# Tasks: Spatial Basemap Renderer

**Input**: Design documents from `specs/001-spatial-basemap-renderer/`

**Prerequisites**: plan.md âœ… | spec.md âœ… | research.md âœ… | data-model.md âœ… | contracts/ âœ…

**Tests**: Not explicitly requested in spec â€” test tasks included in Phase 6 (US4 unit tests) and Phase 7 (Polish integration tests).

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

- [X] T001 Create workspace Cargo.toml at repo root declaring members = ["core", "py-basemapper", "r-basemapper"] and shared dependency version overrides
- [X] T002 [P] Create core/Cargo.toml with package metadata and dependencies: maplibre-rs, wgpu (all backends), tokio (full), reqwest (rustls-tls), prost, image, thiserror, serde, serde_json
- [X] T003 [P] Create py-basemapper/Cargo.toml (pyo3 + path dep on core) and py-basemapper/pyproject.toml (maturin build backend; runtime deps: numpy, pyproj, matplotlib; optional extras: `plotnine = ["plotnine>=0.12"]`; dev deps: great-docs, pytest, responses, plotnine)
- [X] T004 [P] Create r-basemapper/Cargo.toml (extendr-api + path dep on core) and r-basemapper/DESCRIPTION (Package: basemapper, R â‰¥ 4.1, Imports: ggplot2, grid, sf, jsonlite; Suggests: roxygen2, testthat, rextendr, tmap (>= 4.0), stars)
- [X] T005 Scaffold all empty source stubs so `cargo check --workspace` compiles: core/src/lib.rs, py-basemapper/src/lib.rs, r-basemapper/src/rust/src/lib.rs, py-basemapper/src/basemapper/__init__.py, r-basemapper/R/geom_basemap.R, r-basemapper/R/render_basemap_raw.R, r-basemapper/R/bbox_utils.R, r-basemapper/NAMESPACE

---

## Phase 2: Foundational (Rust Core â€” BLOCKS All User Stories)

**Purpose**: The complete Rust core must be implemented before any binding crate can
function. No user story work can begin until T011 is complete and `cargo test --workspace`
passes.

**âš ï¸ CRITICAL**: This phase must be fully completed before Phase 3.

- [X] T006 Implement core/src/error.rs: define BasemapError enum with 9 variants (InvalidBbox, InvalidDimensions, StyleFetchFailed, StyleParseError, TileFetchFailed, TileDecodeError, RenderError, MaxTilesExceeded, InvalidLayerFilter) using #[derive(thiserror::Error)]
- [X] T007 [P] Implement core/src/bbox.rs: SpatialBounds struct, RenderRequest struct with all fields from data-model.md, RenderRequestBuilder, validate() method (bbox ordering, dimension â‰¤ 16384, zoom 0â€“22), zoom auto-computation formula (Web Mercator metres-per-pixel â†’ log2), max_tiles guard that reduces zoom until tile count fits
- [X] T008 [P] Implement core/src/style.rs: StyleInput enum with Url(String) and InlineJson(String) variants, StyleInput::from_str() constructor (starts-with-`{` heuristic), resolve(&reqwest::Client) async method that fetches URL or parses inline JSON, validate_maplibre_style() ensuring version==8 and sources key present
- [X] T009 Implement core/src/tile_fetcher.rs: TileSource enum (XyzRaster, MapboxVectorTile, EsriVectorTile), build_tile_urls(source, bbox, zoom) â†’ Vec<(TileCoord, String)>, fetch_all_tiles(client, urls, timeout_ms, concurrency) using tokio::task::JoinSet returning Vec<(TileCoord, TileData)> where TileData = Vec<u8>, auth_header injection for XyzRaster/ESRI and api_key query param for Mapbox
- [X] T010 Implement core/src/renderer.rs: initialize_wgpu_headless() creating wgpu Instance + Adapter + Device + Queue targeting offscreen use, create_render_texture(device, width, height) â†’ wgpu::Texture, setup_maplibre_map(device, queue, style_json, tiles) configuring maplibre-rs Map with ingested tile data, render_to_texture(map) â†’ texture, read_texture_to_vec(device, queue, texture, width, height) â†’ Vec<u8> (RGBA readback) âš ï¸ H2: GPU readback via map_async() is asynchronous â€” call device.poll(wgpu::Maintain::Wait) immediately after map_async() to block synchronously; do NOT await or spawn a tokio task here or it deadlocks against the thread-local runtime in T011
- [X] T011 Implement core/src/lib.rs: pub fn render(request: RenderRequest) -> Result<RenderResult, BasemapError> orchestrating: (1) request.validate(), (2) StyleInput::resolve(), (3) fetch_all_tiles(), (4) render_to_texture(), (5) read_texture_to_vec(); construct RenderResult { pixels, bounds, width, height, zoom_used }; use std::cell::RefCell thread-local tokio Runtime for sync wrapper

**Checkpoint**: `cargo test --workspace` must pass with unit tests for bbox validation,
zoom formula, and style parsing before Phase 3 begins.

---

## Phase 3: User Story 1 â€” Auto-Infer Plot Extents (Priority: P1) ðŸŽ¯ MVP

**Goal**: Both `add_basemap(ax)` in Python and `geom_basemap()` in R automatically
extract the bounding box and CRS from the live plot object, reproject to EPSG:3857,
and call the Rust core to return a correctly positioned basemap â€” with no manual bbox
argument required from the caller.

**Independent Test**: Execute Scenarios 3 (Python matplotlib) and 6 (R ggplot2) from
`quickstart.md` using a public tile endpoint. Both should produce a spatially aligned
basemap with zero `bbox` arguments passed by the user.

### Python Implementation (US1)

- [X] T012 [P] [US1] Implement py-basemapper/src/basemapper/exceptions.py: BasemapError(RuntimeError) class with a constructor that accepts a message string; ensure the class is importable as `from basemapper import BasemapError`
- [X] T013 [P] [US1] Implement py-basemapper/src/basemapper/bbox_utils.py: detect_crs_from_axes(ax) â†’ pyproj.CRS (supports cartopy axes via ax.projection, GeoDataFrame-backed axes via ax._gdf_crs attribute, EPSG:4326 fallback with UserWarning); reproject_bbox_to_3857(xlim, ylim, crs) â†’ (xmin_3857, ymin_3857, xmax_3857, ymax_3857) using pyproj.Transformer.from_crs
- [X] T014 [US1] Implement py-basemapper/src/lib.rs: declare #[pymodule] fn basemapper(_py: Python, m: &PyModule); add #[pyfunction] render_basemap_raw(py, bbox_3857: Vec<f64>, width: u32, height: u32, style_input: &str, zoom: Option<u8>, tile_timeout_ms: Option<u32>, max_tiles: Option<u32>) -> PyResult<Py<PyBytes>>; call core::render() inside py.allow_threads(|| ...); map Err(BasemapError) to PyErr using the Python BasemapError exception class
- [X] T015 [US1] Implement py-basemapper/src/basemapper/matplotlib_integration.py: add_basemap(ax, style_url, zoom=None, tile_timeout_ms=10_000, max_tiles=256, alpha=1.0) function that calls detect_crs_from_axes, reproject_bbox_to_3857, render_basemap_raw; converts returned bytes to numpy.ndarray shaped (H, W, 4) via np.frombuffer; attaches xmin/ymin/xmax/ymax/crs_epsg/zoom/width/height as ndarray.attrs; returns the ndarray
- [X] T016 [US1] Update py-basemapper/src/basemapper/__init__.py: export add_basemap, render_basemap_raw (from .basemapper extension module), BasemapError; set __all__

### R Implementation (US1)

- [X] T017 [P] [US1] Implement r-basemapper/R/bbox_utils.R: detect_crs_from_coord(coord) â†’ integer EPSG code (extract from coord$crs via sf::st_crs, default 4326 with message); reproject_bbox_to_3857(xmin, ymin, xmax, ymax, from_epsg) â†’ named numeric vector c(xmin, ymin, xmax, ymax) using sf::st_transform on a 2-point POINT geometry
- [X] T018 [P] [US1] Implement r-basemapper/src/rust/src/lib.rs: #[extendr] fn render_basemap_raw(bbox_3857: Vec<f64>, width: i32, height: i32, style_input: &str, zoom: Nullable<i32>, tile_timeout_ms: i32, max_tiles: i32) -> Robj; convert args to RenderRequest; call core::render() on the package-level tokio Runtime (initialized in .onLoad via once_cell::sync::Lazy<tokio::Runtime>); on Err call rpanic! with error message; on Ok return Robj::from raw bytes Vec<u8>
- [X] T019 [US1] Implement r-basemapper/R/render_basemap_raw.R: render_basemap_raw() R wrapper that calls the extendr Rust function; reshapes the returned raw vector to array(dim = c(4L, width, height)); transposes to c(height, width, 4) via aperm(); attaches xmin/ymin/xmax/ymax/crs_epsg/zoom attributes; converts to matrix with dim = c(height, width * 4L) for rasterGrob compatibility
- [X] T020 [US1] Implement r-basemapper/R/geom_basemap.R: ggproto GeomBasemap <- ggproto("GeomBasemap", Geom, required_aes = character(0), draw_panel = function(data, panel_params, coord, style_url, zoom, tile_timeout, max_tiles, alpha) { detect CRS from coord; reproject x_range + y_range to EPSG:3857 via bbox_utils.R; call render_basemap_raw(); return grid::rasterGrob(m, ...) }); geom_basemap(style_url, zoom=NULL, tile_timeout=10000L, max_tiles=256L, alpha=1) factory function

**Checkpoint**: `add_basemap(ax)` and `geom_basemap()` both produce a correctly
positioned basemap with no manual `bbox` argument. Test with Scenario 3 and 6
from quickstart.md.

---

## Phase 4: User Story 2 â€” Native-Resolution Basemap Injection (Priority: P2)

**Goal**: The basemap pixel dimensions exactly match the target plot's pixel dimensions
(width Ã— DPI for Python, device pixels for R). The basemap is injected as the bottom
layer (zorder=0 in Python, bottom grob in R) with no manual scaling required.

**Independent Test**: Execute Scenarios 2â€“3 (Python) and 5â€“6 (R) from `quickstart.md`
with explicit DPI targets (150 DPI, 6Ã—4 inches). Verify returned ndarray/matrix
dimensions equal `width_in * dpi` Ã— `height_in * dpi` (Â±2 px tolerance).

- [X] T021 [P] [US2] Update py-basemapper/src/basemapper/bbox_utils.py: add get_axes_pixel_dims(ax) â†’ (width_px: int, height_px: int) using ax.figure.canvas.draw_idle() to force layout, ax.get_window_extent(renderer=ax.figure.canvas.get_renderer()), multiply by ax.figure.dpi; handle un-drawn figure fallback
- [X] T022 [US2] Update py-basemapper/src/basemapper/matplotlib_integration.py: call get_axes_pixel_dims(ax) and pass width_px, height_px to render_basemap_raw; inject result via ax.imshow(arr, extent=[xlim[0], xlim[1], ylim[0], ylim[1]], origin='upper', zorder=0, interpolation='nearest', aspect='auto', alpha=alpha); assert arr.shape[:2] == (height_px, width_px) âš ï¸ H4: if basemap renders invisible, ax.patch also sits at zorder=0 â€” fix by using zorder=0.5 and calling ax.patch.set_visible(False) or ax.set_facecolor('none')
- [X] T023 [P] [US2] Update r-basemapper/R/geom_basemap.R: add get_panel_pixel_dims() helper inside draw_panel using grid::convertWidth(grid::unit(1,"npc"), "px", valueOnly=TRUE) and grid::convertHeight for height; coerce to integer; pass to render_basemap_raw
- [X] T024 [US2] Update r-basemapper/R/geom_basemap.R draw_panel(): construct grid::rasterGrob(m, x=0.5, y=0.5, width=grid::unit(1,"npc"), height=grid::unit(1,"npc"), default.units="npc", interpolate=FALSE, gp=grid::gpar(alpha=alpha)) ensuring the grob fills the panel viewport exactly at native pixel resolution

**Checkpoint**: Rendered basemap pixel dimensions exactly match target plot dimensions
for both Python (1200Ã—900 at 150 DPI â†’ 1200Ã—900 px array) and R (rasterGrob fills
panel with zero letterboxing).

---

## Phase 5: User Story 3 â€” Multi-Format Tile Sources and Custom Styling (Priority: P3)

**Goal**: Callers can select from XYZ raster, Mapbox Vector Tile, or ESRI Vector Tile
sources and apply a custom MapLibre GL JSON style to control visual appearance. All
three formats render correctly; a greyscale style visibly desaturates the map.

**Independent Test**: Execute Scenarios 1 (Rust headless CLI), 4 (greyscale style),
7 (malformed style error), and 8 (network timeout error) from `quickstart.md`. All
three tile source variants must render a recognizable city-scale basemap.

- [X] T025 [P] [US3] Complete and verify core/src/tile_fetcher.rs for all three TileSource variants: test XyzRaster URL template expansion ({z}/{x}/{y}), MapboxVectorTile with access_token query param appended, EsriVectorTile with Bearer auth_header in reqwest headers; ensure each variant's decoded bytes are passed to maplibre-rs tile ingestion separately
- [X] T026 [P] [US3] Complete core/src/style.rs validation: after checking version==8 and sources present, validate layers array is non-empty; for each layer validate "id" and "type" fields; return StyleParseError("layer[N]: missing required field 'type'") with index and field name for debuggability
- [X] T027 [US3] Update py-basemapper/src/lib.rs: add tile_source parameter to render_basemap_raw as an optional Python dict with keys "type" (xyz|mvt|esri), "url_template", "auth_header" (optional), "api_key" (optional); serialize to Rust TileSource variant; update docstring and BasemapError mapping
- [X] T028 [US3] Update py-basemapper/src/basemapper/matplotlib_integration.py: add tile_source=None parameter to add_basemap(); pass through to render_basemap_raw; document usage with Mapbox and ESRI examples in docstring
- [X] T029 [US3] Update r-basemapper/R/geom_basemap.R: add tile_source=NULL parameter to geom_basemap(); accept named list with names c("type","url_template","auth_header","api_key"); serialize to extendr rust function; add roxygen2 @param documentation with Mapbox/ESRI examples
- [X] T030 [US3] Add core/tests/render_integration.rs: three integration tests (one per tile source type) using a mock HTTP server (httptest crate) that serves a minimal valid PNG tile, MVT protobuf, and ESRI tile; assert RenderResult pixels length == width * height * 4 and zoom_used is in expected range

**Checkpoint**: All three tile formats render correctly; custom JSON style with
`"raster-saturation": -1` produces a visually greyscale output (verified by Scenario 4).

---

## Phase 6: User Story 4 â€” Provider Generator Helpers (Priority: P4)

**Goal**: Data scientists can call a single helper with a bare tile URL â€” no MapLibre
GL JSON knowledge required â€” and pass the returned string directly to `add_basemap()`
or `geom_basemap()`. `VectorProvider` accepts an optional paint dictionary to control
fill and line styling of raw vector geometries.

**Independent Test**: Call `RasterProvider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")`
(Python) or `raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")` (R)
and pass the result to `add_basemap` / `geom_basemap` without any other style argument.
Map renders with a single helper call and zero MapLibre GL JSON written by the user.

### Python Implementation (US4)

- [X] T038 [P] [US4] Create py-basemapper/src/basemapper/providers.py: implement `RasterProvider(url_template, tile_size=256)` with `__str__`/`to_style_json()` returning MapLibre GL JSON (version 8, raster source, raster layer); implement `EsriVectorProvider(base_url)` with `__str__` that strips any trailing slash from `base_url` and returns `f"{base_url.rstrip('/')}/resources/styles/root.json"` as a plain URL string (not inline JSON â€” the Rust core fetches it); implement `EsriRasterProvider(url_template, tile_size=256)` with `__str__`/`to_style_json()` returning MapLibre GL JSON (version 8, raster source with the provided URL and `tileSize`, raster layer); implement `VectorProvider(url_template, paint=None)` splitting paint keys into fill-family (`fill-color`, `fill-opacity`, `fill-outline-color`) and line-family (`line-color`, `line-width`, `line-opacity`) layers, emitting `warnings.warn` for unrecognised keys, using default grey fill/line when paint is absent
- [X] T039 [US4] Update py-basemapper/src/basemapper/__init__.py: export `RasterProvider`, `VectorProvider`, `EsriVectorProvider`, `EsriRasterProvider`; add all four to `__all__`
- [X] T040 [P] [US4] Create py-basemapper/tests/test_providers.py: unit tests covering (a) `RasterProvider` returns valid inline JSON with correct source URL, (b) `EsriVectorProvider("https://example.com/VectorTileServer")` returns `"https://example.com/VectorTileServer/resources/styles/root.json"` and `EsriVectorProvider("https://example.com/VectorTileServer/")` (trailing slash) returns the same URL without double slash, (c) `VectorProvider` with paint dict produces fill + line layers with correct paint values, (d) `VectorProvider` with no paint uses default grey values, (e) unrecognised paint key emits `UserWarning` and is absent from output JSON, (f) `EsriRasterProvider("https://example.com/MapServer/tile/{z}/{y}/{x}")` returns valid inline JSON with raster source and `tileSize: 256`, (g) provider output passable to `render_basemap_raw` without raising (mock HTTP)

### R Implementation (US4)

- [X] T041 [P] [US4] Create r-basemapper/R/providers.R: implement `raster_provider(url_template, tile_size = 256L)` returning a JSON character string serialised via `jsonlite::toJSON(auto_unbox = TRUE)`; implement `esri_vector_provider(base_url)` returning `paste0(sub("/*$", "", base_url), "/resources/styles/root.json")` as a plain URL character string (not JSON â€” fetched by Rust core); implement `esri_raster_provider(url_template, tile_size = 256L)` returning MapLibre GL JSON with a raster source (tileSize = tile_size) serialised via `jsonlite::toJSON(auto_unbox = TRUE)`; implement `vector_provider(url_template, paint = list())` splitting paint list entries into fill-layer and line-layer properties, calling `warning()` for unrecognised keys, applying default grey palette when paint is empty, serialised via `jsonlite::toJSON(auto_unbox = TRUE)`
- [X] T042 [P] [US4] Create r-basemapper/tests/testthat/test-providers.R: unit tests covering (a) `raster_provider` returns parseable JSON with correct tile URL, (b) `esri_vector_provider("https://example.com/VectorTileServer")` returns `"https://example.com/VectorTileServer/resources/styles/root.json"` and `esri_vector_provider("https://example.com/VectorTileServer/")` (trailing slash) returns identical URL without double slash, (c) `vector_provider` with named paint list produces correct layer paint entries, (d) unrecognised paint key triggers a `warning()`, (e) `vector_provider()` with empty list uses default grey values, (f) `esri_raster_provider("https://example.com/MapServer/tile/{z}/{y}/{x}")` returns parseable JSON with raster source and `tileSize: 256`

**Checkpoint**: `RasterProvider("...")` (Python) and `raster_provider("...")` (R) each
return a JSON string passable directly to `render_basemap_raw`; `VectorProvider` with
a paint dict produces visibly styled layers; all unit tests pass.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: CLI example, test suites, linting, full quickstart validation

- [X] T031 [P] Create core/examples/headless_render.rs: clap-based CLI accepting --bbox (comma-separated f64Ã—4), --width, --height, --style (URL or inline JSON), --output (PNG path); call core::render(); encode Vec<u8> to PNG via the image crate; validates Scenario 1 from quickstart.md
- [X] T032 [P] Create core/tests/bbox_unit.rs: unit tests covering (a) valid RenderRequest builds successfully, (b) xminâ‰¥xmax returns InvalidBbox, (c) width=0 returns InvalidDimensions, (d) zoom=23 returns InvalidBbox, (e) zoom auto-formula produces expected value for a known bbox+dims, (f) max_tiles guard reduces zoom correctly
- [X] T033 [P] Create py-basemapper/tests/test_render.py: pytest tests for Scenarios 2, 4, 7, 8 from quickstart.md; use responses library or unittest.mock to mock HTTP tile endpoints for Scenarios 2 and 4; Scenario 7 passes inline JSON with version=7; Scenario 8 uses a deliberately unreachable URL with tile_timeout_ms=1000
- [X] T034 [P] Create r-basemapper/tests/testthat/test-render.R: testthat tests for Scenarios 5â€“6 from quickstart.md; Scenario 5 verifies length(raw) == 400*300*4; Scenario 6 saves to a temp file and verifies file.info()$size > 50000
- [X] T035 [P] Ensure every exported R function (geom_basemap, render_basemap_raw, tm_basemap, raster_provider, vector_provider, esri_vector_provider, esri_raster_provider, list_layers) has complete roxygen2 tags (@title, @param, @return, @examples, @export) in its .R source file; run rextendr::document() to regenerate r-basemapper/NAMESPACE and r-basemapper/man/*.Rd files; verify `R CMD check r-basemapper` exits with 0 errors and 0 warnings
- [X] T036 Run `cargo clippy --workspace -- -D warnings` and resolve all lint violations; run `cargo fmt --all` for consistent formatting across all Rust source files
- [X] T043 [P] Configure and generate Python API reference documentation with great-docs: add great-docs configuration to py-basemapper/pyproject.toml; ensure all public Python symbols (add_basemap, render_basemap_raw, list_layers, geom_basemap [plotnine], RasterProvider, VectorProvider, EsriVectorProvider, EsriRasterProvider) have complete Google-style docstrings (Args, Returns, Raises sections); run great-docs to verify docs build without errors
- [X] T037 Execute all 8 quickstart.md validation scenarios end-to-end on a clean checkout (no pre-built binaries); document any environment-specific setup steps discovered (e.g., Mesa install on headless Linux) in r-basemapper/README.md and py-basemapper/README.md

---

## Phase 8: User Story 5 â€” Layer Filtering and Discovery (Priority: P5)

**Goal**: Data scientists can isolate or suppress named layers in any MapLibre GL style
with a single additional argument, and can discover the layer schema of an unfamiliar
provider without reading raw JSON.

**Independent Test**: Call `render_basemap_raw()` with a 3-layer mock style and
`layers=["water"]`; assert the returned pixels contain only that layer on a transparent
background. Call `list_layers()` on a mocked style URL; assert the returned list matches
the expected IDs.

### Rust Core (US5)

- [X] T044 [US5] Update core/src/bbox.rs: add `layers: Option<Vec<String>>` field to `RenderRequest`; add `validate_layers()` call inside `validate()` that returns `BasemapError::InvalidLayerFilter` with a descriptive message when the list contains a mix of plain and minus-prefixed entries
- [X] T045 [P] [US5] Implement `filter_style_layers(style_json: &str, layers: &[String]) -> Result<String, BasemapError>` in core/src/style.rs: parse the style JSON, detect filter mode (all-positive â†’ inclusion, all-negative â†’ exclusion), filter the `layers` array by matching `id` fields, serialize back to a JSON string; re-export from core/src/lib.rs âš ï¸ H3: touch ONLY the top-level "layers" array â€” do NOT remove entries from "sources", "sprite", or "glyphs"; maplibre-rs validates the full style at parse time and will crash if any referenced source or sprite/glyph URL is absent
- [X] T046 [US5] Update core/src/lib.rs `render()`: after `StyleInput::resolve()` returns `style_json`, call `filter_style_layers()` when `request.layers.is_some()`; set the wgpu clear color to `wgpu::Color::TRANSPARENT` when a layer filter is active so background pixels carry alpha = 0

### Python Implementation (US5)

- [X] T047 [P] [US5] Update Python FFI and integration: add `layers: Optional[List[str]] = None` to `render_basemap_raw()` in `py-basemapper/src/lib.rs` (serialize to `Option<Vec<String>>` for Rust); add `layers=None` to `add_basemap()` in `matplotlib_integration.py` and pass through; update `__init__.py` exports and `__all__`
- [X] T049 [P] [US5] Create py-basemapper/src/basemapper/layer_utils.py: implement `list_layers(style_input: str) -> List[str]` accepting a style URL or inline JSON string; fetch via `requests.get()` if a URL, otherwise `json.loads()`; extract and return all `id` values from the top-level `layers` array as a sorted list; print a two-column table (id, type) to stdout using a plain `print()` loop for interactive use; export from `__init__.py`

### R Implementation (US5)

- [X] T048 [P] [US5] Update R FFI and integration: add `layers = NULL` to the extendr `render_basemap_raw` Rust function in `r-basemapper/src/rust/src/lib.rs` (accept `Nullable<Vec<String>>`); add `layers = NULL` to the R wrapper `render_basemap_raw.R` and to `geom_basemap()` in `geom_basemap.R`; pass through to the extendr function
- [X] T050 [P] [US5] Create r-basemapper/R/layer_utils.R: implement `list_layers(style_input)` accepting a style URL or inline JSON string; fetch via `httr2::request() |> httr2::req_perform()` if a URL, otherwise parse directly with `jsonlite::fromJSON()`; extract and return the `id` field from the `layers` array as a `character` vector; print a `data.frame` of id/type columns to the console; add `@export` roxygen2 tag

### Tests (US5)

- [X] T051 [P] [US5] Create py-basemapper/tests/test_layers.py: (a) positive filter `layers=["water"]` on a 3-layer inline style produces 1-layer filtered JSON, (b) negation filter `layers=["-roads"]` on a 3-layer style produces 2-layer JSON, (c) mixed list `["water", "-roads"]` raises `BasemapError` with `InvalidLayerFilter`, (d) `list_layers()` returns correct IDs from a `responses`-mocked style URL; create r-basemapper/tests/testthat/test-layers.R covering the same four cases using `testthat::expect_equal`, `expect_warning`, and `expect_error`

**Checkpoint**: `render_basemap_raw(layers=["water"])` (Python) and
`render_basemap_raw(layers=c("water"))` (R) each produce a pixel array where all
background pixels have alpha = 0; `list_layers()` returns the expected ID vector from a
mock style URL; all unit tests pass.

---

## Phase 9: Additional Ecosystem Integrations (US6 + US7)

**Goal**: Extend basemapper to tmap v4 (R) and plotnine (Python), giving data scientists
native grammar-of-graphics integration in both ecosystems without leaving their
established plotting workflow.

**Independent Test**: US6 â€” `tm_shape(nc) + tm_basemap(style_url) + tm_sf()` renders a
basemap-backed tmap map with no manual bbox. US7 â€” a plotnine plot with
`geom_basemap(style_url=...)` as the first geom renders a correctly aligned basemap
beneath subsequent geom layers.

### R: tmap Integration (US6)

- [X] T052 [P] [US6] Update r-basemapper/DESCRIPTION: add `tmap (>= 4.0)` and `stars` to the `Suggests` field (soft dependencies â€” only needed for the tmap integration path, not required for ggplot2 or raw-call usage)
- [X] T053 [US6] Create r-basemapper/R/tmap_integration.R: implement `tm_basemap(style_input, bbox=NULL, zoom=NULL, alpha=1, layers=NULL, ...)` function; when `bbox` is NULL, derive bounding box from the tmap pipeline's active shape via `tmap::bb()`; raise an informative `stop()` when no bbox can be resolved; call `reproject_bbox_to_3857()` from `bbox_utils.R`; call `render_basemap_raw()`; reshape and wrap the returned raw vector as a georeferenced `stars::st_as_stars()` object with EPSG:3857 CRS and correct spatial extent; return `tmap::tm_shape(stars_obj) + tmap::tm_rgb(alpha=alpha)` as a composable tmap element; add roxygen2 `@export` tag âš ï¸ H1: if tm_rgb() applies an unwanted colour palette or renders incorrectly, fall back to tm_raster() with explicit band selection â€” try tm_rgb() first since it is designed for multi-band RGBA data
- [X] T054 [P] [US6] Create r-basemapper/tests/testthat/test-tmap.R: (a) verify `tm_basemap()` with an explicit sf bbox calls `render_basemap_raw` and returns an object whose class includes tmap element types, (b) verify `tm_basemap()` with no bbox and no shape context raises a `stop()` error, (c) verify the `alpha` argument is forwarded to the `tm_rgb()` layer, (d) verify `layers` is passed through to `render_basemap_raw`

### Python: plotnine Integration (US7)

- [X] T055 [P] [US7] Update py-basemapper/pyproject.toml: add `[project.optional-dependencies]` section with `plotnine = ["plotnine>=0.12"]` so the integration is available via `pip install basemapper[plotnine]` without being required for the base install; add `plotnine>=0.12` to dev dependencies for the test suite
- [X] T056 [US7] Create py-basemapper/src/basemapper/plotnine_integration.py: implement `geom_basemap` class inheriting from `plotnine.geoms.geom.geom`; set `REQUIRED_AES = set()` and `DEFAULT_AES = aes()`; override `draw_panel(data, panel_params, coord, ax, **kwargs)` to extract coordinate bounds from `panel_params`, call `bbox_utils.reproject_bbox_to_3857()`, call `render_basemap_raw()`, convert to numpy array shaped `(H, W, 4)`, inject via `ax.imshow(arr, extent=..., zorder=0, alpha=self.alpha, interpolation='nearest')`; constructor accepts `style_url`, `zoom=None`, `alpha=1.0`, `layers=None` âš ï¸ H4: same zorder conflict applies here as in T022 â€” if basemap renders invisible, use zorder=0.5 and clear the axes background
- [X] T057 [P] [US7] Update py-basemapper/src/basemapper/__init__.py: add a conditional import guard for `geom_basemap`: `try: from .plotnine_integration import geom_basemap; except ImportError: pass`; add `geom_basemap` to `__all__` inside the same guard so the base package remains importable without plotnine installed
- [X] T058 [P] [US7] Create py-basemapper/tests/test_plotnine.py: (a) verify `geom_basemap(style_url=...)` added to a plotnine plot with mock HTTP produces a renderable plot object without error, (b) verify `alpha=0.5` is forwarded to `ax.imshow()`, (c) verify `layers=["water"]` is passed through to `render_basemap_raw`, (d) verify `import basemapper` succeeds when plotnine is not installed (mock the ImportError)

**Checkpoint**: `tm_shape(nc) + tm_basemap(style_url=...) + tm_sf()` renders without
error in tmap v4; a plotnine plot with `geom_basemap()` as the first geom renders a
correctly positioned basemap; base `import basemapper` succeeds without plotnine
installed.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies â€” start immediately; T002â€“T005 can run in parallel after T001
- **Foundational (Phase 2)**: Depends on Phase 1 completion â€” **BLOCKS all user stories**
  - T007, T008 can run in parallel after T006
  - T009 depends on T007 (bbox) and T008 (style)
  - T010 depends on T009 (needs tile data types)
  - T011 depends on T006â€“T010
- **User Story 1 (Phase 3)**: Depends on T011 â€” Python T012, T013, T017, T018 can run in parallel
- **User Story 2 (Phase 4)**: Depends on Phase 3 completion (updates existing files)
- **User Story 3 (Phase 5)**: Depends on Phase 3 completion â€” T025, T026 can run in parallel
- **User Story 4 (Phase 6)**: Depends on Phase 1 (package structure); T038, T040, T041 can run in parallel after T016 (Python __init__ exported); T039, T042 can run in parallel with T038
- **User Story 5 (Phase 8)**: T044â€“T046 (Rust core) depend on Phase 2; T047â€“T050 depend on Phase 3 (binding stubs exist); T051 (tests) depend on T044â€“T050; T045 can run in parallel with T044
- **US6 tmap (Phase 9)**: T052 (DESCRIPTION update) can run immediately after T004; T053 depends on Phase 3 (render_basemap_raw + bbox_utils available); T054 depends on T053
- **US7 plotnine (Phase 9)**: T055 (pyproject.toml) can run immediately after T003; T056 depends on Phase 3 (render_basemap_raw + bbox_utils available); T057 depends on T056; T058 depends on T056; T055â€“T058 can run in parallel with T052â€“T054
- **Polish (Phase 7)**: Depends on all user story phases; T031â€“T035 can run in parallel

### User Story Dependencies

- **US1 (P1)**: Can start immediately after Foundational â€” no inter-story dependencies
- **US2 (P2)**: Depends on US1 (updates the same add_basemap/draw_panel functions)
- **US3 (P3)**: Depends on US1 (adds tile_source param to same functions); T025â€“T026 (Rust core) can run in parallel with Phase 3
- **US4 (P4)**: Pure host-language code â€” no Rust changes. Depends on Python `__init__.py` (T016) and R package structure (T004). Can run in parallel with Phase 4â€“5 after Phase 3 completes.
- **US5 (P5)**: Rust core changes (T044â€“T046) depend on Phase 2; host-language changes (T047â€“T050) depend on Phase 3 binding stubs; T045 (style filter) and T044 (RenderRequest update) can run in parallel after Phase 2.
- **US6 (P6)**: T053 (`tm_basemap()`) depends on Phase 3 R implementation; T052 (DESCRIPTION) can run after T004.
- **US7 (P7)**: T056 (`geom_basemap` plotnine) depends on Phase 3 Python implementation; T055 (pyproject.toml) can run after T003; no Rust changes required.

### Within Each User Story

- Python and R implementation tasks are independent and can run in parallel within the same phase
- Within each language: exceptions/bbox_utils â†’ core binding (lib.rs) â†’ R/Python wrappers
- Models before services; services before integration

### Parallel Opportunities

```bash
# Phase 1 â€” after T001 completes:
T002, T003, T004 run in parallel

# Phase 2 â€” after T006:
T007, T008 run in parallel

# Phase 3 â€” after T011:
# Python side:
T012, T013 run in parallel
# R side (simultaneously with Python):
T017, T018 run in parallel
# T014 depends on T012+T013; T015 depends on T014
# T019 depends on T018; T020 depends on T017+T019

# Phase 5 â€” T025, T026, T027 can start in parallel after Phase 3 checkpoint
# Phase 6 (US4) â€” after T016 completes (Python) and T004 (R package structure):
T038, T040 run in parallel (Python providers + tests)
T041, T042 run in parallel (R providers + tests)
# Phase 8 (US5) â€” after Phase 2 completes:
T044, T045 run in parallel (RenderRequest update + style filter function)
T046 depends on T044+T045
T047, T048, T049, T050 run in parallel (Python + R FFI updates and list_layers)
T051 depends on T044â€“T050
# Phase 9 (US6 + US7) â€” after Phase 3 completes:
T052, T053, T055, T056 can all start in parallel
T054 depends on T053; T057, T058 depend on T056
# Phase 7 â€” T031, T032, T033, T034, T035 all run in parallel after Phases 5+6+8+9
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001â€“T005)
2. Complete Phase 2: Foundational Rust Core (T006â€“T011) â€” **critical gate**
3. Complete Phase 3: User Story 1 â€” auto-infer bbox (T012â€“T020)
4. **STOP and VALIDATE**: Run Scenarios 3 and 6 from quickstart.md
5. If basemap renders with auto-inferred extents â†’ MVP demonstrated

### Incremental Delivery

1. Phase 1 + 2 â†’ Rust core compiles and tests pass
2. Phase 3 â†’ `add_basemap(ax)` and `geom_basemap()` work (MVP!)
3. Phase 4 â†’ pixel-perfect DPI matching â†’ demo at any resolution
4. Phase 5 â†’ Mapbox/ESRI tile support + custom styles â†’ full feature set
5. Phase 6 â†’ Provider helpers â†’ basemapper works from bare tile URLs
6. Phase 8 â†’ Layer filtering + discovery â†’ compositing-ready
7. Phase 9 â†’ tmap + plotnine integrations â†’ full ecosystem coverage
8. Phase 7 â†’ Polish + test coverage â†’ release-ready

### Parallel Team Strategy

With two developers after Phase 2 completes:

- **Developer A** (Python): T012â€“T016 (US1 Python), T021â€“T022 (US2 Python), T027â€“T028 (US3 Python), T038â€“T040 (US4 Python)
- **Developer B** (R): T017â€“T020 (US1 R), T023â€“T024 (US2 R), T029 (US3 R), T041â€“T042 (US4 R)
- **Either**: T025â€“T026 (Rust core tile format completions, no language preference)

---

## Notes

- [P] tasks operate on different files with no shared mutable state â€” safe to parallelize
- Story label maps each task to a specific user story for traceability and independent testing
- The Rust core (Phase 2) is the single blocking dependency â€” prioritize it
- `cargo check --workspace` should pass after Phase 1; `cargo test --workspace` after Phase 2
- Avoid modifying the same .R or .py file from parallel tasks; per-file ownership is clear
- Stop at each Checkpoint to validate story independence before proceeding

