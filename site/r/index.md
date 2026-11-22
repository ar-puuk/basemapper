# basemapper

> **Also available for Python** — the same Rust core, and the same
> functions and argument names (`verb(m, ...)` here is `m.verb(...)` in
> Python). [Python documentation
> →](https://ar-puuk.github.io/basemapper/python/)

**basemapper** renders MapLibre GL basemaps (vector and raster styles)
headlessly with a Rust core: to PNG, arrays, {terra} rasters or true
vector SVG, at any fractional zoom and pixel ratio, and as {ggplot2} and
{tmap} v4 layers that render at the output device’s resolution.

## Installation

### Prerequisites

The package compiles a Rust extension during installation. Install the
dependencies for your platform before proceeding.

#### Windows

``` powershell
# 1. RTools45 (compiler toolchain required by R on Windows)
winget install RProject.Rtools

# 2. Rust toolchain manager
winget install Rustlang.Rustup
```

After installation, **open a new terminal** so both tools are on your
`PATH`, then add the Windows GNU target that RTools45 uses:

``` powershell
rustup target add x86_64-pc-windows-gnu
```

#### macOS / Linux

``` bash
# Rust toolchain manager (installs rustc and cargo)
curl https://sh.rustup.rs -sSf | sh
```

macOS users also need the Xcode Command Line Tools
(`xcode-select --install`) if not already present.

### Install the package

\
`# Any one of these; each compiles the Rust core from source.`\
`pak``::`[`pak`](https://pak.r-lib.org/reference/pak.html)`(``"ar-puuk/basemapper/r-basemapper"``)`\
`remotes``::`[`install_github`](https://remotes.r-lib.org/reference/install_github.html)`(``"ar-puuk/basemapper"``, subdir ``=`` ``"r-basemapper"``)`\
`devtools``::``install_github``(``"ar-puuk/basemapper"``, subdir ``=`` ``"r-basemapper"``)`

## Quick example

\
[`library`](https://rdrr.io/r/base/library.html)`(`[`basemapper`](https://ar-puuk.github.io/basemapper/r/)`)`\
\
`# Built-in basemaps by leaflet name (list_providers(), providers$...);`\
`# the name works as a style`\
`style`` ``<-`` ``"OpenFreeMap.Liberty"`\
`style`` ``<-`` `[`basemap_provider`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)`(``"Esri.WorldGrayCanvas"``, type ``=`` ``"raster"``)``   ``# pick the form`\
`style`` ``<-`` `[`basemap_provider`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)`(``"OpenFreeMap.Liberty"``, tiles ``=`` ``"data/omt.pmtiles"``)``   ``# your own tiles`\
\
`# Stack parts of basemaps: imagery under a vector basemap's labels`\
`hybrid`` ``<-`` `[`c`](https://rdrr.io/r/base/c.html)`(``"Esri.WorldImagery"``, `[`basemap_provider`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)`(``"OpenFreeMap.Liberty"``, labels ``=`` ``"only"``)``)`\
`# labels = FALSE / "only", features = c("water", "roads") / "-buildings"`\
\
`# A view: centre + zoom, or bounds to fit`\
`m`` ``<-`` `[`basemap`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)`(``style``, ``800``, ``600``, center ``=`` `[`c`](https://rdrr.io/r/base/c.html)`(``-``122.45``, ``37.75``)``, zoom ``=`` ``12``, pixel_ratio ``=`` ``2``)`\
[`render_png`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)`(``m``, path ``=`` ``"sf.png"``)``     ``# 1600 x 1200`\
[`render_svg`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)`(``m``, path ``=`` ``"sf.svg"``)``     ``# true vector`\
[`get_bounds`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md)`(`[`set_view`](https://ar-puuk.github.io/basemapper/r/reference/set_view.md)`(``m``, zoom ``=`` ``13``)``, lonlat ``=`` ``TRUE``)`\
\
`# One-shot render of bounds to a georeferenced [height, width, 4] array`\
`a`` ``<-`` `[`render_basemap_raw`](https://ar-puuk.github.io/basemapper/r/reference/render_basemap_raw.md)`(`[`c`](https://rdrr.io/r/base/c.html)`(``-``122.5``, ``37.7``, ``-``122.4``, ``37.8``)``, ``400``, ``300``, ``style``)`\
\
`# Your own tile data works as a style: tile URLs, TileJSON, PMTiles`\
`# archives (URL or local file), ArcGIS services`\
`m`` ``<-`` `[`basemap`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)`(``"data/area.pmtiles"``, ``800``, ``600``, bounds ``=`` `[`c`](https://rdrr.io/r/base/c.html)`(``-``122.5``, ``37.7``, ``-``122.4``, ``37.8``)``)`\
`style`` ``<-`` `[`tile_style`](https://ar-puuk.github.io/basemapper/r/reference/tile_style.md)`(``"https://example.com/{z}/{x}/{y}.png"``, maxzoom ``=`` ``16``)``   ``# with overrides`\
`style`` ``<-`` `[`wms_style`](https://ar-puuk.github.io/basemapper/r/reference/ogc_styles.md)`(``"https://gis.example.com/wms"``, layers ``=`` ``"imagery"``)`

## Get started

See the [Getting Started
vignette](https://ar-puuk.github.io/basemapper/r/articles/getting-started.md)
for a walkthrough of rendering basemaps with {ggplot2}, {tmap}, and raw
RGBA output.

## Related

- [Python docs](https://ar-puuk.github.io/basemapper/python/) — the
  Python package exposes the same Rust core via PyO3.
