# basemapper <img src="logo.svg" align="right" height="139" alt="" />

<!-- badges: start -->
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.24457061.svg)](https://doi.org/10.5281/zenodo.24457061)
<!-- badges: end -->

**basemapper** renders MapLibre GL basemaps (vector and raster styles)
headlessly with a Rust core: to PNG, NumPy arrays, xarray or true vector
SVG, at any fractional zoom and dpi, in any CRS, and as matplotlib and
plotnine layers that line up with your data and render at the output
resolution.

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

With pip:

```sh
pip install "basemapper @ git+https://github.com/ar-puuk/basemapper#subdirectory=py-basemapper"
```

These install the latest release: the `main` branch, which changes only when a
release is made ([all releases](https://github.com/ar-puuk/basemapper/releases)).
For the development version, put `@dev` after the repository URL.

With [uv](https://docs.astral.sh/uv/), into a project or the current
environment:

```sh
uv add "basemapper @ git+https://github.com/ar-puuk/basemapper#subdirectory=py-basemapper"
uv pip install "basemapper @ git+https://github.com/ar-puuk/basemapper#subdirectory=py-basemapper"
```

With conda (or mamba), conda-forge provides the dependencies and the Rust
toolchain, and pip builds basemapper from GitHub inside the environment:

```sh
conda create -n basemapper -c conda-forge python numpy pyproj matplotlib xarray rust pip
conda activate basemapper
pip install "basemapper @ git+https://github.com/ar-puuk/basemapper#subdirectory=py-basemapper"
```

or as an `environment.yml` (`conda env create -f environment.yml`):

```yaml
name: basemapper
channels: [conda-forge]
dependencies:
  - python>=3.9
  - numpy
  - pyproj
  - matplotlib
  - xarray
  - rust
  - pip
  - pip:
      - "basemapper @ git+https://github.com/ar-puuk/basemapper#subdirectory=py-basemapper"
```

For the plot layers, install with an extra in place of plain `basemapper`:
`basemapper[matplotlib]` for `m.add_to(ax)`, `basemapper[plotnine]` for
plotnine too (or `basemapper[all]`), e.g.
`pip install "basemapper[matplotlib] @ git+https://github.com/ar-puuk/basemapper#subdirectory=py-basemapper"`.

> A PyPI release is planned once the package stabilises.

## Quick example

```python
import basemapper as bm

# A style plus what you know of the map; any three of extent, zoom, image
# size and dpi give the fourth
m = bm.basemap("OpenFreeMap.Liberty", center=(-122.45, 37.75), zoom=12, dpi=192)
m                                  # the view: what was given, derived or defaulted
r = m.render()                     # 1600 x 1200: the default 800 x 600 view, sharper
r.save("sf.png")
a = r.to_numpy()                   # (1200, 1600, 4) uint8
m.render_svg("sf.svg")             # true vector
m.set_view(zoom=13).bounds

# An area: bounds (longitude / latitude, or a GeoDataFrame) or around()
bm.basemap("OpenFreeMap.Liberty", bounds=bm.around((-122.45, 37.75), (10, 6), units="km"))

# Styles: a catalogue name, a MapLibre style, or your own tiles; filters and
# keys belong to the style; a list is stacked bottom to top
hybrid = ["Esri.WorldImagery", bm.style("OpenFreeMap.Liberty", features="labels")]
quiet = bm.style("OpenFreeMap.Liberty", features=["-labels", "-poi"])
own = bm.style(bm.provider("OpenFreeMap.Liberty", tiles="https://tiles.openfreemap.org/planet"))
relief = bm.style(bm.wms("https://ows.terrestris.de/osm/service", layers="SRTM30-Colored-Hillshade"))

# In a plot: the axes give the extent, size and dpi; give their CRS
import geodatasets, geopandas as gpd, matplotlib.pyplot as plt
nc = gpd.read_file(geodatasets.get_path("geoda.sids"))
fig, ax = plt.subplots()
nc.boundary.plot(ax=ax, color="black")
bm.basemap(quiet).add_to(ax, crs=nc.crs)
```

Coming from 0.2? See [Migrating to 0.3](https://ar-puuk.github.io/basemapper/migrating-to-0.3.html).

## Get started

See the [documentation](https://ar-puuk.github.io/basemapper/python/) for a
walkthrough: views, renderings, CRSs, clipping, and basemaps in matplotlib
and plotnine.

## Related

- [R docs](https://ar-puuk.github.io/basemapper/r/) — the R package exposes
  the same Rust core via extendr.
