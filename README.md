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
# Requires remotes (or devtools); compiles the Rust core from source.
remotes::install_github("ar-puuk/basemapper", subdir = "r-basemapper")
```

Building from source needs a [Rust toolchain](https://rustup.rs) and, on
Windows, [RTools45](https://cran.r-project.org/bin/windows/Rtools/) plus the
GNU target: `rustup target add x86_64-pc-windows-gnu`.

Depends on: `ggplot2 (>= 3.4)`, `sf (>= 1.0)`, `jsonlite (>= 1.8)`,
`terra (>= 1.7)`, `grid`.
Optional: `tmap (>= 4.0)` for `tm_basemap()`.

> CRAN and R-Universe packages are planned after an extended testing period.

### Python

```sh
pip install "basemapper @ git+https://github.com/ar-puuk/basemapper#subdirectory=py-basemapper"
```

Building from source needs a [Rust toolchain](https://rustup.rs); pip builds
the extension with maturin automatically.

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
ugrc <- "https://tiles.arcgis.com/tiles/99lidPhWCzftIe9K/arcgis/rest/services"
styles <- sprintf("%s/%s/VectorTileServer/resources/styles/root.json", ugrc,
                  c("VectorHillshade", "LiteBase", "LiteLabels"))   # bottom to top

m <- basemap(styles, 800, 600, center = c(-111.8887, 40.7556), zoom = 12.4,
             pixel_ratio = 2)
render_png(m, path = "slc.png")   # 1600 x 1200, rendered natively at zoom 12.4
render_svg(m, path = "slc.svg")   # true vector; labels as editable text, fonts embedded
a <- render_array(m)              # [1200, 1600, 4] integer array
r <- render_raster(m)             # terra SpatRaster (EPSG:3857)
```

```python
import basemapper
ugrc = "https://tiles.arcgis.com/tiles/99lidPhWCzftIe9K/arcgis/rest/services"
styles = [f"{ugrc}/{s}/VectorTileServer/resources/styles/root.json"
          for s in ("VectorHillshade", "LiteBase", "LiteLabels")]

m = basemapper.basemap(styles, 800, 600, center=(-111.8887, 40.7556), zoom=12.4,
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
arguments `style`, `tile_zoom`, `alpha`, `layers`, `pixel_ratio`,
`auth_token`, `tile_timeout_ms` and `max_tiles`, and render at the output
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
| Inspect style layers | `list_layers()` | `list_layers()` | `list_layers()` |

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

## Layer filtering

Pass a character vector / list to `layers` on any function to keep or
exclude specific layers (`list_layers(style)` lists a style's layers):

```r
# R — keep only roads and labels
render_basemap_raw(..., layers = c("roads", "labels"))

# R — exclude a noisy layer
render_basemap_raw(..., layers = c("-poi"))
```

```python
# Python — same API
render_basemap_raw(..., layers=["roads", "labels"])
render_basemap_raw(..., layers=["-poi"])
```

---

## Updating the documentation

The documentation site is pre-built locally and committed to `site/`.
CI deploys whatever is in that directory on every push to `main` that touches `site/**`.

**Prerequisites** (one-time):
- R 4.x with packages: `roxygen2`, `pkgdown`, `ggplot2`, `sf`, `jsonlite`, `tmap`, `terra`, `knitr`, `rmarkdown` — the build script installs any that are missing automatically
- Rust toolchain + RTools45 (required by `roxygen2::roxygenize()` to compile the R package)
- Python with `basemapper[dev]` installed (`pip install -e ".[dev]"` from `py-basemapper/`)
- `great-docs` CLI on PATH (installed by the above pip command)

```powershell
# Add R and great-docs to PATH if not already present
$env:PATH = "C:\Program Files\R\R-4.6.0\bin;$env:PATH"
# (great-docs is typically at %APPDATA%\Python\PythonXXX\Scripts\)

# Build and assemble site/
.\scripts\build-docs.ps1

# Commit and push to trigger deployment
git add site/
git commit -m "docs: rebuild site"
git push
```

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
