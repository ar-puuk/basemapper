# basemapper

**basemapper** is a headless spatial basemap renderer for R. It fetches
and composites styled map tiles (raster or MapLibre GL vector styles)
into georeferenced RGBA arrays using a Rust core, and exposes the result
through native integrations with ggplot2, tmap v4, and sf.

## Installation

Install from GitHub with:

``` r

# install.packages("remotes")
remotes::install_github("ar-puuk/basemapper", subdir = "r-basemapper")
```

> **Requirements:** [Rust](https://rustup.rs) and
> [RTools45](https://cran.r-project.org/bin/windows/Rtools/) (Windows
> only) must be installed before running `install_github()`.

## Get started

See the [Getting Started
vignette](https://ar-puuk.github.io/basemapper/r/articles/getting-started.md)
for a walkthrough of rendering basemaps with ggplot2, tmap, and raw RGBA
output.

## Related

- [Python docs](https://ar-puuk.github.io/basemapper/python/) — the
  Python package exposes the same Rust core via PyO3.
