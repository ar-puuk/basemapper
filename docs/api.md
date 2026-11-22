# API reference: R, Python and Rust side by side

basemapper has one API in three languages. Functions and arguments have the
same names, defaults and meaning everywhere. The only systematic difference
is call style: R calls `verb(m, ...)` on a view, while Python and Rust call
`m.verb(...)`. R views are values, so `set_view()` returns a modified copy.
Python and Rust views are changed in place, and the methods return the view
so calls chain.

The renderers, the providers and `list_layers()` live in the Rust core
(`basemapper_core`), so the same call returns the same style or pixels in
every language.

## Shared arguments

| Argument | Meaning | Default |
|---|---|---|
| `style` | A MapLibre GL style URL or JSON string, or a list / vector of them stacked bottom to top | — |
| `width`, `height` | View size in CSS pixels | 800, 600 |
| `center`, `zoom` | View centre (longitude, latitude) and MapLibre zoom (0–22, fractional) | (0, 0), 0 |
| `bounds`, `crs` | Bounds to fit instead of `center` / `zoom`, and their CRS | —, EPSG:4326 |
| `pixel_ratio` | Device pixels per CSS pixel (MapLibre's `devicePixelRatio`); images are `width * pixel_ratio` by `height * pixel_ratio` | 1 (plot layers: output dpi / 96) |
| `layers` | Layer filter: plain IDs keep, `-`-prefixed IDs drop | all layers |
| `auth_token` | Access token for the tile service | none |
| `tile_timeout_ms` | Per-tile HTTP timeout | 10000 |
| `max_tiles` | Maximum tiles fetched per style | 256 |
| `tile_concurrency` | Parallel tile downloads | 16 |
| `fail_on_tile_error` | Abort on a failed tile instead of leaving it empty | false |
| `tile_zoom` | Tile detail override (tile zoom on the 256 px grid; vector sources use one less) | as MapLibre picks |
| `path` | Output file for `render_png` / `render_svg`; none returns the result | none |

## Map views

| | R | Python | Rust |
|---|---|---|---|
| Create | `m <- basemap(style, width, height, center =, zoom =, bounds =, crs =, pixel_ratio =, ...)` | `m = basemap(style, width, height, center=, zoom=, bounds=, crs=, pixel_ratio=, ...)` | `let m = Basemap::new(style).size(w, h).center(lon, lat).zoom(z).pixel_ratio(r)` (several styles: `Basemap::stack(styles)`; bounds: `.bounds(b, Crs::LonLat)?`) |
| Set centre / zoom | `m <- set_view(m, center =, zoom =)` | `m.set_view(center=, zoom=)` | `m.set_view(Some((lon, lat)), Some(z))?` |
| Fit bounds | `m <- set_bounds(m, bounds, crs =)` | `m.set_bounds(bounds, crs=)` | `m.set_bounds(bounds, Crs::LonLat)?` |
| Extent | `get_bounds(m, lonlat = FALSE)` | `m.get_bounds(lonlat=False)` | `m.get_bounds(false)` |
| Centre | `get_center(m)` | `m.get_center()` | `m.get_center()` |
| Zoom | `get_zoom(m)` | `m.get_zoom()` | `m.get_zoom()` |
| Image size (device px) | `get_size(m)` | `m.get_size()` | `m.get_size()` |
| PNG | `render_png(m, path = NULL)` | `m.render_png(path=None)` | `m.render_png(Some(path))?` |
| Vector SVG | `render_svg(m, path = NULL, text =, embed_fonts =, font_dirs =)` | `m.render_svg(path=None, text=, embed_fonts=, font_dirs=)` | `m.render_svg(Some(path), &SvgOptions { .. })?` |
| RGBA array | `render_array(m)`: integer `[h, w, 4]` with extent attributes | `m.render_array()`: uint8 `(h, w, 4)` | `m.render_array()?`: `RenderResult { pixels, width, height, .. }` |
| terra raster | `render_raster(m)` | — | — |

`bounds` takes a numeric `c(xmin, ymin, xmax, ymax)` / sequence in `crs`. R
also takes an `sf::st_bbox()` result or an `sf` / `sfc` object, and Python a
dict or a GeoDataFrame / GeoSeries. `crs` takes anything `sf::st_crs()` or
`pyproj.CRS` accepts. Rust takes `Crs::LonLat` or `Crs::WebMercator`, and
`Crs::parse("EPSG:4326")` parses a string.

## One-shot rendering

| R | Python | Rust |
|---|---|---|
| `render_basemap_raw(bounds, width, height, style, crs =, pixel_ratio =, tile_zoom =, layers =, auth_token =, tile_timeout_ms =, max_tiles =, tile_concurrency =, fail_on_tile_error =)` | `render_basemap_raw(bounds, width, height, style, crs=, pixel_ratio=, tile_zoom=, ...)` | `render(RenderRequest { bbox, width, height, style_input, tile_zoom, pixel_ratio, .. })` / `render_stack(requests)` |

Both packages return the pixels of the fitted view: R an integer array with
the input extent as attributes, Python a uint8 array.

## Plot layers

| R | Python |
|---|---|
| `geom_basemap(style, tile_zoom =, alpha =, layers =, pixel_ratio =, auth_token =, tile_timeout_ms =, max_tiles =)` (ggplot2) | `geom_basemap(style, tile_zoom=, alpha=, layers=, pixel_ratio=, auth_token=, tile_timeout_ms=, max_tiles=, crs=)` (plotnine) |
| `tm_basemap(style, ..., width =, height =)` (tmap v4) | `add_basemap(ax, style, ..., crs=)` (matplotlib) |

These layers render when the plot is drawn, for the panel's extent and at
the output resolution. Python's `crs` gives the CRS of the plot's
coordinates; plotnine defaults it to EPSG:4326, and matplotlib detects it
from cartopy axes.

## Styles

`style` takes a MapLibre style (URL, file or JSON), a named basemap, or tile
data whose style is built for you: a tile URL template (`{z}/{x}/{y}`,
`{quadkey}`, `{bbox-epsg-3857}`, WMTS `{TileMatrix}/{TileRow}/{TileCol}`), a
TileJSON, a PMTiles archive (URL, local path or `pmtiles://`), or an ArcGIS
`MapServer` / `VectorTileServer`. Raster or vector is detected from the
archive header, the TileJSON, the URL's extension, or one fetched tile.
Vector data gets a default style that draws every source layer over a
transparent background. `tile_style()` builds the same style with
overrides.

| R | Python | Rust (`basemapper_core::providers`) |
|---|---|---|
| `basemap_provider(name, api_key = NULL)` | `basemap_provider(name, api_key=None)` | `basemap_provider(name, api_key)?` |
| `list_providers()` | `list_providers()` | `PROVIDER_NAMES` |
| `tile_style(source, type =, tile_size =, scheme =, minzoom =, maxzoom =, attribution =, source_layers =, paint =, auth_token =)` | `tile_style(source, type=, tile_size=, scheme=, minzoom=, maxzoom=, attribution=, source_layers=, paint=, auth_token=)` | `basemapper_core::tile_style(source, &TileStyleOptions { .. }, auth_token)?`, which also returns the dropped paint keys |
| `wms_style(url, layers, styles =, version =, format =, transparent =, extra_params =)` | `wms_style(url, layers, styles=, version=, format=, transparent=, extra_params=)` | `wms_style(...)` |
| `wmts_style(url, layer, tilematrixset =, format =, style =, version =, extra_params =)` | `wmts_style(url, layer, tilematrixset=, format=, style=, version=, extra_params=)` | `wmts_style(...)` |
| `list_layers(style, auth_token = NULL)`: data frame of `id`, `type` | `list_layers(style, auth_token=None)`: list of `{"id", "type"}` | `basemapper_core::list_layers(style, auth_token)?`: `Vec<(id, type)>` |
