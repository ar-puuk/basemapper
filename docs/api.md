# The basemapper API: R, Python and Rust

One model, three languages. The names, arguments, defaults and rules are the
same everywhere; only the call style differs (R `verb(m, ...)`, Python
`m.verb(...)` and properties, Rust `m.verb(...)`). Every rule lives in the
Rust core, so the same call gives the same view, pixels and notes in every
language.

This page is the contract. Its machine-readable form,
[`api-contract.json`](api-contract.json), lists every function and argument;
the R and Python test suites check their package against it, and
`tools/api_parity.py` reports both side by side.

## The model

| Concept | What it is |
|---|---|
| **Style** | What to draw: a source and its own filters and key |
| **Basemap** | Styles (bottom to top), what is given of the view, the map's CRS, a clip, fetch options |
| **View** | The extent, zoom, image size and dpi, each given, derived or defaulted |
| **Rendering** | The image, where it is (bounds, CRS), its zoom, dpi and scale, and notes |

### Three controls

| Question | Control | Meaning |
|---|---|---|
| What to show | the extent: `bounds` (an area), or `center` with `zoom` (the camera) | the part of the Earth on the map |
| How much detail | `zoom` | the MapLibre zoom (512 px tiles, fractional): which roads, places and labels the style draws |
| How big the symbols are | `symbol_scale` | lines, labels and icons as a multiple of the style's designed size (1: a CSS pixel is 1/96 inch, so a 12 px label prints at 9 pt) |
| How sharp | `dpi` | image pixels per inch, and nothing else |

Two helpers say the same things in other terms: `width` / `height` (the
image size, in `units`: `"px"`, `"in"`, `"cm"` or `"mm"`) and
`around(center, span, units)` (an area of a ground distance, in `"m"`,
`"km"`, `"mi"` or `"ft"`).

**Any three of extent, zoom, printed size and symbol scale give the
fourth:** `ground width = printed inches × 96 / symbol_scale × metres per
CSS px at the zoom`. The dpi only turns the printed size into pixels
(`px = inches × dpi`). A `width` / `height` in `"in"`, `"cm"` or `"mm"` is
the printed size; in pixels it is a printed size through the dpi (`px /
dpi` inches), so with the pixels fixed, a higher dpi prints the map smaller
(and, with the extent fixed, at a lower zoom). With `symbol_scale` 1 this is
0.3's rule, and 0.3's renders are unchanged.

What nothing gives is defaulted, in this order: dpi (96), symbol scale (1),
image size (800 × 600 px at 96 dpi, scaled with the dpi), extent (the world).
The zoom is never a default. Extent, zoom and a printed size derive the
symbol scale (a *decided* note). Over-determined requests (extent, zoom,
printed size and symbol scale together; `bounds` with `center`; `center`
without `zoom`) are errors that say what to drop. The view never reaches
past the poles.

`zoom_for(bounds, width, units = "in", symbol_scale = 1, crs = NULL)` is the
zoom at which `bounds` drawn `width` wide has that symbol scale, by the same
rules: the zoom to give a plot whose maps must match at several sizes.

### Coordinates and CRSs

- Plain numbers are longitude / latitude (`bounds = c(west, south, east,
  north)`, west > east across the date line). Spatial objects (`sf`,
  GeoDataFrame, GeoSeries) and `around()` bring their own CRS.
- `crs` is the map's CRS: given; else the CRS of a spatial layer giving the
  extent, when it is projected; else Web Mercator. A layer in longitude /
  latitude falls back to Web Mercator (`crs = 4326` when wanted).
- A map in another CRS is drawn in Web Mercator and warped into it, with a
  mesh R (`sf`) and Python (`pyproj`) compute; Web Mercator and WGS 84
  need neither.
- SVG output is Web Mercator: a map in another CRS is an error there.

### In a plot

The plot gives the extent (the panel's limits), the CRS (the basemap is
warped into it), the image size (the panel) and the resolution: on a screen
96 dpi × its pixel ratio, in a raster export the device's dpi, in PDF / SVG
300 dpi. The device's dpi sets the resolution and nothing else, so a
basemap's `dpi` is an error in a plot.

