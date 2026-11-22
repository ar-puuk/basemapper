# Raster and vector basemaps, as PNG and SVG

A basemap is drawn from raster tiles (images, like OpenStreetMap's) or from
vector tiles (geometry styled on the fly, like OpenFreeMap's). Either renders
to a PNG ([`Basemap::render_png`]) or an SVG ([`Basemap::render_svg`]). In an
SVG, vector layers become paths and labels editable text with the style's
fonts embedded (or outlines; see [`SvgOptions`]); raster tiles are embedded as
images.

