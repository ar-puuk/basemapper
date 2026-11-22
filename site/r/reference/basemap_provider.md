# Named basemaps

`basemap_provider()` returns the style of a named basemap, for
[`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)
and the other renderers' `style` argument. Names follow
leaflet-providers (`"CartoDB.Positron"`, `"Esri.WorldImagery"`,
`"OpenStreetMap.HOT"`);
[`list_providers()`](https://ar-puuk.github.io/basemapper/r/reference/list_providers.md)
lists them with their forms, and
[providers](https://ar-puuk.github.io/basemapper/r/reference/providers.md)
completes them. A name also works as `style` directly; use this function
to pick the form, pass a key, or draw your own tiles with the basemap's
style. Implemented in the Rust core, so R and Python
(`basemapper.basemap_provider()`) return the same style.

## Usage

``` r
basemap_provider(
  name,
  type = NULL,
  api_key = NULL,
  tiles = NULL,
  labels = TRUE,
  features = NULL
)
```

## Arguments

- name:

  Basemap name (case-insensitive).

- type:

  `"vector"` (a MapLibre style) or `"raster"` (tiles). By default the
  vector form when there is one, else raster. Asking for a form a
  basemap lacks is an error, never a silent fallback.

- api_key:

  API key for CARTO, MapTiler and Stadia; by default read from
  `CARTO_API_KEY`, `MAPTILER_API_KEY` or `STADIA_API_KEY`. The keys are
  free for moderate use;
  [`list_providers()`](https://ar-puuk.github.io/basemapper/r/reference/list_providers.md)
  shows which basemaps need one.

- tiles:

  Your own tile data to draw with this basemap's style: a `.pmtiles`
  archive (URL or path), tile URL or TileJSON. It must use the same
  schema as the provider's tiles (OpenMapTiles for OpenFreeMap and
  Stadia).

- labels:

  `TRUE` keeps every layer, `FALSE` drops the labels, `"only"` keeps
  only the labels, to stack this basemap's labels over another map.
  Raster tiles switch to the provider's label-free or label-only tiles
  (an error when it has none).

- features:

  Vector form: features to keep (`c("water", "roads")`) or drop
  (`"-buildings"`); see
  [`list_layers()`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md).

## Value

A style string: a URL, or inline style JSON (the raster form, or with
`tiles`, `labels` or `features`).

## See also

[`tile_style()`](https://ar-puuk.github.io/basemapper/r/reference/tile_style.md)
for your own tile data,
[`wms_style()`](https://ar-puuk.github.io/basemapper/r/reference/ogc_styles.md)
for OGC services.

## Examples

``` r
basemap_provider("OpenFreeMap.Liberty")
#> [1] "https://tiles.openfreemap.org/styles/liberty"
basemap_provider("Esri.WorldGrayCanvas", type = "raster")
#> [1] "{\"layers\":[{\"id\":\"raster\",\"source\":\"raster\",\"type\":\"raster\"}],\"metadata\":{\"basemapper:raster\":{\"has_labels\":false,\"labels\":[\"https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Reference/MapServer/tile/{z}/{y}/{x}\"],\"provider\":\"Esri.WorldGrayCanvas\"}},\"sources\":{\"raster\":{\"attribution\":\"Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors, and the GIS User Community\",\"maxzoom\":16,\"tileSize\":256,\"tiles\":[\"https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Base/MapServer/tile/{z}/{y}/{x}\"],\"type\":\"raster\"}},\"version\":8}"
if (FALSE) { # \dontrun{
basemap_provider("CartoDB.Voyager", api_key = "...")   # or set CARTO_API_KEY
basemap_provider("OpenFreeMap.Liberty", tiles = "data/area.pmtiles")
# Imagery under a vector basemap's labels
basemap(c("Esri.WorldImagery", basemap_provider("OpenFreeMap.Liberty", labels = "only")),
        center = c(-111.89, 40.76), zoom = 12)
} # }
```
