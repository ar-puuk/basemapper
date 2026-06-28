# WMS provider style

Build a MapLibre GL Style JSON string that uses a WMS GetMap endpoint as a
raster tile source. The Rust tile engine substitutes `{xmin}`, `{ymin}`,
`{xmax}`, `{ymax}` with EPSG:3857 coordinates for each 256×256 tile.

## Usage

``` r
wms_provider(
  url,
  layers,
  styles = "",
  version = "1.3.0",
  format = "image/png",
  transparent = FALSE,
  extra_params = NULL
)
```

## Arguments

- url:

  Character: base WMS URL (without query parameters).

- layers:

  Character: value for the `LAYERS` parameter.

- styles:

  Character: value for the `STYLES` parameter (default `""`).

- version:

  Character: WMS version string (default `"1.3.0"`).

- format:

  Character: image MIME type (default `"image/png"`).

- transparent:

  Logical: request transparent PNG tiles (default `FALSE`).

- extra_params:

  Character or `NULL`: additional query parameters appended verbatim.

## Value

A JSON character string ready to pass to
[`render_basemap_raw()`](render_basemap_raw.md).

## Examples

``` r
if (FALSE) { # \dontrun{
style <- wms_provider(
  url    = "https://gis.example.com/wms",
  layers = "imagery"
)
m <- render_basemap_raw(
  bbox        = c(-122.5, 37.7, -122.4, 37.8),
  width       = 400L,
  height      = 300L,
  style_input = style
)
} # }
```
