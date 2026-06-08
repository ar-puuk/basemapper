# Feature Specification: Spatial Basemap Renderer

**Feature Branch**: `001-spatial-basemap-renderer`

**Created**: 2026-06-08

**Status**: Draft

**Input**: User description: "Build a cross-platform library ("basemapper") for data scientists that generates perfectly styled, native-resolution spatial basemaps for static plotting."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Auto-Infer Plot Extents (Priority: P1)

A data scientist working in R or Python has already built a spatial plot using their
preferred plotting library. They want to add a styled basemap underneath their data
layers without having to manually look up or specify the bounding box or projection —
the library should extract all necessary spatial context directly from the plot object
they already have.

**Why this priority**: This is the primary value proposition of the library. Removing
manual coordinate entry eliminates the most common friction point when adding basemaps
to existing spatial workflows. Without this, adoption by data scientists drops
significantly.

**Independent Test**: A user can call a single `add_basemap()` function on an existing
ggplot2 or matplotlib spatial plot and receive a correctly positioned basemap with no
additional arguments — delivering a fully usable basemap-enhanced plot.

**Acceptance Scenarios**:

1. **Given** a ggplot2 plot with `coord_sf()` active, **When** `add_basemap()` is called
   with no `bbox` argument, **Then** the library extracts the panel's geographic extent
   and CRS, downloads the appropriate tiles, and returns a basemap pixel array aligned
   to those extents.

2. **Given** a matplotlib `Axes` object configured with a geographic projection (e.g.,
   via cartopy or a GeoDataFrame plot), **When** `add_basemap()` is called with no `bbox`
   argument, **Then** the library extracts the axes bounds and CRS, downloads tiles, and
   returns a basemap pixel array aligned to those extents.

3. **Given** a plot object with no recognized geographic CRS, **When** `add_basemap()` is
   called, **Then** the library raises a clear, descriptive error explaining that a
   geographic coordinate reference system is required.

---

### User Story 2 - Native-Resolution Basemap Injection (Priority: P2)

After the basemap is generated, the data scientist wants it composited underneath their
existing data layers at exactly the right pixel density for the output device — whether
that is a screen preview, a high-DPI PDF, or a publication-quality PNG — without any
manual resizing or re-projection step.

**Why this priority**: Blurry or mis-scaled basemaps undermine the visual quality that
distinguishes publication-ready spatial figures. Correct DPI-matching is essential for
professional output.

**Independent Test**: A user calls `add_basemap()` specifying a target DPI and plot
dimensions; the returned pixel array has exactly the expected pixel count (width × DPI
and height × DPI) and the basemap aligns precisely with the data layer when overlaid.

**Acceptance Scenarios**:

1. **Given** a ggplot2 plot targeting 300 DPI at 6 × 4 inches, **When** `add_basemap()`
   is called, **Then** the returned pixel array is 1800 × 1200 pixels and the basemap
   tile selection matches the zoom level appropriate for that resolution.

2. **Given** a matplotlib figure saved at 150 DPI, **When** the basemap is injected
   using `add_basemap()`, **Then** the rendered figure shows the basemap with no pixel
   seams or scaling artifacts.

3. **Given** a DPI value is not explicitly provided, **When** `add_basemap()` is called,
   **Then** the library infers the DPI from the host plot's current device settings and
   uses that value automatically.

---

### User Story 3 - Multi-Format Tile Sources and Custom Styling (Priority: P3)

A data scientist wants flexibility in which map tile provider they use (including
providers that use vector tiles for sharp rendering at any zoom level) and wants to
apply a custom visual style — for example, a greyscale scheme to reduce visual
competition with their own data overlays.

**Why this priority**: Supporting multiple tile formats and custom styling broadens the
library's applicability across different institutional tile servers, aesthetic
requirements, and data publication contexts.

**Independent Test**: A user constructs a `TileSource` pointing to a Mapbox Vector Tile
URL and passes a JSON style that desaturates all colors; the returned basemap pixel array
reflects the greyscale style with no additional tooling required.

