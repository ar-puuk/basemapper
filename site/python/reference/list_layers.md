## list_layers()


List a style's layers, or preview a filter.


Usage


``` python
list_layers(
    style,
    auth_token=None,
    layers=None,
    labels=True,
    features=None,
)
```


The layers of a style in drawing order: their `id` (what `layers` matches), `type` (`symbol` layers are the labels `labels` keeps or drops), `source_layer` (the vector tile layer drawn, None for background and raster layers) and `feature` (what `features` matches). With `layers`, `labels` or `features`, the layers a render with those filters draws; an empty list when they leave nothing. The same function exists in R ([list_layers()](list_layers.md#basemapper.list_layers), returning a data frame).

A layer's feature comes from its source layer, with tables for the OpenMapTiles (OpenFreeMap, Stadia, MapTiler), Protomaps (CARTO) and Shortbread (VersaTiles) schemas, and keywords for others (Esri's descriptive names, layer ids). It is a convenience: check it here, and use `layers` for exact control.


## Parameters


`style: Any`  
A style (anything [basemap()](basemap.md#basemapper.basemap) takes).

`auth_token: Optional[str] = None`  
Optional access token for the style URL.

`layers: Optional[List[str]] = None`  
Layer filter to preview: plain IDs keep, `-` prefixed IDs drop; `*` and `?` are wildcards.

`labels: Union[bool, str] = ``True`  
`True`, `False` or `"only"`, as the renderers take it.

`features: Optional[List[str]] = None`  
Feature filter to preview, as the renderers take it.


## Returns


`List[Dict[str, Optional[str]]]`  
A list of `{"id", "type", "source_layer", "feature"}` dicts

(`pandas.DataFrame(...)` turns it into a table).


## Raises


`BasemapError`  
If the style cannot be fetched or parsed, or a filter is invalid.
