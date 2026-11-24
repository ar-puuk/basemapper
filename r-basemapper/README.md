# basemapper <img src="man/figures/logo.svg" align="right" height="139" alt="" />

<!-- badges: start -->
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.24457061.svg)](https://doi.org/10.5281/zenodo.24457061)
<!-- badges: end -->

> **Also available for Python** — the same Rust core, and the same functions
> and argument names (`verb(m, ...)` here is `m.verb(...)` in Python).
> [Python documentation →](https://ar-puuk.github.io/basemapper/python/)

**basemapper** renders MapLibre GL basemaps (vector and raster styles)
headlessly with a Rust core: to PNG, arrays, {terra} rasters or true vector
SVG, at any fractional zoom and dpi, in any CRS, and as {ggplot2} and {tmap}
v4 layers that line up with your data and render at the output device's
resolution.

## Installation

### Prerequisites

The package compiles a Rust extension during installation. Install the
dependencies for your platform before proceeding.

#### Windows

```powershell
# 1. RTools45 (compiler toolchain required by R on Windows)
winget install RProject.Rtools

# 2. Rust toolchain manager
winget install Rustlang.Rustup
```

After installation, **open a new terminal** so both tools are on your `PATH`,
then add the Windows GNU target that RTools45 uses:

```powershell
rustup target add x86_64-pc-windows-gnu
```

#### macOS / Linux

```bash
# Rust toolchain manager (installs rustc and cargo)
curl https://sh.rustup.rs -sSf | sh
```

macOS users also need the Xcode Command Line Tools (`xcode-select --install`)
if not already present.

### Install the package

```r
# Any one of these installs the latest release; each compiles the Rust core.
pak::pak("ar-puuk/basemapper/r-basemapper")
remotes::install_github("ar-puuk/basemapper", subdir = "r-basemapper")
devtools::install_github("ar-puuk/basemapper", subdir = "r-basemapper")
```

These install the latest release: the `main` branch, which changes only when a
release is made ([all releases](https://github.com/ar-puuk/basemapper/releases)).
For the development version, install from the `dev` branch: `pak::pak("ar-puuk/basemapper/r-basemapper@dev")`.

## Quick example

```r
library(basemapper)

# A style plus what you know of the map; any three of extent, zoom, printed
# size and symbol scale give the fourth; dpi is sharpness only
m <- basemap("OpenFreeMap.Liberty", center = c(-122.45, 37.75), zoom = 12, dpi = 192)
m                                  # the view: what was given, derived or defaulted
r <- render(m)                     # 1600 x 1200: the default 800 x 600 view, sharper
write_png(r, "sf.png")
a <- as_array(r)                   # [1200, 1600, 4], with bounds and CRS
render_svg(m, path = "sf.svg")     # true vector
get_bounds(set_view(m, zoom = 13))

# An area: bounds (longitude / latitude, or an sf object) or around()
basemap("OpenFreeMap.Liberty", bounds = around(c(-122.45, 37.75), c(10, 6), units = "km"))

# Styles: a catalogue name, a MapLibre style, or your own tiles; filters and
# keys belong to the style; a list is stacked bottom to top
hybrid <- list("Esri.WorldImagery", style("OpenFreeMap.Liberty", features = "labels"))
quiet  <- style("OpenFreeMap.Liberty", features = c("-labels", "-poi"))
own    <- style(provider("OpenFreeMap.Liberty", tiles = "https://tiles.openfreemap.org/planet"))
relief <- style(wms("https://ows.terrestris.de/osm/service", layers = "SRTM30-Colored-Hillshade"))

# In a plot: the plot gives the extent, CRS, size and dpi
library(ggplot2)
nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
ggplot(nc) + basemap(quiet) + geom_sf(fill = NA)
```

Coming from 0.2? See [Migrating to 0.3](https://ar-puuk.github.io/basemapper/migrating-to-0.3.html).

## Get started

See the [Getting Started vignette](articles/getting-started.html) for a
walkthrough: views, renderings, CRSs, clipping, and basemaps in {ggplot2}
and {tmap}.

## Related

- [Python docs](https://ar-puuk.github.io/basemapper/python/) — the Python
  package exposes the same Rust core via PyO3.
