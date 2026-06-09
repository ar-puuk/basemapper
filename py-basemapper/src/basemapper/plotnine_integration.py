"""plotnine integration: geom_basemap."""

from __future__ import annotations

from typing import List, Optional

import numpy as np

# plotnine is an optional dependency; this module is only imported when it is
# available (guarded in __init__.py via try/except ImportError).
from plotnine import aes
from plotnine.geoms.geom import geom


class geom_basemap(geom):
    """Render a styled basemap beneath plotnine spatial layers.

    Defers all tile fetching to draw time so that the plot's coordinate system
    has been established. Uses the same injection logic as :func:`add_basemap`.

    Args:
        style_url: MapLibre GL style URL or inline JSON string.
        zoom: Tile zoom level override (0–22). Auto-computed when None.
        alpha: Opacity of the basemap layer (0.0–1.0).
        layers: Optional layer filter list — plain IDs keep only those layers;
                minus-prefixed IDs exclude those layers.

    Example:
        >>> from plotnine import ggplot, geom_sf
        >>> from basemapper import geom_basemap
        >>> (ggplot(gdf) + geom_basemap(style_url="https://...") + geom_sf())
    """

    REQUIRED_AES: set = set()
    DEFAULT_AES = aes()
    NON_MISSING_AES: set = set()
    GROUP = -1

    def __init__(
        self,
        style_url: str,
        zoom: Optional[int] = None,
        alpha: float = 1.0,
        layers: Optional[List[str]] = None,
        **kwargs: object,
    ) -> None:
        self.style_url = style_url
        self.zoom = zoom
        self.alpha = alpha
        self.layers = layers
        super().__init__(mapping=None, data=None, **kwargs)

    def draw_panel(self, data, panel_params, coord, ax, **kwargs):  # type: ignore[override]
        """Render basemap at draw time and inject via imshow.

        H4: zorder=0 may hide behind ax.patch (zorder=0). If basemap is
        invisible, set ax.set_facecolor('none') and increase zorder to 0.5.
        """
        from .bbox_utils import detect_crs_from_axes, get_axes_pixel_dims, reproject_bbox_to_3857
        from . import render_basemap_raw

        # Extract coordinate bounds from panel_params (plotnine mirrors ggplot2).
        x_range = panel_params.get("x_range", panel_params.get("x.range", [0.0, 1.0]))
        y_range = panel_params.get("y_range", panel_params.get("y.range", [0.0, 1.0]))

        crs = detect_crs_from_axes(ax)
        bbox_3857 = reproject_bbox_to_3857(
            (x_range[0], x_range[1]),
            (y_range[0], y_range[1]),
            crs,
        )

        width_px, height_px = get_axes_pixel_dims(ax)

        raw = render_basemap_raw(
            bbox_3857=list(bbox_3857),
            width=width_px,
            height=height_px,
            style_input=self.style_url,
            zoom=self.zoom,
            layers=self.layers,
        )

        arr = np.frombuffer(raw, dtype=np.uint8).reshape(height_px, width_px, 4)

        # H4: same zorder consideration as matplotlib_integration.py (T022).
        ax.imshow(
            arr,
            extent=[x_range[0], x_range[1], y_range[0], y_range[1]],
            origin="upper",
            zorder=0,
            interpolation="nearest",
            aspect="auto",
            alpha=self.alpha,
        )
