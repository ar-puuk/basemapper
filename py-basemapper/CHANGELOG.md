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
  `{ratio}` and `{bbox-epsg-3857}` tokens. `wms_provider()` uses
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
- Style providers, implemented in the Rust core: `basemap_provider()`,
  `list_providers()`, `raster_provider()`, `vector_provider()`,
  `esri_raster_provider()`, `esri_vector_provider()`, `wms_provider()`,
  `wmts_provider()`, `pmtiles_provider()`.
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
