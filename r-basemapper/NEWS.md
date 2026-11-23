# basemapper (development version)

## New interface for download settings

* New `fetch_options()` gathers how tiles download: `timeout` (now in
  seconds), `max_tiles`, `concurrency` and `on_error` (`"skip"` or
  `"fail"`). `basemap()`, `render_basemap_raw()`, `geom_basemap()` and
  `tm_basemap()` take it as `fetch = fetch_options(...)`, which shortens
  their argument lists by four.
* `tile_timeout_ms`, `max_tiles`, `tile_concurrency` and
  `fail_on_tile_error` are deprecated: they still work, with a warning
  saying what to write instead.
* Arguments after the first ones must now be named (`basemap(style, 800,
  600, zoom = 12)`); an unknown argument name is an error rather than being
  ignored. `geom_basemap()` and `tm_basemap()` now take `...` right after
  `style`.
* Errors carry classes, the same kinds as Python's exceptions:
  `basemapper_validation_error`, `basemapper_style_error`,
  `basemapper_network_error` and `basemapper_render_error`, each also a
  `basemapper_error` (see `?basemapper-errors`).
* `labels` takes `"all"`, `"none"` or `"only"`, with `TRUE` and `FALSE` as
  shorthand (as before).
* sf and terra are now suggested rather than required: views, bounds in
  EPSG:4326 or EPSG:3857, and `get_bounds(lonlat = TRUE)` work without
  them. Bounds in another CRS or as `sf` objects need sf; `render_raster()`
  needs terra. You are offered to install them when needed. On Linux this
  saves installing GDAL, GEOS and PROJ.

## Performance

* Redrawing a plot (resizing it, `ggsave()`, re-knitting) no longer
  downloads the style, TileJSON, glyphs, sprite or PMTiles directories
  again: they are kept in memory for as long as the server's caching
  headers allow (at most an hour). Decoded tiles are kept too, so a redraw
  makes no requests and skips decoding.
* Tiles are decoded on all cores, raster layers and raster basemaps are
  resized in parallel, and the styles of a stacked basemap (`"UGRC.Lite"`)
  download and render concurrently. Connections and TLS sessions are reused
  across renders.
* `plot()` and `geom_basemap()` draw the basemap as a `nativeRaster`: a
  4000 x 3000 px map (e.g. `ggsave(dpi = 300)`) no longer builds an 800 MB
  intermediate array, and the conversion takes milliseconds instead of
  about 8 seconds.

## Bug fixes

