# basemapper: Headless Spatial Basemap Renderer

Renders MapLibre GL styled basemaps (vector and raster tiles) headlessly
with a Rust core: a software port of MapLibre GL JS's renderer, tested
pixel by pixel against MapLibre GL JS. Renders at any fractional zoom
and pixel ratio to PNG, rasters and arrays, or to true vector SVG;
several styles can be stacked. Integrates with ggplot2 via
geom_basemap() and with tmap v4 via tm_basemap(), rendering at the
output device's resolution.

## See also

Useful links:

- <https://ar-puuk.github.io/basemapper/r/>

- <https://github.com/ar-puuk/basemapper>

- Report bugs at <https://github.com/ar-puuk/basemapper/issues>

## Author

**Maintainer**: Pukar Bhandari <arpuuuk@gmail.com>

Authors:

- Pukar Bhandari <arpuuuk@gmail.com>