What the map shows is set in one place: the basemap's `zoom` or its
`symbol_scale` (in R, both go in `geom_basemap()`). The plot fixes the
extent and the printed size, so each sets the other; giving both is an
error. With neither, the symbol scale is 1, and the same plot shows the same
map at any dpi. A larger symbol scale means a lower zoom (fewer roads and
labels), a smaller one more detail. With `symbol_scale` fixed, a smaller
panel shows less detail at the same label size; with `zoom` fixed, it shows
the same picture, shrunk.

`ggsave(dpi = 200)` with `geom_basemap(symbol_scale = 0.84)` draws 1.75 map
pixels per CSS pixel, which is what `dpi = 168` drew in 0.3: to convert,
`symbol_scale = d / D`. A chosen `zoom` reports the symbol scale it gives as
a *decided* note. `last_rendering()` returns what a drawn plot used (its
`zoom`, `symbol_scale`, `dpi`, and the printed 1:N `scale`).

A basemap with its own extent, CRS or size is an error in a plot. A clip is
a mask there, in the plot's CRS.

### Clipping

`clip = layer` draws only inside a layer's polygons (and frames the map on
them, giving the default extent and, when projected, the CRS);
`clip = outside(layer)` draws everywhere but inside them (and sets neither).
Polygons only: a point or line layer is an error that suggests a buffer.
Labels are cut at the edge.

### Notes

Every view and rendering carries notes: a level, a stable `kind`, a message
and, for one that happened several times, a count and details.

