## geom_basemap


Render a styled basemap beneath plotnine spatial layers.


Usage


``` python
geom_basemap()
```


Defers all tile fetching to draw time so that the plot's coordinate system has been established. Uses the same injection logic as [add_basemap()](add_basemap.md#basemapper.add_basemap).


## Parameters


`style_url: str`  
MapLibre GL style URL or inline JSON string.

`zoom: Optional[int] = None`  
Tile zoom level override (0-22). Auto-computed when None.

`alpha: float = ``1.0`  
Opacity of the basemap layer (0.0-1.0).

`layers: Optional[List[str]] = None`  
Optional layer filter list -- plain IDs keep only those layers; minus-prefixed IDs exclude those layers.


## Example

> > > from plotnine import ggplot, geom_sf from basemapper import geom_basemap (ggplot(gdf) + geom_basemap(style_url="https://…") + geom_sf())


## Methods

| Name | Description |
|----|----|
| [draw_panel()](#draw_panel) | Render basemap at draw time and inject via imshow. |

------------------------------------------------------------------------


#### draw_panel()


Render basemap at draw time and inject via imshow.


Usage


``` python
draw_panel(data, panel_params, coord, ax, **kwargs)
```


H4: zorder=0 may hide behind ax.patch (zorder=0). If basemap is invisible, set ax.set_facecolor('none') and increase zorder to 0.5.
