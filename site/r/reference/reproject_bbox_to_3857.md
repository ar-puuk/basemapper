# Reproject a bounding box to EPSG:3857 (Web Mercator).

Reproject a bounding box to EPSG:3857 (Web Mercator).

## Usage

``` r
reproject_bbox_to_3857(xmin, ymin, xmax, ymax, from_epsg)
```

## Arguments

- xmin, ymin, xmax, ymax:

  Bounding-box corners in `from_epsg` units.

- from_epsg:

  Source EPSG code (integer).

## Value

Named numeric vector c(xmin, ymin, xmax, ymax) in EPSG:3857.
