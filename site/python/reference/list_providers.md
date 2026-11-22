## list_providers()


The basemaps [basemap_provider()](basemap_provider.md#basemapper.basemap_provider) knows.


Usage


``` python
list_providers(type=None)
```


## Parameters


`type: Optional[str] = None`  
`"vector"` or `"raster"` to list only basemaps with that form; None lists all.


## Returns


`List[Dict[str, object]]`  
One dict per basemap: `name`, `vector` and `raster` (the forms

it has), `api_key_env` (the environment variable its key is read

from, or None when it needs none) and `attribution`.
