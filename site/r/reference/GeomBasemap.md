# ggproto class for basemap rendering inside a ggplot2 panel.

Tile fetching is deferred to draw time so that `coord_sf()` has already
established the panel's spatial extent.

## Usage

``` r
GeomBasemap
```
