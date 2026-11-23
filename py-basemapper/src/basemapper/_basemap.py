"""Standalone map rendering: :func:`basemap` and the :class:`Basemap` view.

The same API exists in R (``basemap()``, ``set_view(m, ...)``,
``render_png(m, ...)``) and Rust (``Basemap::new()``): the same verbs and
argument names, with R's ``verb(m, ...)`` written ``m.verb(...)`` here.
"""

from __future__ import annotations

import math
from typing import Any, List, Optional, Sequence, Tuple, Union

import numpy as np

from ._fetch import FetchOptions, _resolve_fetch
from ._render import _features_arg, labels_arg

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
        labels: Union[bool, str] = True,
        features: Optional[List[str]] = None,
        auth_token: Optional[str] = None,
        fetch: Optional[FetchOptions] = None,
        **deprecated: Any,
    ) -> None:
        self.fetch = _resolve_fetch(fetch, "Basemap", deprecated)
        self.styles = _style_strings(style)
        if int(width) < 1 or int(height) < 1:
            raise ValueError("width and height must be positive integers")
        _check_pixel_ratio(pixel_ratio)
        self.width = int(width)
        self.height = int(height)
        self.pixel_ratio = float(pixel_ratio)
        self.layers = layers
        labels_arg(labels)  # validate now
        self.labels = labels
        self.features = _features_arg(features)
        self.auth_token = auth_token
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

        lon, lat, zoom = self._view()
        b = tuple(bbox_from_center(lon, lat, zoom, self.width, self.height))
        if not lonlat:
            return b
        import pyproj

        t = pyproj.Transformer.from_crs(3857, 4326, always_xy=True)
        w, s = t.transform(b[0], b[1])
        e, n = t.transform(b[2], b[3])
        return (w, s, e, n)

    def _view(self) -> Tuple[float, float, float]:
        """The view shown: the requested centre and zoom, constrained as
        MapLibre constrains them (never beyond the poles; at least the zoom
        at which the world fills the view)."""
        from ._basemapper import constrain_view

        return tuple(constrain_view(self._center[0], self._center[1], self._zoom, self.width, self.height))

    def get_center(self) -> Tuple[float, float]:
        """``(longitude, latitude)`` of the view centre, as shown: a centre
        whose view would reach past a pole is moved so it stops there."""
        lon, lat, _ = self._view()
        return (lon, lat)

    def get_zoom(self) -> float:
        """The MapLibre zoom of the view, as shown: at least the zoom at
        which the world fills the view."""
        return self._view()[2]

    def get_size(self) -> Tuple[int, int]:
        """``(width, height)`` of rendered images in device pixels."""
        return (_device_px(self.width, self.pixel_ratio), _device_px(self.height, self.pixel_ratio))

    def get_attribution(self) -> str:
        """The attribution text the view's styles ask maps to show (see
        :func:`get_attribution`), e.g. for a caption."""
        from ._basemapper import attribution

        return attribution(self.styles, self.auth_token)

    def __repr__(self) -> str:
        lon, lat, zoom = self._view()
        n = len(self.styles)
        w, h = self.get_size()
        return (
            f"<Basemap {n} style{'' if n == 1 else 's'}, {self.width} x {self.height} @ {self.pixel_ratio:g}x "
            f"({w} x {h} px), center ({lon:.5f}, {lat:.5f}), zoom {zoom:.3f}>"
        )

    def _repr_png_(self):
        """Show the rendered view in Jupyter and Quarto, at its CSS pixel size
        (sharp at any ``pixel_ratio``). ``repr(m)`` describes the view."""
        return self.render_png(), {"width": self.width, "height": self.height}

    # ── Rendering ───────────────────────────────────────────────────────────

    def _options(self) -> dict:
        return dict(
            layers=self.layers,
            labels=labels_arg(self.labels),
            features=self.features,
            auth_token=self.auth_token,
            **self.fetch._core(),
        )

    def _render_buffer(self) -> Tuple[bytes, int, int]:
        from ._render import render_call

        return render_call(
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
        """Render an SVG (``width`` x ``height`` user units; ``pixel_ratio``
        does not apply), written to ``path`` when given: vector layers as
        paths and text, raster tiles (raster basemaps, hillshade) as embedded
        images. Returns the SVG text.

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
    labels: Union[bool, str] = True,
    features: Optional[List[str]] = None,
    auth_token: Optional[str] = None,
    fetch: Optional[FetchOptions] = None,
    **deprecated: Any,
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
            only those layers, ``-`` prefixed IDs drop them; ``*`` and ``?``
            are wildcards (``"-label_*"``). See :func:`list_layers`.
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
        fetch: How tiles are downloaded: a :class:`FetchOptions` (timeout,
            tile limit, parallel downloads, what a failed tile does).
        **deprecated: ``tile_timeout_ms``, ``max_tiles``,
            ``tile_concurrency`` and ``fail_on_tile_error`` are still
            accepted, with a DeprecationWarning: use ``fetch``.

    Returns:
        A :class:`Basemap`.

    Example:
        >>> import basemapper
        >>> # UGRC's Lite basemap: its hillshade, base and labels services
        >>> m = basemapper.basemap(
        ...     "UGRC.Lite", 800, 600, center=(-111.8887, 40.7556), zoom=12.4, pixel_ratio=2,
        ... )
        >>> m.render_png("slc.png")   # 1600 x 1200
        >>> m.render_svg("slc.svg")   # vector
    """
    fetch = _resolve_fetch(fetch, "basemap", deprecated)
    return Basemap(
        style, width, height, center=center, zoom=zoom, bounds=bounds, crs=crs, pixel_ratio=pixel_ratio,
        layers=layers, labels=labels, features=features, auth_token=auth_token, fetch=fetch,
    )
