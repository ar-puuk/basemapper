# Add a styled basemap layer to a ggplot2 spatial plot.

Wraps a lazy `GeomBasemap` ggproto layer that defers tile fetching until
`coord_sf()` has established the panel's geographic extent.

## Usage

``` r
geom_basemap(
  style_url,
  zoom = NULL,
  tile_timeout = 10000L,
  max_tiles = 256L,
  alpha = 1,
  layers = NULL,
  ...
)
```

## Arguments

- style_url:

  Character: a MapLibre GL style URL or inline JSON string.

- zoom:

  Integer zoom level (0–22), or `NULL` for automatic selection.

- tile_timeout:

  Integer per-tile HTTP timeout in milliseconds.

- max_tiles:

  Integer maximum tile downloads per render.

- alpha:

  Numeric opacity of the basemap layer (0–1).

- layers:

  Character vector of layer IDs to filter. Plain IDs keep only those
  layers; minus-prefixed IDs exclude those layers. `NULL` renders all.

- ...:

  Additional arguments passed to
  [`ggplot2::layer()`](https://ggplot2.tidyverse.org/reference/layer.html).

## Value

A ggplot2 layer object.

## Examples

``` r
if (FALSE) { # \dontrun{
library(ggplot2)
library(sf)
nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
ggplot(nc) +
  geom_basemap(style_url = "https://demotiles.maplibre.org/style.json") +
  geom_sf(fill = NA, color = "steelblue") +
  coord_sf()
} # }
```
