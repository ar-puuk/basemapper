# Styles for OGC services

Styles for WMS GetMap and WMTS GetTile (KVP) endpoints, as 256 px tiles
in EPSG:3857 (WMS through MapLibre's `{bbox-epsg-3857}` token). A
RESTful WMTS template (`{TileMatrix}/{TileRow}/{TileCol}`) needs no
builder: pass it as `style`. Python: `basemapper.wms_style()`,
`basemapper.wmts_style()`.

## Usage

``` r
wms_style(
  url,
  layers,
  styles = "",
  version = "1.3.0",
  format = "image/png",
  transparent = FALSE,
  extra_params = NULL
)

wmts_style(
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

  Base service URL without query parameters.

- layers:

  WMS `LAYERS` parameter.

- styles:

  WMS `STYLES` parameter.

- version:

  Service version (WMS 1.1.x sends `SRS`, 1.3.0 `CRS`).

- format:

  Image MIME type.

- transparent:

  Request transparent WMS images.

- extra_params:

  Additional query parameters appended verbatim, or `NULL`.

- layer:

  WMTS `LAYER` parameter.

- tilematrixset:

  WMTS tile matrix set (a Web Mercator one).

- style:

  WMTS `STYLE` parameter.

## Value

A style JSON string.

## Examples

``` r
style <- wms_style("https://example.com/wms", layers = "relief")
style <- wmts_style("https://example.com/wmts", layer = "imagery")
```
