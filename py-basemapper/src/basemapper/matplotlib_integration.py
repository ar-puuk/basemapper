"""matplotlib integration: add_basemap()."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, List, Optional, Sequence, Union

import numpy as np
from matplotlib.image import AxesImage

from .bbox_utils import detect_crs_from_axes

if TYPE_CHECKING:
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
        ratio: Optional[float],
        zoom: Optional[int],
        options: dict,
        alpha: float = 1.0,
    ) -> None:
        super().__init__(ax, interpolation="nearest", origin="upper", zorder=0, alpha=alpha)
        self._styles = styles
        self._crs = crs
        self._ratio = ratio
        self._zoom = zoom
        self._options = options
        self._key: Optional[tuple] = None
        self.last_render: Optional[dict] = None

    def refresh(self, renderer: Any) -> None:
        """Render for the axes' current state if it changed since last time."""
        from ._render import render_styles_raw

        ax = self.axes
        fig = ax.figure
        xlim, ylim = ax.get_xlim(), ax.get_ylim()
        bb = ax.get_window_extent(renderer)
        dpi_now = fig.dpi  # the dpi the window extent is measured in
        w_in, h_in = bb.width / dpi_now, bb.height / dpi_now
        out_dpi = _output_dpi(fig, renderer)
        # CSS px are 1/96 in: by default draw at the output resolution with
        # map symbols at their physical size (ratio = dpi / 96).
        ratio = self._ratio if self._ratio is not None else out_dpi / 96.0
        css_w = max(1, round(w_in * out_dpi / ratio))
        css_h = max(1, round(h_in * out_dpi / ratio))
        key = (tuple(xlim), tuple(ylim), css_w, css_h, ratio)
        if key == self._key:
            return
        buf, w, h = render_styles_raw(
            [xlim[0], ylim[0], xlim[1], ylim[1]], css_w, css_h, self._styles,
            crs=self._crs, pixel_ratio=ratio, zoom=self._zoom, **self._options,
        )
        arr = np.frombuffer(buf, dtype=np.uint8).reshape(h, w, 4)
        self.set_data(arr)
        # Set the extent directly: set_extent would touch data limits.
        self._extent = (xlim[0], xlim[1], ylim[0], ylim[1])
        self._key = key
        self.last_render = {"array": arr, "xlim": xlim, "ylim": ylim, "ratio": ratio, "width": w, "height": h}

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
    style_url: Union[str, Any, Sequence[Any]],
    zoom: Optional[int] = None,
    tile_timeout_ms: int = 10_000,
    max_tiles: int = 256,
    alpha: float = 1.0,
    layers: Optional[List[str]] = None,
    crs: Optional[object] = None,
    ratio: Optional[float] = None,
    auth_token: Optional[str] = None,
) -> "xr.DataArray":
    """Render a basemap beneath existing axes content.

    The basemap is an artist that renders when the figure is drawn, at the
    output resolution: on screen at the canvas dpi, and natively at the
    requested resolution in ``savefig(dpi=...)`` or PDF / SVG output.

    Args:
        ax: An existing matplotlib Axes with a recognized geographic projection.
        style_url: MapLibre GL style URL or inline JSON string, a provider
            object, or a list of those stacked bottom to top.
        zoom: Tile zoom level override (0–22). Auto-computed when None.
        tile_timeout_ms: Per-tile HTTP timeout in milliseconds.
        max_tiles: Maximum tile downloads per render.
        alpha: Opacity of the basemap layer (0.0–1.0).
        layers: Optional layer filter. Plain IDs keep only those layers;
                minus-prefixed IDs exclude those layers.
        crs: CRS of the axes data coordinates (anything ``pyproj.CRS``
            accepts, e.g. ``"EPSG:4326"``, ``gdf.crs``). Required for plain
            matplotlib axes and GeoDataFrame plots, whose axes carry no CRS;
            detected automatically for cartopy axes when omitted.
        ratio: Device pixels per CSS pixel (MapLibre's devicePixelRatio).
            Default: output dpi / 96, so lines and labels keep their physical
            size at any dpi. Larger values draw map symbols smaller.
        auth_token: Optional access token for the tile service.

    Returns:
        xarray.DataArray of shape (height, width, 4) holding the first render
        (at the figure's current dpi), with spatial bounds and CRS in
        ``.attrs``. The live artist is ``ax.images[-1]``.

    Raises:
        BasemapError: If CRS detection fails, the render fails, or the network
            is unreachable.
    """
    import pyproj
    import xarray as xr

    crs = pyproj.CRS.from_user_input(crs) if crs is not None else detect_crs_from_axes(ax)
    xlim, ylim = ax.get_xlim(), ax.get_ylim()
    img = BasemapImage(
        ax, style_url, crs, ratio, zoom,
        dict(tile_timeout_ms=tile_timeout_ms, max_tiles=max_tiles, layers=layers, auth_token=auth_token),
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
            "zoom": zoom,
            "width": r["width"],
            "height": r["height"],
            "ratio": r["ratio"],
        },
    )
