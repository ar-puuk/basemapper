# Quickstart Validation Guide: Spatial Basemap Renderer

**Branch**: `001-spatial-basemap-renderer` | **Date**: 2026-06-08

This guide documents runnable scenarios that prove the feature works end-to-end.
Run these after completing the full implementation to validate all three user stories.

---

## Prerequisites

### System Requirements

- **Rust** ≥ 1.78 (stable) — `rustup update stable`
- **Python** ≥ 3.9 with `pip`
- **R** ≥ 4.1 with `rextendr`, `ggplot2`, `sf` packages
- **Maturin** ≥ 1.4 — `pip install maturin`
- A GPU **or** a Vulkan software renderer (Linux headless: install `mesa-vulkan-drivers`
  or set `WGPU_BACKEND=gl` and start a virtual framebuffer with `Xvfb`)

### Build the workspace

```sh
# From repo root
cargo build --workspace            # verify all crates compile
cargo test  --workspace            # verify all Rust tests pass
```

### Install the Python package (development mode)

```sh
cd py-basemapper
maturin develop                    # compiles and installs into current venv
pip install numpy pyproj matplotlib
```

### Install the R package (development mode)

```r
# From R, in the r-basemapper/ directory
rextendr::document()               # regenerates R wrappers from extendr macros
devtools::install()                # installs the package locally
```

---

## Scenario 1 — Rust Core: Raw Render to File (US3 — Headless)

**Purpose**: Verify the Rust core can fetch tiles and produce a valid RGBA byte
array without any language binding.

```sh
# Run the integration example in core/
cargo run --example headless_render -- \
  --bbox  "-13700000,4500000,-13600000,4600000" \
  --width  800 \
  --height 600 \
  --style  "https://demotiles.maplibre.org/style.json" \
  --output /tmp/basemap_test.png
```

**Expected outcome**:
- Command exits with code 0.
- `/tmp/basemap_test.png` is created, is 800 × 600 pixels, and shows a recognisable
  map of the San Francisco Bay area (the bbox above).
- No C compiler or GDAL library is invoked during the build.

---

## Scenario 2 — Python: `render_basemap_raw` direct call (US3)

**Purpose**: Verify the low-level PyO3 binding returns correctly shaped bytes.

```python
import basemapper
import numpy as np

raw = basemapper.render_basemap_raw(
    bbox_3857    = [-13_700_000, 4_500_000, -13_600_000, 4_600_000],
    width        = 400,
    height       = 300,
    style_input  = "https://demotiles.maplibre.org/style.json",
)

assert isinstance(raw, bytes)
assert len(raw) == 400 * 300 * 4, f"Expected {400*300*4} bytes, got {len(raw)}"

arr = np.frombuffer(raw, dtype=np.uint8).reshape(300, 400, 4)
assert arr.shape == (300, 400, 4)
assert arr.dtype == np.uint8
print("Scenario 2 PASSED")
```

**Expected outcome**: Script exits without assertion errors.

---

## Scenario 3 — Python: `add_basemap` with matplotlib auto-inference (US1 + US2)

**Purpose**: Verify automatic bbox extraction from a matplotlib axes with a geographic
CRS and correct injection of the basemap underneath the data.

```python
import matplotlib
matplotlib.use("Agg")              # headless backend
import matplotlib.pyplot as plt
import geopandas as gpd
import basemapper

world = gpd.read_file(gpd.datasets.get_path("naturalearth_lowres"))
europe = world[world.continent == "Europe"]

fig, ax = plt.subplots(1, 1, figsize=(8, 6), dpi=150)
europe.plot(ax=ax, color="none", edgecolor="black", linewidth=0.5)

# Basemap injection — no bbox specified manually
result_arr = basemapper.add_basemap(
    ax,
    style_url = "https://demotiles.maplibre.org/style.json",
)

assert result_arr is not None
assert hasattr(result_arr, "attrs")
assert "xmin" in result_arr.attrs
assert result_arr.shape[2] == 4    # RGBA channels

fig.savefig("/tmp/europe_basemap.png", dpi=150, bbox_inches="tight")
print("Scenario 3 PASSED — saved to /tmp/europe_basemap.png")
```

**Expected outcome**:
- `/tmp/europe_basemap.png` shows a map of Europe with a styled basemap underneath
  the country outlines.
- No `bbox` argument was passed; the library inferred it from `ax`.
- Pixel dimensions of the basemap match `8 inches × 150 DPI = 1200 px wide` (±2 px).

---

## Scenario 4 — Python: Custom JSON Style (US3)

**Purpose**: Verify that an inline JSON style visibly changes the basemap appearance.

