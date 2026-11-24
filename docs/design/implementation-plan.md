# basemapper 0.3.0 implementation plan

**Implements:** [api-review.md](api-review.md), all decisions D1–D21 as accepted
on 2026-11-23 · **Baseline:** `dev` @ `76fdf91` · **Target:** 0.3.0, one clean
break (D1)

This plan turns the accepted design into ordered, verifiable steps. Each phase
ends in a state where every test passes and the golden images are unchanged,
so work can stop between phases without leaving a half-built system
(principle 2: finish transitions).

---

## Ground rules

- **One branch,** `feat/coherent-api` off `dev`. Phases land on it in order;
  `dev` keeps 0.2.x fixes until the merge. Nothing merges to `main` before
  phase 5.
- **The golden images are the ground truth** for pixels. Every phase that
  touches rendering runs the full suite. A Web Mercator render must stay
  byte-identical to today's; new paths (warping, masks) get their own
  references.
- **Every rule lands with its test first.** Each row of the review's tables
  (§4.2 relation, defaults, conflicts; §4.8 notes; §4.10 clipping) becomes at
  least one test case, written before the code that satisfies it.
- **One contract, three languages.** A shared table of cases (inputs → the
  resolved view and its notes, or the expected error) lives in
  `core/tests/fixtures/contract/` as JSON. The Rust, R and Python test suites
  each run it, so the languages cannot disagree (F2, F8).
- **Performance does not regress:** `examples/bench_render.rs` before and
  after each phase that touches the render path.
- **The vendored core** (`r-basemapper/src/rust/core/`) is synced after every
  core change; the drift gate enforces it.
- **No shims** for 0.2 arguments (D1): removed names are a clear error that
  points to the migration guide.

---

## Phase 0: safety nets (no behaviour change)

| Work | Detail |
|---|---|
| Contract fixtures | The JSON case format and a first set: the §4.2 examples, defaults and conflicts; the date-line box; `around()` |
| Bug tests, marked expected to fail | **F1** plot alignment: a lon/lat panel and a projected (state plane) panel, comparing a known landmark's pixel with the basemap's. **F4** date-line bounds from R and Python. They turn green in phases 3–4 |
| Parity check | A test that lists each language's public functions and arguments and compares them with the contract (`docs/api.md`, rewritten in phase 5), first run in report-only mode |
| Bench baseline | `bench_render` numbers recorded for comparison |

**Exit:** all existing tests pass; the new expected-fail tests fail for the
documented reasons.

**Done (2026-11-23):**

- `tools/contract_cases.py` writes `core/tests/fixtures/contract/cases.json`
  (17 view cases, 3 `around()` cases) from the design's formulas;
  `core/tests/contract.rs` checks them (the resolving test waits for phase 1).
- F1 and F4 tests: `r-basemapper/tests/testthat/test-known-bugs.R`
  (`expect_failure()`), `py-basemapper/tests/test_known_bugs.py`
  (`xfail(strict=True)`). Measured F1 offsets on a Europe-scale lon/lat plot:
  64 px in R, 46 px in Python. The two bindings stretch the image
  differently, which is F2 / F8 at work.
- `tools/api_parity.py` (report-only): 10 of 12 paired functions match; the
  two differences are F11's (`geom_basemap(crs)` in plotnine, the first
  argument of `get_attribution()`).
- Benchmark baseline: `docs/design/bench-baseline-0.2.json`.

---

## Phase 1: the core model (Rust)

The largest phase. It adds the four concepts (§4.1) beside today's code, then
moves the existing entry points onto them, so the R and Python packages keep
working (through the old internal calls) until phases 3 and 4.

### 1a. View resolution: `core/src/view.rs`

- `ViewInputs`: extent (`Bounds` with its CRS already transformed to Web
  Mercator by the host, or `Camera { center: lon/lat }`), `zoom`, image size
  (`width`, `height`, `units`), `dpi`; each `Option`.
- `Context`: `Standalone` or `Plot { extent, panel_px, dpi }`.
- `resolve(inputs, context) -> Result<View, BasemapError>`: the §4.2 relation
  (any three of extent, zoom, image size and dpi give the fourth), the default
  order (dpi → image size → extent; zoom never defaulted), the conflict errors
  (each listing what the alternatives would give), the effective dpi for a
  zoom chosen in a plot, and the MapLibre constraints (`constrain_view`, moved
  here).
