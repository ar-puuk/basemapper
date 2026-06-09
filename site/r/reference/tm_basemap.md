# Add a styled basemap layer to a tmap v4 pipeline.

Derives the map extent from the tmap pipeline's active primary shape via
`tmap::bb()`, renders the basemap via the Rust core, wraps the pixel
array as a georeferenced `stars` object, and returns a composable tmap
element (`tmap::tm_shape() + tmap::tm_rgb()`).

## Usage

``` r
tm_basemap(
  style_input,
  bbox = NULL,
  zoom = NULL,
  alpha = 1,
  layers = NULL,
  width_px = NULL,
  height_px = NULL,
  ...
)
```

## Arguments

- style_input:

  Character: a MapLibre GL style URL or inline JSON string.

- bbox:

  An explicit bounding box (sf bbox, numeric vector
  `c(xmin, ymin, xmax, ymax)` in any CRS, or an sf/stars/sfc object).
  When `NULL`, derived from `tmap::bb()` of the current pipeline's
  primary shape.

- zoom:

  Integer zoom level (0–22), or `NULL` for automatic.

- alpha:

  Numeric opacity (0–1).

- layers:

  Character vector of layer IDs to filter. See
  [`render_basemap_raw()`](https://ar-puuk.github.io/basemapper/r/reference/render_basemap_raw.md).

- width_px, height_px:

  Output dimensions in pixels. When `NULL`, 800×600 is used as a
  sensible default for static maps.

- ...:

  Additional arguments (reserved for future use).

## Value

A composable tmap element: `tmap::tm_shape(stars_obj) + tmap::tm_rgb()`.

## tmap v4 note

H1: If [`tm_rgb()`](https://r-tmap.github.io/tmap/reference/tm_rgb.html)
applies an unwanted colour palette (rare with RGBA stars objects), fall
back to
[`tm_raster()`](https://r-tmap.github.io/tmap/reference/tm_raster.html)
with explicit band selection. Prefer
[`tm_rgb()`](https://r-tmap.github.io/tmap/reference/tm_rgb.html) first.

## Examples

``` r
if (FALSE) { # \dontrun{
library(tmap)
library(sf)
nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
tm_shape(nc) +
  tm_basemap("https://demotiles.maplibre.org/style.json") +
  tm_sf()
} # }
```
