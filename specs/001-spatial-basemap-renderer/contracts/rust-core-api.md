# Contract: Rust Core Public API (`core`)

**Crate**: `core` (workspace member)
**Crate path**: `core/`
**Visibility**: `pub` — consumed by `py-basemapper` and `r-basemapper` only; not
published to crates.io as a standalone crate.

---

## Public Function

```rust
/// Render a basemap tile mosaic to a flat RGBA byte array.
///
/// # Arguments
/// * `request` — validated render parameters (bbox, dimensions, style, options)
///
/// # Returns
/// * `Ok(RenderResult)` — RGBA pixels + bounds metadata
/// * `Err(BasemapError)` — typed error from any pipeline stage
pub fn render(request: RenderRequest) -> Result<RenderResult, BasemapError>
```

### Async note

`render` is a synchronous function. It internally creates (or re-uses a thread-local)
`tokio::Runtime` to drive async tile fetching. Binding crates do NOT need to manage a
runtime themselves for this function; they call it from a blocking context.

---

## Public Types

### `RenderRequest`

```rust
pub struct RenderRequest {
    pub bbox: [f64; 4],           // [xmin, ymin, xmax, ymax], EPSG:3857
    pub width: u32,
    pub height: u32,
    pub style_input: StyleInput,
    pub zoom: Option<u8>,
    pub tile_timeout_ms: u32,     // default 10_000
    pub max_tiles: u32,           // default 256
    pub tile_concurrency: u32,    // default 16
}
```

Builder pattern provided via `RenderRequest::builder()` for ergonomic construction
in the binding crates.

---

### `RenderResult`

```rust
pub struct RenderResult {
    pub pixels: Vec<u8>,      // RGBA, row-major, len = width * height * 4
    pub bounds: SpatialBounds,
    pub width: u32,
    pub height: u32,
    pub zoom_used: u8,
}
```

---

### `SpatialBounds`

```rust
pub struct SpatialBounds {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
    pub crs_epsg: u32,
}
```

---

### `StyleInput`

```rust
pub enum StyleInput {
    Url(String),
    InlineJson(String),
}

impl StyleInput {
    /// Parse from a raw string: starts with `{` → InlineJson, otherwise → Url.
    pub fn from_str(s: &str) -> Self
}
```

---

### `TileSource`

```rust
pub enum TileSource {
    XyzRaster {
        url_template: String,
        auth_header: Option<String>,
    },
    MapboxVectorTile {
        url_template: String,
        api_key: Option<String>,
    },
    EsriVectorTile {
        url_template: String,
        auth_header: Option<String>,
    },
}
```

---

### `BasemapError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum BasemapError {
    #[error("invalid bbox: {0}")]
    InvalidBbox(String),
    #[error("invalid dimensions {0}×{1}")]
    InvalidDimensions(u32, u32),
    #[error("style fetch failed for {url}: HTTP {status}")]
    StyleFetchFailed { url: String, status: u16 },
    #[error("style parse error: {0}")]
    StyleParseError(String),
    #[error("tile fetch failed for {url}: HTTP {status}")]
    TileFetchFailed { url: String, status: u16 },
    #[error("tile decode error: {0}")]
    TileDecodeError(String),
    #[error("render error: {0}")]
    RenderError(String),
    #[error("max tiles exceeded: requested {requested}, limit {limit}")]
    MaxTilesExceeded { requested: u32, limit: u32 },
}
```

---

## Invariants

1. `render()` is safe to call from multiple threads concurrently (each call has its
   own wgpu device context and tokio runtime handle).
2. `render()` never panics under valid inputs; panics are reserved for unrecoverable
   internal state corruption (wgpu device loss).
3. The returned `pixels` vector is always exactly `width * height * 4` bytes long on
   success; no partial results are returned.
4. The `bounds` in `RenderResult` may have slightly larger extents than the input
   `bbox` due to tile-boundary snapping (rounding up to whole tile edges).
