## Basemap


A map view rendered headlessly: one style, or several stacked.


Usage


``` python
Basemap(
    style,
    width=800,
    height=600,
    *,
    center=None,
    zoom=None,
    bounds=None,
    crs=None,
    pixel_ratio=1.0,
    layers=None,
    labels=True,
    features=None,
    auth_token=None,
    tile_timeout_ms=10000,
    max_tiles=256,
    tile_concurrency=16,
    fail_on_tile_error=False
)
```


Create one with [basemap()](basemap.md#basemapper.basemap); see there for the arguments. Change the view with [set_view()](Basemap.set_view.md#basemapper.Basemap.set_view) / [set_bounds()](Basemap.set_bounds.md#basemapper.Basemap.set_bounds) (which return the view, so calls chain), read it with the `get_*` methods, and render it with [render_png()](Basemap.render_png.md#basemapper.Basemap.render_png), [render_svg()](Basemap.render_svg.md#basemapper.Basemap.render_svg) or [render_array()](Basemap.render_array.md#basemapper.Basemap.render_array).
