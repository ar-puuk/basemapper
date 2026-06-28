## wms_provider()


Build a MapLibre GL Style JSON string for a WMS GetMap endpoint.

The Rust tile engine substitutes `{xmin}`, `{ymin}`, `{xmax}`, `{ymax}`
with EPSG:3857 coordinates for each 256×256 tile.


Usage


``` python
wms_provider(
    url,
    layers,
    styles="",
    version="1.3.0",
    format="image/png",
    transparent=False,
    extra_params=None,
)
```


## Parameters


`url: str`  
Base WMS URL (without query parameters).

`layers: str`  
Value for the `LAYERS` parameter.

`styles: str = ""`  
Value for the `STYLES` parameter.

`version: str = "1.3.0"`  
WMS version string.

`format: str = "image/png"`  
Image MIME type.

`transparent: bool = False`  
Request transparent PNG tiles.

`extra_params: Optional[str] = None`  
Additional query parameters appended verbatim.


## Returns


`str`  
MapLibre GL Style JSON string ready to pass to
[render_basemap_raw()](render_basemap_raw.md#basemapper.render_basemap_raw).


## Examples


``` python
from basemapper import wms_provider, render_basemap_raw

style = wms_provider(
    url="https://gis.example.com/wms",
    layers="imagery",
)
pixels = render_basemap_raw(
    bbox=[-122.5, 37.7, -122.4, 37.8],
    width=400, height=300,
    style_input=style,
)
```
