"""The basemap: :func:`basemap` and :class:`Basemap` (docs/design/api-review.md
§4.1, §4.2, §4.6).

A basemap is a value: styles, what is given of the view, the map's CRS, a
clip and fetch options. It renders on its own (:meth:`Basemap.render`) or
goes into a plot (:meth:`Basemap.add_to`, ``ggplot() + m``), which supplies
what it leaves out. It never changes: :meth:`Basemap.set_view` returns a new
one. The same object exists in R (``basemap()``) and Rust (``Basemap``).
"""

from __future__ import annotations

import dataclasses
from typing import Any, Dict, List, Optional, Sequence, Tuple

from . import _crs
from ._fetch import FetchOptions, fetch_arg
from ._notes import Note, notes_of
from ._style import Style, styles_arg
from .exceptions import ValidationError

MIGRATION = "https://ar-puuk.github.io/basemapper/migrating-to-0.3.html"

# Arguments 0.3 removed (D1: no shims), and what replaces each.
REMOVED = {
    "pixel_ratio": "use dpi (dpi = 96 x pixel_ratio)",
    "tile_zoom": "use zoom (the MapLibre zoom)",
    "labels": 'filter the style: style(s, features="-labels"), or features="labels" for only the labels',
    "layers": "filter the style: style(s, layers=[...])",
    "features": "filter the style: style(s, features=[...])",
    "auth_token": "give the style its key: style(s, key=...)",
    "tile_timeout_ms": "use fetch=FetchOptions(timeout=seconds)",
    "max_tiles": "use fetch=FetchOptions(max_tiles=...)",
    "tile_concurrency": "use fetch=FetchOptions(concurrency=...)",
    "fail_on_tile_error": 'use fetch=FetchOptions(on_error="fail")',
}


def removed_arguments(fn: str, given: Dict[str, Any]) -> None:
    """A TypeError for keyword arguments ``fn`` does not take, saying what
    replaces the ones 0.3 removed."""
    for name in given:
        if name in REMOVED:
            raise TypeError(f"{fn}(): `{name}` was removed in basemapper 0.3; {REMOVED[name]}. See {MIGRATION}")
        raise TypeError(f"{fn}() got an unexpected keyword argument {name!r}")


_UNSET: Any = object()


