# basemapper

A headless spatial basemap renderer for **R** and **Python**.

basemapper renders styled basemaps (MapLibre / Mapbox GL styles, raster XYZ,
WMS, WMTS, PMTiles, ESRI vector tiles) with a Rust core, to PNG, arrays and
rasters, or true vector SVG, and adds them to **ggplot2**, **tmap**,
**matplotlib** and **plotnine** plots. No browser, GPU, display server or
GDAL is involved.

Basemaps are rendered for the exact view and output resolution: any
fractional zoom, any pixel ratio (a 300 dpi figure gets a basemap rendered at
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
# Any one of these; each compiles the Rust core from source.
pak::pak("ar-puuk/basemapper/r-basemapper")
remotes::install_github("ar-puuk/basemapper", subdir = "r-basemapper")
devtools::install_github("ar-puuk/basemapper", subdir = "r-basemapper")
```

Building from source needs a [Rust toolchain](https://rustup.rs) and, on
Windows, [RTools45](https://cran.r-project.org/bin/windows/Rtools/) plus the
GNU target: `rustup target add x86_64-pc-windows-gnu`.

Depends on: `ggplot2 (>= 3.4)`, `sf (>= 1.0)`, `jsonlite (>= 1.8)`,
`terra (>= 1.7)`, `grid`.
Optional: `tmap (>= 4.0)` for `tm_basemap()`.

> CRAN and R-Universe packages are planned after an extended testing period.

### Python

With pip:

```sh
pip install "basemapper @ git+https://github.com/ar-puuk/basemapper#subdirectory=py-basemapper"
```

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

For `geom_basemap()` in plotnine, install `basemapper[plotnine]` in place of
`basemapper`.

Building from source needs a [Rust toolchain](https://rustup.rs) (conda's
`rust` package also works); pip and uv build the extension with maturin
automatically.

Depends on: `numpy`, `pyproj`, `matplotlib`, `xarray`.
Optional: `plotnine >= 0.12` for `geom_basemap()`.

> PyPI and conda packages are planned after an extended testing period.

---

## Quick start

The R, Python and Rust APIs are the same: the same functions, the same
argument names and defaults. R calls `verb(m, ...)` where Python and Rust
call `m.verb(...)`. See the [API reference](docs/api.md) for the full
side-by-side list.

### A map on its own

```r
library(basemapper)
# UGRC's Lite basemap: hillshade, base and labels services, stacked
m <- basemap("UGRC.Lite", 800, 600, center = c(-111.8887, 40.7556), zoom = 12.4,
             pixel_ratio = 2)
render_png(m, path = "slc.png")   # 1600 x 1200, rendered natively at zoom 12.4
render_svg(m, path = "slc.svg")   # true vector; labels as editable text, fonts embedded
a <- render_array(m)              # [1200, 1600, 4] integer array
r <- render_raster(m)             # terra SpatRaster (EPSG:3857)
```

```python
import basemapper
m = basemapper.basemap("UGRC.Lite", 800, 600, center=(-111.8887, 40.7556), zoom=12.4,
                       pixel_ratio=2)
m.render_png(path="slc.png")
m.render_svg(path="slc.svg")      # text="paths" for traced glyph outlines
a = m.render_array()              # (1200, 1600, 4) uint8
```

```rust
use basemapper_core::Basemap;

let m = Basemap::stack(styles)
    .size(800, 600)
    .center(-111.8887, 40.7556)
    .zoom(12.4)
    .pixel_ratio(2.0);
m.render_png(Some("slc.png".as_ref()))?;
```

`width` / `height` are CSS pixels and set the view; images are
`pixel_ratio` times larger with the same layout (MapLibre's
devicePixelRatio). `style` also takes your own tile data, building its style for you: a tile
URL, TileJSON, PMTiles archive (URL or local file) or ArcGIS service, e.g.
`basemap("data/area.pmtiles", ...)`. Fit an extent instead of a centre with `bounds =` (R: a
bbox or `sf` object; Python: a bbox or GeoDataFrame). Move the view with
`set_view()` / `set_bounds()` and read it with `get_bounds()`,
`get_center()`, `get_zoom()` and `get_size()`.

### R — ggplot2

```r
library(ggplot2)
library(sf)
library(basemapper)

nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)

