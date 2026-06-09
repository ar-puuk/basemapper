# Reference


## Core rendering


Low-level tile fetch and composite.


[render_basemap_raw()](render_basemap_raw.md#basemapper.render_basemap_raw)  
Fetch and composite map tiles into a raw RGBA pixel buffer.


## Plot integrations


Drop a basemap layer into an existing spatial plot. `geom_basemap` works in both ggplot2 (R) and plotnine (Python).


[add_basemap()](add_basemap.md#basemapper.add_basemap)  
Render a basemap and composite it beneath existing axes content.

[geom_basemap](geom_basemap.md#basemapper.geom_basemap)  
Render a styled basemap beneath plotnine spatial layers.


## Layer utilities


Inspect and filter layers inside a MapLibre GL style.


[list_layers()](list_layers.md#basemapper.list_layers)  
Return all layer IDs from a MapLibre GL style.


## Tile style helpers


Convert bare tile URLs to MapLibre GL Style JSON.


[RasterProvider](RasterProvider.md#basemapper.RasterProvider)  
Generate a MapLibre GL Style JSON string for an XYZ raster tile source.

[VectorProvider](VectorProvider.md#basemapper.VectorProvider)  
Generate a MapLibre GL Style JSON string for an MVT vector tile source.

[EsriVectorProvider](EsriVectorProvider.md#basemapper.EsriVectorProvider)  
Return the root.json style endpoint URL for an ArcGIS VectorTileServer.

[EsriRasterProvider](EsriRasterProvider.md#basemapper.EsriRasterProvider)  
Generate a MapLibre GL Style JSON string for an ArcGIS MapServer raster source.


## Exceptions


[BasemapError](BasemapError.md#basemapper.BasemapError)  
Raised when the Rust basemapper core returns an error.
