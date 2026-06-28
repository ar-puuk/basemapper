## render_basemap_raw()


Fetch and composite map tiles into a raw RGBA pixel buffer.

Supports anti-meridian crossing bboxes (`xmin > xmax` in EPSG:3857): the
render is split internally into west and east halves at proportional pixel
widths, then stitched horizontally.

Tile fetching uses an in-process LRU cache (512 tiles) shared across all
render calls in a session.


Usage


``` python
render_basemap_raw(
    bbox,
    width,
    height,
    style_input,
    crs=None,
    zoom=None,
    tile_timeout_ms=10000,
    max_tiles=256,
    layers=None,
    auth_token=None,
    fail_on_tile_error=True,
    tile_concurrency=16,
)
```


## Parameters


`bbox`  
Bounding box — one of:

- A `[xmin, ymin, xmax, ymax]` list or tuple in the CRS given by *crs*
  (defaults to EPSG:4326 / WGS-84 when *crs* is omitted).
- A `dict` with keys `"xmin"`, `"ymin"`, `"xmax"`, `"ymax"`, and an
  optional `"crs"` key.
- A `geopandas.GeoDataFrame` or `GeoSeries`; CRS is read from `.crs`
  automatically.

Anti-meridian crossing bboxes (`xmin > xmax` in EPSG:3857) are supported.

`width: int`  
Output width in pixels.

`height: int`  
Output height in pixels.

`style_input: str`  
Tile style — a URL to a MapLibre GL Style JSON file, an inline JSON
string, or a [basemap_provider()](basemap_provider.md#basemapper.basemap_provider) /
[RasterProvider](RasterProvider.md#basemapper.RasterProvider) /
[VectorProvider](VectorProvider.md#basemapper.VectorProvider) /
[EsriVectorProvider](EsriVectorProvider.md#basemapper.EsriVectorProvider) /
[EsriRasterProvider](EsriRasterProvider.md#basemapper.EsriRasterProvider) /
[wms_provider()](wms_provider.md#basemapper.wms_provider) /
[wmts_provider()](wmts_provider.md#basemapper.wmts_provider) /
[pmtiles_provider()](pmtiles_provider.md#basemapper.pmtiles_provider) value.

`crs: Optional = None`  
CRS of the input *bbox* — an EPSG integer, WKT/PROJ string, or
`pyproj.CRS` object. Ignored when *bbox* carries its own CRS. Defaults to
EPSG:4326 (WGS-84) with a warning if coordinates appear projected.

`zoom: Optional[int] = None`  
Tile zoom level (0–22). Auto-computed from *bbox* and *width* when `None`,
then clamped to the tile source's advertised `maxzoom`.

`tile_timeout_ms: int = 10000`  
Per-tile HTTP timeout in milliseconds.

`max_tiles: int = 256`  
Maximum number of tiles fetched per render call.

`layers: Optional[List[str]] = None`  
Optional layer filter. Plain layer IDs → keep only those layers;
minus-prefixed IDs (e.g. `["-labels"]`) → exclude those layers. Mixing
inclusion and exclusion raises [BasemapError](BasemapError.md#basemapper.BasemapError).

`auth_token: Optional[str] = None`  
Optional authentication token. Sent as `Authorization: Bearer <token>`
for raster sources; appended as `?access_token=<token>` for Mapbox vector
sources.

`fail_on_tile_error: bool = True`  
When `False`, individual tile fetch failures are skipped (with a warning)
rather than aborting the render.

`tile_concurrency: int = 16`  
Maximum number of tiles fetched in parallel.


## Returns


`bytes`  
RGBA pixel data of length `width * height * 4`.


## Raises


`BasemapError`  
On any rendering or network failure.


## Examples


``` python
from basemapper import render_basemap_raw, basemap_provider, pmtiles_provider

# Default WGS-84 bbox
pixels = render_basemap_raw(
    bbox=[-122.5, 37.7, -122.4, 37.8],
    width=400, height=300,
    style_input=basemap_provider("OpenFreeMap.liberty"),
)

# Anti-meridian bbox (Pacific-centred view)
pixels = render_basemap_raw(
    bbox=[15_000_000, -5_000_000, -15_000_000, 5_000_000],
    width=800, height=400,
    style_input=basemap_provider("OSM.Standard"),
    crs=3857,
)

# PMTiles archive
pixels = render_basemap_raw(
    bbox=[-122.5, 37.7, -122.4, 37.8],
    width=400, height=300,
    style_input=pmtiles_provider("https://example.com/tiles/world.pmtiles"),
)
```
