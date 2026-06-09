## render_basemap_raw()


Fetch and composite map tiles into a raw RGBA pixel buffer.


Usage


``` python
render_basemap_raw(
    bbox_3857,
    width,
    height,
    style_input,
    zoom=None,
    tile_timeout_ms=10000,
    max_tiles=256,
    layers=None
)
```


## Parameters


`bbox_3857: List[float]`  
Bounding box as `[xmin, ymin, xmax, ymax]` in EPSG:3857 metres.

`width: int`  
Output width in pixels.

`height: int`  
Output height in pixels.

`style_input: str`  
Tile style -- a URL to a MapLibre GL Style JSON file, an inline JSON string, or a [RasterProvider](RasterProvider.md#basemapper.RasterProvider) / [VectorProvider](VectorProvider.md#basemapper.VectorProvider) / [EsriVectorProvider](EsriVectorProvider.md#basemapper.EsriVectorProvider) / [EsriRasterProvider](EsriRasterProvider.md#basemapper.EsriRasterProvider) object.

`zoom: Optional[int] = None`  
Tile zoom level (0-22). Auto-computed from *bbox_3857* and *width* when `None`.

`tile_timeout_ms: int = ``10000`  
Per-tile HTTP timeout in milliseconds (default 10 000).

`max_tiles: int = ``256`  
Maximum number of tiles fetched per render call (default 256).

`layers: Optional[List[str]] = None`  
Optional layer filter. Plain layer IDs -\> keep only those layers; minus-prefixed IDs (e.g. `["-labels"]`) -\> exclude those layers. Mixing inclusion and exclusion in one list raises [BasemapError](BasemapError.md#basemapper.BasemapError).


## Returns


`bytes: bytes`  
RGBA pixel data of length `width * height * 4`.


## Raises


`BasemapError`  
On any rendering or network failure.
