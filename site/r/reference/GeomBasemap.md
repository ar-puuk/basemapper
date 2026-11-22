# ggproto class for basemap rendering inside a ggplot2 panel.

Tile fetching is deferred to actual draw time via a custom grob so that
the viewport pixel dimensions are available when the basemap is
rendered. This also means `coord_sf()` has already established the
panel's spatial extent before any fetching begins.

## Usage

``` r
GeomBasemap
```
