## add_basemap()


Render a basemap and composite it beneath existing axes content.


Usage


``` python
add_basemap(
    ax,
    style_url,
    zoom=None,
    tile_timeout_ms=10000,
    max_tiles=256,
    alpha=1.0,
    layers=None
)
```


Extracts the geographic bounding box and CRS directly from *ax*, reprojects to EPSG:3857, calls the Rust core, and injects the result via imshow.


## Parameters


`ax: 'matplotlib.axes.Axes'`  
An existing matplotlib Axes with a recognized geographic projection.

`style_url: str`  
MapLibre GL style URL or inline JSON string.

`zoom: Optional[int] = None`  
Tile zoom level override (0-22). Auto-computed when None.

`tile_timeout_ms: int = ``10000`  
Per-tile HTTP timeout in milliseconds.

`max_tiles: int = ``256`  
Maximum tile downloads per render.

`alpha: float = ``1.0`  
Opacity of the basemap layer (0.0-1.0).

`layers: Optional[List[str]] = None`  
Optional layer filter. Plain IDs keep only those layers; minus-prefixed IDs exclude those layers.


## Returns


`np.ndarray`  
numpy.ndarray of shape (height, width, 4) with spatial bounds in .attrs.


## Raises


`BasemapError`  
If CRS detection fails, the render fails, or the network is unreachable.
