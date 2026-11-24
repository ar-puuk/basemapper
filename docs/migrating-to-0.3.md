---
title: "Migrating to basemapper 0.3"
---

basemapper 0.3 replaces the 0.2 API with one model shared by R, Python and
Rust: a **basemap** of **styles**, whose **view** is any three of extent,
zoom, image size and dpi, rendered to a **rendering** or placed in a plot.
Nothing of 0.2 is kept as a shim: every removed name and argument is an
error that names its replacement and links here.

This page lists every change. The [API reference](https://github.com/ar-puuk/basemapper/blob/main/docs/api.md)
describes 0.3 in full.

## The short version

```r
# 0.2
m <- basemap("OpenFreeMap.Liberty", 800, 600, center = c(-111.89, 40.76), zoom = 12, pixel_ratio = 2,
             labels = FALSE)
render_png(m, "map.png")

# 0.3
m <- basemap(style("OpenFreeMap.Liberty", features = "-labels"),
             center = c(-111.89, 40.76), zoom = 12, width = 1600, height = 1200, dpi = 192)
write_png(render(m), "map.png")
```

```python
# 0.2
m = basemapper.basemap("OpenFreeMap.Liberty", 800, 600, center=(-111.89, 40.76), zoom=12, pixel_ratio=2,
                       labels=False)
m.render_png("map.png")

# 0.3
m = basemapper.basemap(basemapper.style("OpenFreeMap.Liberty", features="-labels"),
                       center=(-111.89, 40.76), zoom=12, width=1600, height=1200, dpi=192)
m.render().save("map.png")
```

```r
# 0.2
ggplot(nc) + geom_basemap("OpenFreeMap.Positron", tile_zoom = 9) + geom_sf()
# 0.3
ggplot(nc) + basemap("OpenFreeMap.Positron", zoom = 8) + geom_sf()   # or geom_basemap(..., zoom = 8)
```

## Removed functions

| 0.2 | 0.3 |
|---|---|
| `render_png(m, path)` / `m.render_png(path)` | R `write_png(render(m), path)`; Python `m.render().save(path)` |
| `render_array(m)` / `m.render_array()` | R `as_array(render(m))`; Python `m.render().to_numpy()` |
| `render_raster(m)` | `as_raster(render(m))` |
| `render_basemap_raw(bounds, width, height, style, ...)` | `render(basemap(style, bounds = ..., width = ..., height = ...))`, then a conversion |
| `basemap_provider(name, type =, api_key =, tiles =, labels =, features =)` | `style(provider(name, form =, tiles =), features =, key =)`; a name on its own still works as a style |
| `tile_style(source, ...)` | `style(tiles(source, ...))`; tile data on its own still works as a style |
| `wms_style(url, layers, ...)`, `wmts_style(url, layer, ...)` | `style(wms(url, layers, ...))`, `style(wmts(url, layer, ...))` |
| Python `m.get_bounds()`, `get_center()`, `get_zoom()`, `get_size()`, `get_attribution()` | the properties `m.bounds`, `m.center`, `m.zoom`, `m.size`; `basemapper.get_attribution(m)` |
| R `get_bounds(m, lonlat = TRUE)` | `get_bounds(m)` is in the map's CRS; `get_center(m)` is longitude / latitude |
| Rust `Basemap::render_png`, `render_array`, `set_view`, `set_bounds`, `pixel_ratio`, `get_*`, `labels`, `features`, `layers`, `auth_token`; the free `render*` functions | `Basemap::render(&Context::Standalone)` returning a `Rendering`; `.dpi()`; filters on `Style`; the free functions are internal |

## Removed arguments

| 0.2 | 0.3 |
|---|---|
| `pixel_ratio` | `dpi = 96 × pixel_ratio`. Note that `width` / `height` are now the **image's** size: a 0.2 `800 × 600` view at `pixel_ratio = 2` is `width = 1600, height = 1200, dpi = 192` |
| `tile_zoom` (plot layers) | `zoom`, the MapLibre zoom (one more than 0.2's 256 px tile zoom for the same detail); in a plot, the dpi follows |
| `labels = FALSE` / `"none"` / `"only"` | on the style: `features = "-labels"` / `features = "labels"` |
| `layers`, `features` (on basemaps and plot layers) | on the style: `style(s, layers = ..., features = ...)`, so each style of a stack has its own |
| `auth_token`, `api_key` | `style(s, key = ...)`: one credential per style |
| `tile_timeout_ms`, `max_tiles`, `tile_concurrency`, `fail_on_tile_error` | `fetch = fetch_options(...)` / `FetchOptions(...)` (deprecated in 0.2, now removed) |
| positional `width`, `height` in `basemap()` | named: `basemap(style, width = 800, height = 600)` |
| `crs` as the CRS of `bounds` | `crs` is the map's CRS. Numbers in `bounds` are always longitude / latitude; for projected coordinates give a spatial object (an `sf` / `sfc` object, a GeoDataFrame or GeoSeries), which brings its CRS |
| tmap `tm_basemap(width =, height =)` | the panel gives the size |

## Behaviour changes

- **The view is one rule.** Any three of extent, zoom, image size and dpi
  give the fourth; what nothing gives is defaulted (dpi 96, then 800 × 600 at
  that dpi, then the world). New: `basemap(s, bounds = nc, zoom = 9)` gives
  the image size for that area at that detail. Giving all four, or `bounds`
  with `center`, is an error.
- **`dpi` is sharpness.** With the size left to its default, a higher dpi
  draws the same map with more pixels: `render(m, dpi = 300)`. With the size
  fixed in pixels, the pixels stay: an area keeps its extent at a lower zoom
  (the labels larger, as on paper), and a centre with a zoom covers less
  ground. With the size in inches, only the pixel count changes.
- **Maps in any CRS.** A map's CRS is `crs`, else a projected layer's
  (`bounds = nc_utm` draws in UTM), else Web Mercator. It is drawn in Web
  Mercator and warped. A layer in longitude / latitude still gives Web
  Mercator (`crs = 4326` when wanted).
- **Plots are aligned in their CRS.** 0.2 stretched the Web Mercator image
  over the panel, which misplaced it in any other CRS (by tens of pixels on a
  longitude / latitude plot). 0.3 warps it into the plot's CRS in ggplot2,
  tmap, matplotlib and plotnine. tmap no longer forces Web Mercator.
- **A zoom chosen in a plot gives an effective dpi** instead of 0.2's tile
  zoom: the plot fixes the extent and the pixels, so the dpi follows, and the
  map's labels are drawn at that dpi. It is reported once.
- **Plot dpi.** On screen 96 (× the pixel ratio in Python), in raster
  exports the device's dpi, in PDF / SVG 300: the export shows the preview's
  map, sharper.
- **Each style of a stack is filtered on its own**, with its own key. A
  filter that empties one style is an error for that style.
- **Renderings.** `render()` returns the image with its bounds, CRS, zoom,
  dpi, scale and notes; one render converts to PNG, arrays and rasters.
  Python's `to_numpy()` is a read-only view (no copy); `.copy()` it to edit.
- **Python basemaps are immutable.** `m.set_view()` and `m.set_bounds()`
  return a new basemap; `m.add_to(ax)` returns the artist (0.2's
  `add_basemap()` returned an xarray of the first render; use
  `artist.rendering.to_xarray()`).
- **Notes replace silent drops.** Failed tiles, missing glyphs, unsupported
  style features and filter names matching nothing were logged or dropped in
  0.2; they are now warnings (`basemapper_problem` / `BasemapProblem`), and
  errors with `on_error = "fail"`. Changes to what you asked are messages
  (`basemapper_adjustment` / `BasemapAdjustment`); defaults are shown on
  request.
- **Clipping is new:** `clip = layer` and `clip = outside(layer)`.
- **`around(center, span, units)` is new:** an area of a ground distance.
- **Errors earlier.** A style is checked when it is made (an unknown
  catalogue name, a missing key, a bad filter); a basemap that cannot go in a
  plot is an error when it is added.