@dataclasses.dataclass(frozen=True, eq=False)
class Basemap:
    """A basemap: styles, a view, a CRS, a clip and fetch options.

    Make one with :func:`basemap`, which describes the arguments. It never
    changes: :meth:`set_view` and :meth:`set_bounds` return a new basemap, so
    one basemap can feed several plots and renders.

    The view's values are properties (``m.bounds``, ``m.zoom``, ...), each
    given, derived or defaulted by the core's one rule (any three of extent,
    zoom, image size and dpi give the fourth).
    """

    styles: Tuple[Style, ...]
    bounds_arg: Any = None
    center_arg: Optional[Tuple[float, float]] = None
    zoom_arg: Optional[float] = None
    width_arg: Optional[float] = None
    height_arg: Optional[float] = None
    units: str = "px"
    dpi_arg: Optional[float] = None
    crs_arg: Any = None
    clip: Any = None
    fetch: FetchOptions = dataclasses.field(default_factory=FetchOptions)

    def __post_init__(self) -> None:
        # Resolve now: an impossible basemap is an error here, not later.
        object.__setattr__(self, "_prepared", self._prepare())

    # ── Resolving ───────────────────────────────────────────────────────────

    def _extent(self) -> Optional[_crs.Extent]:
        if self.bounds_arg is not None:
            return _crs.extent_of(self.bounds_arg)
        clip = self.clip
        if clip is not None and not isinstance(clip, _crs.Outside) and self.center_arg is None:
            return _crs.extent_of(clip)  # an inside clip frames the map
        return None

    def _base_inputs(self) -> Dict[str, Any]:
        inputs: Dict[str, Any] = {}
        if self.center_arg is not None:
            inputs["center"] = self.center_arg
        if self.zoom_arg is not None:
            inputs["zoom"] = float(self.zoom_arg)
        if self.width_arg is not None or self.height_arg is not None:
            if self.width_arg is None or self.height_arg is None:
                raise ValidationError("give both width and height, or neither")
            inputs.update(width=float(self.width_arg), height=float(self.height_arg), units=self.units)
        if self.dpi_arg is not None:
            inputs["dpi"] = float(self.dpi_arg)
        return inputs

    def _prepare(self) -> Dict[str, Any]:
        from . import _basemapper as _core

        extent = self._extent()
        crs = _crs.map_crs(self.crs_arg, extent)
        inputs = self._base_inputs()
        frame = None
        if _crs.is_web_mercator(crs):
            if extent is not None:
                inputs["bounds"] = list(_crs.box_in(extent, crs))
        else:
            if extent is not None:
                b = _crs.box_in(extent, crs)
                inputs["bounds"] = list(b)
                frame = _crs.frame(crs, ((b[0] + b[2]) / 2, (b[1] + b[3]) / 2))
            elif self.center_arg is not None:
                to = _crs._transformer(_crs.crs_of(_crs.LONLAT), crs)
                frame = _crs.frame(crs, to.transform(*self.center_arg))
            else:
                raise ValidationError(
                    f"a map in {_crs.crs_name(crs)} needs an extent: bounds (or a layer), or center with zoom"
                )
        clip = None
        if self.clip is not None:
            keep = "outside" if isinstance(self.clip, _crs.Outside) else "inside"
            clip = (_crs.clip_rings(self.clip, crs), keep)
        view = _core.resolve(inputs, frame)
        return {"crs": crs, "inputs": inputs, "frame": frame, "clip": clip, "view": view}

    @property
    def _view(self) -> Dict[str, Any]:
        return self._prepared["view"]

    # ── The view ────────────────────────────────────────────────────────────

    @property
    def crs(self):
        """The map's CRS, a ``pyproj.CRS``: ``crs`` if given, else the extent
        layer's if projected, else Web Mercator."""
        return self._prepared["crs"]

    @property
    def bounds(self) -> Tuple[float, float, float, float]:
        """``(xmin, ymin, xmax, ymax)`` of the map, in its CRS: at least the
        extent given, grown to the image's shape."""
        v = self._view
        return tuple(v.get("crs_bounds") or v["bounds"])

    @property
    def center(self) -> Tuple[float, float]:
        """``(longitude, latitude)`` of the map's centre."""
        return tuple(self._view["center"])

    @property
    def zoom(self) -> float:
        """The MapLibre zoom: the detail drawn."""
        return self._view["zoom"]

    @property
    def size(self) -> Tuple[int, int]:
        """``(width, height)`` of the image in pixels."""
        return (self._view["width"], self._view["height"])

    @property
    def dpi(self) -> float:
        """Image pixels per inch: the sharpness (96 = one pixel per CSS pixel)."""
        return self._view["dpi"]

    @property
    def scale(self) -> float:
        """The true map scale at the centre, N of 1:N."""
        return self._view["scale"]

    @property
    def provenance(self) -> Dict[str, str]:
        """Where each view value came from: ``extent``, ``size``, ``dpi`` and
        ``zoom``, each ``"given"``, ``"derived"`` or ``"default"``."""
        return dict(self._view["provenance"])

    @property
    def notes(self) -> Tuple[Note, ...]:
        """What the view decided (values nothing gave) or adjusted (to keep
        it inside the world)."""
        return notes_of(self._view["notes"]) + notes_of(self._view["decided"])

    # ── New basemaps ────────────────────────────────────────────────────────

    def set_view(
        self,
        *,
        center: Any = _UNSET,
        zoom: Any = _UNSET,
        width: Any = _UNSET,
        height: Any = _UNSET,
        units: Any = _UNSET,
        dpi: Any = _UNSET,
        **removed: Any,
    ) -> "Basemap":
        """A new basemap with a changed view (``None`` unsets a value). A
        ``center`` replaces the bounds (the camera instead of an area)."""
        removed_arguments("set_view", removed)
        changes: Dict[str, Any] = {}
        if center is not _UNSET:
            changes["center_arg"] = None if center is None else _crs._pair(center, "center")
            if center is not None:
                changes["bounds_arg"] = None
        for name, value in (("zoom_arg", zoom), ("width_arg", width), ("height_arg", height), ("dpi_arg", dpi)):
            if value is not _UNSET:
                changes[name] = value
        if units is not _UNSET:
            changes["units"] = units
        return dataclasses.replace(self, **changes)

    def set_bounds(self, bounds: Any) -> "Basemap":
        """A new basemap showing ``bounds`` (anything :func:`basemap` takes);
        the centre is unset."""
        return dataclasses.replace(self, bounds_arg=bounds, center_arg=None)

    # ── Rendering ───────────────────────────────────────────────────────────

    def _warp_mesh(self, prepared: Dict[str, Any]):
        if prepared["frame"] is None:
            return None
        v = prepared["view"]
        return _crs.mesh(prepared["crs"], v["crs_bounds"], v["width"], v["height"])

    def render(self, *, dpi: Optional[float] = None):
        """Render the map: a :class:`Rendering` (pixels, bounds, CRS, zoom,
        dpi, scale, notes). ``dpi`` is ``set_view(dpi=...).render()``: the
        same map, sharper or coarser.

        Notes are shown as their level asks: *adjusted* as a
        :class:`BasemapAdjustment` warning, *problems* as a
        :class:`BasemapProblem` warning (an error with ``on_error="fail"``),
        *decided* on the ``basemapper`` logger.
        """
        from . import _basemapper as _core
        from ._rendering import rendering_of

        m = self if dpi is None else self.set_view(dpi=dpi)
        p = m._prepared
        raw = _core.render(
            [s._spec() for s in m.styles], p["inputs"], m.fetch._spec(), p["clip"], p["frame"], m._warp_mesh(p),
        )
        return rendering_of(raw, stacklevel=2)

    def render_svg(
        self,
        path: Any = None,
        *,
        text: str = "editable",
        embed_fonts: str = "subset",
        font_dirs: Optional[List[str]] = None,
    ) -> str:
        """Render an SVG document, written to ``path`` when given: vector
        layers as paths and text, raster tiles as embedded images. Returns
        the SVG text. SVG is Web Mercator for now: a map in another CRS is an
        error.

        Args:
            path: A file to write, or None.
            text: ``"editable"``: labels as ``<text>`` with the style's fonts
                embedded (labels without a font file fall back to
                ``"bitmap"``); ``"bitmap"``: labels from the style's glyph
                bitmaps (MapLibre's look, not editable); ``"paths"``: glyph
                outlines as paths.
            embed_fonts: ``"subset"`` (only the glyphs used), ``"full"`` or
                ``"none"`` (fonts by name).
            font_dirs: Folders of .ttf / .otf files to use first.
        """
        from . import _basemapper as _core

        p = self._prepared
        svg = _core.render_svg(
            [s._spec() for s in self.styles], p["inputs"], self.fetch._spec(), p["clip"], p["frame"],
            text=text, embed_fonts=embed_fonts, font_dirs=font_dirs,
        )
        if path is not None:
            with open(path, "w", encoding="utf-8") as f:
                f.write(svg)
        return svg

    # ── Plots ───────────────────────────────────────────────────────────────

    def add_to(self, ax: Any, *, crs: Any = None, alpha: float = 1.0, attribution: Any = True):
        """Draw the basemap under a matplotlib axes' content, and return the
        artist.

        The plot gives the extent (the axes' limits), the image size (the
        axes' pixels) and the dpi (D16: on screen 96 x the pixel ratio, in a
        raster export the figure's dpi, in PDF / SVG 300); the basemap may
        give a ``zoom`` (then the dpi is derived) or a ``dpi``. It renders
        when the figure is drawn, so panning, zooming and ``savefig(dpi=...)``
        draw it afresh, natively. In any CRS other than Web Mercator the
        image is warped into it (no stretching).

        Args:
            ax: A matplotlib Axes.
            crs: The axes' CRS (anything ``pyproj.CRS`` accepts, e.g.
                ``gdf.crs``): plain axes carry none, so give it; read from
                cartopy GeoAxes when omitted.
            alpha: The basemap's opacity (0-1).
            attribution: The providers' credit line in the bottom-right
                corner: True (their own), False (none), or your text.

        Returns:
            The basemap's image artist; its ``rendering`` is the last render.
        """
        from .matplotlib_integration import place

        return place(self, ax, crs=crs, alpha=alpha, attribution=attribution)

    def __radd__(self, plot: Any):
        # plotnine: ggplot(...) + m
        from .plotnine_integration import geom_basemap

        return geom_basemap(self).__radd__(plot)

    def _plot_parts(self) -> Tuple[List[Dict[str, Any]], Dict[str, Any]]:
        """The style specs and view inputs a plot renders with (the plot
        supplies the extent, size and dpi; the core rejects the rest)."""
        if self.crs_arg is not None:
            raise ValidationError(
                "a basemap in a plot takes its CRS from the plot: remove crs (give the plot's CRS to add_to(ax, crs=...))"
            )
        from . import _basemapper as _core

        inputs = self._base_inputs()
        if self.bounds_arg is not None or self.center_arg is not None:
            inputs["bounds" if self.bounds_arg is not None else "center"] = (
                [0.0, 0.0, 1.0, 1.0] if self.bounds_arg is not None else self.center_arg
            )
        # The core's plot rules, on a stand-in panel: a conflict is an error
        # now, not when the plot is drawn.
        probe = {"limits": [0.0, 0.0, 1.0, 1.0], "crs": "EPSG:3857", "px": (100, 100),
                 "device": {"kind": "raster_export", "dpi": 96.0}}
        _core.plan(inputs, probe)
        return [s._spec() for s in self.styles], inputs

    # ── Display ─────────────────────────────────────────────────────────────

    def __repr__(self) -> str:
        n = len(self.styles)
        w, h = self.size
        auto = " (auto)" if self.provenance["zoom"] == "derived" else ""
        lon, lat = self.center
        return (
            f"<Basemap {n} style{'' if n == 1 else 's'}, {w} x {h} px at {self.dpi:g} dpi, zoom {self.zoom:.2f}{auto}, "
            f"{_crs.crs_name(self.crs)}, center ({lon:.4f}, {lat:.4f}), 1:{self.scale:,.0f}>"
        )

    def _repr_png_(self):
        """Show the map in Jupyter and Quarto at its CSS size (sharp at any dpi)."""
        return self.render()._repr_png_()


