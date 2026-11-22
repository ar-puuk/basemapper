# Read a basemap view

- `get_bounds()`: the view's extent, `c(xmin, ymin, xmax, ymax)` in
  EPSG:3857 metres, or degrees with `lonlat = TRUE`.

- `get_center()`: `c(longitude, latitude)` of the centre.

- `get_zoom()`: the MapLibre zoom.

- `get_size()`: `c(width, height)` of rendered images in device pixels.

## Usage

``` r
get_bounds(m, lonlat = FALSE)

get_center(m)

get_zoom(m)

get_size(m)
```

## Arguments

- m:

  A
  [`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)
  object.

- lonlat:

  Return degrees (EPSG:4326) instead of EPSG:3857 metres.

## Value

A named numeric vector.

## Details

Python: `m.get_bounds()`, `m.get_center()`, `m.get_zoom()`,
`m.get_size()`.
