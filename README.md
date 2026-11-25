# basemapper

<!-- badges: start -->
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.24457061.svg)](https://doi.org/10.5281/zenodo.24457061)
<!-- badges: end -->

A headless spatial basemap renderer for **R** and **Python**.

basemapper renders styled basemaps (MapLibre / Mapbox GL styles, raster XYZ,
WMS, WMTS, PMTiles, ESRI vector tiles) with a Rust core, to PNG, arrays and
rasters, or true vector SVG, and adds them to **ggplot2**, **tmap**,
**matplotlib** and **plotnine** plots. No browser, GPU, display server or
GDAL is involved.

Basemaps are rendered for the exact view and output resolution: any
fractional zoom, any dpi (a 300 dpi figure gets a basemap rendered at
300 dpi, not upscaled tiles), and several styles can be stacked (e.g. a
hillshade, base and labels published as separate services).

Vector styles are drawn by basemapper's own CPU renderer, a software port of
MapLibre GL JS's rendering pipeline, verified pixel by pixel against MapLibre
GL JS (see [How rendering works](docs/rendering-engine.md)).

📖 [R documentation](https://ar-puuk.github.io/basemapper/r/) ·
🐍 [Python documentation](https://ar-puuk.github.io/basemapper/python/)

---

## Install

### R

```r
# Any one of these installs the latest release; each compiles the Rust core.
pak::pak("ar-puuk/basemapper/r-basemapper")
remotes::install_github("ar-puuk/basemapper", subdir = "r-basemapper")
devtools::install_github("ar-puuk/basemapper", subdir = "r-basemapper")
```

These install the latest release: the `main` branch, which changes only when a
release is made ([all releases](https://github.com/ar-puuk/basemapper/releases)).
For the development version, install from the `dev` branch: `pak::pak("ar-puuk/basemapper/r-basemapper@dev")`.

Building from source needs a [Rust toolchain](https://rustup.rs) and, on
Windows, [RTools45](https://cran.r-project.org/bin/windows/Rtools/) plus the
GNU target: `rustup target add x86_64-pc-windows-gnu`.

Depends on: `ggplot2 (>= 3.4)`, `cli`, `jsonlite (>= 1.8)`,
`rlang`, `grid`.
Optional: `sf (>= 1.0)` for bounds in other CRSs and `sf` objects,
`terra (>= 1.7)` for `as_raster()`, `tmap (>= 4.0)` for `tm_basemap()`.

> CRAN and R-Universe packages are planned after an extended testing period.

### Python

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

Building from source needs a [Rust toolchain](https://rustup.rs) (conda's
`rust` package also works); pip and uv build the extension with maturin
automatically.

Depends on: `numpy`, `pyproj`.
Optional (extras): `matplotlib` for `m.add_to(ax)`, `xarray` for
`r.to_xarray()`, `plotnine >= 0.12` for plotnine plots.

> PyPI and conda packages are planned after an extended testing period.

### Rust

```toml
[dependencies]
basemapper-core = { git = "https://github.com/ar-puuk/basemapper" }
```

The crate the R and Python packages are built on; its documentation is the
[Rust guide](https://ar-puuk.github.io/basemapper/rust/). This follows the
latest release (the `main` branch; `Cargo.lock` keeps the commit you
built with until `cargo update`). For the development version, add
`branch = "dev"`.

---

## Quick start

The R, Python and Rust APIs are the same: one model, the same functions,
argument names, defaults and rules, all in the Rust core. R calls
`verb(m, ...)` where Python and Rust call `m.verb(...)`. The
[API reference](docs/api.md) is the side-by-side contract; each package's
test suite checks its functions and arguments against it
([`docs/api-contract.json`](docs/api-contract.json)), and the same views,
pixels and notes against shared cases
([`core/tests/fixtures/contract/`](core/tests/fixtures/contract/)).
Coming from 0.2? See [Migrating to 0.3](docs/migrating-to-0.3.md).

### A map on its own

A basemap is a style plus what you know of the map. Each setting has one
meaning: `zoom` is the detail, `symbol_scale` the size of lines and labels,
and `dpi` the sharpness. **Any three of extent, zoom, printed size and
symbol scale give the fourth**, and the dpi turns the printed size into
pixels. What nothing gives is defaulted (dpi 96, symbol scale 1, then 800 ×
600 px, then the world), and the zoom is always derived when not given. In a
plot, the plot gives the extent and size and the device the dpi: set `zoom`
or `symbol_scale`.

```r
library(basemapper)
# UGRC's Lite basemap: hillshade, base and labels services, stacked
m <- basemap("UGRC.Lite", center = c(-111.8887, 40.7556), zoom = 12.4, dpi = 192)
m                                 # the view: given, derived and default values
r <- render(m)                    # 1600 x 1200 px: the default 800 x 600 view, sharper
write_png(r, "slc.png")
a <- as_array(r)                  # [1200, 1600, 4] integer array, with bounds and CRS
x <- as_raster(r)                 # terra SpatRaster
render_svg(m, path = "slc.svg")   # true vector; labels as editable text, fonts embedded
```

```python
import basemapper as bm
m = bm.basemap("UGRC.Lite", center=(-111.8887, 40.7556), zoom=12.4, dpi=192)
r = m.render()
r.save("slc.png")
a = r.to_numpy()                  # (1200, 1600, 4) uint8
x = r.to_xarray()                 # georeferenced DataArray
m.render_svg("slc.svg")           # text="paths" for traced glyph outlines
```

```rust
use basemapper_core::{Basemap, Context};

let m = Basemap::new("UGRC.Lite")
    .center(-111.8887, 40.7556)
    .zoom(12.4)
    .dpi(192.0);
let r = m.render(&Context::Standalone)?;
std::fs::write("slc.png", r.to_png()?)?;
```

`width` / `height` are the image's size, in pixels (or inches, cm, mm with
`units`). `dpi` is sharpness: without a pixel size, a higher dpi gives the
same map with more pixels; with one, the image keeps its pixels and covers
less ground. An area instead of a
camera: `bounds =` (longitude / latitude, an `sf` object or GeoDataFrame,
or `around(center, span, units = "km")`), and the zoom that fits it is
derived. A layer in a projected CRS, or `crs =`, draws the map in that CRS
(warped from Web Mercator). `clip = layer` draws only inside its polygons,
`clip = outside(layer)` only outside. A basemap is a value:
`set_view()` / `set_bounds()` return a new one, and `get_bounds()`,
`get_center()`, `get_zoom()` and `get_size()` (Python: properties) read it.

`style` is a catalogue name, a MapLibre style (URL, file or JSON) or your
own tile data, which gets a style built for it: a tile URL, TileJSON,
PMTiles archive (URL or local file), ArcGIS service, WMS or WMTS. A list of
styles is stacked bottom to top.

### R — ggplot2

A basemap is a layer: the plot gives its extent, CRS, size and dpi, so it
lines up with the data in any CRS, and it is drawn again for each export.

```r
library(ggplot2)
library(sf)
library(basemapper)

nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)

ggplot(nc) +
  basemap("OpenFreeMap.Positron") +       # or geom_basemap("OpenFreeMap.Positron")
  geom_sf(fill = NA, colour = "steelblue")
```

### R — tmap

```r
library(tmap)
library(basemapper)   # after tmap: basemapper's tm_basemap() replaces tmap's

tm_shape(nc) +
  tm_basemap("OpenFreeMap.Positron") +
  tm_borders()
```

### Python — matplotlib

```python
import geodatasets
import geopandas as gpd
import matplotlib.pyplot as plt
import basemapper as bm

nc = gpd.read_file(geodatasets.get_path("geoda.sids"))
fig, ax = plt.subplots(figsize=(8, 3.6))
nc.boundary.plot(ax=ax, color="steelblue")

# Plain axes carry no CRS: give the data's (cartopy GeoAxes are read).
bm.basemap("OpenFreeMap.Positron").add_to(ax, crs=nc.crs)
plt.show()
```

### Python — plotnine

```python
from plotnine import geom_map, ggplot

ggplot(nc) + bm.basemap("OpenFreeMap.Positron") + geom_map(fill=None)
```

In a plot, a basemap may give a `zoom` (the detail) or a `symbol_scale`
(the size of lines and labels; each sets the other, so give one), a `clip`
(a mask) and `fetch` options; the plot gives the rest, and the device's dpi
is the sharpness only (a basemap's `dpi` is an error in a plot). `alpha` fades it, and `attribution` (on by
default) puts the providers' credit line in the panel's corner. The layer
functions `geom_basemap()`, `tm_basemap()` and `add_basemap()` take the
same arguments as `basemap()`. Plots render at the output resolution
(`ggsave(dpi = 300)`, `savefig(dpi=300)`; PDF and SVG at 300 dpi), and the
alignment is tested in every plotting library, in Web Mercator, longitude /
latitude and projected CRSs.

---

## Key functions

| What you want | R | Python | Rust (`basemapper_core`) |
|---|---|---|---|
| A basemap (style + view) | `basemap()` | `basemap()` → `Basemap` | `Basemap::new()`, `Basemap::stack()` |
| Change the view | `set_view()`, `set_bounds()` | `m.set_view()`, `m.set_bounds()` | `m.with_inputs()` |
| Read the view | `get_bounds()`, `get_center()`, `get_zoom()`, `get_size()`, `print()` | `m.bounds`, `m.center`, `m.zoom`, `m.size`, `m.dpi`, `m.provenance` | `m.view()` |
| An area by its ground size | `around()` | `around()` | `view::around()` |
| Render | `render()` → rendering | `m.render()` → `Rendering` | `m.render()` → `Rendering` |
| PNG, array, raster | `write_png()`, `as_array()`, `as_raster()` | `r.save()`, `r.to_numpy()`, `r.to_xarray()` | `r.to_png()`, `r.pixels` |
| Vector SVG | `render_svg()` | `m.render_svg()` | `m.render_svg()` |
| ggplot2 / plotnine layer | `ggplot(df) + m`, `geom_basemap()` | `ggplot(df) + m`, `geom_basemap()` | — |
| matplotlib layer | — | `m.add_to(ax)`, `add_basemap()` | — |
| tmap layer | `tm_basemap()` | — | — |
| A style with filters or a key | `style()` | `style()` | `Style::new()` |
| Sources | `provider()`, `tiles()`, `wms()`, `wmts()` | same names | `Source::{provider, tiles, wms, wmts}` |
| Named basemaps | `list_providers()`, `providers` | `list_providers()`, `providers` | `providers::list_providers()` |
| Inspect a style's layers (id, type, feature) | `list_layers()` | `list_layers()` | `list_style_layers()` |
| Clip to a layer | `clip = layer`, `outside()` | `clip=layer`, `outside()` | `Basemap::clip()`, `mask::Shape` |
| Download settings | `fetch_options()` | `FetchOptions` | `FetchOptions` |
| Tile cache | `tile_cache()` | `tile_cache()` | `cache::{dir, set_dir, clear}` |
| Credit line (for a caption) | `get_attribution()` | `get_attribution()` | `m.attribution()` |

---

## Date line and world copies

Plain bounds are longitude / latitude, and a box whose west is greater than
its east crosses the date line: it is drawn as one map, without a seam
(tested in R and Python). A view that extends past ±180° (a whole-world map
at low zoom) shows the wrapped world copy there, as MapLibre does.

```r
basemap("OpenStreetMap.Mapnik", bounds = c(150, -40, -150, 10), width = 800, height = 400)
```

```python
bm.basemap("OpenStreetMap.Mapnik", bounds=(150, -40, -150, 10), width=800, height=400)
```

---

## Labels, features and layers

Filters belong to a style, so stacked styles are filtered each on their
own. `features` keeps or (`-` prefixed) drops parts of the map: background,
water, landcover, parks, buildings, roads, rail, boundaries, places, poi,
terrain, and labels. `"labels"` keeps only the labels, on a transparent
background; `"-labels"` drops them; they combine (`c("roads", "labels")`
keeps the road labels). They work on any vector style, from tables for the
OpenMapTiles, Protomaps and Shortbread schemas and Esri's layer names;
catalogue raster basemaps switch to their label-free or label-only tiles.

```r
# R — imagery under a vector basemap's labels
basemap(list("Esri.WorldImagery", style("OpenFreeMap.Liberty", features = "labels")),
        center = c(-111.89, 40.76), zoom = 12)

# R — water and roads only, without their names
basemap(style("OpenFreeMap.Liberty", features = c("water", "roads", "-labels")), bounds = nc)
```

```python
# Python — same API
bm.basemap(["Esri.WorldImagery", bm.style("OpenFreeMap.Liberty", features="labels")],
           center=(-111.89, 40.76), zoom=12)
bm.basemap(bm.style("OpenFreeMap.Liberty", features=["water", "roads", "-labels"]), bounds=nc)
```

`list_layers(style)` shows each layer's id, type, source layer and
feature; given a filtered style, it previews what a render would draw. For
exact control, `layers` keeps or drops layers by id, with `*` and `?`
wildcards: `style("OpenFreeMap.Liberty", layers = c("-poi*", "-label_*"))`.
A filter that leaves nothing is an error; a name that matches no layer is a
warning, so a typo does not go unnoticed.

---

## Updating the documentation

The site at <https://ar-puuk.github.io/basemapper/> has four parts, built by
one script and assembled in `site/`:

| Part | Built by | From |
|---|---|---|
| Landing page (`site/index.html`) | copied | `docs/index.html` |
| R (`site/r/`) | pkgdown | `r-basemapper/` (reference from roxygen; articles in `vignettes/articles/`) |
| Python (`site/python/`) | great-docs (Quarto) | `py-basemapper/` (reference from docstrings; guide in `user_guide/`) |
| Rust (`site/rust/`) | rustdoc | `core/` (guide in `core/guide/`, its programs in `core/examples/guide_*.rs`) |

The articles, guide pages and Rust examples are **executed** during the
build: every map and plot on the site is the output of the code shown with
it. Building therefore needs network access (tiles are fetched from the
providers).

**Deploying:** the `Documentation` workflow (`.github/workflows/docs.yml`)
builds the site on Ubuntu and deploys it to GitHub Pages on every push to
`main` that touches the packages, docs or scripts. On `dev` and on pull
requests it only builds, and uploads the site as an artifact to check.
`site/` is not committed (it is git-ignored): a local build is for
previewing. Build on Linux or in CI for anything published: on Windows,
whose file names ignore case, the Python reference's `Basemap.html` (the
class) overwrites `basemap.html` (the function).

### Building by hand

Prerequisites (one-time):

- [Rust](https://rustup.rs), and on Windows [Rtools](https://cran.r-project.org/bin/windows/Rtools/) for R
- R 4.x. The script installs any missing R packages it needs (roxygen2,
  pkgdown, bslib, ggplot2, sf, jsonlite, tmap, stars, knitr, rmarkdown).
- Python 3.9+ with pip. The script installs basemapper and great-docs
  (`pip install -e ".[dev]"` in `py-basemapper/`).
- The [Quarto](https://quarto.org/docs/get-started/) CLI, which great-docs
  renders with
- PowerShell: built into Windows; on macOS or Linux install
  [PowerShell 7](https://learn.microsoft.com/powershell/scripting/install/installing-powershell)
  and run the script with `pwsh`

`Rscript`, `python` and `great-docs` must be on `PATH`, for example on
Windows:

```powershell
$env:PATH = "C:\Program Files\R\R-4.6.0\bin;$env:APPDATA\Python\Python313\Scripts;$env:PATH"
```

Then, from the repository root:

```powershell
.\scripts\build-docs.ps1          # Windows PowerShell
pwsh ./scripts/build-docs.ps1     # macOS / Linux
```

It stops at the first step that fails. In order, it:

1. installs the Python package with its docs extras (and into
   `py-basemapper/.venv` too, if that exists: great-docs runs the guide with
   that environment when it is there);
2. installs missing R packages and copies the Protomaps styles of the PMTiles
   examples (`core/guide/styles/`) next to the R articles and Python guide;
3. regenerates the R man pages (roxygen2) and builds the R site (pkgdown);
4. builds the Python site (great-docs);
5. runs the Rust guide examples, saving their output and images, and builds
   the Rust docs (`scripts/docs_rust.py`);
6. copies the landing page and adds the shared R | Python | Rust switcher
   (`docs/assets/site-nav.html`) to every page (`scripts/docs_postprocess.py`).

Open `site/index.html` through a local server to check it, for example
`python -m http.server 8765 --directory site`, then
<http://127.0.0.1:8765/>. To publish, push the source changes and merge
them into `main`; CI builds and deploys the site.

### Executed on every build

The R articles, the Python guide and the Rust guide's examples run on every
build in CI, so the published site always shows what the current code
draws.

Locally, the Python guide uses great-docs' default freeze: a page's code
runs again only when its `.qmd` changes, and its results are kept in
`py-basemapper/_freeze/` (git-ignored; CI starts without it). After a change
to basemapper itself, delete `py-basemapper/_freeze/` to run every page
again, or re-run one page from `py-basemapper/`:
`great-docs freeze user_guide/05-own-data.qmd`.

### Building in CI by hand

The `Documentation` workflow runs the same script on Ubuntu. Start it from
the repository's **Actions** tab (**Documentation**, then **Run workflow**),
or with the GitHub CLI. Run on `main`, it also deploys; on another branch it
only builds. To get the site it built (download it on Linux or macOS, or
open the zip without extracting it on Windows, because of the case clash
above):

```sh
gh workflow run docs.yml --ref dev
gh run list --workflow docs.yml                # find the run
gh run download <run-id> --name site --dir site-ci
```

### Generated assets

- Logo, mark and favicons: `python tools/logo/build_logo.py` (needs
  fontTools; Chrome or Edge for the PNG favicons).
- Protomaps styles for the PMTiles examples: `tools/protomaps-style/generate.cjs`
  (see `core/guide/styles/README.md`).

---

## Releases

R, Python and the Rust core share one version (0.1.0 and on), which each
package reports: `packageVersion("basemapper")` in R,
`basemapper.__version__` in Python and `basemapper_core::VERSION` in Rust.
Releases are git tags, `v` and the version (`v0.1.0`); the earlier `v0.0.x`
tags mark the milestones before it. To release:

1. Set the new version in every file that holds it, together:
   `python tools/version.py set 0.2.0` (`python tools/version.py check`
   confirms the versions agree and that no install command names a
   release; CI checks it on every pull request).
2. Write what changed for users in `r-basemapper/NEWS.md` and
   `py-basemapper/CHANGELOG.md`, and regenerate the root changelog from the
   commit history: `python tools/changelog.py --release v0.2.0`.
3. Merge `dev` into `main` through a pull request.
4. Tag the merge commit and push the tag:
   `git tag -a v0.2.0 -m "v0.2.0: ..." && git push origin v0.2.0`. CI checks
   that the tag matches the packages' version.

---

## How rendering works

See [docs/rendering-engine.md](docs/rendering-engine.md): the raster and
vector pipelines, which parts of MapLibre GL JS the vector renderer ports and
the details that make its output match, SVG (true vector) output, what is not
supported yet, and how fidelity is verified.

---

## Testing

| What | Command | Notes |
|---|---|---|
| Rust core: unit, integration and golden-image tests | `cargo test -p basemapper-core` | 212 golden cases (some at 168, 192, 200 and 300 dpi) compared with MapLibre GL JS references, run in parallel; `"svg": true` cases also score the SVG output (rasterised with resvg); `svg_seams` checks SVG tile edges as PDF viewers draw them |
| Python | build the wheel with `maturin build`, install it with `pytest plotnine`, then run `pytest` in `py-basemapper/` | Includes end-to-end renders against the golden fixtures |
| R | `R CMD INSTALL r-basemapper`, then `testthat::test_dir("r-basemapper/tests/testthat")` | |
| Visual comparison | open `check/index.html` after `cargo test` | Overview grid, side-by-side, swipe, blink and diff views |
| Re-render MapLibre references | `cd tools/reference-render`, `npm install`, `npx playwright install chromium`, `node render.mjs` | Needed after adding or re-capturing fixtures |
| Keyed services (live) | `UGRC_QUAD_WORD=... cargo test -p basemapper-core --test ugrc_discover_live` | Renders UGRC Discover basemaps; skipped without the key. CI (`Live services` workflow) uses the `UGRC_QUAD_WORD` repository secret, UGRC's test quad word. Keep keys in a git-ignored `.env`, never in the repository or in the docs build |

Fixtures are captured from live services with
`cargo run -p basemapper-core --example capture_fixture`; see
[core/tests/fixtures/golden/README.md](core/tests/fixtures/golden/README.md).

---

## Rust developer build prerequisites

`cargo build --workspace` works from a fresh clone with only a Rust
toolchain: no sibling checkouts, no GPU, no `protoc`, no system libraries.
Vector tiles are rendered by basemapper's own CPU renderer (`core/src/vector/`),
a software port of MapLibre GL JS's rendering pipeline that is checked
pixel-by-pixel against MapLibre GL JS by the golden-image tests
(`core/tests/golden.rs`).

---

## Developer setup (Windows, R package)

On Windows the R package is built with the **`x86_64-pc-windows-gnu`** Rust
target and RTools45: the MSVC toolchain cannot import R.dll's DATA-exported
globals. `R CMD INSTALL` does the rest: `configure.win` generates the R import
library, and `src/Makevars.win` syncs the core crate into the package and runs
cargo.

```powershell
# One-time setup: install RTools45 (R sets RTOOLS45_HOME), then
rustup target add x86_64-pc-windows-gnu

# Build and install
R CMD INSTALL r-basemapper
```

On Linux and macOS, `R CMD INSTALL r-basemapper` uses `src/Makevars` and the
host Rust target.

---

## License

MIT © Pukar Bhandari
