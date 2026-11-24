# Changelog

## 0.4.0 (2026-11-24)

Each setting now has one meaning everywhere: `zoom` is the detail,
`symbol_scale` the size of lines, labels and icons, and `dpi` the sharpness.

### Breaking changes

- **A basemap's `dpi` is an error in a plot.** The device sets the sharpness
  (`savefig(dpi=)`, 300 dpi for PDF / SVG), and changes nothing else. In 0.3,
  `basemap(s, dpi=168).add_to(ax)` on a 200 dpi device also set the size of the map's lines and
  labels, and through it the zoom. Use `symbol_scale = d / D` for that now:
  `dpi = 168` saved at 200 dpi is `symbol_scale = 0.84`.
- **Extent, zoom and a size used to be over-determined; now they give the
  symbol scale.** `basemap(s, bounds=..., zoom=..., width=6, height=4, units="in")` was an error in 0.3; it now derives the
  symbol scale (a *decided* note), as a plot with a chosen zoom does.

### New features

- **`symbol_scale` in `basemap()` and `set_view()`** (and so in
  `m.add_to()`, `add_basemap()` and plotnine's `geom_basemap()`): lines, labels and icons as a multiple of the style's designed
  size. At 1, the default, a CSS pixel is 1/96 inch, so a label the style
  sizes at 12 px prints at 9 pt. The rule is now: any three of extent, zoom,
  printed size and symbol scale give the fourth, and the dpi turns the
  printed size into pixels. A size in pixels is a printed size through the
  dpi (`px / dpi` inches).
- **In a plot, give `zoom` or `symbol_scale`.** The plot fixes the extent and
  printed size, so each sets the other: a larger symbol scale means a lower
  zoom (fewer roads and labels), a smaller one more detail. With neither, the
  map is the same at any device dpi.
- **`zoom_for(bounds, width, units="in", symbol_scale=1)`**: the zoom at
  which `bounds` drawn `width` wide has that symbol scale, by the same rules
  as `basemap()`, in any CRS. Give it as the zoom to make a page map and its
  thumbnail the same picture.
- **`last_rendering()`** returns the rendering a plot last drew, to read its
  `zoom`, `symbol_scale` and `dpi`. Renderings and views carry
  `symbol_scale`, and in a plot a rendering's `dpi` is the device's and its
  1:N `scale` the printed one.
- **A chosen zoom in a plot is a decided note**, "zoom 13.1 draws lines,
  labels and icons at 0.84x their designed size", no longer an "effective
  dpi" adjustment.

### Unchanged

- **Renders on their own are the same as 0.3's** with `symbol_scale` left out
  (it is 1): every 0.3 view resolves identically, which the contract cases
  check in R, Python and Rust.

## 0.3.2 (2026-11-24)

SVG output has no seams in PDFs, and every output is now checked against
MapLibre GL JS at the dpis plots use.

- **SVG tiles meet without hairlines.** Each vector tile's paths are
  clipped to the tile, and PDF viewers (Typst, poppler, pdf.js, Acrobat)
  antialias clip edges, so 0.3.1 showed a faint light line at every tile
  boundary. Opaque paths are now clipped a little past their tile, so
  neighbouring tiles overlap by one CSS pixel. A layer whose paths share
  one opacity (most basemap fills and lines, such as UGRC's hillshade) is
  drawn opaque in a group that carries the opacity, so the overlap does not
  darken it. Lines cross tile edges without a notch, and labels and icons
  were never clipped to tiles.
- **Raster layers in SVG are embedded at the output's dpi** (raster,
  hillshade, patterned lines and extrusions), instead of at 96 dpi and
  enlarged by the viewer.
- **Tests:**
  - Renders at 168, 200 and 300 dpi, at fractional zooms, of the stacked
    UGRC Lite basemap and of OpenFreeMap Liberty, match MapLibre GL JS to
    within 0.02% of pixels.
  - A Web Mercator plot panel shows the rendering's pixels unchanged, with
    a 1-pixel line centred to within 0.1 px, in ggplot2, tmap, matplotlib
    and plotnine.
  - Rasterised as a PDF viewer draws it, the SVG has no seam at tile edges
    and matches the PNG.

## 0.3.1 (2026-11-24)

Maps are sharp again at any dpi. When an image's size at its dpi was not a
whole number of CSS pixels (720 px at 168 dpi is 411.43), 0.3.0 drew it one
pixel smaller and stretched it to the size asked for, which softened every
road edge and label by about a pixel. The map is now drawn a little larger on
the image's own pixel grid and cropped, so the pixels are never resampled; a
Web Mercator plot panel is the same image, pixel for pixel. Maps warped into
another CRS also sample their source exactly (they were off by up to a tenth
of a pixel), and raster tiles are laid on the same pixel grid as vector
layers at fractional pixel ratios.

## 0.3.0 (2026-11-23)

basemapper 0.3 replaces the 0.2 API with one model shared by Python, R and
Rust: a **basemap** of **styles**, whose **view** is any three of extent,
zoom, image size and dpi, rendered to a **rendering** or placed in a plot.
Every rule lives in the Rust core. Removed names and arguments are errors
that name their replacement: see
[Migrating to 0.3](https://ar-puuk.github.io/basemapper/migrating-to-0.3.html)
for every change.

### Breaking changes

- `m.render()` returns a `Rendering` (pixels, bounds, CRS, zoom, dpi,
  scale, notes): `save()`, `to_png()`, `to_numpy()` (a read-only view, no
  copy) and `to_xarray()`. `render_png()`, `render_array()` and
  `render_basemap_raw()` are removed.
- `basemap(style, *, bounds, center, zoom, width, height, units, dpi, crs,
  clip, fetch)`: `width` / `height` are the image's size and keyword-only;
  `pixel_ratio` is `dpi` (96 x the ratio). Any three of extent, zoom, size
  and dpi give the fourth; giving all four is an error.
- `Basemap` is immutable: `set_view()` and `set_bounds()` return a new one.
  The view is read from properties (`bounds`, `center`, `zoom`, `size`,
  `dpi`, `crs`, `scale`, `provenance`, `notes`); the `get_*()` methods are
  removed (`basemapper.get_attribution(m)` stays).
- Styles are values: `style(source, layers=, features=, key=)`, with the
  source helpers `provider()`, `tiles()`, `wms()` and `wmts()`. Filters and
  keys belong to each style of a stack. `basemap_provider()`,
  `tile_style()`, `wms_style()` and `wmts_style()` are removed, as are the
  `labels`, `layers`, `features`, `auth_token` and `api_key` arguments
  (labels are the feature `"labels"`: `features="-labels"`).
- `crs` is the map's CRS. Numbers in `bounds` are always longitude /
  latitude; projected coordinates come as a GeoDataFrame or GeoSeries.
- `m.add_to(ax)` places a basemap in matplotlib axes and returns its
  artist; `add_basemap()` builds one and does the same (0.2 returned an
  xarray of the first render: use `artist.rendering.to_xarray()`). Plot
  layers take `zoom` (the MapLibre zoom) in place of `tile_zoom`.
- The 0.2 download arguments (`tile_timeout_ms`, `max_tiles`,
  `tile_concurrency`, `fail_on_tile_error`) are removed: use
  `fetch=FetchOptions()`.

### New features

- Maps in any CRS: `crs`, or a projected GeoDataFrame as `bounds`, draws
  the map in that CRS, warped from Web Mercator (with pyproj).
- `clip=layer` draws only inside a layer's polygons; `clip=outside(layer)`
  only outside.
- `around(center, span, units)`: an area of a ground distance.
- A basemap is a plotnine layer itself: `ggplot(df) + basemap(...)`.
- Basemaps and renderings show themselves in Jupyter, at their CSS size.
- Notes: what basemapper decided, adjusted or could not draw. Decisions go
  to the `basemapper` logger; adjustments are `BasemapAdjustment` warnings;
  problems (failed tiles, missing glyphs, unsupported style features,
  filter names that match nothing) are `BasemapProblem` warnings, every
  time, and errors with `FetchOptions(on_error="fail")`. They were logged
  or dropped in 0.2.
- A style is checked when it is made (unknown names, missing keys, bad
  filters), and a basemap that cannot go in a plot is an error when added.

### Bug fixes

- Plots are aligned in their CRS. 0.2 stretched the Web Mercator image over
  the axes, which misplaced it outside Web Mercator (by tens of pixels on a
  longitude / latitude plot). The basemap is now warped into the axes' CRS
  in matplotlib and plotnine.
- A box crossing the date line (west > east) renders as one map.
- In PDF and SVG output the basemap is drawn at 300 dpi; on screen at 96 x
  the screen's pixel ratio.
- A view never reaches past the poles, as in MapLibre: when the world is
  smaller than the view the zoom rises until the world fills it, and a
  centre near a pole moves so the view stops at it.
- SVG text reads fonts with skrifa (Google Fonts' fontations) instead of
  the unmaintained ttf-parser; the SVGs written are the same.

## 0.2.0 (2026-11-23)

### New interface for download settings

- New `FetchOptions` gathers how tiles download: `timeout` (now in
  seconds), `max_tiles`, `concurrency` and `on_error` (`"skip"` or
  `"fail"`). `basemap()`, `Basemap`, `render_basemap_raw()`,
  `add_basemap()` and `geom_basemap()` take it as
  `fetch=FetchOptions(...)`, which shortens their argument lists.
- `tile_timeout_ms`, `max_tiles`, `tile_concurrency` and
  `fail_on_tile_error` are deprecated: they still work, with a
  `DeprecationWarning` saying what to write instead. An unknown keyword
  is a `TypeError`.
- `add_basemap()` and `geom_basemap()` take their options as keywords only
  (`add_basemap(ax, style, alpha=0.8)`).
- `labels` also takes `"all"` and `"none"`, as in R (`True` and `False`
  still work).
- matplotlib and xarray are now optional: `pip install
  'basemapper[matplotlib]'` (or `[plotnine]`, `[all]`) for `add_basemap()`
  and `geom_basemap()`, which explain what to install if they are missing.
  Rendering images and SVG needs only numpy and pyproj.

### Performance

- Rendering the same view again (a figure redrawn, a notebook re-run) no
  longer downloads the style, TileJSON, glyphs, sprite or PMTiles
  directories: they are kept in memory for as long as the server's caching
  headers allow (at most an hour; never for servers on the local machine,
  as for the disk cache). Decoded tiles are kept too, so a repeat render
  makes no requests and skips decoding.
- Tiles are decoded on all cores, raster layers and raster basemaps are
  resized in parallel, and the styles of a stacked basemap (`"UGRC.Lite"`)
  download and render concurrently. Connections and TLS sessions are reused
  across renders.

### Bug fixes

- `max_tiles` now limits vector tiles too. It used to be checked against the
  raster tile grid only, so a large vector view (or `render_svg()`, or a
  style's extra raster, hillshade or vector sources) could fetch many more
  tiles than the limit. Raster layers in vector styles drop to coarser tiles
  to fit, as raster basemaps do; vector sources that need more tiles than
  the limit raise `ValidationError`.
- An invalid `pixel_ratio` or `tile_zoom` is reported as such, not as an
  "invalid bbox".
- PMTiles archives honour `fail_on_tile_error` and `tile_concurrency`.
- A broken sprite index is skipped instead of stopping the render.

### Security

- Access tokens (`auth_token`) are sent only to the origin the style came
  from (and to the tile servers a TileJSON from there names), no longer to
  every server a style mentions, such as a third-party overlay. Styles given
  as JSON or a local file are yours: their servers all get the token, as
  before.
- API keys in URLs (`key=`, `api_key=`, `access_token=`, UGRC's quad word,
  ...) are replaced by `***` in error messages and logs.
- Downloads have size limits, counted after decompression (tiles 32 MB,
  styles 64 MB, GeoJSON 512 MB, glyphs, sprites and fonts 32 MB), and
  gzip-compressed tiles may inflate to at most 64 MB.
- A style downloaded from the web can no longer read local PMTiles files.
- Downloaded fonts are cached in the tile cache's folder, so
  `BASEMAPPER_CACHE_DIR=off` turns the font cache off too.

### Documentation

- The install instructions name no version: they install the latest
  release, and `@dev` the development version.

## 0.1.1 (2026-11-22)

- The install instructions in the README, the get-started guide and the
  site name the release (`...basemapper@v0.1.1#subdirectory=py-basemapper`),
  so an install gets a tested version rather than the development branch.
- The guide's pages run on every documentation build; their results are no
  longer kept in the repository.
- No change to the package's code.

## 0.1.0 (2026-11-22)

The first numbered release.

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
