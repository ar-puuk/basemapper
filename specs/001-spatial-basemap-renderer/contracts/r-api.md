# Contract: R Public API (`r-basemapper`)

**Package**: `basemapper` (CRAN / r-universe)
**Build**: `rextendr::document()` + `R CMD build`
**R requirement**: ≥ 4.1.0
**Runtime dependencies**: `ggplot2`, `grid`, `sf` (for CRS reprojection)

---

## High-Level API (pure R, ggplot2 integration)

### `geom_basemap()`

```r
geom_basemap <- function(
  style_url,
  zoom         = NULL,
  tile_timeout = 10000L,
  max_tiles    = 256L,
  ...
)
```

**Returns**: A ggplot2 layer object that can be added to any ggplot with `coord_sf()`.

**Behaviour** (lazy — no network call at layer construction time):
1. Stores `style_url`, `zoom`, `tile_timeout`, `max_tiles` as layer parameters.
2. At render time (`draw_panel()`):
   a. Extracts `x_range` and `y_range` from `panel_params`.
   b. Determines the active CRS from `coord$crs` (set by `coord_sf()`).
   c. Calls `sf::st_transform()` to reproject the four corner points to EPSG:3857.
   d. Reads device pixel dimensions via `grid::convertWidth()` /
      `grid::convertHeight()` on the panel's viewport.
   e. Calls `render_basemap_raw(bbox_3857, width, height, style_url, zoom, ...)`.
   f. Reshapes the returned raw vector to an `array(dim = c(height, width, 4))`.
   g. Returns `grid::rasterGrob(arr, width = 1, height = 1, interpolate = FALSE)`.

**Usage**:

```r
library(ggplot2)
library(sf)
library(basemapper)

nc <- st_read(system.file("shape/nc.shp", package = "sf"))

ggplot(nc) +
  geom_basemap(style_url = "https://demotiles.maplibre.org/style.json") +
  geom_sf(fill = NA, color = "red") +
  coord_sf()
```

---

## Low-Level API (extendr native)

### `render_basemap_raw()`

```r
render_basemap_raw <- function(
  bbox_3857,          # numeric vector length 4: c(xmin, ymin, xmax, ymax), EPSG:3857
  width,              # integer: output pixel width
  height,             # integer: output pixel height
  style_input,        # character: style URL or inline JSON string
  zoom        = NULL, # integer or NULL: tile zoom level (0–22); auto if NULL
  tile_timeout_ms = 10000L,
  max_tiles       = 256L
)
# Returns: raw vector of length width * height * 4 (RGBA bytes, row-major)
# Signals: stop() with a descriptive message on any error
```

**Caller responsibility**: reshape the raw vector to a matrix/array and attach
spatial attributes before passing downstream.

```r
raw_bytes <- render_basemap_raw(
  bbox_3857    = c(-13700000, 4500000, -13600000, 4600000),
  width        = 800L,
  height       = 600L,
  style_input  = "https://demotiles.maplibre.org/style.json"
)

# Reshape to height × width × 4 array
m <- array(as.integer(raw_bytes), dim = c(4L, 800L, 600L))
m <- aperm(m, c(3, 2, 1))   # reorder to [height, width, channels]

# Attach spatial metadata
attr(m, "xmin")     <- -13700000
attr(m, "ymin")     <-   4500000
attr(m, "xmax")     <- -13600000
attr(m, "ymax")     <-   4600000
attr(m, "crs_epsg") <- 3857L
```

---

## Provider Helpers

### `vector_provider(url_template, source_layer, paint = list())`

`source_layer` is **required** — it names the layer within each MVT tile to render
(the MapLibre GL `"source-layer"` property). This is tile-server specific (e.g.,
`"water"`, `"roads"`, `"land"`). Omitting it stops with an error.

---

## Return Value Conventions

- `render_basemap_raw` returns a `raw` vector of exactly `width * height * 4` bytes.
- Channel order: R, G, B, A (same as RGBA raster standard).
- Row-major order, top-left origin.
- The `geom_basemap()` ggproto layer wraps this automatically; direct callers must
  reshape and transpose as shown above.

---

## Error Handling

All errors from the Rust core are surfaced via `stop()` with a message that includes
the stage name and detail string (e.g., `"tile fetch failed for ...: HTTP 429"`).

---

## Threading Contract

`render_basemap_raw` is called synchronously on R's main thread. Internally, the
extendr binding dispatches tile fetching onto a dedicated `tokio::Runtime` owned by
the package (initialized at `.onLoad()` time) so that the R event loop is not blocked
by network I/O. The function itself blocks in R until the render completes, but the
underlying I/O is non-blocking at the OS level.
