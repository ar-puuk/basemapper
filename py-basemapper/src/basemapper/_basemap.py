"""Standalone map rendering: :func:`basemap` and the :class:`Basemap` view.

The same API exists in R (``basemap()``, ``set_view(m, ...)``,
``render_png(m, ...)``) and Rust (``Basemap::new()``): the same verbs and
argument names, with R's ``verb(m, ...)`` written ``m.verb(...)`` here.
"""

from __future__ import annotations

import math
from typing import Any, List, Optional, Sequence, Tuple, Union

import numpy as np

StyleLike = Union[str, Any]


def _style_strings(style: Union[StyleLike, Sequence[StyleLike]]) -> List[str]:
    """One style or a list (stacked bottom to top) as URL / JSON strings."""
    styles = list(style) if isinstance(style, (list, tuple)) else [style]
    if not styles:
        raise ValueError("at least one style is required")
    return [str(s) for s in styles]


def _bounds_to_3857(bounds: Any, crs: Optional[Any]) -> List[float]:
    import pyproj

    from .bbox_utils import normalize_bbox, reproject_bbox_to_3857

    xmin, ymin, xmax, ymax, source_crs = normalize_bbox(bounds, crs)
    if source_crs == pyproj.CRS.from_epsg(3857):
        return [xmin, ymin, xmax, ymax]
    return list(reproject_bbox_to_3857(xmin, ymin, xmax, ymax, source_crs))


def _device_px(css: int, pixel_ratio: float) -> int:
    """Device pixels for a CSS size, as the core rounds them (halves away
    from zero; Python's round() would round them to even)."""
    return max(1, int(math.floor(css * pixel_ratio + 0.5)))


def _check_pixel_ratio(pixel_ratio: float) -> None:
    if not (isinstance(pixel_ratio, (int, float)) and pixel_ratio > 0):
        raise ValueError("pixel_ratio must be a number > 0")


