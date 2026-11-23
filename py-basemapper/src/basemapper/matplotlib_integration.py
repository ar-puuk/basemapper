"""matplotlib integration: add_basemap()."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, List, Optional, Sequence, Union

import numpy as np

from matplotlib.image import AxesImage
from matplotlib.text import Text

from ._render import _features_arg, labels_arg
from .bbox_utils import detect_crs_from_axes

if TYPE_CHECKING:
    from ._fetch import FetchOptions
    import matplotlib.axes
    import xarray as xr


def _output_dpi(figure: Any, renderer: Any) -> float:
    """Resolution images are drawn at: the savefig / canvas dpi for raster
    output, the image dpi for vector output (PDF, SVG, PS)."""
    for r in (renderer, getattr(renderer, "_renderer", None)):
        d = getattr(r, "image_dpi", None)
        if d:
            return float(d)
    return float(getattr(renderer, "dpi", None) or figure.dpi)


_ATTRIBUTION_CACHE: dict = {}


def attribution_text(attribution: Union[bool, str, None], styles: Any, auth_token: Optional[str]) -> str:
    """The text a plot layer's ``attribution`` asks for: True the styles' own
    (cached per style set; empty when it cannot be read), False / None none,
    a string as given."""
    if attribution is None or attribution is False:
        return ""
    if isinstance(attribution, str):
        return attribution
    if attribution is not True:
        raise ValueError(f"attribution must be True, False or a string, not {attribution!r}")
    from ._basemap import _style_strings
    from ._basemapper import attribution as _attribution

    key = (tuple(_style_strings(styles)), auth_token)
    if key not in _ATTRIBUTION_CACHE:
        try:
            _ATTRIBUTION_CACHE[key] = _attribution(list(key[0]), auth_token)
        except Exception:
            _ATTRIBUTION_CACHE[key] = ""
    return _ATTRIBUTION_CACHE[key]


class AttributionText(Text):
    """The credit line in the bottom-right corner of the axes, over the
    plot, rewrapped to the axes width whenever it is drawn."""

    def __init__(self, ax: "matplotlib.axes.Axes", text: str, fontsize: float = 6.0) -> None:
        super().__init__(
            0.995, 0.005, text, transform=ax.transAxes, ha="right", va="bottom", fontsize=fontsize,
            color="0.15", zorder=10, bbox=dict(boxstyle="square,pad=0.2", fc="white", ec="none", alpha=0.7),
        )
        self._full_text = text
        self.set_gid("basemap_attribution")

    def draw(self, renderer: Any) -> None:
        import textwrap

        ax = self.axes
        width_pt = ax.get_window_extent(renderer).width * 72.0 / ax.figure.dpi
        chars = max(20, int(width_pt / (self.get_fontsize() * 0.5)))
        self.set_text(textwrap.fill(self._full_text, chars))
        super().draw(renderer)


def add_attribution(ax: "matplotlib.axes.Axes", text: str) -> Optional[AttributionText]:
    """Add the credit line to ``ax`` (nothing for empty text)."""
    if not text:
        return None
    artist = AttributionText(ax, text)
    ax.add_artist(artist)
    return artist


class BasemapImage(AxesImage):
    """An image artist that renders the basemap when it is drawn, for the
    axes' current extent, size and output resolution.

    Rendering at draw time means ``savefig(dpi=300)`` (or a PDF) gets a
    basemap rendered natively at that resolution rather than an upscaled
    screen image, and panning or zooming re-renders the visible area. Renders
    are cached per (extent, size, resolution).
    """

    def __init__(
        self,
        ax: "matplotlib.axes.Axes",
        styles: Any,
        crs: Any,
        pixel_ratio: Optional[float],
        tile_zoom: Optional[int],
        options: dict,
        alpha: float = 1.0,
    ) -> None:
        super().__init__(ax, interpolation="nearest", origin="upper", zorder=0, alpha=alpha)
        self._styles = styles
        self._crs = crs
        self._pixel_ratio = pixel_ratio
        self._tile_zoom = tile_zoom
        self._options = options
        self._key: Optional[tuple] = None
        self.last_render: Optional[dict] = None

    def refresh(self, renderer: Any) -> None:
        """Render for the axes' current state if it changed since last time."""
        from ._render import _render_buffer

        ax = self.axes
        fig = ax.figure
        xlim, ylim = ax.get_xlim(), ax.get_ylim()
        bb = ax.get_window_extent(renderer)
        dpi_now = fig.dpi  # the dpi the window extent is measured in
        w_in, h_in = bb.width / dpi_now, bb.height / dpi_now
        out_dpi = _output_dpi(fig, renderer)
        # CSS px are 1/96 in: by default draw at the output resolution with
        # map symbols at their physical size (pixel_ratio = dpi / 96).
        ratio = self._pixel_ratio if self._pixel_ratio is not None else out_dpi / 96.0
        css_w = max(1, round(w_in * out_dpi / ratio))
        css_h = max(1, round(h_in * out_dpi / ratio))
        key = (tuple(xlim), tuple(ylim), css_w, css_h, ratio)
        if key == self._key:
            return
        buf, w, h = _render_buffer(
            [xlim[0], ylim[0], xlim[1], ylim[1]], css_w, css_h, self._styles,
            crs=self._crs, pixel_ratio=ratio, tile_zoom=self._tile_zoom, **self._options,
        )
        arr = np.frombuffer(buf, dtype=np.uint8).reshape(h, w, 4)
        self.set_data(arr)
        # Set the extent directly: set_extent would touch data limits.
        self._extent = (xlim[0], xlim[1], ylim[0], ylim[1])
        self._key = key
        self.last_render = {"array": arr, "xlim": xlim, "ylim": ylim, "pixel_ratio": ratio, "width": w, "height": h}

    def draw(self, renderer: Any) -> None:
        try:
            self.refresh(renderer)
        except Exception:
            if self.last_render is None:
                raise
            # Keep showing the previous render if a re-render fails.
        super().draw(renderer)


