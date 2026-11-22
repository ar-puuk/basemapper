# Normalise a bbox input to coordinates plus an sf CRS object.

Dispatches on the type of `bbox`:

- [`sf::st_bbox()`](https://r-spatial.github.io/sf/reference/st_bbox.html)
  result (class `"bbox"`) — CRS read from the object.

- `sfc`, `sfg`, or `sf` data frame —
  [`sf::st_bbox()`](https://r-spatial.github.io/sf/reference/st_bbox.html)
  called first, then dispatch as above.

- Numeric vector of length 4 — CRS taken from `crs`; defaults to
  EPSG:4326 (WGS-84) with a message when coordinates appear projected.

## Usage

``` r
normalize_bbox(bbox, crs = NULL)
```

## Arguments

- bbox:

  Bounding box input (see above).

- crs:

  CRS override — anything accepted by
  [`sf::st_crs()`](https://r-spatial.github.io/sf/reference/st_crs.html),
  e.g. an integer EPSG code, WKT string, or `crs` object. Overrides a
  CRS baked into a `"bbox"` object when supplied.

## Value

A list with `vals` (named numeric `c(xmin, ymin, xmax, ymax)`) and
`crs_obj` (an
[`sf::crs`](https://r-spatial.github.io/sf/reference/coerce-methods.html)
object).
