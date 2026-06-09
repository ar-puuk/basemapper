# Data Model: Spatial Basemap Renderer

**Branch**: `001-spatial-basemap-renderer` | **Date**: 2026-06-08

All types below are defined in `core/src/`. The binding crates (`py-basemapper`,
`r-basemapper`) expose a subset of these types over the FFI boundary.

---

## Core Entities

### `RenderRequest`

The sole input to the `core` rendering pipeline.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `bbox` | `[f64; 4]` | ✅ | `[xmin, ymin, xmax, ymax]` in **EPSG:3857** (Web Mercator metres) |
| `width` | `u32` | ✅ | Output pixel width (must be > 0) |
| `height` | `u32` | ✅ | Output pixel height (must be > 0) |
| `style_input` | `StyleInput` | ✅ | Either a URL (String) or inline JSON (String) |
| `zoom` | `Option<u8>` | ❌ | Override zoom level (0–22); auto-computed if absent |
| `tile_timeout_ms` | `u32` | ❌ | Per-tile HTTP timeout (default: 10 000 ms) |
| `max_tiles` | `u32` | ❌ | Maximum tiles fetched; zoom reduced if exceeded (default: 256) |
| `tile_concurrency` | `u32` | ❌ | Max concurrent tile HTTP requests (default: 16) |

**Validation rules**:
- `bbox[0] < bbox[2]` (xmin < xmax) and `bbox[1] < bbox[3]` (ymin < ymax)
- `width` and `height` must each be ≤ 16 384 (guard against runaway memory allocation)
- `zoom` must be 0–22 inclusive if provided

---

### `RenderResult`

The sole output of the `core` rendering pipeline.

| Field | Type | Description |
|-------|------|-------------|
| `pixels` | `Vec<u8>` | Flat RGBA byte array; length = `width * height * 4`; row-major, top-left origin |
| `bounds` | `SpatialBounds` | The exact bounding box that was rendered (may differ slightly from input due to tile alignment) |
| `width` | `u32` | Pixel width of the output (echoes the request) |
| `height` | `u32` | Pixel height of the output (echoes the request) |
| `zoom_used` | `u8` | Actual zoom level used for tile fetching |

---

### `SpatialBounds`

A geographic bounding box with CRS information.

| Field | Type | Description |
|-------|------|-------------|
| `xmin` | `f64` | Western bound (in the CRS unit) |
| `ymin` | `f64` | Southern bound |
| `xmax` | `f64` | Eastern bound |
| `ymax` | `f64` | Northern bound |
| `crs_epsg` | `u32` | EPSG code of the coordinate reference system (e.g., `3857`) |

**Derived property**: `aspect_ratio = (xmax - xmin) / (ymax - ymin)`

---

### `TileSource`

A discriminated union representing the three supported tile provider formats.

```
TileSource =
  | XyzRaster       { url_template, auth_header }
  | MapboxVectorTile { url_template, api_key }
  | EsriVectorTile  { url_template, auth_header }
```

| Variant | Field | Type | Description |
|---------|-------|------|-------------|
| `XyzRaster` | `url_template` | `String` | URL with `{z}`, `{x}`, `{y}` placeholders |
| `XyzRaster` | `auth_header` | `Option<String>` | Raw `Authorization` header value |
| `MapboxVectorTile` | `url_template` | `String` | URL with `{z}/{x}/{y}.mvt` placeholders |
| `MapboxVectorTile` | `api_key` | `Option<String>` | Appended as `?access_token=` query param |
| `EsriVectorTile` | `url_template` | `String` | ESRI tile URL pattern |
| `EsriVectorTile` | `auth_header` | `Option<String>` | Raw `Authorization` header value |

---

### `StyleInput`

A newtype over `String` that is resolved at request time.

```
StyleInput =
  | Url(String)        // starts with "http://" or "https://"
  | InlineJson(String) // starts with "{"
```

**Resolution behaviour**: If the input string starts with `{`, it is parsed immediately
as inline JSON. Otherwise, it is fetched via `reqwest` before rendering begins. The
fetched JSON is cached in-process for the duration of the render session.

