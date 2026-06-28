## basemap_provider()


Return a style URL or inline MapLibre GL JSON for a named basemap provider.


Usage


``` python
basemap_provider(name, api_key=None)
```


## Parameters


`name: str`  
Provider name, e.g. `"OpenFreeMap.liberty"` or `"Carto.DarkMatter"`. See
the catalog below for all available names.

`api_key: Optional[str] = None`  
API key required by some providers (e.g. MapTiler). Ignored for providers
that do not use a key. Stadia providers work without a key at low volume;
supply one to lift rate limits.


## Returns


`str`  
A MapLibre GL style URL or inline JSON string, ready to pass to
[render_basemap_raw()](render_basemap_raw.md#basemapper.render_basemap_raw).


## Raises


`ValueError`  
For unknown provider names or when a required `api_key` is missing.


## Provider catalog

**Vector GL styles** (no key required): `"OpenFreeMap.liberty"`,
`"OpenFreeMap.bright"`, `"OpenFreeMap.positron"`, `"Carto.Positron"`,
`"Carto.DarkMatter"`, `"Carto.Voyager"`, `"ESRI.WorldBasemap"`

**MapTiler GL styles** (require `api_key`): `"MapTiler.Streets"`,
`"MapTiler.Satellite"`, `"MapTiler.Topo"`, `"MapTiler.Outdoor"`

**ESRI raster** (no key required): `"ESRI.WorldImagery"`,
`"ESRI.WorldStreetMap"`, `"ESRI.WorldTopoMap"`, `"ESRI.WorldShadedRelief"`,
`"ESRI.WorldGrayCanvas"`, `"ESRI.NatGeoWorldMap"`

**OSM raster** (no key required): `"OSM.Standard"`, `"OSM.CyclOSM"`

**Stadia/Stamen** (key optional): `"Stadia.AlidadeSmooth"`,
`"Stadia.AlidadeSmoothDark"`, `"Stadia.OSMBright"`,
`"Stadia.StamenToner"`, `"Stadia.StamenTerrain"`,
`"Stadia.StamenWatercolor"`


## Examples


``` python
from basemapper import basemap_provider, render_basemap_raw

style = basemap_provider("OpenFreeMap.liberty")
style = basemap_provider("OSM.Standard")
style = basemap_provider("MapTiler.Streets", api_key="YOUR_KEY")

pixels = render_basemap_raw(
    bbox=[-122.5, 37.7, -122.4, 37.8],
    width=400, height=300,
    style_input=basemap_provider("Carto.Positron"),
)
```
