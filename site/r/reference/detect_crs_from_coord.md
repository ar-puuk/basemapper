# Detect the EPSG code from a ggplot2 coord/panel_params object.

Detect the EPSG code from a ggplot2 coord/panel_params object.

## Usage

``` r
detect_crs_from_coord(coord, panel_params = NULL)
```

## Arguments

- coord:

  A ggplot2 coord object (typically from coord_sf()).

- panel_params:

  Panel parameters from draw_panel (may contain resolved CRS).

## Value

Integer EPSG code, or 4326L with a message if not detectable.
