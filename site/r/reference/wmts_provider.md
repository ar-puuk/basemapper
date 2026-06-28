# WMTS provider style

Build a MapLibre GL Style JSON string that uses a WMTS GetTile KVP endpoint
as a raster tile source.

## Usage

``` r
wmts_provider(
  url,
  layer,
  tilematrixset = "GoogleMapsCompatible",
  format = "image/png",
  style = "default",
  version = "1.0.0",
  extra_params = NULL
)
```

## Arguments

- url:

  Character: base WMTS URL (without query parameters).

- layer:

  Character: value for the `LAYER` parameter.

- tilematrixset:

  Character: tile matrix set (default `"GoogleMapsCompatible"`).

- format:

  Character: image MIME type (default `"image/png"`).

- style:

  Character: style identifier (default `"default"`).

- version:

  Character: WMTS version string (default `"1.0.0"`).

- extra_params:

  Character or `NULL`: additional query parameters appended verbatim.

## Value

A JSON character string ready to pass to
[`render_basemap_raw()`](render_basemap_raw.md).

## Examples

``` r
if (FALSE) { # \dontrun{
style <- wmts_provider(
  url   = "https://tiles.example.com/wmts",
  layer = "BasemapAT_grau"
)
m <- render_basemap_raw(
  bbox        = c(14.0, 48.0, 17.0, 50.0),
  width       = 400L,
  height      = 300L,
  style_input = style
)
} # }
```
