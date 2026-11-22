# Create a basemap view

A map view to render headlessly: one style, or several stacked. The view
is a centre and MapLibre zoom (512 px tiles; fractional zooms are
rendered natively, not resampled), or bounds to fit. `width` and
`height` are CSS pixels; images are `pixel_ratio` times larger with the
same view and label layout (MapLibre's `devicePixelRatio`):
`pixel_ratio = 2` gives a sharp 2x image, `pixel_ratio = dpi / 96`
matches a print resolution.

## Usage

``` r
basemap(
  style,
  width = 800L,
  height = 600L,
  center = NULL,
  zoom = NULL,
  bounds = NULL,
  crs = NULL,
  pixel_ratio = 1,
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

- style:

  A MapLibre GL style URL or JSON string (e.g. from
  [`basemap_provider()`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)),
  or a character vector / list of those stacked bottom to top (e.g. a
  basemap published as separate hillshade, base and label services).

- width, height:

  View size in CSS pixels.

- center:

  `c(longitude, latitude)` of the view centre.

- zoom:

  MapLibre zoom (0–22, fractional allowed).

- bounds:

  Bounds to fit instead of `center`/`zoom`: `c(xmin, ymin, xmax, ymax)`
  in `crs` (default EPSG:4326), an
  [`sf::st_bbox()`](https://r-spatial.github.io/sf/reference/st_bbox.html)
  result, or an `sf` / `sfc` object.

- crs:

  CRS of `bounds` (anything
  [`sf::st_crs()`](https://r-spatial.github.io/sf/reference/st_crs.html)
  accepts).

- pixel_ratio:

  Device pixels per CSS pixel.

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

A `basemap` object.

## Details

Render it with
[`render_png()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md),
[`render_svg()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md),
[`render_raster()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)
or
[`render_array()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md).
The same function exists in Python (`basemapper.basemap()`) and Rust
(`Basemap::new()`), with the same arguments.

## Examples

``` r
if (FALSE) { # \dontrun{
ugrc <- "https://tiles.arcgis.com/tiles/99lidPhWCzftIe9K/arcgis/rest/services"
styles <- sprintf("%s/%s/VectorTileServer/resources/styles/root.json", ugrc,
                  c("VectorHillshade", "LiteBase", "LiteLabels"))
m <- basemap(styles, 800, 600, center = c(-111.8887, 40.7556), zoom = 12.4,
             pixel_ratio = 2)
render_png(m, "slc.png")   # 1600 x 1200
render_svg(m, "slc.svg")   # vector
r <- render_raster(m)      # terra SpatRaster in EPSG:3857

# Fit a layer's extent
nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
m <- basemap("OpenFreeMap.Positron", bounds = nc)
} # }
```
