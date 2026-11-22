# basemapper <img src="logo.svg" align="right" height="139" alt="" />

**basemapper** renders MapLibre GL basemaps (vector and raster styles)
headlessly with a Rust core: to PNG, NumPy arrays or true vector SVG, at any
fractional zoom and pixel ratio, and as matplotlib and plotnine layers that
render at the output resolution.

> **Also available for R** — the same Rust core, and the same functions and
> argument names (R's `verb(m, ...)` is `m.verb(...)` here).
> [R documentation](https://ar-puuk.github.io/basemapper/r/)

## Installation

### Prerequisites

The package compiles a Rust extension during installation. Install Rust for
your platform before proceeding.

#### Windows

```powershell
winget install Rustlang.Rustup
```

After installation, **open a new terminal** so `rustup` and `cargo` are on
your `PATH`.

#### macOS

```bash
brew install rustup
rustup-init
```

Or without Homebrew:

```bash
curl https://sh.rustup.rs -sSf | sh
```

#### Linux

```bash
curl https://sh.rustup.rs -sSf | sh
```

### Install the package

Install the development version from GitHub:

```bash
pip install "git+https://github.com/ar-puuk/basemapper.git#subdirectory=py-basemapper"
```

> A PyPI release is planned once the package stabilises.

## Quick example

```python
import basemapper
from basemapper import basemap_provider, tile_style, wms_style

# 25 built-in providers (basemapper.list_providers()) — no tile URL needed
style = basemap_provider("OpenFreeMap.liberty")

# A view: centre + zoom, or bounds to fit
m = basemapper.basemap(style, 800, 600, center=(-122.45, 37.75), zoom=12, pixel_ratio=2)
m.render_png(path="sf.png")        # 1600 x 1200
m.render_svg(path="sf.svg")        # true vector
m.set_view(zoom=13).get_bounds(lonlat=True)

# One-shot render of bounds to a (height, width, 4) uint8 array
a = basemapper.render_basemap_raw([-122.5, 37.7, -122.4, 37.8], 400, 300, style)

# Your own tile data works as a style: tile URLs, TileJSON, PMTiles
# archives (URL or local file), ArcGIS services
m = basemapper.basemap("data/area.pmtiles", 800, 600, bounds=[-122.5, 37.7, -122.4, 37.8])
style = tile_style("https://example.com/{z}/{x}/{y}.png", maxzoom=16)   # with overrides
style = wms_style("https://gis.example.com/wms", layers="imagery")
```

## Get started

See the [documentation](https://ar-puuk.github.io/basemapper/python/) for a
walkthrough of rendering basemaps with matplotlib, plotnine, and raw RGBA
output.

## Related

- [R docs](https://ar-puuk.github.io/basemapper/r/) — the R package exposes
  the same Rust core via extendr.