```python
import json, basemapper, numpy as np

# Minimal greyscale override style
greyscale_style = json.dumps({
    "version": 8,
    "sources": {
        "osm": {
            "type": "raster",
            "tiles": ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"],
            "tileSize": 256
        }
    },
    "layers": [{"id": "osm", "type": "raster", "source": "osm",
                "paint": {"raster-saturation": -1}}]
})

raw = basemapper.render_basemap_raw(
    bbox_3857   = [-13_700_000, 4_500_000, -13_600_000, 4_600_000],
    width       = 256,
    height      = 256,
    style_input = greyscale_style,
)

arr = np.frombuffer(raw, dtype=np.uint8).reshape(256, 256, 4)
# A greyscale raster has R ≈ G ≈ B for all pixels
r, g, b = arr[:,:,0], arr[:,:,1], arr[:,:,2]
mean_saturation_diff = abs(r.astype(int) - g.astype(int)).mean()
assert mean_saturation_diff < 5, "Map does not appear greyscale"
print("Scenario 4 PASSED — greyscale style verified")
```

---

## Scenario 5 — R: `render_basemap_raw` direct call (US3)

**Purpose**: Verify the extendr binding returns correct raw bytes.

```r
library(basemapper)

raw <- render_basemap_raw(
  bbox_3857   = c(-13700000, 4500000, -13600000, 4600000),
  width       = 400L,
  height      = 300L,
  style_input = "https://demotiles.maplibre.org/style.json"
)

stopifnot(is.raw(raw))
stopifnot(length(raw) == 400L * 300L * 4L)
cat("Scenario 5 PASSED\n")
```

---

## Scenario 6 — R: `geom_basemap()` with ggplot2 auto-inference (US1 + US2)

**Purpose**: Verify the ggproto geom extracts extents automatically and injects a
basemap at the correct resolution.

```r
library(ggplot2)
library(sf)
library(basemapper)

nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)

p <- ggplot(nc) +
  geom_basemap(style_url = "https://demotiles.maplibre.org/style.json") +
  geom_sf(fill = NA, color = "steelblue") +
  coord_sf() +
  theme_void()

# Save to verify rendering
ggsave("/tmp/nc_basemap.png", p, width = 8, height = 5, dpi = 150)

# Verify the file was created and is non-trivially large
info <- file.info("/tmp/nc_basemap.png")
stopifnot(info$size > 50000)    # at least 50 KB — confirms pixels were rendered
cat("Scenario 6 PASSED — saved to /tmp/nc_basemap.png\n")
```

**Expected outcome**: `/tmp/nc_basemap.png` shows North Carolina county outlines
overlaid on a styled basemap. No `bbox` argument was provided.

---

## Scenario 7 — Error Handling: Malformed Style (US3 edge case)

**Purpose**: Verify that a malformed JSON style raises a clear error before tile
fetching begins.

```python
import basemapper

try:
    basemapper.render_basemap_raw(
        bbox_3857   = [-13_700_000, 4_500_000, -13_600_000, 4_600_000],
        width       = 256,
        height      = 256,
        style_input = '{"version": 7}',   # invalid: version must be 8
    )
    assert False, "Should have raised BasemapError"
except basemapper.BasemapError as e:
    assert "style" in str(e).lower() or "version" in str(e).lower()
    print(f"Scenario 7 PASSED — error: {e}")
```

---

## Scenario 8 — Error Handling: Network Timeout

**Purpose**: Verify that tile fetch failures surface a descriptive error within the
configured timeout.

```python
import time, basemapper

start = time.monotonic()
try:
    basemapper.render_basemap_raw(
        bbox_3857       = [-13_700_000, 4_500_000, -13_600_000, 4_600_000],
        width           = 256,
        height          = 256,
        style_input     = "https://invalid.nonexistent.tld/style.json",
        tile_timeout_ms = 3_000,   # 3 second timeout
    )
    assert False, "Should have raised BasemapError"
except basemapper.BasemapError as e:
    elapsed = time.monotonic() - start
    assert elapsed < 15, f"Should fail within 15 s, took {elapsed:.1f} s"
    assert "fetch" in str(e).lower() or "style" in str(e).lower()
    print(f"Scenario 8 PASSED in {elapsed:.1f}s — error: {e}")
```

---

## Validation Checklist

After running all scenarios, verify:

- [ ] Scenario 1: Rust headless example produces valid PNG, no GDAL/C++ invocation
- [ ] Scenario 2: Python raw call returns correctly sized bytes
- [ ] Scenario 3: Python auto-inference places basemap without manual bbox, correct size
- [ ] Scenario 4: Custom greyscale JSON style visibly applied
- [ ] Scenario 5: R raw call returns correct raw vector
- [ ] Scenario 6: R ggplot2 geom places basemap without manual bbox, file > 50 KB
- [ ] Scenario 7: Malformed style raises `BasemapError` before tile fetching
- [ ] Scenario 8: Network failure raises `BasemapError` within timeout
