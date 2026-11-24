"""matplotlib: ``m.add_to(ax)`` and its alias :func:`add_basemap`
(docs/design/api-review.md §4.5, D14, D16).

The basemap is an image artist that renders when the figure is drawn, for
the axes' current limits, pixels and device, through the core's plot plan:
the same plan as plotnine (and R's ggplot2 and tmap). In a CRS other than
Web Mercator the image is warped into it, so it lines up with the data (F1).
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict, Optional, Sequence, Tuple, Union

from matplotlib.backends.backend_agg import FigureCanvasAgg
from matplotlib.backend_bases import FigureCanvasBase
from matplotlib.image import AxesImage
from matplotlib.text import Text

from . import _crs
from .exceptions import ValidationError

if TYPE_CHECKING:
    import matplotlib.axes

    from ._basemap import Basemap


# ── The device (D16) ─────────────────────────────────────────────────────────


def device_of(fig: Any, renderer: Any, bb: Any, dpi: Optional[float]) -> Tuple[Dict[str, Any], Tuple[int, int]]:
    """The device a figure is drawn on, and the axes' size in its pixels.

    - A vector export (PDF, SVG, PS): no pixels of its own; the image is
      the axes' size at the basemap's ``dpi``, else 300.
    - A raster export (``savefig``), or a canvas that is no screen: the
      renderer's dpi.
    - A screen: 96 x its pixel ratio.
    """
    if hasattr(renderer, "_vector_renderer"):
        k = (dpi or 300.0) / 72.0  # the extent is in points
        return {"kind": "vector_export"}, (max(1, round(bb.width * k)), max(1, round(bb.height * k)))
    px = (max(1, round(bb.width)), max(1, round(bb.height)))
    canvas = fig.canvas
    interactive = type(canvas) not in (FigureCanvasAgg, FigureCanvasBase)
    if interactive and not getattr(canvas, "_is_saving", False):
        return {"kind": "screen", "pixel_ratio": float(getattr(canvas, "device_pixel_ratio", 1.0) or 1.0)}, px
    return {"kind": "raster_export", "dpi": float(getattr(renderer, "dpi", None) or fig.dpi)}, px


def plot_crs(ax: Any, crs: Any):
    """The axes' CRS: given, or a cartopy GeoAxes' projection."""
    if crs is not None:
        return _crs.crs_of(crs)
    projection = getattr(ax, "projection", None)
    if projection is not None:
        try:
            return _crs.crs_of(projection)
        except ValidationError:
            pass
    raise ValidationError(
        "the axes carry no CRS: give it, e.g. m.add_to(ax, crs=gdf.crs) or crs=\"EPSG:4326\" "
        "(cartopy GeoAxes are read automatically)"
    )


# ── Attribution ──────────────────────────────────────────────────────────────

_ATTRIBUTION: Dict[Any, str] = {}


def attribution_text(attribution: Union[bool, str, None], styles: Sequence[Any]) -> str:
    """The credit line ``attribution`` asks for: True the styles' own (cached
    per style set; empty when it cannot be read), False / None none, a
    string as given."""
    if attribution is None or attribution is False:
        return ""
    if isinstance(attribution, str):
        return attribution
    if attribution is not True:
        raise ValidationError(f"attribution must be True, False or a string, not {attribution!r}")
    from . import _basemapper as _core

    key = tuple(styles)
    if key not in _ATTRIBUTION:
        try:
            _ATTRIBUTION[key] = _core.attribution([s._spec() for s in styles])
        except Exception:
            _ATTRIBUTION[key] = ""
    return _ATTRIBUTION[key]


class AttributionText(Text):
    """The credit line in the axes' bottom-right corner, laid out by the core
    (the same layout as R's) for the axes' width whenever it is drawn."""

    def __init__(self, ax: "matplotlib.axes.Axes", text: str) -> None:
        super().__init__(0.0, 0.0, "", transform=ax.transAxes, ha="right", va="bottom", zorder=10, linespacing=1.0)
        self._full_text = text
        self.set_gid("basemap_attribution")

    def draw(self, renderer: Any) -> None:
        from . import _basemapper as _core

        ax = self.axes
        bb = ax.get_window_extent(renderer)
        width_pt = bb.width * 72.0 / renderer.points_to_pixels(72.0)
        layout = _core.attribution_layout(self._full_text, width_pt)
        if layout is None:
            return
        lines, size_pt, color, background, alpha, pad_pt = layout
        self.set_text("\n".join(lines))
        self.set_fontsize(size_pt)
        self.set_color(color)
        self.set_bbox(dict(boxstyle=f"square,pad={pad_pt / size_pt:.3f}", fc=background, ec="none", alpha=alpha))
        # The box's corner at the axes' corner: the text inset by the padding.
        inset = renderer.points_to_pixels(2 * pad_pt)
        self.set_position((1.0 - inset / max(bb.width, 1.0), inset / max(bb.height, 1.0)))
        super().draw(renderer)


def add_attribution(ax: "matplotlib.axes.Axes", text: str) -> Optional[AttributionText]:
    """Add the credit line to ``ax`` (nothing for empty text)."""
    if not text:
        return None
    artist = AttributionText(ax, text)
    ax.add_artist(artist)
    return artist


# ── The basemap artist ───────────────────────────────────────────────────────


class BasemapImage(AxesImage):
    """An image artist that renders the basemap when it is drawn, for the
    axes' limits, pixels and device: panning, zooming and ``savefig(dpi=...)``
    draw it afresh, natively. Renders are kept per (limits, pixels, device).

    Attributes:
        rendering: The last :class:`Rendering`, or None before the first draw.
    """

    def __init__(self, ax: "matplotlib.axes.Axes", basemap: "Basemap", crs: Any, alpha: float = 1.0) -> None:
        super().__init__(ax, interpolation="nearest", origin="upper", zorder=0, alpha=alpha)
        self._basemap = basemap
        self._crs = crs
        self._styles, self._inputs = basemap._plot_parts()
        self._clip = None
        if basemap.clip is not None:
            keep = "outside" if isinstance(basemap.clip, _crs.Outside) else "inside"
            self._clip = (_crs.clip_rings(basemap.clip, crs), keep)
        self._key: Optional[tuple] = None
        self.rendering = None

    def refresh(self, renderer: Any) -> None:
        """Render for the axes' current state, if it changed."""
        from . import _basemapper as _core
        from ._rendering import rendering_of

        ax = self.axes
        (x0, x1), (y0, y1) = ax.get_xlim(), ax.get_ylim()
        limits = [min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)]
        bb = ax.get_window_extent(renderer)
        device, px = device_of(ax.figure, renderer, bb, self._inputs.get("dpi"))
        key = (tuple(limits), px, tuple(sorted(device.items())))
        if key == self._key:
            return
        panel, mesh = _crs.panel_spec(limits, self._crs, px, device)
        raw = _core.render(self._styles, self._inputs, self._basemap.fetch._spec(), self._clip, None, mesh, panel)
        r = rendering_of(raw, stacklevel=4)
        self.set_data(r.to_numpy())
        # The image covers the panel's limits (set directly: set_extent
        # would touch the data limits). Flipped axes flip the image.
        self._extent = (x0, x1, y0, y1)
        self._key = key
        self.rendering = r
        from ._rendering import _LAST

        _LAST["rendering"] = r

    def draw(self, renderer: Any) -> None:
        try:
            self.refresh(renderer)
        except Exception:
            if self.rendering is None:
                raise
            # Keep showing the previous render if a re-render fails.
        super().draw(renderer)


