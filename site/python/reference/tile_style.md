## tile_style()


A style for tile data, with overrides.


Usage


``` python
tile_style(
    source,
    type=None,
    tile_size=None,
    scheme=None,
    minzoom=None,
    maxzoom=None,
    attribution=None,
    source_layers=None,
    paint=None,
    auth_token=None
)
```


`source` is what you could pass as `style` directly; use this function to override what is detected, or to get the style JSON to edit:

- a tile URL template (`{z}/{x}/{y}`, `{quadkey}`, `{bbox-epsg-3857}`, WMTS `{TileMatrix}/{TileRow}/{TileCol}`);
- a TileJSON URL or file;
- a PMTiles archive: URL, local path or `pmtiles://` URL;
- an ArcGIS `.../MapServer` (raster tiles) or `.../VectorTileServer` (its published style);
- a style URL or file (returned with relative URLs made absolute).

Raster tiles get one raster layer. Vector tiles get a default style drawing every source layer (polygons as translucent fills, lines and outlines, points as circles, a colour per source layer) over a transparent background.


## Parameters


`source: str`  
The tile data, as above.

`type: Optional[str] = None`  
`"raster"` or `"vector"`; detected when None (from the archive header, the TileJSON, the URL's extension, or one fetched tile).

`tile_size: Optional[int] = None`  
Raster tile size in px (default 256 for tile URLs and archives).

`scheme: Optional[str] = None`  
`"xyz"` or `"tms"` (rows counted from the south).

`minzoom: Optional[int] = None`  
Minimum zoom of the tiles.

`maxzoom: Optional[int] = None`  
Maximum zoom of the tiles (overzoomed beyond).

`attribution: Optional[str] = None`  
Attribution text for the source.

`source_layers: Optional[List[str]] = None`  
Vector tiles: the source layers to draw (default all).

`paint: Optional[Dict[str, object]] = None`  
Vector tiles: paint properties for the generated `fill-*`, `line-*` and `circle-*` layers, e.g. `{"fill-color": "#ccc"}`. Others are dropped with a warning.

`auth_token: Optional[str] = None`  
Optional access token for the tile service.


## Returns


`str`  
The style JSON.


## Raises


`ValidationError`  
If the type cannot be detected or does not match the data, or `source` is not found.
