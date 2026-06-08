# Contract: Python Public API (`py-basemapper`)

**Package**: `basemapper` (PyPI)
**Build**: `maturin build --release` → produces `basemapper-*.whl`
**Python requirement**: ≥ 3.9
**Runtime dependencies**: `numpy`, `pyproj`, `matplotlib` (for the high-level wrapper)

---

## High-Level API (pure Python wrapper)

### `basemapper.add_basemap`

```python
def add_basemap(
    ax: matplotlib.axes.Axes,
    style_url: str,
    zoom: int | None = None,
    tile_timeout_ms: int = 10_000,
    max_tiles: int = 256,
) -> numpy.ndarray:
    """
    Fetch and inject a basemap under the existing content of a matplotlib Axes.

    The bounding box and CRS are inferred from `ax`. The basemap is placed at
    zorder=0 (below all existing artists). The input axes is mutated in-place.

    Returns the rendered RGBA ndarray (H × W × 4, dtype=uint8) with spatial
    bounds attached as ndarray attributes (xmin, ymin, xmax, ymax, crs_epsg,
    zoom, width, height).

    Raises:
        ValueError: if `ax` has no recognisable geographic extent or CRS.
        BasemapError: if tile fetching or rendering fails.
    """
```

**Behaviour**:
1. Extract `xlim, ylim` from `ax.get_xlim()` / `ax.get_ylim()`.
2. Detect CRS from `ax` (cartopy CRS attribute, GeoDataFrame plot CRS, or assume
   EPSG:4326 as fallback — with a warning).
3. Reproject bounds to EPSG:3857 via `pyproj.Transformer`.
4. Determine pixel dimensions from `ax.get_window_extent()` and `ax.figure.dpi`.
5. Call `render_basemap_raw(...)`.
6. Reshape bytes to `numpy.ndarray` shaped `(height, width, 4)`.
7. Call `ax.imshow(arr, extent=[xmin_orig, xmax_orig, ymin_orig, ymax_orig],
   aspect='auto', zorder=0, interpolation='nearest')`.
8. Return the `ndarray`.

---

## Low-Level API (PyO3 native extension)

### `basemapper.render_basemap_raw`

```python
def render_basemap_raw(
    bbox_3857: list[float],
    width: int,
    height: int,
    style_input: str,
    zoom: int | None = None,
    tile_timeout_ms: int = 10_000,
    max_tiles: int = 256,
) -> bytes:
    """
    Render a basemap and return raw RGBA bytes.

    Parameters
    ----------
    bbox_3857 : list of 4 floats — [xmin, ymin, xmax, ymax] in EPSG:3857.
    width : int — output pixel width.
    height : int — output pixel height.
    style_input : str — MapLibre GL style URL or inline JSON string.
    zoom : int | None — tile zoom level (0–22); auto-computed if None.
    tile_timeout_ms : int — per-tile HTTP timeout in milliseconds.
    max_tiles : int — maximum tile count; zoom is reduced if exceeded.

    Returns
    -------
    bytes — RGBA pixel data, length = width * height * 4, row-major.

    Raises
    ------
    BasemapError — on any validation, network, or rendering failure.
    """
```

---

## Exception Class

```python
class BasemapError(RuntimeError):
    """
    Raised by render_basemap_raw on any failure.
    The string representation includes the failure stage and detail.
    """
```

---

## Return Value Conventions

- `render_basemap_raw` returns `bytes` with length exactly `width * height * 4`.
- Pixels are in row-major order, top-left origin, RGBA channel order.
- Callers convert to numpy via: `np.frombuffer(result, dtype=np.uint8).reshape(height, width, 4)`.
- The `ndarray` returned by `add_basemap` has these attributes:

```python
arr.attrs = {
    "xmin":      float,   # EPSG:3857
    "ymin":      float,
    "xmax":      float,
    "ymax":      float,
    "crs_epsg":  int,     # 3857
    "zoom":      int,
    "width":     int,
    "height":    int,
}
```

---

## Threading Contract

`render_basemap_raw` releases the GIL for the entire duration of the render call.
It is safe to call from Python threads concurrently. Each call manages its own
internal tokio runtime context.