| Level | When | R | Python |
|---|---|---|---|
| decided | a value nothing gave (the automatic zoom, the default dpi, the scale a plot's zoom gives) | `print()`; messages with `options(basemapper.verbose = TRUE)` | `repr()`; the `basemapper` logger at INFO |
| adjusted | something asked for was changed (the view kept inside the world, coarser tiles for `max_tiles`, tiles enlarged past the source's zoom, 300 dpi for PDF / SVG) | a `basemapper_adjustment` message, once per kind per session | a `BasemapAdjustment` warning (once per call site) |
| problem | the output is incomplete (tiles, glyphs, sprites or DEM missing, unsupported style features, filter names matching nothing) | a `basemapper_problem` warning, every time | a `BasemapProblem` warning, every time |

`fetch_options(on_error = "fail")` / `FetchOptions(on_error="fail")` turns
problems into errors.

## Basemaps

| | R | Python | Rust |
|---|---|---|---|
| Create | `basemap(style, ..., bounds, center, zoom, width, height, units, dpi, symbol_scale, crs, clip, fetch)` | `basemap(style, *, bounds, center, zoom, width, height, units, dpi, symbol_scale, crs, clip, fetch)` | `Basemap::new(style)` / `Basemap::stack(styles)`, then `.bounds(b, Crs)`, `.center(lon, lat)`, `.zoom(z)`, `.size(w, h)` / `.size_in(w, h, Units)`, `.dpi(d)`, `.symbol_scale(s)`, `.fetch(..)`, `.clip(Shape)` |
| Change the view (a new basemap) | `set_view(m, center =, zoom =, width =, height =, units =, dpi =, symbol_scale =)`, `set_bounds(m, bounds)` | `m.set_view(center=, zoom=, width=, height=, units=, dpi=, symbol_scale=)`, `m.set_bounds(bounds)` | `m.with_inputs(ViewInputs)` |
| Read the view | `get_bounds(m)`, `get_center(m)`, `get_zoom(m)`, `get_size(m)`; `print(m)` shows the dpi, symbol scale, CRS, scale and provenance | `m.bounds`, `m.center`, `m.zoom`, `m.size`, `m.dpi`, `m.symbol_scale`, `m.crs`, `m.scale`, `m.provenance`, `m.notes` | `m.view(&Context::Standalone)?`: a `View` |
| Render | `render(m, dpi =)` | `m.render(dpi=)` | `m.render(&Context::Standalone)?`; in another CRS `m.render_view(&view, Some(&warp))?` |
| SVG | `render_svg(m, path =, text =, embed_fonts =, font_dirs =)` | `m.render_svg(path=, text=, embed_fonts=, font_dirs=)` | `m.render_svg(&context, &SvgOptions)?` |
| Credit line | `get_attribution(m)` | `get_attribution(m)` | `m.attribution()?` |

Values never change: `set_view()` / `set_bounds()` return a new basemap in
both R and Python. `set_view(center = ...)` replaces the bounds;
`set_bounds()` unsets the centre; `NULL` / `None` unsets a value.
`render(m, dpi = 300)` is `render(set_view(m, dpi = 300))`.

## Renderings

| | R | Python | Rust (`Rendering`) |
|---|---|---|---|
| Size, place, detail | `r$width`, `r$height`, `r$bounds`, `r$crs`, `r$zoom`, `r$tile_zoom`, `r$dpi`, `r$symbol_scale`, `r$scale` | `r.width`, `r.height`, `r.bounds`, `r.crs` (`pyproj.CRS`), `r.zoom`, `r.tile_zoom`, `r.dpi`, `r.symbol_scale`, `r.scale` | the same fields; `scale()` a method |
| Notes | `r$notes`: a data frame (`level`, `kind`, `message`, `count`, `details`) | `r.notes`: `Note` tuples; `r.notes_at(level)` | `notes: Vec<Note>`; `notes_at(Level)` |
| PNG | `write_png(r, path)` | `r.save(path)`, `r.to_png()` | `to_png()?` |
| Pixels | `as_array(r)`: integer `[h, w, 4]` with `bounds` and `crs` attributes | `r.to_numpy()`: uint8 `(h, w, 4)`, a read-only view without a copy | `pixels` (RGBA, straight alpha) |
| Georeferenced raster | `as_raster(r)`: a terra SpatRaster | `r.to_xarray()`: an xarray DataArray | — |
| Show | `plot(r)`; knitr shows it at its CSS size | Jupyter shows it at its CSS size | — |

The scale is 1:N at the map's centre, from the zoom and the symbol scale (a
CSS pixel prints at `symbol_scale / 96` inch), the same at any dpi.

## Plots

| | R | Python |
|---|---|---|
| Place a basemap | `ggplot(nc) + m` (ggplot2); `tm_basemap(m)` (tmap v4) | `m.add_to(ax, crs=, alpha=, attribution=)` (matplotlib); `ggplot(df) + m` (plotnine) |
| Aliases | `geom_basemap(style, ..., alpha =, attribution =)`; `tm_basemap(style, ..., alpha =, attribution =)` | `add_basemap(ax, style, *, crs=, alpha=, attribution=, **basemap_args)`; `geom_basemap(style, *, crs=, alpha=, attribution=, ...)` |
| What a plot drew | `last_rendering()` | `last_rendering()`; the artist's `.rendering` |
| Plan a zoom | `zoom_for(bounds, width, units =, symbol_scale =, crs =)` | `zoom_for(bounds, width, units=, symbol_scale=, crs=)` |

The aliases build the basemap from `basemap()`'s arguments that apply in a
plot (`zoom` or `symbol_scale`, `clip`, `fetch`). Each library draws the credit line
in the panel's bottom-right corner with the core's layout (`attribution =
TRUE` by default, `FALSE` for none, or your text). The basemap renders when
the plot is drawn, for the panel's limits, pixels and device, so panning,
zooming and exporting at another dpi draw it afresh.

What differs by language, on purpose:

- matplotlib axes carry no CRS: `add_to(ax, crs = ...)` gives it (cartopy
  GeoAxes are read). plotnine has no spatial coordinates either: its
  `geom_basemap(crs=)` gives the plot's CRS, else a GeoDataFrame's, else
  longitude / latitude. ggplot2 and tmap read their own.
- tmap is R only; matplotlib is Python only.

## Styles

| | R | Python | Rust (`Source`, `Style`) |
|---|---|---|---|
| A style | `style(source, ..., layers =, features =, key =)` | `style(source, *, layers=, features=, key=)` | `Style::new(source).layers(..).features(..).key(..)` |
| A catalogue basemap, by form | `provider(name, form =, tiles =)` | `provider(name, form=, tiles=)` | `Source::provider(name, form, tiles)` |
| Tile data | `tiles(input, ..., type =, tile_size =, scheme =, minzoom =, maxzoom =, attribution =, source_layers =, paint =)` | `tiles(input, *, type=, ...)` | `Source::tiles(input, TileStyleOptions)` |
| OGC services | `wms(url, layers, ...)`, `wmts(url, layer, ...)` | `wms(url, layers, ...)`, `wmts(url, layer, ...)` | `Source::wms(url, layers)`, `Source::wmts(url, layer)` |
| A style's layers (and a filter's preview) | `list_layers(style)`: data frame of `id`, `type`, `source_layer`, `feature` | `list_layers(style)`: list of dicts, same keys | `list_style_layers(&style)?` |
| The catalogue | `list_providers(type =)`, `providers$CartoDB.Positron` | `list_providers(type=)`, `providers.CartoDB.Positron` | `providers::list_providers(kind)` |