def add_basemap(
    ax: "matplotlib.axes.Axes",
    style: Union[str, Any, Sequence[Any]],
    *,
    tile_zoom: Optional[int] = None,
    alpha: float = 1.0,
    layers: Optional[List[str]] = None,
    labels: Union[bool, str] = True,
    features: Optional[List[str]] = None,
    pixel_ratio: Optional[float] = None,
    auth_token: Optional[str] = None,
    fetch: Optional["FetchOptions"] = None,
    attribution: Union[bool, str] = True,
    crs: Optional[object] = None,
    **deprecated: Any,
) -> "xr.DataArray":
    """Render a basemap beneath existing axes content.

    The basemap is an artist that renders when the figure is drawn, at the
    output resolution: on screen at the canvas dpi, and natively at the
    requested resolution in ``savefig(dpi=...)`` or PDF / SVG output. Takes
    the same arguments as :class:`geom_basemap` (and R's ``geom_basemap()``).

    Args:
        ax: An existing matplotlib Axes.
        style: A MapLibre GL style URL or JSON string (e.g. from
            :func:`basemap_provider`), or a list of those stacked bottom to
            top.
        tile_zoom: Tile detail override: the tile zoom on the 256 px grid
            (vector sources use one less), or None to pick tiles as MapLibre
            does for the view.
        alpha: Opacity of the basemap layer (0.0–1.0).
        layers: Optional layer filter. Plain IDs keep only those layers;
            minus-prefixed IDs drop those layers; ``*`` and ``?`` are
            wildcards.
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
        pixel_ratio: Device pixels per CSS pixel (MapLibre's
            devicePixelRatio). Default: output dpi / 96, so lines and labels
            keep their physical size at any dpi. Larger values draw map
            symbols smaller.
        auth_token: Optional access token for the tile service (sent only to the style's own server).
        fetch: How tiles are downloaded: a :class:`FetchOptions`.
        attribution: The credit line the basemap's providers ask for, drawn
            small in the bottom-right corner over the plot: ``True`` (the
            default) for the styles' own (see :func:`get_attribution`),
            ``False`` for none, or your own text.
        **deprecated: ``tile_timeout_ms`` and ``max_tiles`` are still
            accepted, with a DeprecationWarning: use ``fetch``.
        crs: CRS of the axes data coordinates (anything ``pyproj.CRS``
            accepts, e.g. ``"EPSG:4326"``, ``gdf.crs``). Required for plain
            matplotlib axes and GeoDataFrame plots, whose axes carry no CRS;
            detected automatically for cartopy axes when omitted.

    Returns:
        xarray.DataArray of shape (height, width, 4) holding the first render
        (at the figure's current dpi), with spatial bounds and CRS in
        ``.attrs``. The live artist is ``ax.images[-1]``.

    Raises:
        BasemapError: If CRS detection fails, the render fails, or the network
            is unreachable.
    """
    import pyproj

    try:
        import xarray as xr
    except ImportError as e:
        raise ImportError(
            "add_basemap needs xarray: pip install 'basemapper[matplotlib]' (or install xarray)"
        ) from e

    from ._fetch import _resolve_fetch

    fetch = _resolve_fetch(fetch, "add_basemap", deprecated)
    crs = pyproj.CRS.from_user_input(crs) if crs is not None else detect_crs_from_axes(ax)
    xlim, ylim = ax.get_xlim(), ax.get_ylim()
    img = BasemapImage(
        ax, style, crs, pixel_ratio, tile_zoom,
        dict(layers=layers, labels=labels_arg(labels), features=_features_arg(features), auth_token=auth_token,
             **fetch._core()),
        alpha=alpha,
    )
    # First render now, so errors surface here and a result can be returned.
    fig = ax.figure
    try:
        renderer = fig.canvas.get_renderer()
    except Exception:
        from matplotlib.backends.backend_agg import FigureCanvasAgg

        renderer = FigureCanvasAgg(fig).get_renderer()
    img.refresh(renderer)
    ax.add_image(img)
    add_attribution(ax, attribution_text(attribution, style, auth_token))

    # Make the axes panel background transparent so the basemap (zorder 0)
    # shows through the axes patch (zorder 1).
    ax.set_facecolor("none")
    ax.set_xlim(xlim)
    ax.set_ylim(ylim)

    r = img.last_render
    return xr.DataArray(
        r["array"].copy(),
        dims=["y", "x", "band"],
        attrs={
            "xmin": xlim[0],
            "ymin": ylim[0],
            "xmax": xlim[1],
            "ymax": ylim[1],
            "crs_epsg": crs.to_epsg(),
            "tile_zoom": tile_zoom,
            "width": r["width"],
            "height": r["height"],
            "pixel_ratio": r["pixel_ratio"],
        },
    )
