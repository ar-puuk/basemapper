# Render a basemap and return raw RGBA bytes.

Low-level wrapper around the Rust FFI. The returned raw vector must be
reshaped before use; see the example below.

## Usage

``` r
render_basemap_raw(
  bbox_3857,
  width,
  height,
  style_input,
  zoom = NULL,
  tile_timeout_ms = 10000L,
  max_tiles = 256L,
  layers = NULL
)
```

## Arguments

- bbox_3857:

  Numeric vector of length 4: `c(xmin, ymin, xmax, ymax)` in EPSG:3857
  metres.

- width:

  Integer output pixel width.

- height:

  Integer output pixel height.

- style_input:

  Character: a MapLibre GL style URL or inline JSON string.

- zoom:

  Integer zoom level (0–22), or `NULL` for automatic selection.

- tile_timeout_ms:

  Integer per-tile HTTP timeout in milliseconds.

- max_tiles:

  Integer maximum tile downloads per render.

- layers:

  Character vector of layer IDs to filter. Plain IDs keep only those
  layers; minus-prefixed IDs (e.g. `"-roads"`) exclude those layers.
  Mixed positive/negative lists raise an error. `NULL` renders all
  layers.

## Value

A `raw` vector of length `width * height * 4` (RGBA, row-major).

## Examples

``` r
if (FALSE) { # \dontrun{
raw <- render_basemap_raw(
  bbox_3857   = c(-13700000, 4500000, -13600000, 4600000),
  width       = 400L,
  height      = 300L,
  style_input = "https://demotiles.maplibre.org/style.json"
)
m <- array(as.integer(raw), dim = c(4L, 400L, 300L))
m <- aperm(m, c(3, 2, 1))  # [height, width, channels]
} # }
```
