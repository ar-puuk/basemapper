# Change a basemap view

`set_view()` sets the centre and/or zoom; `set_bounds()` fits bounds
(keeping the view's size). Python: `m.set_view()`, `m.set_bounds()`.

## Usage

``` r
set_view(m, center = NULL, zoom = NULL)

set_bounds(m, bounds, crs = NULL)
```

## Arguments

- m:

  A
  [`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)
  object.

- center:

  `c(longitude, latitude)`, or `NULL` to keep it.

- zoom:

  MapLibre zoom, or `NULL` to keep it.

- bounds:

  Bounds to fit instead of `center`/`zoom`: `c(xmin, ymin, xmax, ymax)`
  in `crs` (default EPSG:4326), an
  [`sf::st_bbox()`](https://r-spatial.github.io/sf/reference/st_bbox.html)
  result, or an `sf` / `sfc` object.

- crs:

  CRS of `bounds` (anything
  [`sf::st_crs()`](https://r-spatial.github.io/sf/reference/st_crs.html)
  accepts).

## Value

The modified `basemap`.
