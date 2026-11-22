## Basemap.render_svg()


Render an SVG (`width` x `height` user units; `pixel_ratio`


Usage


``` python
Basemap.render_svg(
    path=None, text="editable", embed_fonts="subset", font_dirs=None
)
```


does not apply), written to `path` when given: vector layers as paths and text, raster tiles (raster basemaps, hillshade) as embedded images. Returns the SVG text.


## Parameters


`path: Optional[str] = None`  
Output file, or None.

`text: str = ``"editable"`  
`"editable"`: labels are `<text>` elements (editable, selectable) at MapLibre's glyph positions, halos as strokes, with the style's fonts embedded; labels whose font file is not available fall back to `"bitmap"`. `"bitmap"`: every label drawn from the style's own glyph bitmaps, thresholded by SVG filters as MapLibre's shader does (MapLibre's look at any zoom; not editable). `"paths"`: glyph outlines traced from those bitmaps (plain paths, no filters).

`embed_fonts: str = ``"subset"`  
`"subset"` (only the glyphs used), `"full"` (whole fonts, for retyping labels) or `"none"` (fonts by name only). Fonts come from `font_dirs`, the `BASEMAPPER_FONT_DIRS` folders, a local cache, then Google Fonts for open fonts; others are referenced by name.

`font_dirs: Optional[List[str]] = None`  
Folders of .ttf / .otf files to use first.
