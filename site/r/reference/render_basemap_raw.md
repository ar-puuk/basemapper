# Render a basemap and return raw RGBA bytes.

Low-level wrapper around the Rust FFI. Reprojects `bbox` to EPSG:3857
internally; all output attributes reflect the *input* CRS.

Supports anti-meridian crossing bboxes (`xmin > xmax` in EPSG:3857): the
render is split internally into west and east halves at proportional pixel
widths, then stitched horizontally.

Tile fetching uses an in-process LRU cache (512 tiles) shared across all
render calls in a session.

## Usage

``` r
render_basemap_raw(
  bbox,
  width,
  height,
  style_input,
  crs = NULL,
  zoom = NULL,
  tile_timeout_ms = 10000L,
  max_tiles = 256L,
  layers = NULL,
  auth_token = NULL,
  fail_on_tile_error = TRUE,
  tile_concurrency = 16L
)
```

## Arguments

- bbox:

  Bounding box — one of:

  - Numeric vector of length 4: `c(xmin, ymin, xmax, ymax)` in the CRS
    given by `crs` (defaults to EPSG:4326 / WGS-84).
  - An `sf::st_bbox()` result; CRS is read from the object.
  - An `sfc`, `sfg`, or `sf` data frame; bounding box and CRS are
    extracted automatically.

  Anti-meridian crossing bboxes (where `xmin > xmax` in EPSG:3857) are
  supported.

- width:

  Integer output pixel width.

- height:

  Integer output pixel height.

- style_input:

  Character: a MapLibre GL style URL or inline JSON string. Provider
  helpers such as [`basemap_provider()`](basemap_provider.md),
  [`raster_provider()`](raster_provider.md),
  [`wms_provider()`](wms_provider.md),
  [`wmts_provider()`](wmts_provider.md), and
  [`pmtiles_provider()`](pmtiles_provider.md) all return values accepted
  here.

- crs:

  CRS of `bbox` — anything accepted by `sf::st_crs()`, e.g. an integer
  EPSG code, WKT string, or `crs` object. Ignored when `bbox` already
  carries a CRS (e.g. an `st_bbox` result). Defaults to EPSG:4326
  (WGS-84) with a message when coordinates appear to be in a projected
  system.

- zoom:

  Integer zoom level (0–22), or `NULL` for automatic selection. When
  `NULL`, zoom is computed from the bbox width and output pixel width,
  then clamped to the tile source's advertised `maxzoom`.

- tile_timeout_ms:

  Integer per-tile HTTP timeout in milliseconds (default `10000L`).

- max_tiles:

  Integer maximum tile downloads per render (default `256L`).

- layers:

  Character vector of layer IDs to filter. Plain IDs keep only those
  layers; minus-prefixed IDs (e.g. `"-roads"`) exclude those layers.
  Mixed positive/negative lists raise an error. `NULL` renders all layers.

- auth_token:

  Character authentication token, or `NULL`. Sent as
  `Authorization: Bearer <token>` for raster sources; appended as
  `?access_token=<token>` for Mapbox vector sources.

- fail_on_tile_error:

  Logical. When `FALSE`, individual tile fetch failures are skipped (with
  a warning) rather than aborting the render. Defaults to `TRUE`.

- tile_concurrency:

  Integer maximum number of tiles fetched in parallel (default `16L`).

## Value

A 3-D integer array `[height, width, 4]` (RGBA, row-major) with
spatial-extent attributes `xmin`, `ymin`, `xmax`, `ymax` in the *input*
CRS, and `crs_epsg` set to the input EPSG code (`NA_integer_` for
non-EPSG CRS specifications).

## Examples

``` r
if (FALSE) { # \dontrun{
# WGS-84 bounding box (default CRS)
m <- render_basemap_raw(
  bbox        = c(-122.5, 37.7, -122.4, 37.8),
  width       = 400L,
  height      = 300L,
  style_input = basemap_provider("OpenFreeMap.liberty")
)

# sf::st_bbox() result — CRS inferred automatically
library(sf)
nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
m <- render_basemap_raw(
  bbox        = sf::st_bbox(nc),
  width       = 400L,
  height      = 300L,
  style_input = basemap_provider("Carto.Positron")
)

# Explicit EPSG:3857 bbox
m <- render_basemap_raw(
  bbox        = c(-13700000, 4500000, -13600000, 4600000),
  crs         = 3857L,
  width       = 400L,
  height      = 300L,
  style_input = basemap_provider("OSM.Standard")
)

# Anti-meridian bbox (Pacific-centred view)
m <- render_basemap_raw(
  bbox        = c(15000000, -5000000, -15000000, 5000000),
  crs         = 3857L,
  width       = 800L,
  height      = 400L,
  style_input = basemap_provider("OSM.Standard")
)

# PMTiles archive
m <- render_basemap_raw(
  bbox        = c(-122.5, 37.7, -122.4, 37.8),
  width       = 400L,
  height      = 300L,
  style_input = pmtiles_provider("https://example.com/tiles/world.pmtiles")
)
} # }
```
