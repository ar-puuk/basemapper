# List a style's layers, or preview a filter

The layers of a style in drawing order: their `id` (what `layers`
matches), `type` (`symbol` layers are the labels `labels` keeps or
drops), `source_layer` (the vector tile layer drawn; `NA` for background
and raster layers) and `feature` (what `features` matches). With
`layers`, `labels` or `features`, the layers a render with those filters
draws; no rows when they leave nothing. Python:
`basemapper.list_layers()`.

## Usage

``` r
list_layers(
  style,
  auth_token = NULL,
  layers = NULL,
  labels = TRUE,
  features = NULL
)
```

## Arguments

- style:

  A style (anything
  [`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)
  takes).

- auth_token:

  Optional access token for the style URL.

- layers, labels, features:

  Filters to preview, as the renderers take them (see
  [`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)).

## Value

A data frame with columns `id`, `type`, `source_layer` and `feature`.

## Details

A layer's feature comes from its source layer, with tables for the
OpenMapTiles (OpenFreeMap, Stadia, MapTiler), Protomaps (CARTO) and
Shortbread (VersaTiles) schemas and keywords for others (Esri's
descriptive names, layer ids). The features are background, water,
landcover, parks, buildings, roads, rail, boundaries, places, poi,
terrain and other. They are a convenience: check them here, and use
`layers` for exact control.

## Examples

``` r
if (FALSE) { # \dontrun{
list_layers("OpenFreeMap.Positron")
list_layers("OpenFreeMap.Positron", labels = "only")      # what would be drawn
list_layers("OpenFreeMap.Positron", layers = "highway_*")
} # }
```
