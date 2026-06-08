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

### Edge Cases

- What happens when tiles for the requested bounding box / zoom level are unavailable
  from the remote source (network error, 404, rate limit)?
- How does the library behave when the plot's extent straddles the anti-meridian (180°)?
- What happens when the plot extent is so large that an appropriate zoom level would
  require more than a configurable maximum number of tile downloads?
- What happens when the JSON style references a font or sprite sheet that is not
  bundled with the library?

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

### Key Entities

- **BasemapRequest**: Encapsulates tile source, optional style definition, target pixel
  dimensions (width, height, DPI), and spatial bounds — either inferred or caller-supplied.
- **SpatialBounds**: A geographic bounding box (min/max longitude and latitude) paired
  with a coordinate reference system identifier.
- **TileSource**: A discriminated configuration for one of three tile formats: raster
  XYZ, Mapbox Vector Tile, or ESRI Vector Tile — including the URL template and any
  required authentication tokens.
- **StyleDefinition**: A validated JSON object conforming to a Mapbox-style-compatible
  layer specification that governs the visual rendering of vector features.
- **RenderedBasemap**: The output of a render call — a raw RGBA pixel array at the
  requested dimensions, paired with a `SpatialBounds` for downstream alignment.

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