**Validation**: After resolution, the JSON must be a valid MapLibre GL Style object
with at least `version` (must be `8`) and `sources` fields present.

---

### `BasemapError`

All errors are represented as variants of this enum, returned in `Result<RenderResult, BasemapError>`.

| Variant | Trigger |
|---------|---------|
| `InvalidBbox(String)` | `xmin ≥ xmax`, `ymin ≥ ymax`, or non-finite values |
| `InvalidDimensions(u32, u32)` | `width` or `height` is 0 or exceeds 16 384 |
| `StyleFetchFailed { url, status }` | HTTP error fetching the style URL |
| `StyleParseError(String)` | Style JSON is invalid or missing required fields |
| `TileFetchFailed { url, status }` | HTTP error fetching one or more tiles |
| `TileDecodeError(String)` | Tile bytes cannot be decoded (corrupt PNG/MVT) |
| `RenderError(String)` | `wgpu` pipeline failure (GPU initialization or draw failure) |
| `MaxTilesExceeded { requested, limit }` | Tile count at computed zoom exceeds `max_tiles` |

---

## FFI Surface (Binding Crates)

The binding crates expose a flattened function signature rather than the full
`RenderRequest` struct, to keep the FFI boundary simple.

### Python (`py-basemapper`)

```python
def render_basemap_raw(
    bbox_3857: list[float],   # [xmin, ymin, xmax, ymax]
    width: int,
    height: int,
    style_input: str,         # URL or inline JSON
    zoom: int | None = None,
    tile_timeout_ms: int = 10_000,
    max_tiles: int = 256,
) -> bytes                    # RGBA bytes, len = width * height * 4
```

Raises `BasemapError` (a Python exception class) on any failure.

The public `add_basemap(ax, style_url, zoom=None)` wrapper function is pure Python;
it calls `render_basemap_raw` internally and attaches a `spatial_bounds` attribute to
the returned `ndarray`.

---

### R (`r-basemapper`)

```r
render_basemap_raw(
  bbox_3857,        # numeric vector of length 4: c(xmin, ymin, xmax, ymax)
  width,            # integer
  height,           # integer
  style_input,      # character: URL or inline JSON string
  zoom = NULL,      # integer or NULL
  tile_timeout_ms = 10000L,
  max_tiles = 256L
)
# Returns: raw vector of length width * height * 4 (RGBA bytes)
# Throws: stop() with a descriptive message on error
```

The public `geom_basemap()` ggproto layer is pure R; it calls `render_basemap_raw`
inside `draw_panel()` and returns a `grid::rasterGrob`.

---

## State Transitions

### Render lifecycle (within a single `render()` call)

```
Idle
  → ValidatingRequest    (bbox, dims, zoom)
    → FetchingStyle      (download or parse inline JSON)
      → ValidatingStyle  (MapLibre GL schema check)
        → ComputingZoom  (formula or explicit override)
          → FetchingTiles (concurrent HTTP, max_tiles guard)
            → Rendering  (wgpu pipeline → offscreen texture)
              → Done     (Vec<u8> + SpatialBounds returned)
```

Any step may transition to `Error(BasemapError)`, which aborts the pipeline and
propagates the typed error to the caller.

---

## Attribute Conventions

### Python `ndarray` output attributes

After `add_basemap()` returns, the `ndarray` carries:

```python
arr.attrs = {
    "xmin": float,   # EPSG:3857
    "ymin": float,
    "xmax": float,
    "ymax": float,
    "crs_epsg": int, # always 3857
    "zoom": int,
    "width": int,
    "height": int,
}
```

### R `matrix` output attributes

After `render_basemap_raw()` returns the raw bytes, the R wrapper reshapes to a
`matrix` and sets:

```r
attr(m, "xmin")      <- numeric  # EPSG:3857
attr(m, "ymin")      <- numeric
attr(m, "xmax")      <- numeric
attr(m, "ymax")      <- numeric
attr(m, "crs_epsg")  <- 3857L
attr(m, "zoom")      <- integer
```
