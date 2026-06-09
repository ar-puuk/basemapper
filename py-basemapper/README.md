# basemapper <img src="logo.svg" align="right" height="139" alt="" />

**basemapper** is a headless spatial basemap renderer for Python. It fetches and
composites styled map tiles (raster or MapLibre GL vector styles) into
georeferenced RGBA arrays using a Rust core, and exposes the result through
native integrations with matplotlib, plotnine, and NumPy.

## Installation

Install from PyPI with:

```bash
pip install basemapper
```

Or install the development version directly from GitHub:

```bash
pip install "git+https://github.com/ar-puuk/basemapper.git#subdirectory=py-basemapper"
```

> **Requirements:** [Rust](https://rustup.rs) must be installed to build from
> source. The PyPI wheel ships a pre-compiled extension.

## Get started

See the [documentation](https://ar-puuk.github.io/basemapper/python/) for a
walkthrough of rendering basemaps with matplotlib, plotnine, and raw RGBA
output.

## Related

- [R docs](https://ar-puuk.github.io/basemapper/r/) — the R package exposes
  the same Rust core via extendr.