class Basemap:
    """A map view rendered headlessly: one style, or several stacked.

    Create one with :func:`basemap`; see there for the arguments. Change the
    view with :meth:`set_view` / :meth:`set_bounds` (which return the view, so
    calls chain), read it with the ``get_*`` methods, and render it with
    :meth:`render_png`, :meth:`render_svg` or :meth:`render_array`.
    """

    def __init__(
        self,
        style: Union[StyleLike, Sequence[StyleLike]],
        width: int = 800,
        height: int = 600,
        *,
        center: Optional[Tuple[float, float]] = None,
        zoom: Optional[float] = None,
        bounds: Optional[Any] = None,
        crs: Optional[Any] = None,
        pixel_ratio: float = 1.0,
        layers: Optional[List[str]] = None,
        auth_token: Optional[str] = None,
        tile_timeout_ms: int = 10_000,
        max_tiles: int = 256,
        tile_concurrency: int = 16,
        fail_on_tile_error: bool = False,
    ) -> None:
        self.styles = _style_strings(style)
        if int(width) < 1 or int(height) < 1:
            raise ValueError("width and height must be positive integers")
        _check_pixel_ratio(pixel_ratio)
        self.width = int(width)
        self.height = int(height)
        self.pixel_ratio = float(pixel_ratio)
        self.layers = layers
        self.auth_token = auth_token
        self.tile_timeout_ms = int(tile_timeout_ms)
        self.max_tiles = int(max_tiles)
        self.tile_concurrency = int(tile_concurrency)
        self.fail_on_tile_error = bool(fail_on_tile_error)
        self._center: Tuple[float, float] = (0.0, 0.0)
        self._zoom: float = 0.0
        if bounds is not None:
            if center is not None or zoom is not None:
                raise ValueError("give either bounds or center/zoom, not both")
            self.set_bounds(bounds, crs=crs)
        else:
            self.set_view(center=center or (0.0, 0.0), zoom=0.0 if zoom is None else zoom)

    # ── View ────────────────────────────────────────────────────────────────

    def set_view(self, center: Optional[Tuple[float, float]] = None, zoom: Optional[float] = None) -> "Basemap":
        """Set the centre ``(longitude, latitude)`` and/or the MapLibre zoom
        (0-22, fractional allowed); ``None`` keeps the current value."""
        if zoom is not None:
            if not 0 <= zoom <= 22:
                raise ValueError("zoom must be between 0 and 22")
            self._zoom = float(zoom)
        if center is not None:
            if len(center) != 2:
                raise ValueError("center must be (longitude, latitude)")
            self._center = (float(center[0]), float(center[1]))
        return self

    def set_bounds(self, bounds: Any, crs: Optional[Any] = None) -> "Basemap":
        """Fit the view to ``bounds`` (in ``crs``, default EPSG:4326), keeping
        the view's size."""
        from ._basemapper import fit_bounds

        lon, lat, zoom = fit_bounds(_bounds_to_3857(bounds, crs), self.width, self.height)
        self._center = (lon, lat)
        self._zoom = min(max(zoom, 0.0), 22.0)
        return self

    def get_bounds(self, lonlat: bool = False) -> Tuple[float, float, float, float]:
        """The view's ``(xmin, ymin, xmax, ymax)`` in EPSG:3857 metres, or in
        degrees with ``lonlat=True``."""
        from ._basemapper import bbox_from_center

        b = tuple(bbox_from_center(self._center[0], self._center[1], self._zoom, self.width, self.height))
        if not lonlat:
            return b
        import pyproj

        t = pyproj.Transformer.from_crs(3857, 4326, always_xy=True)
        w, s = t.transform(b[0], b[1])
        e, n = t.transform(b[2], b[3])
        return (w, s, e, n)

    def get_center(self) -> Tuple[float, float]:
        """``(longitude, latitude)`` of the view centre."""
        return self._center

    def get_zoom(self) -> float:
        """The MapLibre zoom of the view."""
        return self._zoom

    def get_size(self) -> Tuple[int, int]:
        """``(width, height)`` of rendered images in device pixels."""
        return (_device_px(self.width, self.pixel_ratio), _device_px(self.height, self.pixel_ratio))

    def __repr__(self) -> str:
        lon, lat = self._center
        n = len(self.styles)
        w, h = self.get_size()
        return (
            f"<Basemap {n} style{'' if n == 1 else 's'}, {self.width} x {self.height} @ {self.pixel_ratio:g}x "
            f"({w} x {h} px), center ({lon:.5f}, {lat:.5f}), zoom {self._zoom:.3f}>"
        )

    # ── Rendering ───────────────────────────────────────────────────────────

    def _options(self) -> dict:
        return dict(
            layers=self.layers,
            auth_token=self.auth_token,
            tile_timeout_ms=self.tile_timeout_ms,
            max_tiles=self.max_tiles,
            tile_concurrency=self.tile_concurrency,
            fail_on_tile_error=self.fail_on_tile_error,
        )

    def _render_buffer(self) -> Tuple[bytes, int, int]:
        from ._basemapper import render_raw

        return render_raw(
            list(self.get_bounds()), self.width, self.height, self.styles, self.pixel_ratio, **self._options()
        )

    def render_array(self) -> np.ndarray:
        """Render to a ``(height, width, 4)`` uint8 RGBA array (device pixels)."""
        buf, w, h = self._render_buffer()
        return np.frombuffer(buf, dtype=np.uint8).reshape(h, w, 4).copy()

    def render_png(self, path: Optional[str] = None) -> bytes:
        """Render a PNG, written to ``path`` when given. Returns the PNG bytes."""
        from ._basemapper import encode_png

        buf, w, h = self._render_buffer()
        png = encode_png(buf, w, h)
        if path is not None:
            with open(path, "wb") as f:
                f.write(png)
        return png

    def render_svg(
        self,
        path: Optional[str] = None,
        text: str = "editable",
        embed_fonts: str = "subset",
        font_dirs: Optional[List[str]] = None,
    ) -> str:
        """Render a vector SVG (``width`` x ``height`` user units;
        ``pixel_ratio`` does not apply), written to ``path`` when given.
        Returns the SVG text.

        Args:
            path: Output file, or None.
            text: ``"editable"``: labels are ``<text>`` elements (editable,
                selectable) at MapLibre's glyph positions, halos as strokes,
                with the style's fonts embedded; labels whose font file is not
                available fall back to ``"bitmap"``. ``"bitmap"``: every label
                drawn from the style's own glyph bitmaps, thresholded by SVG
                filters as MapLibre's shader does (MapLibre's look at any
                zoom; not editable). ``"paths"``: glyph outlines traced from
                those bitmaps (plain paths, no filters).
            embed_fonts: ``"subset"`` (only the glyphs used), ``"full"``
                (whole fonts, for retyping labels) or ``"none"`` (fonts by
                name only). Fonts come from ``font_dirs``, the
                ``BASEMAPPER_FONT_DIRS`` folders, a local cache, then Google
                Fonts for open fonts; others are referenced by name.
            font_dirs: Folders of .ttf / .otf files to use first.
        """
        from ._basemapper import render_svg_raw

        svg = render_svg_raw(
            list(self.get_bounds()), self.width, self.height, self.styles, **self._options(),
            text=text, embed_fonts=embed_fonts, font_dirs=font_dirs,
        )
        if path is not None:
            with open(path, "w", encoding="utf-8") as f:
                f.write(svg)
        return svg


