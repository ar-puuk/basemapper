"""One-shot rendering: :func:`render_basemap_raw`."""

from __future__ import annotations

from typing import Any, List, Optional, Tuple

import numpy as np


def _render_buffer(
    bounds: Any,
    width: int,
    height: int,
    style: Any,
    crs: Optional[Any] = None,
    pixel_ratio: float = 1.0,
    tile_zoom: Optional[int] = None,
    **options: Any,
) -> Tuple[bytes, int, int]:
    """Render over bounds; returns ``(bytes, width, height)`` in device px."""
    from ._basemap import _bounds_to_3857, _check_pixel_ratio, _style_strings
    from ._basemapper import render_raw

    _check_pixel_ratio(pixel_ratio)
    return render_raw(
        _bounds_to_3857(bounds, crs), int(width), int(height), _style_strings(style), float(pixel_ratio),
        tile_zoom=tile_zoom, **options,
    )


def render_basemap_raw(
    bounds: Any,
    width: int,
    height: int,
    style: Any,
    crs: Optional[Any] = None,
    pixel_ratio: float = 1.0,
    tile_zoom: Optional[int] = None,
    layers: Optional[List[str]] = None,
    auth_token: Optional[str] = None,
    tile_timeout_ms: int = 10_000,
    max_tiles: int = 256,
    tile_concurrency: int = 16,
    fail_on_tile_error: bool = False,
) -> np.ndarray:
    """Render a basemap for bounds in one call.

    A one-shot render: fit ``bounds`` into a ``width`` x ``height`` CSS-pixel
    view and return the pixels. :func:`basemap` with
    :meth:`Basemap.render_array` gives the same result with a reusable view.
    The same function exists in R (``render_basemap_raw()``).

    Args:
        bounds: Bounding box: a ``[xmin, ymin, xmax, ymax]`` sequence in
            ``crs`` (default EPSG:4326, with a warning when the values look
            projected), a dict with ``xmin``/``ymin``/``xmax``/``ymax`` (and
            optionally ``crs``) keys, or a GeoDataFrame / GeoSeries (CRS read
            from it).
        width: View width in CSS pixels.
        height: View height in CSS pixels.
        style: A MapLibre GL style URL or JSON string (e.g. from
            :func:`basemap_provider`), or a list of those stacked bottom to
            top.
        crs: CRS of ``bounds`` (anything ``pyproj.CRS`` accepts).
        pixel_ratio: Device pixels per CSS pixel; the image is
            ``width * pixel_ratio`` by ``height * pixel_ratio``.
        tile_zoom: Tile detail override: the tile zoom on the 256 px grid
            (vector sources use one less, their tiles being 512 px), or
            ``None`` to pick tiles as MapLibre does for the view.
        layers: Optional layer filter: plain IDs keep only those layers,
            ``-`` prefixed IDs drop them.
        auth_token: Optional access token for the tile service.
        tile_timeout_ms: Per-tile HTTP timeout in milliseconds.
        max_tiles: Maximum tiles fetched per style.
        tile_concurrency: Parallel tile downloads.
        fail_on_tile_error: Abort on a failed tile instead of leaving it empty.

    Returns:
        numpy.ndarray: ``(height * pixel_ratio, width * pixel_ratio, 4)``
        uint8 RGBA pixels.

    Raises:
        BasemapError: On any rendering or network failure.
    """
    buf, w, h = _render_buffer(
        bounds, width, height, style, crs=crs, pixel_ratio=pixel_ratio, tile_zoom=tile_zoom,
        layers=layers, auth_token=auth_token, tile_timeout_ms=tile_timeout_ms, max_tiles=max_tiles,
        tile_concurrency=tile_concurrency, fail_on_tile_error=fail_on_tile_error,
    )
    return np.frombuffer(buf, dtype=np.uint8).reshape(h, w, 4).copy()
