# Render a basemap view

- `render_png()` writes or returns a PNG.

- `render_svg()` writes or returns a vector SVG document (`width` x
  `height` user units; `pixel_ratio` does not apply).

- `render_array()` returns an integer array `[height, width, 4]` (RGBA,
  0–255) with the extent as attributes, like
  [`render_basemap_raw()`](https://ar-puuk.github.io/basemapper/r/reference/render_basemap_raw.md).

- `render_raster()` (R only) returns a
  [`terra::rast()`](https://rspatial.github.io/terra/reference/rast.html)
  (EPSG:3857, layers red, green, blue, alpha).

## Usage

``` r
render_png(m, path = NULL)

render_svg(
  m,
  path = NULL,
  text = c("editable", "bitmap", "paths"),
  embed_fonts = c("subset", "full", "none"),
  font_dirs = NULL
)

render_array(m)

render_raster(m)
```

## Arguments

- m:

  A
  [`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)
  object.

- path:

  Output file, or `NULL` to return the result.

- text:

  For `render_svg()`: `"editable"` writes labels as `<text>` elements
  (editable, selectable) at MapLibre's glyph positions, halos as
  strokes, with the style's fonts embedded, and falls back to `"bitmap"`
  for labels whose font file is not available; `"bitmap"` draws every
  label from the style's own glyph bitmaps, thresholded by SVG filters
  as MapLibre's shader does (MapLibre's look at any zoom, not editable);
  `"paths"` writes glyph outlines traced from those bitmaps (plain
  paths).

- embed_fonts:

  For `render_svg()`: `"subset"` (only the glyphs used), `"full"` (whole
  fonts, for retyping labels) or `"none"` (fonts by name only). Fonts
  come from `font_dirs`, the `BASEMAPPER_FONT_DIRS` folders, a local
  cache, then Google Fonts for open fonts; others are referenced by
  name.

- font_dirs:

  For `render_svg()`: folders of .ttf / .otf files to use first.

## Value

`render_png()`: the PNG as a raw vector (invisibly when written).
`render_svg()`: the SVG as a string (invisibly when written).

## Details

Image sizes are `width * pixel_ratio` by `height * pixel_ratio` device
pixels. Python: `m.render_png()`, `m.render_svg()`, `m.render_array()`.