A string works wherever a style does and means `style(string)`: a catalogue
name (`"OpenFreeMap.Liberty"`), a MapLibre style URL, file or JSON, a tile
URL template (`{z}/{x}/{y}`, `{quadkey}`, `{bbox-epsg-3857}`), a TileJSON, a
PMTiles archive (URL, path or `pmtiles://`), or an ArcGIS `MapServer` /
`VectorTileServer`. Several styles are stacked bottom to top as a list.

- **Filters belong to each style.** `features` keeps or (`-`-prefixed)
  drops feature classes (background, water, landcover, parks, buildings,
  roads, rail, boundaries, places, poi, terrain, other) and `"labels"`:
  `"labels"` keeps only the labels, `"-labels"` drops them, and they combine
  (`c("roads", "labels")` keeps the road labels). `layers` matches layer ids
  (`*` and `?` wildcards, `-` to drop). A filter that leaves nothing of a
  style is an error; a name that matches nothing is a problem note.
  Catalogue raster basemaps switch to their label-free or label-only tiles.
  A catalogue stack (`"UGRC.Lite"`) is one style, its services filtered as
  one map.
- **One credential, `key`:** an API key for catalogue basemaps (CARTO,
  MapTiler, Stadia, UGRC; default from `CARTO_API_KEY`, `MAPTILER_API_KEY`,
  `STADIA_API_KEY`, `UGRC_QUAD_WORD`), else a token sent only to the style's
  own server. A missing key, an unknown name or a form a basemap lacks is an
  error when the style is made.

## Fetching and the cache

| | R | Python | Rust |
|---|---|---|---|
| Download settings | `fetch_options(timeout =, max_tiles =, concurrency =, on_error =)` | `FetchOptions(timeout=, max_tiles=, concurrency=, on_error=)` | `FetchOptions::new().timeout(..).max_tiles(..)...` |
| The tile cache | `tile_cache(dir =, clear =, max_size =)` | `tile_cache(dir=, clear=, max_size=)` | `cache::dir()`, `cache::set_dir(..)`, `cache::clear()` |

Defaults: 10 s, 256 tiles, 16 at once, `"skip"`. Tiles are cached on disk as
long as the server's caching headers allow (a day without them), in
`BASEMAPPER_CACHE_DIR` (`off` disables it) or the platform cache directory;
past 250 MB (`BASEMAPPER_CACHE_MAX_SIZE`) the least recently used go first.

## Errors

| Kind | R condition class | Python exception | Rust |
|---|---|---|---|
| Invalid input (a view, style, filter, CRS or clip that cannot work) | `basemapper_validation_error` | `ValidationError` (also a `ValueError`) | `BasemapError::kind() == "validation"` |
| A style that cannot be fetched or read | `basemapper_style_error` | `StyleError` | `"style"` |
| A failed download | `basemapper_network_error` | `NetworkError` | `"network"` |
| Anything else | `basemapper_render_error` | `BasemapError` | `"render"` |

Every R condition also has class `basemapper_error`; every Python exception
derives from `BasemapError`. Names removed in 0.3 fail with their
replacement: see [Migrating to 0.3](https://ar-puuk.github.io/basemapper/migrating-to-0.3.html).
