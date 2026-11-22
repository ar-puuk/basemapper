# API Reference


## Basemap views


Create a view (`basemap()`), change it (`set_view`, `set_bounds`), read it (`get_bounds`, `get_center`, `get_zoom`, `get_size`) and render it (`render_png`, `render_svg`, `render_array`). The same functions and arguments exist in R.


[basemap()](basemap.md#basemapper.basemap)  
Create a basemap view.

[Basemap](Basemap.qmd#basemapper.Basemap)  
A map view rendered headlessly: one style, or several stacked.


## Basemap Methods


Methods for the Basemap class


[Basemap.set_view()](Basemap.set_view.md#basemapper.Basemap.set_view)  
Set the centre `(longitude, latitude)` and/or the MapLibre zoom

[Basemap.set_bounds()](Basemap.set_bounds.md#basemapper.Basemap.set_bounds)  
Fit the view to `bounds` (in `crs`, default EPSG:4326), keeping

[Basemap.get_bounds()](Basemap.get_bounds.md#basemapper.Basemap.get_bounds)  
The view's `(xmin, ymin, xmax, ymax)` in EPSG:3857 metres, or in

[Basemap.get_center()](Basemap.get_center.md#basemapper.Basemap.get_center)  
`(longitude, latitude)` of the view centre.

[Basemap.get_zoom()](Basemap.get_zoom.md#basemapper.Basemap.get_zoom)  
The MapLibre zoom of the view.

[Basemap.get_size()](Basemap.get_size.md#basemapper.Basemap.get_size)  
`(width, height)` of rendered images in device pixels.

[Basemap.get_attribution()](Basemap.get_attribution.md#basemapper.Basemap.get_attribution)  
The attribution text the view's styles ask maps to show (see

[Basemap.__repr__()](Basemap.__repr__.md#basemapper.Basemap.__repr__)  

[Basemap.render_array()](Basemap.render_array.md#basemapper.Basemap.render_array)  
Render to a `(height, width, 4)` uint8 RGBA array (device pixels).

[Basemap.render_png()](Basemap.render_png.md#basemapper.Basemap.render_png)  
Render a PNG, written to `path` when given. Returns the PNG bytes.

[Basemap.render_svg()](Basemap.render_svg.md#basemapper.Basemap.render_svg)  
Render a vector SVG (`width` x `height` user units;


## One-shot rendering


Render bounds to an RGBA array in one call.


[render_basemap_raw()](render_basemap_raw.md#basemapper.render_basemap_raw)  
Render a basemap for bounds in one call.


## Plot integrations


Drop a basemap layer into an existing plot, rendered at the output resolution. `geom_basemap` takes the same arguments in plotnine and ggplot2 (R).


[add_basemap()](add_basemap.md#basemapper.add_basemap)  
Render a basemap beneath existing axes content.

[geom_basemap](geom_basemap.md#basemapper.geom_basemap)  
Render a styled basemap beneath plotnine spatial layers.


## Styles


Named basemaps (leaflet-providers names, usable as `style`), styles for your own tile data, and OGC services. Tile data (tile URLs, TileJSON, PMTiles, ArcGIS services) also works as `style` directly.


[basemap_provider()](basemap_provider.md#basemapper.basemap_provider)  
A named basemap's style.

[list_providers()](list_providers.md#basemapper.list_providers)  
The basemaps `basemap_provider()` knows.

[providers](providers.md#basemapper.providers)  
Basemap names for autocompletion: `providers.CartoDB.Positron` is the

[tile_style()](tile_style.md#basemapper.tile_style)  
A style for tile data, with overrides.

[wms_style()](wms_style.md#basemapper.wms_style)  
Style for an OGC WMS GetMap endpoint, as 256 px tiles in EPSG:3857.

[wmts_style()](wmts_style.md#basemapper.wmts_style)  
Style for an OGC WMTS GetTile (KVP) endpoint. A RESTful WMTS


## Style layers and attribution


A style's layers with their features, for the `layers` and `features` filters, and the attribution text it asks maps to show.


[list_layers()](list_layers.md#basemapper.list_layers)  
List a style's layers, or preview a filter.

[get_attribution()](get_attribution.md#basemapper.get_attribution)  
The attribution text a style asks maps to show.


## Tile cache


[tile_cache()](tile_cache.md#basemapper.tile_cache)  
The disk cache for map tiles.


## Exceptions


[BasemapError](BasemapError.md#basemapper.BasemapError)  
Base exception for all basemapper errors.

[ValidationError](ValidationError.md#basemapper.ValidationError)  
Raised for invalid bbox, dimensions, zoom, or layer filter.

[StyleError](StyleError.md#basemapper.StyleError)  
Raised when a style cannot be fetched or parsed.

[NetworkError](NetworkError.md#basemapper.NetworkError)  
Raised when tile or style network requests fail.
