## add_basemap()


Render a basemap beneath existing axes content.


Usage


``` python
add_basemap(
    ax,
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
    crs=None
)
```


The basemap is an artist that renders when the figure is drawn, at the output resolution: on screen at the canvas dpi, and natively at the requested resolution in `savefig(dpi=...)` or PDF / SVG output. Takes the same arguments as [geom_basemap](geom_basemap.md#basemapper.geom_basemap) (and R's [geom_basemap()](geom_basemap.md#basemapper.geom_basemap)).


## Parameters


`ax: 'matplotlib.axes.Axes'`  
An existing matplotlib Axes.

`style: Union[str, Any, Sequence[Any]]`  
A MapLibre GL style URL or JSON string (e.g. from [basemap_provider()](basemap_provider.md#basemapper.basemap_provider)), or a list of those stacked bottom to top.

`tile_zoom: Optional[int] = None`  
Tile detail override: the tile zoom on the 256 px grid (vector sources use one less), or None to pick tiles as MapLibre does for the view.

`alpha: float = ``1.0`  
Opacity of the basemap layer (0.0-1.0).

`layers: Optional[List[str]] = None`  
Optional layer filter. Plain IDs keep only those layers; minus-prefixed IDs drop those layers; `*` and `?` are wildcards.

`labels: Union[bool, str] = ``True`  
`True` keeps every layer, `False` drops the labels (text and icons), `"only"` keeps only the labels on a transparent background, to stack over another map. Catalogue raster basemaps switch to their label-free or label-only tiles.

`features: Optional[List[str]] = None`  
Optional feature filter applied to every style: plain names keep those features, `-` prefixed names drop them, e.g. `["water", "roads"]` or `["-buildings"]`. Features: background, water, landcover, parks, buildings, roads, rail, boundaries, places, poi, terrain, other (see the `feature` of [list_layers()](list_layers.md#basemapper.list_layers)).

`pixel_ratio: Optional[float] = None`  
Device pixels per CSS pixel (MapLibre's devicePixelRatio). Default: output dpi / 96, so lines and labels keep their physical size at any dpi. Larger values draw map symbols smaller.

`auth_token: Optional[str] = None`  
Optional access token for the tile service.

`tile_timeout_ms: int = ``10000`  
Per-tile HTTP timeout in milliseconds.

`max_tiles: int = ``256`  
Maximum tile downloads per style.

`attribution: Union[bool, str] = ``True`  
The credit line the basemap's providers ask for, drawn small in the bottom-right corner over the plot: `True` (the default) for the styles' own (see [get_attribution()](get_attribution.md#basemapper.get_attribution)), `False` for none, or your own text.

`crs: Optional[object] = None`  
CRS of the axes data coordinates (anything `pyproj.CRS` accepts, e.g. `"EPSG:4326"`, `gdf.crs`). Required for plain matplotlib axes and GeoDataFrame plots, whose axes carry no CRS; detected automatically for cartopy axes when omitted.


## Returns


`'xr.DataArray'`  
xarray.DataArray of shape (height, width, 4) holding the first render

(at the figure's current dpi), with spatial bounds and CRS in

`.attrs`. The live artist is `ax.images[-1]`.


## Raises


`BasemapError`  
If CRS detection fails, the render fails, or the network is unreachable.
