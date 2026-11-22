# Add a styled basemap layer to a tmap v4 pipeline.

Returns a deferred tmap element that renders tiles at draw time using
the bounding box resolved from the active `tm_shape()` — no explicit
`bbox` argument is needed. This mirrors how tmap's own `tm_basemap()`
works. Takes the same arguments as
[`geom_basemap()`](https://ar-puuk.github.io/basemapper/r/reference/geom_basemap.md),
plus the view size.

## Usage

``` r
tm_basemap(
  style,
  tile_zoom = NULL,
  alpha = 1,
  layers = NULL,
  labels = TRUE,
  features = NULL,
  pixel_ratio = NULL,
  auth_token = NULL,
  tile_timeout_ms = 10000L,
  max_tiles = 256L,
  attribution = TRUE,
  width = NULL,
  height = NULL,
  ...
)
```

## Arguments

- style:

  A MapLibre GL style URL or JSON string (e.g. from
  [`basemap_provider()`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)),
  or a vector / list of styles stacked bottom to top.

- tile_zoom:

  Tile detail override: the tile zoom on the 256 px grid (vector sources
  use one less, their tiles being 512 px), or `NULL` to pick tiles as
  MapLibre does for the view.

- alpha:

  Opacity of the basemap layer (0–1).

- layers:

  Optional layer filter applied to every style: plain IDs keep only
  those layers, minus-prefixed IDs drop them; `*` and `?` are wildcards
  (`"-label_*"`). See
  [`list_layers()`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md).
  Names that match no layer of any style warn.

- labels:

  `TRUE` keeps every layer, `FALSE` drops the labels (text and icons),
  `"only"` keeps only the labels on a transparent background, to stack
  over another map. Catalogue raster basemaps switch to their label-free
  or label-only tiles.

- features:

  Optional feature filter applied to every style: plain names keep those
  features, minus-prefixed names drop them, e.g. `c("water", "roads")`
  or `"-buildings"`. Features: background, water, landcover, parks,
  buildings, roads, rail, boundaries, places, poi, terrain, other (the
  `feature` column of
  [`list_layers()`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md)).
  Features no layer of any style belongs to warn.

- pixel_ratio:

  Device pixels per CSS pixel: the basemap raster is
  `width * pixel_ratio` by `height * pixel_ratio` pixels with the same
  zoom and label layout. Default: device dpi / 96 (300 dpi for PDF / SVG
  devices), so `tmap_save(dpi = 300)` gets a basemap rendered at 300
  dpi.

- auth_token:

  Optional access token for the tile service.

- tile_timeout_ms:

  Per-tile HTTP timeout in milliseconds.

- max_tiles:

  Maximum tiles fetched per style.

- attribution:

  The credit line the basemap's providers ask for, as a
  [`tmap::tm_credits()`](https://r-tmap.github.io/tmap/reference/tm_credits.html)
  element in the bottom-right corner: `TRUE` (the default) for the
  styles' own (see
  [`get_attribution()`](https://ar-puuk.github.io/basemapper/r/reference/get_attribution.md)),
  `FALSE` for none, or your own text.

- width, height:

  View size in CSS pixels (sets the zoom). By default the map panel's
  size is estimated from the device size, margins and facet layout, at
  96 CSS px per inch.

- ...:

  Reserved for future use.

## Value

A composable tmap element (class `"tmap"`). Put it before
[`tm_shape()`](https://r-tmap.github.io/tmap/reference/tm_shape.html) so
it renders behind subsequent layers.

## Examples

``` r
if (FALSE) { # \dontrun{
library(tmap)
library(sf)
nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
basemapper::tm_basemap("OpenFreeMap.Positron") +
  tm_shape(nc) +
  tm_sf(fill = NA, col = "steelblue")
} # }
```
