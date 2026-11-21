"""Standalone map rendering: the :class:`Map` object (modelled on pymgl)."""

from __future__ import annotations

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


class Map:
    """A map view rendered headlessly: one style, or several stacked.

    The view is a centre and MapLibre zoom (512 px tiles; fractional zooms are
    rendered natively, not resampled), or bounds to fit. ``width`` and
    ``height`` are CSS pixels; images are ``ratio`` times larger with the same
    view and label layout (MapLibre's ``devicePixelRatio``): ``ratio=2`` gives
    a sharp 2x image, ``ratio=dpi / 96`` matches a print resolution.

    Args:
        style: A MapLibre GL style URL or JSON string, a provider object, or a
            list of those stacked bottom to top (e.g. a basemap published as
            separate hillshade, base and label services).
        width: View width in CSS pixels.
        height: View height in CSS pixels.
        ratio: Device pixels per CSS pixel.
        center: ``(longitude, latitude)`` of the view centre.
        zoom: MapLibre zoom (0-22, fractional allowed).
        bounds: Bounds to fit instead of ``center``/``zoom``: a
            ``[xmin, ymin, xmax, ymax]`` sequence (in ``crs``, default
            EPSG:4326), a dict, or a GeoDataFrame / GeoSeries.
        crs: CRS of ``bounds`` (anything ``pyproj.CRS`` accepts).
        layers: Optional layer filter (plain IDs keep, ``-`` prefixed IDs
            drop), applied to every style.
        auth_token: Optional access token for the tile service.
        tile_timeout_ms: Per-tile HTTP timeout.
        max_tiles: Maximum tiles fetched per style.
        tile_concurrency: Parallel tile downloads.
        fail_on_tile_error: Abort on a failed tile instead of leaving it empty.

    Example:
        >>> import basemapper
        >>> ugrc = "https://tiles.arcgis.com/tiles/99lidPhWCzftIe9K/arcgis/rest/services"
        >>> m = basemapper.Map(
        ...     [f"{ugrc}/{s}/VectorTileServer/resources/styles/root.json"
        ...      for s in ("VectorHillshade", "LiteBase", "LiteLabels")],
        ...     width=800, height=600, ratio=2,
        ...     center=(-111.8887, 40.7556), zoom=12.4,
        ... )
        >>> m.render_png("slc.png")   # 1600 x 1200
        >>> m.render_svg("slc.svg")   # vector
    """

    def __init__(
        self,
        style: Union[StyleLike, Sequence[StyleLike]],
        width: int = 800,
        height: int = 600,
        ratio: float = 1.0,
        *,
        center: Optional[Tuple[float, float]] = None,
        zoom: Optional[float] = None,
        bounds: Optional[Any] = None,
        crs: Optional[Any] = None,
        layers: Optional[List[str]] = None,
        auth_token: Optional[str] = None,
        tile_timeout_ms: int = 10_000,
        max_tiles: int = 256,
        tile_concurrency: int = 16,
        fail_on_tile_error: bool = False,
    ) -> None:
        self.styles = _style_strings(style)
        if width < 1 or height < 1:
            raise ValueError("width and height must be positive")
        if not ratio > 0:
            raise ValueError("ratio must be > 0")
        self.width = int(width)
        self.height = int(height)
        self.ratio = float(ratio)
        self.layers = layers
        self.auth_token = auth_token
        self.tile_timeout_ms = tile_timeout_ms
        self.max_tiles = max_tiles
        self.tile_concurrency = tile_concurrency
        self.fail_on_tile_error = fail_on_tile_error
        self._center: Tuple[float, float] = (0.0, 0.0)
        self._zoom: float = 0.0
        if bounds is not None:
            if center is not None or zoom is not None:
                raise ValueError("give either bounds or center/zoom, not both")
            self.set_bounds(bounds, crs=crs)
        else:
            self.set_center(*(center or (0.0, 0.0)), zoom=0.0 if zoom is None else zoom)

    # ── View ────────────────────────────────────────────────────────────────

    @property
    def center(self) -> Tuple[float, float]:
        """``(longitude, latitude)`` of the view centre."""
        return self._center

    @property
    def zoom(self) -> float:
        """MapLibre zoom of the view."""
        return self._zoom

    def set_center(self, longitude: float, latitude: float, zoom: Optional[float] = None) -> "Map":
        """Centre the view on ``(longitude, latitude)``, optionally at ``zoom``."""
        if zoom is not None:
            self.set_zoom(zoom)
        self._center = (float(longitude), float(latitude))
        return self

    def set_zoom(self, zoom: float) -> "Map":
        """Set the MapLibre zoom (0-22, fractional allowed)."""
        if not 0 <= zoom <= 22:
            raise ValueError("zoom must be between 0 and 22")
        self._zoom = float(zoom)
        return self

    def set_bounds(self, bounds: Any, crs: Optional[Any] = None) -> "Map":
        """Fit the view to ``bounds`` (keeping the view's aspect ratio)."""
        from ._basemapper import fit_bounds

        lon, lat, zoom = fit_bounds(_bounds_to_3857(bounds, crs), self.width, self.height)
        self._center = (lon, lat)
        self._zoom = min(max(zoom, 0.0), 22.0)
        return self

    @property
    def bounds(self) -> Tuple[float, float, float, float]:
        """The view's ``(xmin, ymin, xmax, ymax)`` in EPSG:3857 metres."""
        from ._basemapper import bbox_from_center

        return tuple(bbox_from_center(self._center[0], self._center[1], self._zoom, self.width, self.height))

    @property
    def bounds_lonlat(self) -> Tuple[float, float, float, float]:
        """The view's ``(west, south, east, north)`` in degrees."""
        import pyproj

        t = pyproj.Transformer.from_crs(3857, 4326, always_xy=True)
        xmin, ymin, xmax, ymax = self.bounds
        w, s = t.transform(xmin, ymin)
        e, n = t.transform(xmax, ymax)
        return (w, s, e, n)

    @property
    def size(self) -> Tuple[int, int]:
        """``(width, height)`` of rendered images in device pixels."""
        return (round(self.width * self.ratio), round(self.height * self.ratio))

    def __repr__(self) -> str:
        lon, lat = self._center
        return (
            f"Map({len(self.styles)} style{'s' if len(self.styles) != 1 else ''}, "
            f"{self.width}x{self.height} @ {self.ratio:g}x, center=({lon:.5f}, {lat:.5f}), "
            f"zoom={self._zoom:.3f})"
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

    def render_buffer(self) -> Tuple[bytes, int, int]:
        """Render to straight-alpha RGBA bytes; returns ``(bytes, width, height)``."""
        from ._basemapper import render_raw

        return render_raw(list(self.bounds), self.width, self.height, self.styles, self.ratio, **self._options())

    def render_array(self) -> np.ndarray:
        """Render to a ``(height, width, 4)`` uint8 RGBA array (device pixels)."""
        buf, w, h = self.render_buffer()
        return np.frombuffer(buf, dtype=np.uint8).reshape(h, w, 4).copy()

    def render_png(self, path: Optional[str] = None) -> bytes:
        """Render a PNG; written to ``path`` when given. Returns the PNG bytes."""
        from ._basemapper import encode_png

        buf, w, h = self.render_buffer()
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
        """Render a vector SVG (``width`` x ``height`` user units; ``ratio``
        does not apply). Written to ``path`` when given. Returns the SVG text.

        Args:
            path: Output file, or None.
            text: ``"editable"``: labels are ``<text>`` elements (editable,
                selectable) at MapLibre's glyph positions, halos as strokes,
                with the style's fonts embedded. ``"paths"``: glyph outlines
                traced from the style's glyphs (not editable, identical
                everywhere, closest to MapLibre's pixels).
            embed_fonts: ``"subset"`` (only the glyphs used), ``"full"``
                (whole fonts, for retyping labels) or ``"none"`` (fonts by
                name only). Fonts come from ``font_dirs``, the
                ``BASEMAPPER_FONT_DIRS`` folders, a local cache, then Google
                Fonts for open fonts; others are referenced by name.
            font_dirs: Folders of .ttf / .otf files to use first.
        """
        from ._basemapper import render_svg_raw

        svg = render_svg_raw(
            list(self.bounds), self.width, self.height, self.styles, **self._options(),
            text=text, embed_fonts=embed_fonts, font_dirs=font_dirs,
        )
        if path is not None:
            with open(path, "w", encoding="utf-8") as f:
                f.write(svg)
        return svg