ggplot(nc) +
  geom_basemap("OpenFreeMap.Positron") +
  geom_sf(fill = NA, colour = "steelblue") +
  coord_sf()
```

### R — tmap

```r
library(tmap)
library(sf)
library(basemapper)

nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)

tm_shape(nc) +
  tm_basemap("OpenFreeMap.Positron") +
  tm_sf()
```

### Python — matplotlib

```python
import geopandas as gpd
import matplotlib.pyplot as plt
from basemapper import add_basemap, basemap_provider

world = gpd.read_file(
    "https://naciscdn.org/naturalearth/110m/cultural/ne_110m_admin_0_countries.zip"
)
europe = world[world.CONTINENT == "Europe"].cx[-25:45, 34:72]
fig, ax = plt.subplots(figsize=(10, 6))
europe.plot(ax=ax, facecolor="none", edgecolor="black")

# Axes from a GeoDataFrame plot carry no CRS: pass it explicitly.
add_basemap(ax, "OpenFreeMap.Positron", crs=europe.crs)
plt.show()
```

### Python — plotnine

```python
import pandas as pd
from plotnine import ggplot, aes, geom_point
from basemapper import basemap_provider, geom_basemap

cities = pd.DataFrame({"lon": [-111.89, -111.98], "lat": [40.76, 40.66]})

(
    ggplot(cities, aes("lon", "lat"))
    + geom_basemap("OpenFreeMap.Positron")   # data CRS defaults to EPSG:4326
    + geom_point(colour="red", size=3)
)
```

The plot layers (`geom_basemap`, `tm_basemap`, `add_basemap`) share the
arguments `style`, `tile_zoom`, `alpha`, `layers`, `labels`, `features`,
`pixel_ratio`, `auth_token`, `tile_timeout_ms`, `max_tiles` and
`attribution` (the providers' credit line in the corner, on by default), and render at the output
resolution (`ggsave(dpi = 300)`, `savefig(dpi=300)`).

---

## Key functions

| What you want | R | Python | Rust (`basemapper_core`) |
|---|---|---|---|
| A map view (centre + zoom, or bounds) | `basemap()` | `basemap()` → `Basemap` | `Basemap::new()` |
| Change the view | `set_view()`, `set_bounds()` | `m.set_view()`, `m.set_bounds()` | `m.set_view()`, `m.set_bounds()` |
| Read the view | `get_bounds()`, `get_center()`, `get_zoom()`, `get_size()` | `m.get_bounds()`, … | `m.get_bounds()`, … |
| PNG | `render_png()` | `m.render_png()` | `m.render_png()` |
| Vector SVG | `render_svg()` | `m.render_svg()` | `m.render_svg()` |
| Array | `render_array()` | `m.render_array()` | `m.render_array()` |
| terra raster | `render_raster()` | — | — |
| One-shot render of bounds | `render_basemap_raw()` | `render_basemap_raw()` | `render()`, `render_stack()` |
| ggplot2 / plotnine layer | `geom_basemap()` | `geom_basemap()` | — |
| matplotlib layer | — | `add_basemap()` | — |
| tmap layer | `tm_basemap()` | — | — |
| Named basemaps (leaflet names; the name works as `style`) | `basemap_provider()`, `list_providers()`, `providers` | `basemap_provider()`, `list_providers()`, `providers` | `providers::basemap_provider()`, `providers::PROVIDERS` |
| Tile cache | `tile_cache()` | `tile_cache()` | `cache::{dir, set_dir, clear}` |
| Your own tile data (tile URL, TileJSON, PMTiles, ArcGIS) | pass it as `style`; `tile_style()` for overrides | same | `tile_style()` |
| OGC WMS / WMTS | `wms_style()`, `wmts_style()` | same names | `providers::wms_style()`, `providers::wmts_style()` |
| Attribution text (for a caption) | `get_attribution()` | `get_attribution()`, `m.get_attribution()` | `attribution()`, `m.get_attribution()` |
| Labels / features (stack parts of basemaps) | `labels =`, `features =` | same | `Basemap::labels()`, `Basemap::features()` |
| Inspect style layers (id, type, feature) | `list_layers()` | `list_layers()` | `list_layers()` |

---

## Anti-meridian and world copies

Bboxes where `xmin > xmax` in EPSG:3857 (Pacific-centred views, date-line
crossings) are rendered by splitting into west and east halves and stitching
the result. A view that extends past ±180° (for example a whole-world map at
low zoom) shows the wrapped world copy there, as MapLibre does; vector styles
draw it directly on one canvas.

```r
# R — Pacific-centred map crossing the date line
render_basemap_raw(c(15000000, -5000000, -15000000, 5000000), 800, 400,
                   "OpenStreetMap.Mapnik", crs = 3857)
