## pmtiles_provider()


Build a MapLibre GL Style JSON string for a PMTiles raster archive.

The Rust core detects the `pmtiles://` scheme in the `tiles` array and
routes tile fetching through the PMTiles HTTP range-request pipeline.


Usage


``` python
pmtiles_provider(url)
```


## Parameters


`url: str`  
HTTPS URL of the `.pmtiles` archive file.


## Returns


`str`  
MapLibre GL Style JSON string ready to pass to
[render_basemap_raw()](render_basemap_raw.md#basemapper.render_basemap_raw).


## Examples


``` python
from basemapper import pmtiles_provider, render_basemap_raw

style = pmtiles_provider("https://example.com/tiles/world.pmtiles")
pixels = render_basemap_raw(
    bbox=[-122.5, 37.7, -122.4, 37.8],
    width=400, height=300,
    style_input=style,
)
```
