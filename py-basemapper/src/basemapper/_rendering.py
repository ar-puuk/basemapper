"""Renderings: one render, many conversions (docs/design/api-review.md §4.4)."""

from __future__ import annotations

from typing import Any, Dict, Optional, Tuple

import numpy as np

from ._notes import Note, notes_of


class Rendering:
    """A rendered map: its pixels and where they are.

    Made by :meth:`Basemap.render`. The same in R (``render(m)``) and Rust.

    Attributes:
        width: Image width in pixels.
        height: Image height in pixels.
        bounds: ``(xmin, ymin, xmax, ymax)`` of the image in ``crs`` units
            (``xmin > xmax`` across the date line in Web Mercator).
        crs: The image's CRS, a ``pyproj.CRS``.
        zoom: The MapLibre zoom the map was drawn at.
        tile_zoom: The zoom of the most detailed tiles fetched.
        dpi: Image pixels per inch: the sharpness (in a plot, the figure's).
        symbol_scale: Lines, labels and icons as a multiple of the style's
            designed size: given, or what the zoom made it.
        scale: The true map scale at the centre, N of 1:N (the same at any dpi).
        notes: What basemapper decided, adjusted, or could not do
            (:class:`Note`).
    """

    __slots__ = ("_pixels", "width", "height", "bounds", "crs", "zoom", "tile_zoom", "dpi", "symbol_scale", "scale", "notes")

    def __init__(self, raw: Dict[str, Any]) -> None:
        from ._crs import crs_of

        self._pixels: bytes = raw["pixels"]
        self.width: int = raw["width"]
        self.height: int = raw["height"]
        self.bounds: Tuple[float, float, float, float] = tuple(raw["bounds"])
        self.crs = crs_of(raw["crs"])
        self.zoom: float = raw["zoom"]
        self.tile_zoom: int = raw["tile_zoom"]
        self.dpi: float = raw["dpi"]
        self.symbol_scale: float = raw["symbol_scale"]
        self.scale: float = raw["scale"]
        self.notes: Tuple[Note, ...] = notes_of(raw["notes"])

    def to_numpy(self) -> np.ndarray:
        """The pixels as a ``(height, width, 4)`` uint8 RGBA array (straight
        alpha). A read-only view of the rendering's memory, without a copy;
        ``.copy()`` it to change it."""
        return np.frombuffer(self._pixels, dtype=np.uint8).reshape(self.height, self.width, 4)

    def to_png(self) -> bytes:
        """The image as PNG bytes."""
        from . import _basemapper as _core

        return _core.encode_png(self._pixels, self.width, self.height)

    def save(self, path: Any) -> None:
        """Write the image as a PNG file."""
        with open(path, "wb") as f:
            f.write(self.to_png())

    def to_xarray(self):
        """The image as an ``xarray.DataArray`` ``(y, x, band)`` with pixel-centre
        coordinates in ``crs`` units and the CRS in ``attrs["crs"]`` (WKT), for
        rioxarray and friends. Needs xarray (``pip install 'basemapper[matplotlib]'``)."""
        try:
            import xarray as xr
        except ImportError as e:
            raise ImportError("to_xarray() needs xarray: pip install xarray") from e
        xmin, ymin, xmax, ymax = self.bounds
        width_units = xmax - xmin
        if width_units <= 0 and self.crs.to_epsg() == 3857:  # across the date line
            width_units += 2 * 20_037_508.342_789_244
        x = xmin + (np.arange(self.width) + 0.5) * width_units / self.width
        y = ymax - (np.arange(self.height) + 0.5) * (ymax - ymin) / self.height
        return xr.DataArray(
            self.to_numpy(),
            dims=("y", "x", "band"),
            coords={"y": y, "x": x, "band": ["r", "g", "b", "a"]},
            attrs={"crs": self.crs.to_wkt(), "zoom": self.zoom, "dpi": self.dpi, "scale": self.scale},
        )

    def css_size(self) -> Tuple[float, float]:
        """The image's size in CSS pixels (1/96 inch): how big it shows."""
        k = 96.0 / self.dpi
        return self.width * k, self.height * k

    def _repr_png_(self):
        w, h = self.css_size()
        return self.to_png(), {"width": round(w), "height": round(h)}

    def __repr__(self) -> str:
        from ._crs import crs_name

        n = len(self.notes)
        return (
            f"<Rendering {self.width} x {self.height} px, {crs_name(self.crs)}, zoom {self.zoom:.2f}, "
            f"{self.dpi:g} dpi, 1:{self.scale:,.0f}, {n} note{'' if n == 1 else 's'}>"
        )

    def notes_at(self, level: str) -> Tuple[Note, ...]:
        """The notes at ``level``: ``"decided"``, ``"adjusted"`` or ``"problem"``."""
        return tuple(n for n in self.notes if n.level == level)


def rendering_of(raw: Dict[str, Any], stacklevel: Optional[int] = None) -> Rendering:
    """A :class:`Rendering`, its notes shown as their level asks, attributed
    to the user's line: ``stacklevel`` frames above the caller of this
    function (1: the caller's own caller)."""
    from ._notes import surface

    r = Rendering(raw)
    if stacklevel is not None:
        # surface() and this function are two frames below the caller.
        surface(r.notes, stacklevel=stacklevel + 2)
    return r


# The last rendering a plot drew (matplotlib or plotnine).
_LAST: Dict[str, Any] = {}


def last_rendering() -> Optional["Rendering"]:
    """The last basemap a plot drew.

    A plot renders its basemap when it is drawn (shown, saved), for the
    axes' size and the figure's dpi. This returns that rendering, to read
    what it was drawn at: ``zoom``, ``symbol_scale``, ``dpi``, ``width``,
    ``height``, ``scale`` (the printed 1:N) and ``notes``. With several axes or
    basemaps, it is the last one drawn. R: ``last_rendering()``.

    Returns:
        A :class:`Rendering`, or None before any plot drew a basemap.
    """
    return _LAST.get("rendering")
