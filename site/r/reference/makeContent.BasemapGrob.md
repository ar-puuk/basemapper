# makeContent method for BasemapGrob.

Called by grid at draw time (inside an active viewport). Renders the
basemap for the panel's size at the device's resolution and returns a
rasterGrob child.

## Usage

``` r
# S3 method for class 'BasemapGrob'
makeContent(x)
```

## Arguments

- x:

  A `BasemapGrob` created by
  [`geom_basemap()`](https://ar-puuk.github.io/basemapper/r/reference/geom_basemap.md).

## Value

A gTree with the rendered basemap as its rasterGrob child.