- `View`: the resolved extent (Web Mercator), zoom, pixel size, dpi, map scale
  (1:N), and each value's provenance (given / derived / default) for the notes.
- Date-line bounds (`xmin > xmax`) are a first-class extent here (F4).
- `around(center, span, units)` in the core: the box on the sphere, so R and
  Python share it.

### 1b. Styles as values: `core/src/style.rs`

- `Style { source, layers, features, key, … }` with one resolver for every
  source kind: catalogue name, style URL/JSON, tile URL, TileJSON, PMTiles,
  ArcGIS, WMS / WMTS (D2). `basemap_provider`, `tile_style`, `wms_style` and
  `wmts_style` become constructors of it.
- Filters and the key belong to the style (F6): the stack's "as one basemap"
  special cases in `render_stack` go.
- One filter model (D3): `features` gains `"labels"`; `labels` folds into it.
- One credential, `key`: put into the URL for catalogue providers, sent as a
  Bearer token (origin-scoped, as now) to other servers.

**1a and 1b done (2026-11-23):**

- `core/src/view.rs` resolves all 20 view cases and 3 `around()` cases of
  the contract. Physical units (`in`, `cm`, `mm`) fix the layout, so `dpi`
  then changes only the pixel count, and bounds + zoom + a physical size is
  an error. `core/src/notes.rs` holds the note levels.
- `Style` and `Source` in `core/src/style.rs`: a string is auto-detected;
  `Source::provider`, `tiles`, `wms` and `wmts` carry each source's own
  options. `features` takes `"labels"` / `"-labels"`. `key` is the one
  credential (a catalogue key, or an origin-scoped token).
