# A style for tile data

Builds a style for tile data, with overrides. `source` is anything you
could pass as `style` directly; use this function to override what is
detected, or to get the style JSON to edit:

## Usage

``` r
tile_style(
  source,
  type = NULL,
  tile_size = NULL,
  scheme = NULL,
  minzoom = NULL,
  maxzoom = NULL,
  attribution = NULL,
  source_layers = NULL,
  paint = list(),
  auth_token = NULL
)
```

## Arguments

- source:

  The tile data, as above.

- type:

  `"raster"` or `"vector"`; detected when `NULL` (from the archive
  header, the TileJSON, the URL's extension, or one fetched tile).

- tile_size:

  Raster tile size in px (default 256 for tile URLs and archives).

- scheme:

  `"xyz"` or `"tms"` (rows counted from the south).

- minzoom, maxzoom:

  Zoom range of the tiles (overzoomed beyond `maxzoom`).

- attribution:

  Attribution text for the source.

- source_layers:

  Vector tiles: the source layers to draw (default all).

- paint:

  Vector tiles: a named list of paint properties for the generated
  `fill-*`, `line-*` and `circle-*` layers, e.g.
  `list("fill-color" = "#cccccc")`. Others are dropped with a warning.

- auth_token:

  Optional access token for the tile service.

## Value

The style JSON string.

## Details

- a tile URL template (`{z}/{x}/{y}`, `{quadkey}`, `{bbox-epsg-3857}`,
  WMTS `{TileMatrix}/{TileRow}/{TileCol}`);

- a TileJSON URL or file;

- a PMTiles archive: URL, local path or `pmtiles://` URL;

- an ArcGIS `.../MapServer` (raster tiles) or `.../VectorTileServer`
  (its published style);

- a style URL or file (returned with relative URLs made absolute).

Raster tiles get one raster layer. Vector tiles get a default style that
draws every source layer (polygons as translucent fills, lines and
outlines, points as circles, a colour per source layer) over a
transparent background. The same function exists in Python
(`basemapper.tile_style()`).

## Examples

``` r
style <- tile_style("https://tile.openstreetmap.org/{z}/{x}/{y}.png",
                    maxzoom = 19, attribution = "© OpenStreetMap contributors")
style <- tile_style("https://example.com/tiles/{z}/{x}/{y}.mvt",
                    source_layers = "land", paint = list("fill-color" = "#e8e0d8"))
if (FALSE) { # \dontrun{
# Detected from the data; also usable directly as `style`
tile_style("data/area.pmtiles")
basemap("data/area.pmtiles", 800, 600, bounds = nc)
} # }
```
