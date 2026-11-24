
<img src="../img/quickstart.png" alt="OpenFreeMap Positron over Salt Lake City" width="720">

[`Basemap::render`] returns a [`Rendering`]: the pixels (straight-alpha RGBA)
with their bounds, CRS, zoom, dpi and scale, and [`notes`](Rendering::notes)
on what was decided (the default dpi above), adjusted, or could not be done.
[`Basemap::view`] resolves the view without rendering, and
[`Basemap::render_svg`] draws an SVG.

