## get_attribution()


The attribution text a style asks maps to show.


Usage


``` python
get_attribution(
    style,
    auth_token=None,
)
```


Every source the style's layers draw from contributes its attribution (from the style, the source's TileJSON or PMTiles metadata), as MapLibre's attribution control collects it, as plain text without duplicates, joined with `" | "`. Use it for a caption or a credit line, e.g. `ax.annotate(get_attribution(style), ...)`. [Basemap.get_attribution()](Basemap.get_attribution.md#basemapper.Basemap.get_attribution) gives a view's. The same function exists in R ([get_attribution()](get_attribution.md#basemapper.get_attribution)).


## Parameters


`style: Any`  
A style (anything [basemap()](basemap.md#basemapper.basemap) takes), or a list of styles stacked bottom to top.

`auth_token: Optional[str] = None`  
Optional access token for the tile service.


## Returns


`str`  
The attribution; an empty string when the styles give none.