- Stacks: each request has a `group` (its style). A group is filtered as one
  map (a catalogue stack's services), and every group must draw something.
  The bindings put all requests in group 0, so they behave as in 0.2 until
  phases 3–4. `Basemap` holds `Style` values; its view-wide `layers`,
  `labels`, `features` and `auth_token` setters are gone.
- All 255 golden renders (PNG and SVG) are byte-identical to before.

### 1c. Renderings and notes: `core/src/rendering.rs`, `core/src/notes.rs`

**Done (2026-11-23):** `Rendering` has pixels, size, bounds, crs, zoom,
tile zoom, dpi and notes, with `scale()` derived from the zoom and the
centre's latitude (the same at any dpi). Notes are collected with
`notes::report()` into the render's `Collector`. The collector is current
around each poll of a wrapped future and in each `par_map` worker, so it
follows the work into spawned tasks and threads with no signature changes.
Notes of one kind merge, with a count and details. All 18 warnings became
notes (a per-tile log line stays a log line), as did the filter warnings
and the unmatched `paint` keys. Two *adjusted* notes are new: raster tiles
coarsened for `max_tiles`, and tiles enlarged past the source's zoom.
Logging is unchanged, so the 0.2 bindings behave as before. The golden
renders are byte-identical.


- `Rendering { pixels, width, height, bounds, crs, zoom, dpi, scale, notes }`:
  one render, many conversions (D4).
- `Note { level: Decided | Adjusted | Problem, kind, message, details }`,
  collected during the render. The 19 `log::warn!` calls and the filter
  `warnings` field become notes (F12, D13).

### 1d. Warping and masks: `core/src/warp.rs`, `core/src/mask.rs`

- **Warp (D7):** a Web Mercator image plus a coarse mesh (output pixel grid →
  Web Mercator coordinates, supplied by the host) → the image in the map CRS:
  bilinear, parallel by rows, the mesh interpolated between nodes.
- **Mask (D20, D21):** polygon rings in output pixels → an antialiased alpha
  mask with the existing `raster_polygon` (at 4×, even-odd), inverted for
  `outside()`, after simplifying to the output resolution.
- **SVG:** the clip as `<clipPath>` (even-odd for `outside()`); SVG of a map
  not in Web Mercator is an error until paths are warped (§4.2).

**Done (2026-11-23):** `core/src/warp.rs`: a `Mesh` (output pixel
positions → Web Mercator, NaN outside the CRS's area) and `warp()`. It uses
bilinear interpolation in premultiplied alpha, runs rows in parallel, and
handles the date line. `core/src/mask.rs`: a `Shape` (polygons in the map's
CRS, `Keep::Inside` / `Outside`) with its default extent (none for outside),
mapped to pixels and simplified to 1/16 px. `alpha_mask()` takes 4 × 4
samples per pixel as bits from the renderer's `raster_polygon`: even-odd
within a polygon, united across polygons, so overlaps don't cancel.
`svg_clip_path()` produces the SVG clip. `core/tests/warp_mask.rs` checks
the alignment of a 1° checkerboard warped to lon/lat and to transverse
Mercator against the projection formulas, plus inside, outside and holed
clips, with reference images in `core/tests/fixtures/warp/`. Wiring the SVG
clip and the error for SVG outside Web Mercator needs the map's CRS, so it
moves to 1e.

### 1e. The facade

- `Basemap { styles, inputs, fetch, clip }`: `render(context)`,
  `render_svg(context, options)`; Rust `FetchOptions` (F9).
- The six `render_*` functions and `RenderRequest` become internal; modules
  become `pub(crate)`; public structs `#[non_exhaustive]` with constructors
  (D9).

**Exit:** every contract case passes in Rust; the golden images are
byte-identical; new references for warped (UTM, lon/lat) and masked (inside,
outside, holes) renders; bench within noise.

**1e done, and phase 1 complete (2026-11-23):**

- `Basemap { styles, inputs, fetch, clip }` with `view(context)`,
  `render(context)`, `render_view(view, warp)` (the two steps a host takes
  for another CRS, since the mesh depends on the image size), `render_svg`
  and `render_svg_view` (an error for a warped map), and `attribution()`.
  The Rust `FetchOptions` matches R's `fetch_options()`; `on_error = Fail`
  makes any *problem* note an error. An inside clip gives the default
  extent, standalone only. Sizes that aren't whole CSS pixels are resampled
  to the exact pixel size.
- Renderings carry *decided* notes (auto zoom, default dpi, size, extent).
- The SVG stack filters each style on its own, as raster stacks do, and
  carries the clip as a `<clipPath>`.
- The documented API is the facade. Internal modules and the old entry
  points are `#[doc(hidden)]`; the bindings reach them through
  `basemapper_core::internal` until phases 3–4, and the test suites keep
  using it. `Rendering`, `Note`, `Style`, `Source`, `View`, `ViewInputs`,
  `FetchOptions`, `Mesh`, `Warp` and the view enums are `#[non_exhaustive]`.
  `LayerFilter`, `TileStyleOptions` and `ProviderOptions` aren't yet: the
  bindings build them literally, and they move behind `Source` in phases 3–4.
- The Rust guide and examples use the facade. The quickstart renders
  pixel-identical to the published guide image.
- Exit checks: all contract cases pass; the 255 golden renders are
  byte-identical; the warp and mask references are in place; bench within
  noise of `bench-baseline-0.2.json` (every case within ±1.5%, except
  +0.2 ms on a 3 ms raster render), with the same request counts.

---

## Phase 2: plot support in the core

- **The plot plan:** panel limits and CRS, panel pixels and dpi → a `Context::Plot`
  view (§4.5). One implementation for every plot library (F8).
- **The dpi rule (D16):** the caller passes the device kind (screen, raster
  export, vector export) and its pixel ratio; the core applies 96 × ratio, the
  device dpi, or 300.
- **Attribution layout:** the wrapped credit text and its placement, computed
  once; each library draws it with its own primitive.

**Exit:** contract cases for plots (screen / export / vector, a chosen zoom →
effective dpi) pass in Rust.

---

## Phase 3: the Python package as an adapter

| Area | Work |
|---|---|
| Objects | `Basemap`, `Rendering`, `FetchOptions` as `#[pyclass]` over the core; immutable (`set_view()` returns a new basemap, D5); view values as properties (`m.zoom`, `m.bounds`, D8) |
| Constructors | `basemap(style, *, bounds, center, zoom, width, height, units, dpi, crs, clip, fetch)`, `style()`, `around()`, `outside()` |
| CRS (host side) | bounds normalisation (numbers are lon/lat; GeoDataFrame / GeoSeries bring their CRS; projected CRSs inferred, geographic fall back to Web Mercator, D18, D19); the warp mesh with `pyproj`; clip polygons transformed and passed as rings |
| Rendering | `to_numpy()` (from the core's buffer, without the extra copy), `save()`, `bounds`, `crs`, `to_xarray()` (extra), `render_svg()` |
| Plots | matplotlib `m.add_to(ax)` (alias `add_basemap`), plotnine `ggplot() + m` (alias `geom_basemap`), both on the phase 2 plan with the device kind from the canvas (`device_pixel_ratio`, saving vs drawing) |
| Notes | decided → `repr` / verbose logging; adjusted → `warnings.warn` (once per call site); problem → `UserWarning`, or an error with `on_error = "fail"` |
| Removed | `render_basemap_raw`, `_render_buffer`, `bbox_utils` duplicates, `tile_zoom`, `pixel_ratio`, the 0.2 deprecated keywords |

**Exit:** the contract cases pass from Python; F1 and F4 tests green for
matplotlib and plotnine; parity check passes for Python.

---

## Phase 4: the R package as an adapter

| Area | Work |
|---|---|
| Objects | an S3 `basemap` whose fields the core produces and validates (value semantics kept); `rendering` objects holding raw bytes with their georeferencing |
| Constructors | `basemap(style, ..., bounds, center, zoom, width, height, units, dpi, crs, clip, fetch)`, `style()`, `around()`, `outside()`, `fetch_options()` |
| Verbs | `set_view()`, `get_bounds()` / `get_center()` / `get_zoom()` / `get_size()`, `render()`, `render_svg()`, `write_png()`, `as_array()`, `as_raster()` (terra suggested), `plot()`, `knit_print()` |
| CRS (host side) | bounds normalisation with the same rules; the warp mesh with `sf` (suggested; lon/lat and Web Mercator need none); clip polygons |
| Plots | ggplot2 via `ggplot_add.basemap` (alias `geom_basemap()`); tmap via `tm_basemap(m)` / `tm_basemap(style)`, whose separate `terra::project` path goes; device kind from the device type (screen devices vs `png` / `ragg` / `pdf` / `svglite`) |
| Notes | decided → `print()` / `options(basemapper.verbose = TRUE)`; adjusted → `cli::cli_inform(.frequency = "once")`; problem → `warning()`, or an error with `on_error = "fail"` |
| Removed | `render_basemap_raw`, `render_png()` / `render_array()` / `render_raster()` (replaced by `render()` and its conversions), `bbox_utils.R` duplicates, `tile_zoom`, `pixel_ratio`, the 0.2 deprecated arguments, `basemap_provider()` / `tile_style()` / `wms_style()` / `wmts_style()` (replaced by `style()`) |

**Exit:** the contract cases pass from R; F1 and F4 tests green for ggplot2
and tmap; `R CMD check` clean; parity check passes for R.

---

## Phase 5: documentation and release

- `docs/api.md` rewritten as the contract for all three languages; the parity
  check switches from report-only to failing.
- Guides (R articles, Python user guide, Rust guide) rewritten around the three
  controls, styles, renderings, plots and clipping.
- A **migration guide**: every removed name with its replacement, and the
  behaviour changes (CRS inference, label scaling replaced by an effective
  dpi, `width` / `height` now the image size).
- README claims backed by tests (§7 of the review).
- Changelogs; version 0.3.0 (`tools/version.py set 0.3.0`); release PR to
  `main` with the full CI matrix; tag and release.

---

## Decisions by phase

| Phase | Decisions |
|---|---|
| 1 | D2, D3, D4, D7, D9, D10, D13, D17, D20, D21 (core side) |
| 2 | D6, D14 (core side), D15, D16 |
| 3, 4 | D5, D8, D11, D12, D14, D18, D19, and the host sides of D7, D20, D21 |
| 5 | D1 (the break is released) |

---

## Technical questions to settle during the work

These are implementation details, not design choices; each is decided by
a test or a measurement:

- **The warp mesh's density:** the coarsest grid whose interpolation error
  stays under half a pixel (measured on UTM, Albers and lon/lat panels).
- **Python buffers under `abi3-py39`:** the buffer protocol is only in the
  stable ABI from Python 3.11, so `to_numpy()` either uses `numpy`'s C API
  through the `numpy` crate or keeps one copy on older Pythons.
- **The R object:** fields validated by the core on every change, or an
  external pointer to the core's `Basemap`. Value semantics favour the first.
- **Screen pixel ratio in R:** whether RStudio / Positron expose it; if not,
  assume 1 on screens (the D16 fallback).
- **Label warping limits:** at what extent and CRS distortion warped labels
  become noticeably skewed, for the docs' guidance.
