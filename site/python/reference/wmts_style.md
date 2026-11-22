## wmts_style()


Style for an OGC WMTS GetTile (KVP) endpoint. A RESTful WMTS


Usage


``` python
wmts_style(
    url,
    layer,
    tilematrixset="GoogleMapsCompatible",
    format="image/png",
    style="default",
    version="1.0.0",
    extra_params=None
)
```


template (`{TileMatrix}/{TileRow}/{TileCol}`) needs no builder: pass it as `style`.


## Parameters


`url: str`  
Base service URL without query parameters.

`layer: str`  
`LAYER` parameter.

`tilematrixset: str = ``"GoogleMapsCompatible"`  
Tile matrix set (a Web Mercator one).

`format: str = ``"image/png"`  
Image MIME type.

`style: str = ``"default"`  
`STYLE` parameter.

`version: str = ``"1.0.0"`  
Service version.

`extra_params: Optional[str] = None`  
Additional query parameters appended verbatim.
