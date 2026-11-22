# basemapper 0.1.0

First version, not yet released.

## One API across R, Python and Rust

The R and Python packages (and the Rust core's `Basemap`) share function
names, argument names and defaults; R's `verb(m, ...)` is `m.verb(...)` in
Python. See `docs/api.md` in the repository.

* Shared arguments: `style`, `width` / `height` (CSS px), `center` + `zoom`
  or `bounds` + `crs`, `pixel_ratio`, `layers`, `auth_token`,
  `tile_timeout_ms`, `max_tiles`, `tile_concurrency`, `fail_on_tile_error`
  (default `FALSE` everywhere), `tile_zoom` (tile detail override) and
  `path` (output file).
* Providers and `list_layers()` are implemented once in the Rust core, so
  both languages return the same styles; `list_layers()` returns a data
  frame of `id` and `type` in drawing order; `list_providers()` is new.

## Rendering

* Vector styles are drawn by an in-house CPU port of MapLibre GL JS's
  renderer, tested pixel by pixel against MapLibre GL JS (168 reference
  views: CARTO Positron, Dark Matter and Voyager and the UGRC basemap at
  zooms 1-22, fractional zoom, pixel ratio 2, OSM raster tiles). No GPU,
  browser or display is needed; the earlier maplibre-rs/wgpu engine is gone.
* Basemaps render at any fractional zoom and pixel ratio (MapLibre's
  devicePixelRatio): the same view and labels, drawn at the output
  resolution.
* Several styles can be stacked bottom to top, e.g. a basemap published as
  separate hillshade, base and label services.
* True vector SVG output: fills and lines as paths, icons embedded, labels
  as editable `<text>` at MapLibre's glyph positions with the style's fonts
  embedded (subset; open fonts from Google Fonts or your `font_dirs`), or as
  traced glyph outlines with `render_svg(text = "paths")`.
* Views crossing the anti-meridian render MapLibre's world copies.
* Tile URLs are requested as MapLibre requests them: all `tiles` templates
  (spread over subdomains), `scheme: "tms"`, and the `{quadkey}`, `{prefix}`,
  `{ratio}` and `{bbox-epsg-3857}` tokens. `wms_style()` uses
  `{bbox-epsg-3857}` and sends `SRS` for WMS 1.1.x.

## New functions

* `basemap()` creates a map view from a centre and zoom or from bounds;
  `set_view()` and `set_bounds()` change it, `get_bounds()`, `get_center()`,
  `get_zoom()` and `get_size()` read it.
* `render_png()`, `render_svg()`, `render_array()` and `render_raster()`
  (a `terra` SpatRaster) render a view.
* `render_basemap_raw(bounds, width, height, style, ...)` renders bounds in
  one call; it accepts stacked styles and `pixel_ratio`.
* `style` accepts tile data as well as styles, building the style for it: a
  tile URL, TileJSON, PMTiles archive (URL or local file, raster or vector,
  read from the archive), ArcGIS MapServer or VectorTileServer, or a local
  style file. Vector data gets a default style drawing every source layer.
* Styles: `basemap_provider()` (named catalogue), `tile_style()` (a style for
  tile data, with overrides), `wms_style()` and `wmts_style()`;
  `list_layers()` inspects a style.
* Named basemaps are a hand-curated catalogue with leaflet-providers names
  (`CartoDB.Positron`, `Esri.WorldImagery`, `OpenStreetMap.HOT`, plus
  OpenFreeMap): 42 basemaps, each with a vector form, a raster form or both.
  A name works as `style` directly. `basemap_provider(name, type, api_key,
  tiles)` picks the form (vector by default; asking for a missing form is an
  error, never a fallback), reads API keys from `CARTO_API_KEY`,
  `MAPTILER_API_KEY` or `STADIA_API_KEY`, and draws your own tiles with a
  basemap's style. Unknown names suggest close ones. `list_providers()`
  returns a data frame of names, forms, key variables and attribution;
  `providers` completes names (`providers$CartoDB.Positron`). CARTO now
  needs a free API key.
* Tiles are cached on disk as long as the server's caching headers allow;
  `tile_cache()` shows, moves, disables or clears the cache (in
  `tools::R_user_dir("basemapper", "cache")` by default). Requests identify
  basemapper in their User-Agent.
* ArcGIS vector styles whose source names the `VectorTileServer` without a
  trailing slash (Esri's item styles) load their tiles.

## Plot layers

* `geom_basemap()` renders when the plot is drawn, for the panel's size at
  the device's resolution: `ggsave(dpi = 300)` gets a basemap rendered at
  300 dpi (PDF and SVG devices also use 300 dpi). The pixel ratio defaults to
  dpi / 96 so lines and labels keep their physical size; `pixel_ratio =`
  overrides.
* `tm_basemap()` (tmap v4) sizes its basemap from the map panel and device
  resolution, so `tmap_save(dpi = 300)` gets a native basemap too.
* Both accept a vector of styles to stack.
