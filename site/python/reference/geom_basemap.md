## geom_basemap


Render a styled basemap beneath plotnine spatial layers.


Usage


``` python
geom_basemap(
    style,
    tile_zoom=None,
    alpha=1.0,
    layers=None,
    labels=True,
    features=None,
    pixel_ratio=None,
    auth_token=None,
    tile_timeout_ms=10000,
    max_tiles=256,
    attribution=True,
    crs="EPSG:4326",
    **kwargs
)
```


Defers all tile fetching to draw time so that the plot's coordinate system has been established. Uses the same injection logic as [add_basemap()](add_basemap.md#basemapper.add_basemap).

The basemap renders when the plot is drawn, at the output resolution (natively at `save(dpi=...)`), like [add_basemap()](add_basemap.md#basemapper.add_basemap). The same layer exists in R (ggplot2 [geom_basemap()](geom_basemap.md#basemapper.geom_basemap)), with the same arguments.


## Parameters


`style: Any`  
A MapLibre GL style URL or JSON string (e.g. from [basemap_provider()](basemap_provider.md#basemapper.basemap_provider)), or a list of those stacked bottom to top.

`tile_zoom: Optional[int] = None`  
Tile detail override: the tile zoom on the 256 px grid (vector sources use one less), or None to pick tiles as MapLibre does for the view.

`alpha: float = ``1.0`  
Opacity of the basemap layer (0.0-1.0).

`layers: Optional[List[str]] = None`  
Optional layer filter list -- plain IDs keep only those layers; minus-prefixed IDs drop those layers; `*` and `?` are wildcards.

`labels: Union[bool, str] = ``True`  
`True` keeps every layer, `False` drops the labels (text and icons), `"only"` keeps only the labels on a transparent background, to stack over another map. Catalogue raster basemaps switch to their label-free or label-only tiles.

`features: Optional[List[str]] = None`  
Optional feature filter applied to every style: plain names keep those features, `-` prefixed names drop them, e.g. `["water", "roads"]` or `["-buildings"]`. Features: background, water, landcover, parks, buildings, roads, rail, boundaries, places, poi, terrain, other (see the `feature` of [list_layers()](list_layers.md#basemapper.list_layers)).

`pixel_ratio: Optional[float] = None`  
Device pixels per CSS pixel; default output dpi / 96.

`auth_token: Optional[str] = None`  
Optional access token for the tile service.

`tile_timeout_ms: int = ``10000`  
Per-tile HTTP timeout in milliseconds.

`max_tiles: int = ``256`  
Maximum tile downloads per style.

`attribution: Union[bool, str] = ``True`  
The credit line the basemap's providers ask for, drawn small in the bottom-right corner over the plot: `True` (the default) for the styles' own (see [get_attribution()](get_attribution.md#basemapper.get_attribution)), `False` for none, or your own text.

`crs: object = ``"EPSG:4326"`  
CRS of the plot's x/y data (anything `pyproj.CRS` accepts). plotnine has no spatial coordinate system to read it from, so it defaults to `"EPSG:4326"` (longitude/latitude). Pass e.g. `crs=gdf.crs` or `crs="EPSG:3857"` for projected data.


## Example

> > > import pandas as pd from plotnine import ggplot, aes, geom_point from basemapper import basemap_provider, geom_basemap df = pd.DataFrame({"lon": \[-111.95, -111.80\], "lat": \[40.68, 40.80\]}) (ggplot(df, aes("lon", "lat")) … + geom_basemap("OpenFreeMap.Positron") … + geom_point())


## Methods

| Name | Description |
|----|----|
| [draw_panel()](#draw_panel) | Add a basemap artist that renders at draw time. |

------------------------------------------------------------------------


#### draw_panel()


Add a basemap artist that renders at draw time.


Usage


``` python
draw_panel(data, panel_params, coord, ax, **kwargs)
```


The panel background is made transparent so the basemap shows through. Axis limits are kept at the panel ranges.
