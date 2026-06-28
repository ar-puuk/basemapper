# basemapper <img src="man/figures/logo.svg" align="right" height="139" alt="" />

<!-- badges: start -->
<!-- badges: end -->

> **Also available for Python** — same Rust core, PyO3 bindings.
> [Python documentation →](https://ar-puuk.github.io/basemapper/python/)

**basemapper** is a headless spatial basemap renderer for R. It fetches and
composites styled map tiles (raster or MapLibre GL vector styles) into
georeferenced RGBA arrays using a Rust core, and exposes the result through
native integrations with {ggplot2}, {tmap} v4, and {sf}.

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
# install.packages("remotes")
remotes::install_github("ar-puuk/basemapper", subdir = "r-basemapper")
```

## Quick example

```r
library(basemapper)

# 25 built-in providers — no tile URL needed
style <- basemap_provider("OpenFreeMap.liberty")
style <- basemap_provider("Carto.DarkMatter")
style <- basemap_provider("OSM.Standard")

# OGC WMS source
style <- wms_provider("https://gis.example.com/wms", layers = "imagery")

# PMTiles archive (HTTP range requests — no tile server required)
style <- pmtiles_provider("https://example.com/world.pmtiles")

# Render — anti-meridian bboxes (xmin > xmax) are handled automatically
m <- render_basemap_raw(
  bbox        = c(-122.5, 37.7, -122.4, 37.8),
  width       = 400L,
  height      = 300L,
  style_input = style
)
```

## Get started

See the [Getting Started vignette](articles/getting-started.html) for a
walkthrough of rendering basemaps with {ggplot2}, {tmap}, and raw RGBA output.

## Related

- [Python docs](https://ar-puuk.github.io/basemapper/python/) — the Python
  package exposes the same Rust core via PyO3.
