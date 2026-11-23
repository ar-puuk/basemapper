"""One-shot rendering: :func:`render_basemap_raw`."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, List, Optional, Tuple, Union

import numpy as np

if TYPE_CHECKING:
    from ._fetch import FetchOptions


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

    _check_pixel_ratio(pixel_ratio)
    return render_call(
        _bounds_to_3857(bounds, crs), int(width), int(height), _style_strings(style), float(pixel_ratio),
        tile_zoom=tile_zoom, **options,
    )


def render_call(*args: Any, **kwargs: Any) -> Tuple[bytes, int, int]:
    """The core's render, with its filter warnings (names in ``layers`` or
    ``features`` that matched no layer) issued as Python warnings."""
    import warnings

    from ._basemapper import render_raw

    buf, w, h, notes = render_raw(*args, **kwargs)
    for note in notes:
        warnings.warn(note, UserWarning, stacklevel=3)
    return buf, w, h


def labels_arg(labels: Union[bool, str]) -> str:
    """``labels`` as the core's "all" / "none" / "only" (True and False are
    shorthand for "all" and "none", as in R)."""
    if labels is True:
        return "all"
    if labels is False:
        return "none"
    if isinstance(labels, str) and labels.lower() in ("all", "none", "only"):
        return labels.lower()
    raise ValueError(f'labels must be "all", "none" or "only" (or True / False), not {labels!r}')


def _features_arg(features: Optional[Any]) -> Optional[List[str]]:
    if features is None:
        return None
    return [features] if isinstance(features, str) else [str(f) for f in features]


def render_basemap_raw(
    bounds: Any,
    width: int,
    height: int,
    style: Any,
    crs: Optional[Any] = None,
    pixel_ratio: float = 1.0,
    tile_zoom: Optional[int] = None,
    layers: Optional[List[str]] = None,
    labels: Union[bool, str] = True,
    features: Optional[List[str]] = None,
    auth_token: Optional[str] = None,
    fetch: Optional["FetchOptions"] = None,
    **deprecated: Any,
) -> np.ndarray:
    """Render a basemap for bounds in one call.

    A one-shot render: fit ``bounds`` into a ``width`` x ``height`` CSS-pixel
    view and return the pixels. This is the low-level one-shot render; for
    most uses :func:`basemap` with :meth:`Basemap.render_array` is simpler
    and gives a view to reuse. The same function exists in R
    (``render_basemap_raw()``).

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
            ``-`` prefixed IDs drop them; ``*`` and ``?`` are wildcards
            (``"-label_*"``).
        labels: ``"all"`` (or ``True``) keeps every layer, ``"none"`` (or ``False``) drops the labels (text
            and icons), ``"only"`` keeps only the labels on a transparent
            background, to stack over another map. Catalogue raster
            basemaps switch to their label-free or label-only tiles.
        features: Optional feature filter applied to every style: plain
            names keep those features, ``-`` prefixed names drop them, e.g.
            ``["water", "roads"]`` or ``["-buildings"]``. Features:
            background, water, landcover, parks, buildings, roads, rail,
            boundaries, places, poi, terrain, other (see the ``feature``
            of :func:`list_layers`).
        auth_token: Optional access token for the tile service (sent only to the style's own server).
        fetch: How tiles are downloaded: a :class:`FetchOptions`.
        **deprecated: ``tile_timeout_ms``, ``max_tiles``,
            ``tile_concurrency`` and ``fail_on_tile_error`` are still
            accepted, with a DeprecationWarning: use ``fetch``.

    Returns:
        numpy.ndarray: ``(height * pixel_ratio, width * pixel_ratio, 4)``
        uint8 RGBA pixels.

    Raises:
        BasemapError: On any rendering or network failure.
    """
    from ._fetch import _resolve_fetch

    fetch = _resolve_fetch(fetch, "render_basemap_raw", deprecated)
    buf, w, h = _render_buffer(
        bounds, width, height, style, crs=crs, pixel_ratio=pixel_ratio, tile_zoom=tile_zoom,
        layers=layers, labels=labels_arg(labels), features=_features_arg(features), auth_token=auth_token,
        **fetch._core(),
    )
    return np.frombuffer(buf, dtype=np.uint8).reshape(h, w, 4).copy()
