# basemapper

A headless spatial basemap renderer for **R** and **Python**.

basemapper fetches and composites styled map tiles into RGBA pixel arrays
using a Rust core (reqwest + image). It integrates with **ggplot2**,
**tmap**, **matplotlib**, and **plotnine** so you can add basemaps to spatial
plots without a browser, a display server, or GDAL.

📖 [R documentation](https://ar-puuk.github.io/basemapper/r/) ·
🐍 [Python documentation](https://ar-puuk.github.io/basemapper/python/)

---

## Install

### R

```r
# Requires devtools or remotes
remotes::install_github("ar-puuk/basemapper", subdir = "r-basemapper")
```

Depends on: `ggplot2 (>= 3.4)`, `sf (>= 1.0)`, `jsonlite (>= 1.8)`, `grid`.  
Optional: `tmap (>= 4.0)`, `stars (>= 0.6)` for `tm_basemap()`.

> CRAN and R-Universe packages are planned after an extended testing period.

### Python

```sh
pip install "basemapper @ git+https://github.com/ar-puuk/basemapper#subdirectory=py-basemapper"
```

Depends on: `numpy`, `pyproj`, `matplotlib`, `requests`.  
Optional: `plotnine >= 0.12` for `geom_basemap()`.

> PyPI and conda packages are planned after an extended testing period.

---

## Quick start

### R — ggplot2

```r
library(ggplot2)
library(sf)
library(basemapper)

nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
style <- raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")

ggplot(nc) +
  geom_basemap(style_url = style) +
  geom_sf(fill = NA, colour = "steelblue") +
  coord_sf()
```

### R — tmap

```r
library(tmap)
library(sf)
library(basemapper)

nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
style <- raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")

tm_shape(nc) +
  tm_basemap(style) +
  tm_sf()
```

### Python — matplotlib

```python
import geopandas as gpd
import matplotlib.pyplot as plt
from basemapper import add_basemap, RasterProvider

world = gpd.read_file(gpd.datasets.get_path("naturalearth_lowres"))
fig, ax = plt.subplots(figsize=(10, 6))
world[world.continent == "Europe"].plot(ax=ax, facecolor="none", edgecolor="black")

style = str(RasterProvider("https://tile.openstreetmap.org/{z}/{x}/{y}.png"))
add_basemap(ax, style_url=style)
plt.show()
```

### Python — plotnine

```python
from plotnine import ggplot, geom_sf, coord_fixed
from basemapper import geom_basemap, RasterProvider
import geopandas as gpd

world = gpd.read_file(gpd.datasets.get_path("naturalearth_lowres"))
style = str(RasterProvider("https://tile.openstreetmap.org/{z}/{x}/{y}.png"))

(
    ggplot(world)
    + geom_basemap(style_url=style)
    + geom_sf(fill="none", colour="steelblue")
    + coord_fixed()
)
```

---

## Key functions

| What you want | R | Python |
|---|---|---|
| Raw RGBA render | `render_basemap_raw()` | `render_basemap_raw()` |
| ggplot2 / plotnine layer | `geom_basemap()` | `geom_basemap()` |
| matplotlib inject | — | `add_basemap()` |
| tmap layer | `tm_basemap()` | — |
| OSM / raster style | `raster_provider()` | `RasterProvider` |
| MVT vector style | `vector_provider()` | `VectorProvider` |
| ESRI vector style | `esri_vector_provider()` | `EsriVectorProvider` |
| ESRI raster style | `esri_raster_provider()` | `EsriRasterProvider` |
| Inspect style layers | `list_layers()` | `list_layers()` |

---

## Layer filtering

Pass a character vector / list to `layers` on any function to keep or
exclude specific layers:

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

## Developer setup (Windows — R package only)

The R package ships a pre-compiled `basemapper.dll`, so end users need only
R itself. Developers who want to rebuild the Rust code must use the
**`x86_64-pc-windows-gnu`** target with RTools45 — the MSVC toolchain cannot
correctly import R.dll's DATA-exported global variables.

```powershell
# One-time setup
rustup target add x86_64-pc-windows-gnu
.\scripts\build-r-importlib.ps1   # regenerates .r-lib/libR.dll.a

# Build
$env:PATH = "C:\rtools45\x86_64-w64-mingw32.static.posix\bin;C:\rtools45\usr\bin;" + $env:PATH
$env:R_HOME = "C:\Program Files\R\R-4.6.0"
cargo build --release -p r-basemapper --target x86_64-pc-windows-gnu
Copy-Item target\x86_64-pc-windows-gnu\release\r_basemapper.dll `
          r-basemapper\src\basemapper.dll -Force
```

Linux and macOS builds use the standard `cargo build` flow.

---

## License

MIT © Pukar Bhandari
