# Changelog

## Unreleased (0.1.0)

First version, not yet released.

### Rendering

- Vector styles are drawn by an in-house CPU port of MapLibre GL JS's
  renderer, tested pixel by pixel against MapLibre GL JS (168 reference
  views). No GPU, browser or display is needed.
- Any fractional zoom and pixel ratio (MapLibre's devicePixelRatio); styles
  can be stacked bottom to top; true vector SVG output (paths and glyph
  outlines); anti-meridian views render world copies.

### API

- `basemapper.Map(style | [styles], width, height, ratio, center=, zoom=,
  bounds=, crs=)` with `set_center()`, `set_zoom()`, `set_bounds()`,
  `bounds`, `bounds_lonlat`, and `render_png()`, `render_svg()`,
  `render_array()`, `render_buffer()`.
- `render_styles_raw()`: low-level render of one or several styles over a
  bbox at a pixel ratio; `render_basemap_raw()` gains `pixel_ratio`.
- Style providers: `basemap_provider()`, `RasterProvider`, `VectorProvider`,
  `EsriVectorProvider`, `EsriRasterProvider`, `wms_provider()`,
  `wmts_provider()`, `pmtiles_provider()`; `list_layers()`.

### Plot layers

- `add_basemap()` adds a `BasemapImage` artist that renders at draw time for
  the axes' extent and output resolution: `savefig(dpi=300)` and PDF output
  get a basemap rendered natively, and panning re-renders. `ratio` defaults
  to dpi / 96; `crs=` for axes without a CRS; style lists stack.
- plotnine `geom_basemap()` uses the same artist (works with plotnine 0.15);
  `crs=` (default EPSG:4326) and `ratio=`.
