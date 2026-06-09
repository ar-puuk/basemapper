## RasterProvider


Generate a MapLibre GL Style JSON string for an XYZ raster tile source.


Usage


``` python
RasterProvider()
```


Cast to `str` to obtain the inline MapLibre GL Style JSON.


## Parameters


`url_template: str`  
Slippy-map tile URL with `{z}`, `{x}`, `{y}` placeholders.

`tile_size: int = ``256`  
Tile size in pixels (default 256).


## Methods

| Name | Description |
|----|----|
| [to_style_json()](#to_style_json) | Return a MapLibre GL Style JSON string. |

------------------------------------------------------------------------


#### to_style_json()


Return a MapLibre GL Style JSON string.


Usage


``` python
to_style_json()
```
