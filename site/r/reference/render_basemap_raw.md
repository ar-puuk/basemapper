# Render a basemap for bounds in one call

A one-shot render: fit `bounds` into a `width` x `height` CSS-pixel view
and return the pixels.
[`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)
with
[`render_array()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)
gives the same result with a reusable view; plot integrations use this
function. The same function exists in Python
(`basemapper.render_basemap_raw()`).

## Usage

``` r
render_basemap_raw(
  bounds,
  width,
  height,
  style,
  crs = NULL,
  pixel_ratio = 1,
  tile_zoom = NULL,
  layers = NULL,
  labels = TRUE,
  features = NULL,
  auth_token = NULL,
  tile_timeout_ms = 10000L,
  max_tiles = 256L,
  tile_concurrency = 16L,
  fail_on_tile_error = FALSE
)
```

## Arguments

- bounds:

  Bounding box: `c(xmin, ymin, xmax, ymax)` in `crs` (default EPSG:4326,
  with a message when the values look projected), an
  [`sf::st_bbox()`](https://r-spatial.github.io/sf/reference/st_bbox.html)
  result, or an `sf` / `sfc` / `sfg` object (CRS read from the object).

- width, height:

  View size in CSS pixels.

- style:

  A MapLibre GL style URL or JSON string (e.g. from
  [`basemap_provider()`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)),
  or a character vector / list of those stacked bottom to top (e.g. a
  basemap published as separate hillshade, base and label services).

- crs:

  CRS of `bounds` (anything
  [`sf::st_crs()`](https://r-spatial.github.io/sf/reference/st_crs.html)
  accepts).

- pixel_ratio:

  Device pixels per CSS pixel.

- tile_zoom:

  Tile detail override: the tile zoom on the 256 px grid (vector sources
  use one less, their tiles being 512 px), or `NULL` to pick tiles as
  MapLibre does for the view.

- layers:

  Optional layer filter applied to every style: plain IDs keep only
  those layers, minus-prefixed IDs drop them; `*` and `?` are wildcards
  (`"-label_*"`). See
  [`list_layers()`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md).
  Names that match no layer of any style warn.

- labels:

  `TRUE` keeps every layer, `FALSE` drops the labels (text and icons),
  `"only"` keeps only the labels on a transparent background, to stack
  over another map. Catalogue raster basemaps switch to their label-free
  or label-only tiles.

- features:

  Optional feature filter applied to every style: plain names keep those
  features, minus-prefixed names drop them, e.g. `c("water", "roads")`
  or `"-buildings"`. Features: background, water, landcover, parks,
  buildings, roads, rail, boundaries, places, poi, terrain, other (the
  `feature` column of
  [`list_layers()`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md)).
  Features no layer of any style belongs to warn.

- auth_token:

  Optional access token for the tile service.

- tile_timeout_ms:

  Per-tile HTTP timeout in milliseconds.

- max_tiles:

  Maximum tiles fetched per style.

- tile_concurrency:

  Parallel tile downloads.

- fail_on_tile_error:

  Abort on a failed tile instead of leaving it empty.

## Value

An integer array `[height * pixel_ratio, width * pixel_ratio, 4]` (RGBA,
0–255) with the input extent as attributes `xmin`, `ymin`, `xmax`,
`ymax` and `crs_epsg` (`NA` for a non-EPSG CRS).

## Examples

``` r
if (FALSE) { # \dontrun{
# WGS-84 bounds (default CRS)
a <- render_basemap_raw(c(-122.5, 37.7, -122.4, 37.8), 400, 300,
                        "OpenFreeMap.Positron")

# sf object: CRS read from it; two styles stacked; 2x pixels
nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
a <- render_basemap_raw(nc, 400, 300,
                        c("Esri.WorldShadedRelief",
                          "OpenFreeMap.Positron"),
                        pixel_ratio = 2)
} # }
```
