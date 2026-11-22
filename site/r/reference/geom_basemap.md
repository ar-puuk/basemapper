# Add a styled basemap layer to a ggplot2 spatial plot.

Wraps a lazy `GeomBasemap` ggproto layer that defers tile fetching until
`coord_sf()` has established the panel's geographic extent.

## Usage

``` r
geom_basemap(
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

  Device pixels per CSS pixel (MapLibre's devicePixelRatio). Default:
  device dpi / 96, so lines and labels keep their physical size at any
  resolution. Larger values draw map symbols smaller.

- auth_token:

  Optional access token for the tile service.

- tile_timeout_ms:

  Per-tile HTTP timeout in milliseconds.

- max_tiles:

  Maximum tiles fetched per style.

- attribution:

  The credit line the basemap's providers ask for, drawn small in the
  bottom-right corner of each panel: `TRUE` (the default) for the
  styles' own (see
  [`get_attribution()`](https://ar-puuk.github.io/basemapper/r/reference/get_attribution.md)),
  `FALSE` for none, or your own text. It is drawn with the basemap, so
  later layers can cover it.

- ...:

  Additional arguments passed to
  [`ggplot2::layer()`](https://ggplot2.tidyverse.org/reference/layer.html).

## Value

A ggplot2 layer object.

## Details

The basemap is rendered when the plot is drawn, for the panel's size at
the device's resolution: `ggsave(dpi = 300)` gets a basemap rendered
natively at 300 dpi (and PDF / SVG output at 300 dpi), not an upscaled
screen image. Python (plotnine): `basemapper.geom_basemap()`, with the
same arguments.

## Examples

``` r
if (FALSE) { # \dontrun{
library(ggplot2)
library(sf)
nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
ggplot(nc) +
  geom_basemap("OpenFreeMap.Positron") +
  geom_sf(fill = NA, color = "steelblue") +
  coord_sf()
} # }
```
