## render_basemap_raw()


Render a basemap for bounds in one call.


Usage


``` python
render_basemap_raw(
    bounds,
    width,
    height,
    style,
    crs=None,
    pixel_ratio=1.0,
    tile_zoom=None,
    layers=None,
    labels=True,
    features=None,
    auth_token=None,
    tile_timeout_ms=10000,
    max_tiles=256,
    tile_concurrency=16,
    fail_on_tile_error=False
)
```


A one-shot render: fit `bounds` into a `width` x `height` CSS-pixel view and return the pixels. [basemap()](basemap.md#basemapper.basemap) with [Basemap.render_array()](Basemap.render_array.md#basemapper.Basemap.render_array) gives the same result with a reusable view. The same function exists in R ([render_basemap_raw()](render_basemap_raw.md#basemapper.render_basemap_raw)).


## Parameters


`bounds: Any`  
Bounding box: a `[xmin, ymin, xmax, ymax]` sequence in `crs` (default EPSG:4326, with a warning when the values look projected), a dict with `xmin`/`ymin`/`xmax`/`ymax` (and optionally `crs`) keys, or a GeoDataFrame / GeoSeries (CRS read from it).

`width: int`  
View width in CSS pixels.

`height: int`  
View height in CSS pixels.

`style: Any`  
A MapLibre GL style URL or JSON string (e.g. from [basemap_provider()](basemap_provider.md#basemapper.basemap_provider)), or a list of those stacked bottom to top.

`crs: Optional[Any] = None`  
CRS of `bounds` (anything `pyproj.CRS` accepts).

`pixel_ratio: float = ``1.0`  
Device pixels per CSS pixel; the image is `width * pixel_ratio` by `height * pixel_ratio`.

`tile_zoom: Optional[int] = None`  
Tile detail override: the tile zoom on the 256 px grid (vector sources use one less, their tiles being 512 px), or `None` to pick tiles as MapLibre does for the view.

`layers: Optional[List[str]] = None`  
Optional layer filter: plain IDs keep only those layers, `-` prefixed IDs drop them; `*` and `?` are wildcards (`"-label_*"`).

`labels: Union[bool, str] = ``True`  
`True` keeps every layer, `False` drops the labels (text and icons), `"only"` keeps only the labels on a transparent background, to stack over another map. Catalogue raster basemaps switch to their label-free or label-only tiles.

`features: Optional[List[str]] = None`  
Optional feature filter applied to every style: plain names keep those features, `-` prefixed names drop them, e.g. `["water", "roads"]` or `["-buildings"]`. Features: background, water, landcover, parks, buildings, roads, rail, boundaries, places, poi, terrain, other (see the `feature` of [list_layers()](list_layers.md#basemapper.list_layers)).

`auth_token: Optional[str] = None`  
Optional access token for the tile service.

`tile_timeout_ms: int = ``10000`  
Per-tile HTTP timeout in milliseconds.

`max_tiles: int = ``256`  
Maximum tiles fetched per style.

`tile_concurrency: int = ``16`  
Parallel tile downloads.

`fail_on_tile_error: bool = ``False`  
Abort on a failed tile instead of leaving it empty.


## Returns


`np.ndarray`  
numpy.ndarray: `(height * pixel_ratio, width * pixel_ratio, 4)`

uint8 RGBA pixels.


## Raises


`BasemapError`  
On any rendering or network failure.