```

```python
# Python — same
render_basemap_raw([15_000_000, -5_000_000, -15_000_000, 5_000_000], 800, 400,
                   "OpenStreetMap.Mapnik", crs=3857)
```

---

## Labels, features and layers

Every function takes `labels` and `features` to draw parts of a style, so
basemaps can be stacked. `labels = FALSE` drops the labels (text and
icons); `labels = "only"` keeps only the labels, on a transparent
background. `features` keeps (`c("water", "roads")`) or drops
(`"-buildings"`) parts of the map: background, water, landcover, parks,
buildings, roads, rail, boundaries, places, poi, terrain. They work on any
vector style, from tables for the OpenMapTiles, Protomaps and Shortbread
schemas and Esri's layer names; catalogue raster basemaps switch to their
label-free or label-only tiles.

```r
# R — imagery under a vector basemap's labels; each style filtered alone
basemap(c("Esri.WorldImagery", basemap_provider("OpenFreeMap.Liberty", labels = "only")),
        center = c(-111.89, 40.76), zoom = 12)

# R — water and roads only, without their names
basemap("OpenFreeMap.Liberty", labels = FALSE, features = c("water", "roads"), bounds = nc)
```

```python
# Python — same API
basemap(["Esri.WorldImagery", basemap_provider("OpenFreeMap.Liberty", labels="only")],
        center=(-111.89, 40.76), zoom=12)
basemap("OpenFreeMap.Liberty", labels=False, features=["water", "roads"], bounds=gdf)
```

`list_layers(style)` shows each layer's id, type, source layer and
feature; given `layers`, `labels` or `features`, it previews what a render
would draw. Names that match no layer warn. For exact control, `layers`
keeps or drops layers by id, with `*` and `?` wildcards:

```r
# R — keep only roads and labels
render_basemap_raw(..., layers = c("roads", "labels"))

# R — exclude a noisy layer, or a family of layers
render_basemap_raw(..., layers = c("-poi"))
render_basemap_raw(..., layers = "-label_*")
```

```python
# Python — same API
render_basemap_raw(..., layers=["roads", "labels"])
render_basemap_raw(..., layers=["-poi"])
render_basemap_raw(..., layers=["-label_*"])
```

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
<http://127.0.0.1:8765/>. To publish, push the source changes (and any
updated `py-basemapper/_freeze/`, see below) and merge them into `main`; CI
builds and deploys the site.

### Python pages and the freeze cache

The Python guide uses Quarto's `freeze: auto`: a page's code runs only when
its `.qmd` file changes, and the results are kept in `py-basemapper/_freeze/`,
which is committed so that every build (including CI) reuses them. When a
change in basemapper itself alters a page's output, re-run that page, from
`py-basemapper/`:

```sh
great-docs freeze user_guide/05-own-data.qmd   # one page
great-docs freeze --clean user_guide/*.qmd     # all of them, from scratch
great-docs freeze --info                       # what is cached
```

and commit the updated `_freeze/`. The R articles and the Rust examples have
no such cache: they run on every build.

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

1. Set the new version in all six files that hold it, together:
   `python tools/version.py set 0.2.0` (`python tools/version.py check`
   confirms they agree; CI checks it on every pull request).
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
| Rust core: unit, integration and golden-image tests | `cargo test -p basemapper-core` | 168 golden cases (incl. pixel ratio 2) compared with MapLibre GL JS references, run in parallel; `"svg": true` cases also score the SVG output (rasterised with resvg) |
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
