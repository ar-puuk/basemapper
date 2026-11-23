The Rust core of [basemapper](https://github.com/ar-puuk/basemapper): render
MapLibre GL basemaps headlessly, with no browser, display server, GPU or GDAL.
Vector styles are drawn by a CPU port of MapLibre GL JS's renderer, tested
pixel by pixel against MapLibre GL JS; raster tiles, PMTiles archives, ArcGIS
services and OGC services work too. Images come out at any fractional zoom
and pixel ratio, as PNG, RGBA pixels or true vector SVG.

The R and Python packages are built on this crate and share its functions and
argument names: R's `verb(m, ...)` and Python's `m.verb(...)` are
[`Basemap`]'s `m.verb(...)` here.

```toml
[dependencies]
basemapper-core = { git = "https://github.com/ar-puuk/basemapper" }
```

Every example on this page is a program in the crate's `examples/` folder.
The documentation build runs them; the images and output shown are what they
produced. Run one yourself with
`cargo run --release --example guide_quickstart`.

