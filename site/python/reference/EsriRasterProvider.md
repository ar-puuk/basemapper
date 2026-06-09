## EsriRasterProvider


Generate a MapLibre GL Style JSON string for an ArcGIS MapServer raster source.


Usage


``` python
EsriRasterProvider()
```


## Parameters


`url_template: str`  
ArcGIS MapServer tile URL (e.g., `.../MapServer/tile/{z}/{y}/{x}`).

`tile_size: int = ``256`  
Tile size in pixels (default 256).
