# Detect the EPSG code from a ggplot2 coord object.

Detect the EPSG code from a ggplot2 coord object.

## Usage

``` r
detect_crs_from_coord(coord)
```

## Arguments

- coord:

  A ggplot2 coord object (typically from coord_sf()).

## Value

Integer EPSG code, or 4326L with a message if not detectable.
