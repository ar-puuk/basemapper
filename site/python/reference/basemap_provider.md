## basemap_provider()


A named basemap's style.


Usage


``` python
basemap_provider(
    name, type=None, api_key=None, tiles=None, labels=True, features=None
)
```


Names follow leaflet-providers (`"CartoDB.Positron"`, `"Esri.WorldImagery"`, `"OpenStreetMap.HOT"`); see [list_providers()](list_providers.md#basemapper.list_providers) or the [providers](providers.md#basemapper.providers) object. A name also works as `style` directly; this function picks the form, passes a key, or swaps in your own tiles.


## Parameters


`name: str`  
Basemap name (case-insensitive).

`type: Optional[str] = None`  
`"vector"` (a MapLibre style) or `"raster"` (tiles). By default the vector form when there is one, else raster. Asking for a form a basemap lacks is an error, never a silent fallback.

`api_key: Optional[str] = None`  
API key for CARTO, MapTiler and Stadia; by default read from `CARTO_API_KEY`, `MAPTILER_API_KEY` or `STADIA_API_KEY`. The keys are free for moderate use; [list_providers()](list_providers.md#basemapper.list_providers) shows which basemaps need one.

`tiles: Optional[str] = None`  
Your own tile data to draw with this basemap's style: a `.pmtiles` archive (URL or path), tile URL or TileJSON. It must use the same schema as the provider's tiles (OpenMapTiles for OpenFreeMap and Stadia).

`labels: Union[bool, str] = ``True`  
`True` keeps every layer, `False` drops the labels, `"only"` keeps only the labels, to stack this basemap's labels over another map. Raster tiles switch to the provider's label-free or label-only tiles (an error when it has none).

`features: Optional[List[str]] = None`  
Vector form: features to keep (`["water", "roads"]`) or drop (`["-buildings"]`); see [list_layers()](list_layers.md#basemapper.list_layers).


## Returns


`str`  
A style URL, or inline style JSON (the raster form, or with

`tiles`, `labels` or `features`).


## Raises


`ValidationError`  
For an unknown name (with suggestions), a form the basemap lacks, or a missing API key.
