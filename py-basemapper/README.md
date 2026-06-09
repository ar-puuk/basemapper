# basemapper <img src="logo.svg" align="right" height="139" alt="" />

**basemapper** is a headless spatial basemap renderer for Python. It fetches and
composites styled map tiles (raster or MapLibre GL vector styles) into
georeferenced RGBA arrays using a Rust core, and exposes the result through
native integrations with matplotlib, plotnine, and NumPy.

> **Also available for R** — same Rust core, extendr bindings.
> [R documentation →](https://ar-puuk.github.io/basemapper/r/)

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

## Get started

See the [documentation](https://ar-puuk.github.io/basemapper/python/) for a
walkthrough of rendering basemaps with matplotlib, plotnine, and raw RGBA
output.

## Related

- [R docs](https://ar-puuk.github.io/basemapper/r/) — the R package exposes
  the same Rust core via extendr.