def basemap(
    style: Union[StyleLike, Sequence[StyleLike]],
    width: int = 800,
    height: int = 600,
    *,
    center: Optional[Tuple[float, float]] = None,
    zoom: Optional[float] = None,
    bounds: Optional[Any] = None,
    crs: Optional[Any] = None,
    pixel_ratio: float = 1.0,
    layers: Optional[List[str]] = None,
    auth_token: Optional[str] = None,
    tile_timeout_ms: int = 10_000,
    max_tiles: int = 256,
    tile_concurrency: int = 16,
    fail_on_tile_error: bool = False,
) -> Basemap:
    """Create a basemap view.

    The view is a centre and MapLibre zoom (512 px tiles; fractional zooms are
    rendered natively, not resampled), or bounds to fit. ``width`` and
    ``height`` are CSS pixels; images are ``pixel_ratio`` times larger with
    the same view and label layout (MapLibre's ``devicePixelRatio``):
    ``pixel_ratio=2`` gives a sharp 2x image, ``pixel_ratio=dpi / 96`` matches
    a print resolution. The same function exists in R (``basemap()``) and
    Rust (``Basemap::new()``), with the same arguments.

    Args:
        style: A MapLibre GL style URL or JSON string (e.g. from
            :func:`basemap_provider`), or a list of those stacked bottom to
            top (e.g. a basemap published as separate hillshade, base and
            label services).
        width: View width in CSS pixels.
        height: View height in CSS pixels.
        center: ``(longitude, latitude)`` of the view centre.
        zoom: MapLibre zoom (0-22, fractional allowed).
        bounds: Bounds to fit instead of ``center``/``zoom``: a
            ``[xmin, ymin, xmax, ymax]`` sequence (in ``crs``, default
            EPSG:4326), a dict, or a GeoDataFrame / GeoSeries.
        crs: CRS of ``bounds`` (anything ``pyproj.CRS`` accepts).
        pixel_ratio: Device pixels per CSS pixel.
        layers: Optional layer filter applied to every style: plain IDs keep
            only those layers, ``-`` prefixed IDs drop them (see
            :func:`list_layers`).
        auth_token: Optional access token for the tile service.
        tile_timeout_ms: Per-tile HTTP timeout in milliseconds.
        max_tiles: Maximum tiles fetched per style.
        tile_concurrency: Parallel tile downloads.
        fail_on_tile_error: Abort on a failed tile instead of leaving it empty.

    Returns:
        A :class:`Basemap`.

    Example:
        >>> import basemapper
        >>> ugrc = "https://tiles.arcgis.com/tiles/99lidPhWCzftIe9K/arcgis/rest/services"
        >>> m = basemapper.basemap(
        ...     [f"{ugrc}/{s}/VectorTileServer/resources/styles/root.json"
        ...      for s in ("VectorHillshade", "LiteBase", "LiteLabels")],
        ...     800, 600, center=(-111.8887, 40.7556), zoom=12.4, pixel_ratio=2,
        ... )
        >>> m.render_png("slc.png")   # 1600 x 1200
        >>> m.render_svg("slc.svg")   # vector
    """
    return Basemap(
        style, width, height, center=center, zoom=zoom, bounds=bounds, crs=crs, pixel_ratio=pixel_ratio,
        layers=layers, auth_token=auth_token, tile_timeout_ms=tile_timeout_ms, max_tiles=max_tiles,
        tile_concurrency=tile_concurrency, fail_on_tile_error=fail_on_tile_error,
    )