def _renderer(fig: Any) -> Any:
    try:
        return fig.canvas.get_renderer()
    except Exception:
        return FigureCanvasAgg(fig).get_renderer()


def place(basemap: "Basemap", ax: Any, *, crs: Any = None, alpha: float = 1.0, attribution: Any = True) -> BasemapImage:
    """Put ``basemap`` under ``ax``'s content (see :meth:`Basemap.add_to`)."""
    crs = plot_crs(ax, crs)
    xlim, ylim = ax.get_xlim(), ax.get_ylim()
    img = BasemapImage(ax, basemap, crs, alpha=alpha)
    # Render once now, so a bad basemap is an error here.
    img.refresh(_renderer(ax.figure))
    ax.add_image(img)
    add_attribution(ax, attribution_text(attribution, basemap.styles))
    # The axes' background would cover the basemap (zorder 0): make it clear.
    ax.set_facecolor("none")
    ax.set_xlim(xlim)
    ax.set_ylim(ylim)
    return img


def add_basemap(
    ax: "matplotlib.axes.Axes",
    style: Any,
    *,
    crs: Any = None,
    alpha: float = 1.0,
    attribution: Any = True,
    **basemap_args: Any,
) -> BasemapImage:
    """Draw a basemap under a matplotlib axes' content: an alias of
    ``basemapper.basemap(style, ...).add_to(ax, ...)``.

    Args:
        ax: A matplotlib Axes.
        style: A style, a list of styles, or a :class:`Basemap`.
        crs: The axes' CRS (see :meth:`Basemap.add_to`).
        alpha: The basemap's opacity (0-1).
        attribution: The credit line: True, False, or your text.
        **basemap_args: :func:`basemap`'s arguments that apply in a plot:
            ``zoom`` or ``symbol_scale``, ``clip``, ``fetch``.

    Returns:
        The basemap's image artist; its ``rendering`` is the last render.
    """
    from ._basemap import Basemap, basemap

    if isinstance(style, Basemap):
        if basemap_args:
            raise TypeError("add_basemap(ax, m): a Basemap already has its arguments; change them with m.set_view(...)")
        m = style
    else:
        m = basemap(style, **basemap_args)
    return m.add_to(ax, crs=crs, alpha=alpha, attribution=attribution)