**Acceptance Scenarios**:

1. **Given** a tile source configured as a raster XYZ URL (e.g., OpenStreetMap slippy-map
   format), **When** `add_basemap()` is called, **Then** the basemap is rendered from
   those raster tiles.

2. **Given** a tile source configured as a Mapbox Vector Tile endpoint, **When**
   `add_basemap()` is called with a JSON style definition, **Then** the library renders
   the vector features according to the style and returns a raster pixel array.

3. **Given** a tile source configured as an ESRI Vector Tile endpoint, **When**
   `add_basemap()` is called, **Then** the basemap is rendered from those vector tiles.

4. **Given** a malformed JSON style definition, **When** `add_basemap()` is called,
   **Then** the library raises a descriptive validation error before any tile fetching
   begins.

---

### User Story 4 - Provider Generator Helpers (Priority: P4)

A data scientist has been given a bare tile URL (e.g., from a GIS colleague, a data
portal, or a cloud provider's documentation) and wants to use it as a basemapper source
immediately — without learning the MapLibre GL Style Specification or writing any JSON
by hand. The library should provide simple helper functions that accept a raw tile URL
and return a ready-to-use style string.

**Why this priority**: The MapLibre GL Style JSON format is verbose and unfamiliar to
most data scientists. Without these helpers, every user who starts from a raw tile URL
faces a steep learning curve just to supply the `style_url` parameter. P4 because the
helpers build on the established tile source infrastructure (US3) and are ergonomic
enhancements, not blockers for core functionality.

**Independent Test**: Call `RasterProvider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")`
(Python) or `raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")` (R)
and pass the returned string directly to `add_basemap()` / `geom_basemap()` with no
other arguments. The map renders a recognisable basemap with a single helper call and
zero MapLibre GL JSON knowledge.

**Acceptance Scenarios**:

1. **Given** a bare XYZ raster tile URL, **When** `RasterProvider(url)` (Python) or
   `raster_provider(url)` (R) is called, **Then** the returned value is a valid JSON
   string containing `"version": 8`, a raster source entry whose `tiles` array contains
   the provided URL, and at least one raster layer — and can be passed directly to
   `add_basemap()` or `render_basemap_raw()` without further modification.

2. **Given** a bare MVT tile URL and a paint dictionary `{"fill-color": "#e8e0d8",
   "line-color": "#aaa", "line-width": 1}`, **When** `VectorProvider(url, paint=paint)`
   (Python) or `vector_provider(url, paint=list(...))` (R) is called, **Then** the
   returned JSON contains a vector source and at least one fill layer using `fill-color`
   and one line layer using `line-color` and `line-width`.

3. **Given** a `VectorProvider` is called with no `paint` argument, **When** the result
   is passed to `render_basemap_raw`, **Then** the map renders with sensible default
   styling (light grey fill, medium grey lines) without requiring any additional input.

4. **Given** a base ArcGIS VectorTileServer URL (e.g., `https://basemaps.arcgis.com/
   arcgis/rest/services/World_Basemap_v2/VectorTileServer`), **When**
   `EsriVectorProvider(base_url)` (Python) or `esri_vector_provider(base_url)` (R) is
   called, **Then** the returned string is the constructed style endpoint URL
   `<base_url>/resources/styles/root.json`, which the Rust core fetches directly to
   obtain the complete, ESRI-published MapLibre GL style JSON.

5. **Given** a `VectorProvider` paint argument containing an unrecognised key (e.g.,
   `{"circle-radius": 5}`), **When** the helper is called, **Then** the unrecognised
   key is silently dropped, a user-visible warning is emitted naming the dropped key,
   and the returned JSON is still valid and renderable.

6. **Given** a bare ESRI raster tile URL template (e.g., an ArcGIS MapServer tile
   endpoint of the form `https://.../MapServer/tile/{z}/{y}/{x}`), **When**
   `EsriRasterProvider(url)` (Python) or `esri_raster_provider(url)` (R) is called,
   **Then** the returned string is a valid inline MapLibre GL Style JSON with a raster
   source containing the provided URL and `tileSize: 256`, renderable immediately
   without any further configuration.

---

### User Story 5 - Layer Filtering and Discovery (Priority: P5)

A data scientist has a multi-layer basemap style and wants to isolate specific visual
layers — for example, rendering only water features or suppressing road and label layers
— so they can composite basemap elements independently beneath their own data overlays.
They also want to inspect unfamiliar style schemas to discover which layer IDs are
available before deciding which ones to include or exclude.

**Why this priority**: Layer isolation enables advanced cartographic compositing workflows
(e.g., a water-only underlay with a label-only overlay). Discovering layer IDs removes
the need to read raw MapLibre GL JSON, lowering the barrier for users unfamiliar with a
provider's schema. P5 because it extends multi-format rendering (US3) and requires the
Rust core filtering infrastructure to be in place first.

**Independent Test**: Call `render_basemap_raw()` with a style that contains water, roads,
and labels layers, passing `layers=["water"]` (Python) or `layers=c("water")` (R). The
returned pixel array contains only the water layer on a fully transparent background.
Call `list_layers(style_url)` and receive a complete list of available layer IDs without
any manual JSON inspection.

**Acceptance Scenarios**:

1. **Given** a style JSON with layers `["water", "roads", "labels"]`, **When**
   `render_basemap_raw(layers=["water", "roads"])` (Python) or
   `render_basemap_raw(layers=c("water", "roads"))` (R) is called, **Then** only the
   "water" and "roads" layers appear in the rendered output and the "labels" layer is
   absent.

2. **Given** a style JSON with layers `["water", "roads", "labels"]`, **When**
   `render_basemap_raw(layers=["-roads", "-labels"])` (Python) or
   `render_basemap_raw(layers=c("-roads", "-labels"))` (R) is called, **Then** all
   layers except "roads" and "labels" are rendered (only "water" remains).

3. **Given** a `layers` list containing both a plain entry and a minus-prefixed entry
   (e.g., `["water", "-roads"]`), **When** `render_basemap_raw` is called, **Then** the
   library raises a clear validation error before any style fetching or tile downloading,
   explaining that positive and negation filters cannot be mixed in the same list.

4. **Given** any filtered render (i.e., `layers` is non-empty), **When** the render
   completes, **Then** the output pixel array uses a fully transparent background
   (alpha = 0 for all background pixels) so the isolated layer can be composited
   seamlessly over other plot elements without masking them.

5. **Given** a style URL or inline style JSON string, **When** `list_layers(style_input)`
   is called in Python or R, **Then** the function returns all layer `id` values as a
   Python `List[str]` or R `character` vector, and prints a two-column human-readable
   table of `id` and `type` to the console for interactive exploration.

---

### User Story 6 - tmap Integration (Priority: P6)

A data scientist working in R has built a spatial map using tmap v4 (`tm_shape()` +
`tm_sf()` or similar layers). They want to add a Rust-rendered styled basemap as the
bottommost layer in the same tmap pipeline without manually specifying coordinates,
reprojecting data, or leaving the tmap syntax.

**Why this priority**: tmap is one of the most widely used spatial visualisation
packages in R alongside ggplot2. Supporting it natively extends the library's reach to
a large community that does not use ggplot2 for maps. P6 because it requires the US1
bbox/reprojection infrastructure and introduces a new soft dependency (`stars`) for
wrapping the pixel array as a georeferenced raster.

**Independent Test**: Build a tmap pipeline `tm_shape(sf_object) + tm_basemap(style_input) + tm_sf()` and confirm a styled basemap renders beneath the sf layer with no
manual bbox argument and no coordinate reprojection by the caller.

**Acceptance Scenarios**:

1. **Given** a tmap pipeline with a primary `tm_shape(sf_object)`, **When**
   `tm_basemap(style_input)` is added, **Then** the bounding box is inferred from the
   primary shape via `tmap::bb()`, tiles are fetched, and a georeferenced raster
   basemap renders beneath all subsequent tmap layers.

2. **Given** an explicit `bbox` argument passed to `tm_basemap()`, **When** the function
   is called, **Then** the provided bbox overrides any shape-inferred extent.

3. **Given** `tm_basemap()` is called with no active shape context and no `bbox`
   argument, **When** tmap evaluates the pipeline, **Then** a clear, informative error
   is raised before any tile fetching begins.

4. **Given** an `alpha` argument between 0 and 1 is passed to `tm_basemap()`, **When**
   the map renders, **Then** the basemap layer is composited at that opacity beneath
   the overlying sf layers.

---

### User Story 7 - plotnine Integration (Priority: P7)

A data scientist working in Python with plotnine wants to add a styled basemap to their
grammar-of-graphics spatial plot using a single `geom_basemap()` call added to the plot
chain. The basemap must render lazily at draw time, aligned to the established plot
coordinate bounds, with no manual matplotlib interaction required.

**Why this priority**: plotnine mirrors ggplot2's grammar-of-graphics in Python and
shares matplotlib as its rendering backend, making the integration path closely analogous
to the existing ggplot2 implementation. P7 because it requires the plotnine soft
dependency and a new Geom subclass, but reuses the existing `bbox_utils` and
`render_basemap_raw` infrastructure with no Rust changes.

**Independent Test**: Build a plotnine plot with `geom_sf(data=gdf)` and add
`geom_basemap(style_url=...)`. The plot renders a styled basemap beneath the sf layer
with no explicit bbox argument and no manual `ax` interaction.

**Acceptance Scenarios**:

1. **Given** a plotnine plot with spatial data and a geographic coordinate system,
   **When** `geom_basemap(style_url=style)` is added as the first geom, **Then** the
   basemap is rendered lazily at draw time, aligned to the established coordinate
   bounds, and composited beneath subsequent geom layers.

2. **Given** `geom_basemap` with `alpha=0.5`, **When** the plot renders, **Then** the
   basemap is drawn at 50% opacity via the underlying `ax.imshow(alpha=0.5)` call.

3. **Given** `geom_basemap` with a `layers` filter list, **When** the plot renders,
   **Then** only the specified layers are visible in the basemap, consistent with the
   `add_basemap()` `layers` behaviour.

---

### Edge Cases

- What happens when tiles for the requested bounding box / zoom level are unavailable
  from the remote source (network error, 404, rate limit)?
- How does the library behave when the plot's extent straddles the anti-meridian (180°)?
- What happens when the plot extent is so large that an appropriate zoom level would
  require more than a configurable maximum number of tile downloads?
- What happens when the JSON style references a font or sprite sheet that is not
  bundled with the library?
- What happens when a `VectorProvider` paint dictionary contains exclusively
  unrecognised keys — does it produce a style JSON with empty layers, and will the
  Rust core accept a layers array that has no renderable content?
- What if a provider URL template is missing the `{z}`, `{x}`, or `{y}` placeholders
  required by the slippy-map convention?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The library MUST extract the geographic bounding box and coordinate
  reference system from a ggplot2 spatial plot object without any manual bbox input.
- **FR-002**: The library MUST extract the geographic bounding box and coordinate
  reference system from a matplotlib `Axes` object without any manual bbox input.
- **FR-003**: When auto-inference succeeds, the extracted bounding box MUST be used
  as-is for tile fetching; the caller MUST also be able to override it with an explicit
  bbox for advanced use cases.
- **FR-004**: The library MUST support raster XYZ tile sources (standard slippy-map/
  Leaflet-compatible URLs of the form `{z}/{x}/{y}`).
- **FR-005**: The library MUST support Mapbox Vector Tile (MVT) sources served over
  HTTP/HTTPS.
- **FR-006**: The library MUST support ESRI Vector Tile sources served over HTTP/HTTPS.
- **FR-007**: The library MUST accept a JSON style definition that controls the visual
  rendering of vector tile features (colors, line widths, fill opacity, label
  visibility).
- **FR-008**: The library MUST render the basemap at the exact pixel dimensions
  specified by the caller (width in pixels × height in pixels).
- **FR-009**: The library MUST select tile zoom levels automatically based on the
  provided bounding box and target pixel dimensions; callers MAY override zoom level
  explicitly.
- **FR-010**: The rendered output MUST be a raw RGBA pixel array paired with the
  spatial bounding box metadata; it MUST NOT be an interactive component, a browser
  object, or a file path.
- **FR-011**: The pixel array MUST be surfaced as a NumPy `ndarray` in Python and as
  a native `matrix` in R, with the bounding box attached as a named attribute.
- **FR-012**: The library MUST operate without requiring any OS-level GIS library
  installation (e.g., no `apt-get install gdal`, no Homebrew prerequisites).
- **FR-013**: When tile fetching fails (network error, HTTP 4xx/5xx), the library MUST
  surface a clear error with the tile URL and HTTP status; it MUST NOT silently return
  blank tiles.
- **FR-014**: The library MUST provide a `RasterProvider` class (Python) and
  `raster_provider()` function (R) that each accept a bare XYZ tile URL template and
  return a fully compliant MapLibre GL Style JSON string containing a raster source and
  a raster layer definition.
- **FR-015**: The library MUST provide a `VectorProvider` class (Python) and
  `vector_provider()` function (R) that each accept a bare MVT tile URL template, an
  optional paint parameter (Python `dict`, R named `list`), and return a fully compliant
  MapLibre GL Style JSON string. Recognised paint keys are the fill family
  (`fill-color`, `fill-opacity`, `fill-outline-color`) and the line family
  (`line-color`, `line-width`, `line-opacity`). Unrecognised keys MUST be dropped
  silently with a user-visible warning naming each dropped key. An absent or empty
  paint parameter MUST produce a sensible default style (light grey fill, medium grey
  lines).
- **FR-016**: The library MUST provide an `EsriVectorProvider` class (Python) and
  `esri_vector_provider()` function (R) that each accept a base ArcGIS VectorTileServer
  URL and return the constructed style endpoint URL
  (`<base_url>/resources/styles/root.json`). Any trailing slash in the input MUST be
  stripped before constructing the endpoint URL. The Rust core fetches this URL to
  obtain the full, ESRI-published MapLibre GL Style JSON.
- **FR-017**: The output of all four providers MUST be accepted without error by
  `render_basemap_raw()` and `add_basemap()` / `geom_basemap()` when passed directly as
  the `style_input` / `style_url` argument. (`RasterProvider`, `VectorProvider`, and
  `EsriRasterProvider` return inline JSON strings; `EsriVectorProvider` returns a URL
  string — both forms are valid inputs to the core rendering pipeline.)
- **FR-018**: The library MUST provide an `EsriRasterProvider` class (Python) and
  `esri_raster_provider()` function (R) that each accept a bare ESRI raster tile URL
  template (e.g., an ArcGIS MapServer tile endpoint of the form
  `https://.../MapServer/tile/{z}/{y}/{x}`) and return a fully compliant MapLibre GL
  Style JSON string with a raster source configured with `tileSize: 256`.
- **FR-019**: The `RenderRequest` struct MUST include an optional `layers: Option<Vec<String>>`
  field. When `None`, all layers in the style JSON are rendered unchanged. When `Some`,
  the field drives the filtering rules in FR-020 and FR-021.
- **FR-020**: When all entries in `layers` are plain strings (no `-` prefix), the Rust core
  MUST filter the style JSON `layers` array to retain **only** those entries whose `id`
  matches one of the provided values. Layer IDs not present in the style MUST be silently
  ignored.
- **FR-021**: When all entries in `layers` are minus-prefixed strings (e.g., `"-roads"`), the
  Rust core MUST filter the style JSON `layers` array to exclude every entry whose `id`
  matches the de-prefixed value, retaining all other layers.
- **FR-022**: When `layers` contains a mix of plain and minus-prefixed entries, the library
  MUST return `BasemapError::InvalidLayerFilter` immediately — before any style fetching or
  tile downloading — with a message that identifies the conflicting entries.
- **FR-023**: When `layers` is `Some(...)`, the wgpu render texture MUST be initialised with
  a fully transparent clear color (`[0, 0, 0, 0]`) so that the caller can composite the
  filtered output over other plot elements without masking the underlying content.
- **FR-024**: The `add_basemap()` Python function and `geom_basemap()` R function MUST each
  expose an optional `layers` parameter (`layers=None` in Python; `layers=NULL` in R) and
  pass it through the FFI layer to `RenderRequest.layers`.
- **FR-025**: The library MUST provide a `list_layers(style_input)` function in both Python
  and R. The function MUST accept a style URL or inline JSON string, fetch the style if a URL,
  parse the `layers` array, and return all `id` values. Both implementations MUST also print
  a two-column human-readable table of `id` and `type` to the console for interactive use.
- **FR-026**: `add_basemap()` (Python) and `geom_basemap()` (R) MUST each expose an optional
  `alpha` parameter (Python `float`, R `numeric`; range 0.0–1.0; default 1.0) that controls
  the opacity of the composited basemap. In Python the value is passed to `ax.imshow(...,
  alpha=alpha)`; in R it is passed to `grid::rasterGrob(..., gp=grid::gpar(alpha=alpha))`.
  No Rust changes are required — this is a host-language compositing concern only.
- **FR-027**: The library MUST provide a `tm_basemap(style_input, bbox=NULL, zoom=NULL,
  alpha=1, layers=NULL, ...)` R function compatible with tmap v4. When `bbox` is NULL,
  the function MUST derive the map extent from the tmap pipeline's primary shape via
  `tmap::bb()`. It MUST reproject to EPSG:3857, call `render_basemap_raw()`, wrap the
  pixel array as a georeferenced `stars` object, and return a composable tmap element
  (`tmap::tm_shape(stars_obj) + tmap::tm_rgb(alpha=alpha)`) that renders beneath
  subsequent tmap layers.
- **FR-028**: When `tm_basemap()` is called with no shape context and `bbox=NULL`, the
  function MUST raise an informative error via `stop()` before any style fetching or
  tile downloading.
- **FR-029**: The library MUST provide a `geom_basemap` class in the Python package,
  inheriting from `plotnine.geoms.geom.geom`, that defers all tile fetching to draw
  time. At draw time it MUST extract the panel coordinate bounds from the plotnine
  coordinate system, reproject to EPSG:3857 via `bbox_utils.py`, call
  `render_basemap_raw()`, and inject the result via the underlying matplotlib
  `ax.imshow()` using the same injection logic as `add_basemap()`.
- **FR-030**: The plotnine `geom_basemap` class MUST accept `style_url`, `zoom=None`,
  `alpha=1.0`, and `layers=None` parameters, consistent with the `add_basemap()`
  interface. The class MUST be importable from the top-level `basemapper` namespace
  only when `plotnine` is installed; the base package MUST remain importable without it.

### Key Entities

- **BasemapRequest**: Encapsulates tile source, optional style definition, target pixel
  dimensions (width, height, DPI), spatial bounds (either inferred or caller-supplied),
  and an optional layer filter list (`layers: Option<Vec<String>>`). When the filter list
  is present, only matching layers are rendered on a transparent canvas.
- **SpatialBounds**: A geographic bounding box (min/max longitude and latitude) paired
  with a coordinate reference system identifier.
- **TileSource**: A discriminated configuration for one of three tile formats: raster
  XYZ, Mapbox Vector Tile, or ESRI Vector Tile — including the URL template and any
  required authentication tokens.
- **StyleDefinition**: A validated JSON object conforming to a Mapbox-style-compatible
  layer specification that governs the visual rendering of vector features.
- **RenderedBasemap**: The output of a render call — a raw RGBA pixel array at the
  requested dimensions, paired with a `SpatialBounds` for downstream alignment.
- **StyleProvider**: A pure host-language helper (no Rust involvement) that accepts a
  tile URL template and optional styling parameters and serialises a valid MapLibre GL
  Style JSON string. The three concrete providers — `RasterProvider`,
  `VectorProvider`, `EsriVectorProvider`, and `EsriRasterProvider` (Python) /
  `raster_provider()`, `vector_provider()`, `esri_vector_provider()`, and
  `esri_raster_provider()` (R) — are the public entry points. `RasterProvider`,
  `VectorProvider`, and `EsriRasterProvider` return inline MapLibre GL JSON strings;
  `EsriVectorProvider` returns a URL string (the constructed `root.json` endpoint).
  All four outputs are accepted directly by `render_basemap_raw()` as `style_input`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A data scientist can add a styled basemap to an existing spatial plot in
  R or Python using a single additional function call with no manual coordinate entry.
- **SC-002**: The rendered basemap pixel dimensions exactly match the caller-specified
  target dimensions in every tested case (zero tolerance for off-by-one pixel errors at
  the array level).
- **SC-003**: The library installs cleanly via `pip install basemapper` (Python) and
  `install.packages("basemapper")` (R) on a system with no pre-installed GIS software,
  on macOS, Linux, and Windows.
- **SC-004**: All three tile source types (raster XYZ, Mapbox Vector Tile, ESRI Vector
  Tile) render a visually correct basemap for a standard bounding box covering a major
  city.
- **SC-005**: A custom greyscale JSON style definition visibly changes the basemap color
  scheme compared to the default style in the same render call, with no additional
  tooling.
- **SC-006**: Tile fetch errors surface a human-readable error message within 10 seconds
  of request (accounting for configurable timeout), rather than hanging indefinitely.
- **SC-007**: A data scientist who has only a bare tile URL can produce a working
  basemap in R or Python using a single provider helper call with no MapLibre GL JSON
  knowledge; the full workflow from URL to rendered basemap requires no more than two
  function calls total.
- **SC-008**: A data scientist can isolate a single named layer from any MapLibre GL style
  using one additional argument to `render_basemap_raw()`, and the resulting pixel array
  composites seamlessly over an existing plot with no masking artifacts (transparent
  background verified by inspecting the alpha channel of all non-rendered pixels).
- **SC-009**: A data scientist using tmap v4 in R can add a styled basemap to a tmap
  pipeline using a single `tm_basemap()` call with no manual bbox specification; the
  rendered map shows the basemap correctly aligned beneath sf data layers.
- **SC-010**: A data scientist using plotnine in Python can add `+ geom_basemap(style_url=...)`
  to a plotnine plot chain and receive a correctly positioned, lazily-rendered basemap
  beneath their data geoms, with no manual matplotlib interaction required.

## Assumptions

- The host plot object (ggplot2 or matplotlib) exposes its geographic bounding box and
  CRS through its public API; the library reads these via language-level introspection,
  not via screen scraping or image analysis.
- Callers supply authentication credentials (e.g., Mapbox API keys) as a parameter;
  the library does not manage or store API keys itself.
- Tile zoom level is selected automatically using standard slippy-map zoom-from-extent
  heuristics; a conservative maximum tile count is enforced by default to prevent
  accidental large downloads (exact limit to be set during planning).
- Font rendering for vector tile labels uses a bundled font set; external font
  dependencies are not required.
- The library targets interactive data science environments (Jupyter, RStudio, VS Code)
  as the primary usage context, with batch/server rendering as a secondary context.
- Mobile support is out of scope for the initial version.
- Offline tile caching (on-disk persistence across sessions) is out of scope for the
  initial version; in-memory caching within a single session is in scope.
