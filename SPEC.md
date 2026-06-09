# Technical Specification: basemapper
**A Headless MapLibre Renderer for R and Python**

## 1. Project Overview
**Goal:** Build a high-performance, cross-platform library that generates native-resolution basemap matrices for R and Python static spatial plotting (`ggplot2`, `tmap`, `matplotlib`).
**Core Mechanism:** It takes an inferred bounding box and a Mapbox Style JSON (supporting Leaflet raster providers, Mapbox MVTs, and ESRI Vector Tiles), and uses a pure-Rust headless rendering engine (`maplibre-rs`) to output a multi-dimensional pixel array.
**Critical Requirement:** The library must utilize "deferred rendering." The user should not manually provide coordinates; the frontends must automatically infer the bounding box and resolution directly from the plot's active limits just before rendering.

## 2. Monorepo Architecture
The project utilizes a Cargo Workspace to share the core rendering logic between the Python and R wrappers.

```text
basemapper/
├── Cargo.toml                # Workspace manifest
├── core_engine/              # Pure Rust rendering and spatial logic
│   ├── Cargo.toml
│   └── src/
├── python/                   # Python wrapper (PyO3 + Maturin)
│   ├── pyproject.toml
│   └── src/
└── r_package/                # R wrapper (extendr)
    ├── DESCRIPTION
    └── src/rust/src/
```

## 3. Spatial Coordinate Preservation & Deferred Logic
To seamlessly integrate with plotting libraries while keeping the Rust FFI boundary clean, the frontend handles CRS complexity before calling Rust:

1. **Automated Inference:** The R or Python wrappers intercept the plot creation (e.g., `ggplot2` panel drawing or `matplotlib` axes rendering), reading the current plot limits and target graphic device dimensions.
2. **Projection Normalization:** The wrappers automatically reproject these inferred limits into Web Mercator (EPSG:3857). The Rust core *only* accepts flat Web Mercator floats.
3. **The Output Object:** The Rust engine returns a compound structure consisting of:
   * `image_data`: The flat `Vec<u8>` RGBA byte array.
   * `bbox_3857`: The exact EPSG:3857 `[xmin, ymin, xmax, ymax]` that the generated image represents.
   * `width` & `height`: Exact pixel dimensions.

## 4. `core_engine` (Rust) Specification
**Dependencies (`Cargo.toml`):**
* `maplibre-rs`: Core rendering engine (utilizing its headless `wgpu` capabilities).
* `tokio`: Async runtime for concurrent tile fetching.
* `reqwest`: HTTP client for fetching Style JSON and tiles.

**Core Data Structures (`src/lib.rs`):**
```rust
pub struct RenderResult {
    pub rgba_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub extent_3857: [f64; 4], // [xmin, ymin, xmax, ymax]
}

pub struct RenderParams {
    pub bbox_3857: [f64; 4], // Clean, normalized Web Mercator
    pub style_url: String,
    pub width: u32,
    pub height: u32,
    pub dpi: f32,
}
```

**Main Function:**
```rust
pub async fn render_headless_map(params: RenderParams) -> Result<RenderResult, String> {
    // 1. Initialize maplibre-rs headless surface (wgpu texture) via dimensions
    // 2. Fetch Style JSON and necessary tiles concurrently based on bbox_3857
    // 3. Render map to texture
    // 4. Extract bytes from wgpu buffer
    // 5. Return RenderResult
}
```

## 5. Python Wrapper Specification (`python/`)
**Build System:** `maturin`
**Dependencies:** `pyo3`, `numpy`, `pyproj`

**Implementation Details (`basemap_rs.py`):**
1. Implement a helper function `add_basemap(ax, style_url, dpi=None)`.
2. Extract limits dynamically using `ax.get_xlim()` and `ax.get_ylim()`. Use `ax.figure.dpi` and bbox dimensions to calculate the exact target width and height in pixels.
3. Reproject limits to EPSG:3857 using `pyproj`.
4. Spin up a local `tokio` runtime via PyO3 to call `render_headless_map` from `core_engine`.
5. Convert `rgba_bytes` into a `(height, width, 4)` NumPy array.
6. Overlay matrix natively using `ax.imshow(extent=...)` underneath the vector data.

## 6. R Wrapper Specification (`r_package/`)
**Build System:** `extendr` via `rextendr`
**Dependencies:** `extendr-api`, `ggplot2`, `grid`, `sf`

**Implementation Details (`R/geom_basemap.R`):**
1. Create a `ggproto` subclass called `GeomBasemap` inheriting from `Geom`.
2. Override the `draw_panel(data, panel_params, coord)` method so rendering is deferred until the exact plot time.
3. Inside `draw_panel`:
   * Extract coordinates from `panel_params`.
   * Reproject to EPSG:3857 if `coord` states a different native CRS.
   * Calculate physical pixel dimensions based on the active graphic device size (`grDevices::dev.size()`).
4. Call the `extendr` Rust FFI to generate the map.
5. Return the array as a `grid::rasterGrob()`.

```r
#' @export
geom_basemap <- function(style_url) {
  ggplot2::layer(
    stat = "identity", position = "identity",
    geom = GeomBasemap, inherit.aes = FALSE,
    params = list(style_url = style_url)
  )
}
```

## 7. Implementation Phasing for AI Agent
*Instruct the AI to follow this exact order of operations to avoid compilation hell:*

* **Phase 1: `core_engine` Scaffolding.** Set up the Cargo workspace and implement the dummy `wgpu` rendering logic returning a hardcoded `Vec<u8>` (e.g., a solid blue image).
* **Phase 2: R/Python Bindings.** Implement the PyO3 and extendr bindings to ensure matrices and extents are passed flawlessly to the data science environments.
* **Phase 3: Automated Extent Inference.** Wire up the `GeomBasemap` ggproto object in R and the `add_basemap` matplotlib helper in Python to prove the extent calculations work.
* **Phase 4: The `maplibre-rs` Integration.** Fully wire up the Style JSON parsing and tile fetching logic within the Rust core.