def basemap(
    style: Any,
    *,
    bounds: Any = None,
    center: Optional[Sequence[float]] = None,
    zoom: Optional[float] = None,
    width: Optional[float] = None,
    height: Optional[float] = None,
    units: str = "px",
    dpi: Optional[float] = None,
    crs: Any = None,
    clip: Any = None,
    fetch: Optional[FetchOptions] = None,
    **removed: Any,
) -> Basemap:
    """Create a basemap.

    Three controls, each with one meaning everywhere: the extent (``bounds``,
    or ``center`` with ``zoom``), ``zoom`` (the feature detail) and ``dpi``
    (the sharpness); ``width`` / ``height`` and :func:`around` express them in
    other terms. Any three of extent, zoom, image size and dpi give the
    fourth. What nothing gives is defaulted, in this order: dpi (96), image
    size (800 x 600 at 96 dpi, scaled with the dpi), extent (the world); the
    zoom is never a default. Over-determined requests are errors that say
    what to drop.

    Args:
        style: A style, or a list stacked bottom to top: a catalogue name
            (``"OpenFreeMap.Liberty"``), a style URL or JSON, tile data, or a
            :func:`style`.
        bounds: The area to show: ``(west, south, east, north)`` in
            longitude / latitude (west > east across the date line),
            :func:`around` (a ground distance around a point), or a spatial
            layer (GeoDataFrame, GeoSeries; its CRS is read).
        center: ``(longitude, latitude)`` of the camera; needs ``zoom``.
        zoom: The MapLibre zoom (0-22, fractional): which roads, places and
            labels the style draws.
        width: The image width, in ``units``.
        height: The image height, in ``units``.
        units: ``"px"`` (default), ``"in"``, ``"cm"`` or ``"mm"``; with a
            physical unit, ``dpi`` sets only the pixel count.
        dpi: Image pixels per inch: the sharpness (default 96).
        crs: The map's CRS (anything ``pyproj.CRS`` accepts, or an object
            with a ``crs``). Default: the ``bounds`` layer's CRS when it is
            projected, else Web Mercator. In another CRS the map is drawn in
            Web Mercator and warped into it.
        clip: Draw only inside a layer's polygons (a GeoDataFrame,
            GeoSeries), or outside them with :func:`outside`. An inside clip
            also gives the default extent (and CRS, when projected).
        fetch: How the data is downloaded: a :class:`FetchOptions`.

    Returns:
        A :class:`Basemap`.

    Example:
        >>> import basemapper
        >>> m = basemapper.basemap("OpenFreeMap.Liberty", center=(-111.8887, 40.7556), zoom=12.4, dpi=192)
        >>> img = m.render()          # 1600 x 1200 px, sharp
        >>> img.save("slc.png")
        >>> m.render_svg("slc.svg")   # vector
    """
    removed_arguments("basemap", removed)
    return Basemap(
        styles_arg(style),
        bounds_arg=bounds,
        center_arg=None if center is None else _crs._pair(center, "center"),
        zoom_arg=zoom,
        width_arg=width,
        height_arg=height,
        units=units,
        dpi_arg=dpi,
        crs_arg=crs,
        clip=clip,
        fetch=fetch_arg(fetch),
    )
