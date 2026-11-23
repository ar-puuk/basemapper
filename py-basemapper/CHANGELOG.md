# Changelog

## Unreleased (0.1.0)

First version, not yet released.

### Rendering

- Vector styles are drawn by an in-house CPU port of MapLibre GL JS's
  renderer, tested pixel by pixel against MapLibre GL JS (168 reference
  views). No GPU, browser or display is needed.
- Any fractional zoom and pixel ratio (MapLibre's devicePixelRatio); styles
  can be stacked bottom to top; anti-meridian views render world copies.
- True vector SVG output; labels as editable `<text>` with embedded (subset)
  fonts by default, or traced glyph outlines (`render_svg(text="paths")`).
- Tile URLs are requested as MapLibre requests them: all `tiles` templates
  (spread over subdomains), `scheme: "tms"`, and the `{quadkey}`, `{prefix}`,
  `{ratio}` and `{bbox-epsg-3857}` tokens. `wms_style()` uses
  `{bbox-epsg-3857}` and sends `SRS` for WMS 1.1.x.

### API

The Python and R packages (and the Rust core's `Basemap`) share function
names, argument names and defaults; R's `verb(m, ...)` is `m.verb(...)`
here. See `docs/api.md` in the repository.

- `basemapper.basemap(style | [styles], width, height, center=, zoom=,
  bounds=, crs=, pixel_ratio=, ...)` returns a `Basemap` view with
  `set_view()`, `set_bounds()`, `get_bounds(lonlat=)`, `get_center()`,
  `get_zoom()`, `get_size()`, `render_png(path=)`, `render_svg(path=)` and
  `render_array()`.
- `render_basemap_raw(bounds, width, height, style, crs=, pixel_ratio=,
  tile_zoom=, ...)` renders bounds in one call and returns a
  `(height, width, 4)` uint8 array; it accepts stacked styles.
- `style` accepts tile data as well as styles, building the style for it: a
  tile URL, TileJSON, PMTiles archive (URL or local file, raster or vector,
  read from the archive), ArcGIS MapServer or VectorTileServer, or a local
  style file. Vector data gets a default style drawing every source layer.
- Styles, implemented in the Rust core: `basemap_provider()`,
  `list_providers()`, `tile_style()` (a style for tile data, with
  overrides), `wms_style()` and `wmts_style()`.
- Named basemaps are a hand-curated catalogue with leaflet-providers names
  (`CartoDB.Positron`, `Esri.WorldImagery`, `OpenStreetMap.HOT`, plus
  OpenFreeMap): 42 basemaps, each with a vector form, a raster form or both.
  A name works as `style` directly. `basemap_provider(name, type, api_key,
  tiles)` picks the form (vector by default; asking for a missing form is an
  error, never a fallback), reads API keys from `CARTO_API_KEY`,
  `MAPTILER_API_KEY` or `STADIA_API_KEY`, and draws your own tiles with a
  basemap's style. Unknown names suggest close ones. `list_providers()`
  returns a list of dicts of names, forms, key variables and attribution;
  `providers` completes names (`providers.CartoDB.Positron`). CARTO now
  needs a free API key.
- Tiles are cached on disk as long as the server's caching headers allow;
  `tile_cache()` shows, moves, disables or clears the cache (in the platform
  cache directory by default). Requests identify
  basemapper in their User-Agent.
- `labels` and `features` draw parts of a style, so basemaps can be
  stacked: `labels=False` drops the labels, `labels="only"` keeps just
  them on a transparent background, and `features` keeps (`["water",
  "roads"]`) or drops (`["-buildings"]`) background, water, landcover, parks,
  buildings, roads, rail, boundaries, places, poi or terrain. They work on
  every renderer and plot layer (for the whole stack) and on
  `basemap_provider()` (for one basemap, e.g. imagery under a vector
  basemap's labels). Features come from tables for the OpenMapTiles,
  Protomaps and Shortbread schemas and keywords for Esri's layer names;
  catalogue raster basemaps switch to their label-free or label-only tiles
  (CARTO, Esri, Stamen). `list_layers()` gains a `feature` key.
- `get_attribution()` and `Basemap.get_attribution()` returns the attribution text a view or style asks
  maps to show, for a caption: every source's attribution (from the style,
  its TileJSON or PMTiles metadata) as plain text, without duplicates.
- Plot layers (`geom_basemap()`, `add_basemap()`) draw the
  basemap's credit line in the bottom-right corner: `attribution=True` by
  default, `False` to turn it off, or your own text.
- `layers` takes `*` and `?` wildcards (`["-label_*"]`). Names in `layers` or
  `features` that match no layer of any style warn.
- `list_layers()` shows each layer's `source_layer`, and takes `layers`,
  `labels` and `features` to preview what a render draws.
- `render_svg()` takes raster basemaps too: raster tiles are embedded as
  images (vector layers stay paths and text).
- UGRC's vector basemaps of Utah: `"UGRC.Lite"`, `"UGRC.Dark"`,
  `"UGRC.Hybrid"` and `"UGRC.Outdoors"`. Each is published as separate
  services (hillshade, base and labels; Hybrid is Esri's imagery under UGRC's
  overlay), and its name stands for the stack anywhere a style goes;
  `basemap_provider()` returns its styles as a list.
- UGRC Discover's raster layers of Utah, with its quad word as the key
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
    and `"UGRC.DOQ1990s"`; `labels="only"` gives UGRC's overlay for them.

  A quad word opens the layers UGRC gives it, as an API key's plan does; a
  missing key's error names the variable and where to get one.
- A tile server that refuses every tile (HTTP 401 or 403, a key without
  access), or answers with a web page instead of tiles (as some do for a
  wrong key), is now an error that says to check the key, instead of an
  empty map. The message leaves out the tile URL, which may carry the key.
- The tile cache looks after itself: expired tiles are deleted, and past
  its size limit (250 MB; `tile_cache(max_size=)` or `BASEMAPPER_CACHE_MAX_SIZE` in
  MB) the least recently used tiles go first, in the background.
  `BASEMAPPER_CACHE_DIR` sets its folder for R and Python alike; the
  getting-started guide has a section on it. Tests use a temporary cache.
- `labels`, `features` and `layers` treat a stack of styles as one basemap
  (e.g. UGRC's hillshade, base and labels services): a style they leave
  nothing of is skipped, and it is an error only when they leave nothing of
  the whole stack. Before, `labels="only"` failed on any stack with a
  style that has no labels.
- A view shown in Jupyter or Quarto (`m` as a cell's last line) displays the
  map, at its CSS pixel size.
- Raster basemaps no longer show a one-pixel white line at tile seams when
  rendered at a fractional pixel ratio (as a figure's dpi gives).
- ArcGIS vector styles whose source names the `VectorTileServer` without a
  trailing slash (Esri's item styles) load their tiles.
- `list_layers(style)` returns `[{"id", "type"}, ...]` in drawing order.
- `fail_on_tile_error` defaults to `False` everywhere; `tile_zoom` is the
  tile detail override; `requests` is no longer a dependency.

### Plot layers

- `add_basemap()` adds a `BasemapImage` artist that renders at draw time for
  the axes' extent and output resolution: `savefig(dpi=300)` and PDF output
  get a basemap rendered natively, and panning re-renders. `pixel_ratio`
  defaults to dpi / 96; `crs=` for axes without a CRS; style lists stack.
- plotnine `geom_basemap()` uses the same artist (works with plotnine 0.15);
  `crs=` (default EPSG:4326).
- Both take the same arguments as R's `geom_basemap()`: `style`,
  `tile_zoom`, `alpha`, `layers`, `pixel_ratio`, `auth_token`,
  `tile_timeout_ms`, `max_tiles`.
