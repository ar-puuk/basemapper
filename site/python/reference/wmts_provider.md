## wmts_provider()


Build a MapLibre GL Style JSON string for a WMTS GetTile KVP endpoint.


Usage


``` python
wmts_provider(
    url,
    layer,
    tilematrixset="GoogleMapsCompatible",
    format="image/png",
    style="default",
    version="1.0.0",
    extra_params=None,
)
```


## Parameters


`url: str`  
Base WMTS URL (without query parameters).

`layer: str`  
Value for the `LAYER` parameter.

`tilematrixset: str = "GoogleMapsCompatible"`  
Tile matrix set identifier.

`format: str = "image/png"`  
Image MIME type.

`style: str = "default"`  
Style identifier.

`version: str = "1.0.0"`  
WMTS version string.

`extra_params: Optional[str] = None`  
Additional query parameters appended verbatim.


## Returns


`str`  
MapLibre GL Style JSON string ready to pass to
[render_basemap_raw()](render_basemap_raw.md#basemapper.render_basemap_raw).


## Examples


``` python
from basemapper import wmts_provider, render_basemap_raw

style = wmts_provider(
    url="https://tiles.example.com/wmts",
    layer="BasemapAT_grau",
)
pixels = render_basemap_raw(
    bbox=[14.0, 48.0, 17.0, 50.0],
    width=400, height=300,
    style_input=style,
)
```
