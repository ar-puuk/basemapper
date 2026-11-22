## wms_style()


Style for an OGC WMS GetMap endpoint, as 256 px tiles in EPSG:3857.


Usage


``` python
wms_style(
    url,
    layers,
    styles="",
    version="1.3.0",
    format="image/png",
    transparent=False,
    extra_params=None
)
```


## Parameters


`url: str`  
Base service URL without query parameters.

`layers: str`  
`LAYERS` parameter.

`styles: str = ``""`  
`STYLES` parameter.

`version: str = ``"1.3.0"`  
Service version (1.1.x sends `SRS`, 1.3.0 `CRS`).

`format: str = ``"image/png"`  
Image MIME type.

`transparent: bool = ``False`  
Request transparent images.

`extra_params: Optional[str] = None`  
Additional query parameters appended verbatim.
