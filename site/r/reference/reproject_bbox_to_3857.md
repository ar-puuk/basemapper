# Reproject a bounding box to EPSG:3857 (Web Mercator).

Projects all four sides of the bbox (not just two corners) by converting
to a polygon first, ensuring the reprojected envelope is correct even at
high latitudes or for large geographic extents.

## Usage

``` r
reproject_bbox_to_3857(xmin, ymin, xmax, ymax, from_crs)
```

## Arguments

- xmin, ymin, xmax, ymax:

  Bounding-box corners in `from_crs` units.

- from_crs:

  Source CRS — anything accepted by
  [`sf::st_crs()`](https://r-spatial.github.io/sf/reference/st_crs.html),
  including integer EPSG codes, WKT strings, and `crs` objects.

## Value

Named numeric vector c(xmin, ymin, xmax, ymax) in EPSG:3857.
