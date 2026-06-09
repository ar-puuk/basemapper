## EsriVectorProvider


Return the root.json style endpoint URL for an ArcGIS VectorTileServer.


Usage


``` python
EsriVectorProvider()
```


The Rust core fetches this URL to obtain the full, ESRI-published MapLibre GL style JSON.


## Parameters


`base_url: str`  
Base ArcGIS VectorTileServer URL (trailing slashes stripped).


## Returns


A URL string (not inline JSON) when cast to `str`.
