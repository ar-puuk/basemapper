# Package index

## Package

- [`basemapper`](https://ar-puuk.github.io/basemapper/r/reference/basemapper-package.md)
  [`basemapper-package`](https://ar-puuk.github.io/basemapper/r/reference/basemapper-package.md)
  : basemapper: Headless Spatial Basemap Renderer

## Basemap views

Create a view
([`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)),
change it
([`set_view()`](https://ar-puuk.github.io/basemapper/r/reference/set_view.md),
[`set_bounds()`](https://ar-puuk.github.io/basemapper/r/reference/set_view.md)),
read it
([`get_bounds()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md),
[`get_center()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md),
[`get_zoom()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md),
[`get_size()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md))
and render it
([`render_png()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md),
[`render_svg()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md),
[`render_array()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md),
[`render_raster()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)).
The same functions and arguments exist in Python.

- [`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)
  : Create a basemap view
- [`set_view()`](https://ar-puuk.github.io/basemapper/r/reference/set_view.md)
  [`set_bounds()`](https://ar-puuk.github.io/basemapper/r/reference/set_view.md)
  : Change a basemap view
- [`get_bounds()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md)
  [`get_center()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md)
  [`get_zoom()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md)
  [`get_size()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md)
  : Read a basemap view
- [`render_png()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)
  [`render_svg()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)
  [`render_array()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)
  [`render_raster()`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)
  : Render a basemap view
- [`plot(`*`<basemap>`*`)`](https://ar-puuk.github.io/basemapper/r/reference/plot.basemap.md)
  [`knit_print(`*`<basemap>`*`)`](https://ar-puuk.github.io/basemapper/r/reference/plot.basemap.md)
  : Show a basemap view

## One-shot rendering

Render bounds to a georeferenced RGBA array in one call.

- [`render_basemap_raw()`](https://ar-puuk.github.io/basemapper/r/reference/render_basemap_raw.md)
  : Render a basemap for bounds in one call

## Plot integrations

Drop a basemap layer into an existing plot, rendered at the output
resolution.
[`geom_basemap()`](https://ar-puuk.github.io/basemapper/r/reference/geom_basemap.md)
takes the same arguments in ggplot2 and plotnine (Python).

- [`geom_basemap()`](https://ar-puuk.github.io/basemapper/r/reference/geom_basemap.md)
  : Add a styled basemap layer to a ggplot2 spatial plot.
- [`GeomBasemap`](https://ar-puuk.github.io/basemapper/r/reference/GeomBasemap.md)
  : ggproto class for basemap rendering inside a ggplot2 panel.
- [`tm_basemap()`](https://ar-puuk.github.io/basemapper/r/reference/tm_basemap.md)
  : Add a styled basemap layer to a tmap v4 pipeline.

## Styles

Named basemaps (leaflet-providers names, usable as `style`), styles for
your own tile data, and OGC services. Tile data (tile URLs, TileJSON,
PMTiles, ArcGIS services) also works as `style` directly.

- [`basemap_provider()`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)
  : Named basemaps
- [`list_providers()`](https://ar-puuk.github.io/basemapper/r/reference/list_providers.md)
  : The built-in basemaps
- [`providers`](https://ar-puuk.github.io/basemapper/r/reference/providers.md)
  : Basemap names, for autocompletion
- [`tile_style()`](https://ar-puuk.github.io/basemapper/r/reference/tile_style.md)
  : A style for tile data
- [`wms_style()`](https://ar-puuk.github.io/basemapper/r/reference/ogc_styles.md)
  [`wmts_style()`](https://ar-puuk.github.io/basemapper/r/reference/ogc_styles.md)
  : Styles for OGC services

## Style layers and attribution

A style’s layers with their features, for the `layers` and `features`
filters, and the attribution text it asks maps to show.

- [`list_layers()`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md)
  : List a style's layers, or preview a filter
- [`get_attribution()`](https://ar-puuk.github.io/basemapper/r/reference/get_attribution.md)
  : The attribution a style asks maps to show

## Tile cache

- [`tile_cache()`](https://ar-puuk.github.io/basemapper/r/reference/tile_cache.md)
  : The tile cache

## Coordinate utilities

Internal helpers exposed for advanced use.

- [`reproject_bbox_to_3857()`](https://ar-puuk.github.io/basemapper/r/reference/reproject_bbox_to_3857.md)
  : Reproject a bounding box to EPSG:3857 (Web Mercator).
- [`detect_crs_from_coord()`](https://ar-puuk.github.io/basemapper/r/reference/detect_crs_from_coord.md)
  : Detect the EPSG code from a ggplot2 coord/panel_params object.