* `max_tiles` now limits vector tiles too. It used to be checked against the
  raster tile grid only, so a large vector view (or `render_svg()`, or a
  style's extra raster, hillshade or vector sources) could fetch many more
  tiles than the limit. Raster layers in vector styles drop to coarser tiles
  to fit, as raster basemaps do; vector sources that need more tiles than
  the limit are an error.
* Out-of-range integers are errors instead of wrapping: `tile_zoom = 256`
  used to render at zoom 0, and a negative `max_tiles` or `tile_timeout_ms`
  turned the limit off.
* A failed attribution lookup (a network blip) is retried after a minute,
  rather than leaving every later plot of the session without its credit
  line.
* An invalid `pixel_ratio` or `tile_zoom` is reported as such, not as an
  "invalid bbox".
* PMTiles archives honour `fail_on_tile_error` and `tile_concurrency`.
* A broken sprite index is skipped instead of stopping the render.

## Security

* Access tokens (`auth_token`) are sent only to the origin the style came
  from (and to the tile servers a TileJSON from there names), no longer to
  every server a style mentions, such as a third-party overlay. Styles given
  as JSON or a local file are yours: their servers all get the token, as
  before.
* API keys in URLs (`key=`, `api_key=`, `access_token=`, UGRC's quad word,
  ...) are replaced by `***` in error messages and logs.
* Downloads have size limits, counted after decompression (tiles 32 MB,
  styles 64 MB, GeoJSON 512 MB, glyphs, sprites and fonts 32 MB), and
  gzip-compressed tiles may inflate to at most 64 MB.
* A style downloaded from the web can no longer read local PMTiles files.
* Downloaded fonts are cached in the tile cache's folder
  (`tools::R_user_dir("basemapper", "cache")`), so turning the cache off
  turns the font cache off too.
* The Rust library is built with `cargo build --locked`, from the exact
  dependency versions in `Cargo.lock`.

# basemapper 0.1.1

* The install instructions in the README, the getting-started article and
  the site name the release (`pak::pak("ar-puuk/basemapper/r-basemapper@v0.1.1")`),
  so an install gets a tested version rather than the development branch.
* No change to the package's code.

# basemapper 0.1.0

The first numbered release (2026-11-22).

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
* `labels` and `features` draw parts of a style, so basemaps can be
  stacked: `labels = FALSE` drops the labels, `labels = "only"` keeps just
  them on a transparent background, and `features` keeps (`c("water",
  "roads")`) or drops (`"-buildings"`) background, water, landcover, parks,
  buildings, roads, rail, boundaries, places, poi or terrain. They work on
  every renderer and plot layer (for the whole stack) and on
  `basemap_provider()` (for one basemap, e.g. imagery under a vector
  basemap's labels). Features come from tables for the OpenMapTiles,
  Protomaps and Shortbread schemas and keywords for Esri's layer names;
  catalogue raster basemaps switch to their label-free or label-only tiles
  (CARTO, Esri, Stamen). `list_layers()` gains a `feature` column.
* `get_attribution()` returns the attribution text a view or style asks
  maps to show, for a caption: every source's attribution (from the style,
  its TileJSON or PMTiles metadata) as plain text, without duplicates.
* Plot layers (`geom_basemap()`, `tm_basemap()`, `add_basemap()`) draw the
  basemap's credit line in the bottom-right corner: `attribution = TRUE` by
  default, `FALSE` to turn it off, or your own text.
* `layers` takes `*` and `?` wildcards (`"-label_*"`). Names in `layers` or
  `features` that match no layer of any style warn.
* `list_layers()` shows each layer's `source_layer`, and takes `layers`,
  `labels` and `features` to preview what a render draws.
* `render_svg()` takes raster basemaps too: raster tiles are embedded as
  images (vector layers stay paths and text).
* UGRC's vector basemaps of Utah: `"UGRC.Lite"`, `"UGRC.Dark"`,
  `"UGRC.Hybrid"` and `"UGRC.Outdoors"`. Each is published as separate
  services (hillshade, base and labels; Hybrid is Esri's imagery under UGRC's
  overlay), and its name stands for the stack anywhere a style goes;
  `basemap_provider()` returns its styles as a character vector.
* UGRC Discover's raster layers of Utah, with its quad word as the key
  (`api_key`, or the `UGRC_QUAD_WORD` environment variable), all 36 of its
  layers:
  - maps: `"UGRC.Terrain"`, `"UGRC.Topo"`, `"UGRC.LiteRaster"`,
    `"UGRC.HybridRaster"`, `"UGRC.Hillshade"`, and the transparent
    `"UGRC.Overlay"` (roads and names) and `"UGRC.AddressPoints"`, which
    draw without a background to go over another map;
  - imagery: `"UGRC.Imagery"` (the latest statewide imagery, 15 cm from
    Hexagon), `"UGRC.ImageryHexagon15cm"` / `30cm`, `"UGRC.ImageryGoogle"`
    and its archives `"UGRC.ImageryGoogle2011"` to `2018` (some years cover
    only parts of the state), NAIP 2006 to 2024 (`"UGRC.NAIP2024"`, and
    `"UGRC.NAIP2024NRG"` for false colour), `"UGRC.HRO2012"` (Wasatch Front)
    and `"UGRC.DOQ1990s"`; `labels = "only"` gives UGRC's overlay for them.

  A quad word opens the layers UGRC gives it, as an API key's plan does; a
  missing key's error names the variable and where to get one.
* A tile server that refuses every tile (HTTP 401 or 403, a key without
  access), or answers with a web page instead of tiles (as some do for a
  wrong key), is now an error that says to check the key, instead of an
  empty map. The message leaves out the tile URL, which may carry the key.
* The tile cache looks after itself: expired tiles are deleted, and past
  its size limit (250 MB; `tile_cache(max_size =)` or `BASEMAPPER_CACHE_MAX_SIZE` in
  MB) the least recently used tiles go first, in the background.
  `BASEMAPPER_CACHE_DIR` sets its folder for R and Python alike; the
  getting-started guide has a section on it. Tests use a temporary cache.
* `labels`, `features` and `layers` treat a stack of styles as one basemap
  (e.g. UGRC's hillshade, base and labels services): a style they leave
  nothing of is skipped, and it is an error only when they leave nothing of
  the whole stack. Before, `labels = "only"` failed on any stack with a
  style that has no labels.
* A view printed in R Markdown or Quarto (`m` on its own line) shows the map,
  at its CSS pixel size; `plot(m)` draws it on the current device.
* `geom_basemap()` and `tm_basemap()` no longer fail on a stack that includes
  inline style JSON (e.g. `basemap_provider(..., labels = "only")`): the
  attribution cache used the style as a variable name.
* Raster basemaps no longer show a one-pixel white line at tile seams when
  rendered at a fractional pixel ratio (as a figure's dpi gives).
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
