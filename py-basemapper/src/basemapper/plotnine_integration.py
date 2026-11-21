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
        crs: CRS of the plot's x/y data (anything ``pyproj.CRS`` accepts).
            plotnine has no spatial coordinate system to read it from, so it
            defaults to ``"EPSG:4326"`` (longitude/latitude). Pass e.g.
            ``crs=gdf.crs`` or ``crs="EPSG:3857"`` for projected data.

    Example:
        >>> import pandas as pd
        >>> from plotnine import ggplot, aes, geom_point
        >>> from basemapper import geom_basemap
        >>> df = pd.DataFrame({"lon": [-111.95, -111.80], "lat": [40.68, 40.80]})
        >>> (ggplot(df, aes("lon", "lat"))
        ...  + geom_basemap(style_url="https://basemaps.cartocdn.com/gl/positron-gl-style/style.json")
        ...  + geom_point())
    """

    REQUIRED_AES: set = set()
    DEFAULT_AES = aes()
    NON_MISSING_AES: set = set()
    # plotnine >= 0.15 reads stat/position from DEFAULT_PARAMS (the base class
    # no longer supplies them).
    DEFAULT_PARAMS = {"stat": "identity", "position": "identity", "na_rm": False}
    GROUP = -1

    def __init__(
        self,
        style_url: str,
        zoom: Optional[int] = None,
        alpha: float = 1.0,
        layers: Optional[List[str]] = None,
        crs: object = "EPSG:4326",
        **kwargs: object,
    ) -> None:
        self.style_url = style_url
        self.zoom = zoom
        self.alpha = alpha
        self.layers = layers
        self.crs = crs
        super().__init__(mapping=None, data=None, **kwargs)

    def draw_panel(self, data, panel_params, coord, ax, **kwargs):  # type: ignore[override]
        """Render basemap at draw time and inject via imshow.

        The panel background is made transparent automatically so the basemap
        shows through. Axis limits are restored after imshow.
        """
        import pyproj

        from .bbox_utils import get_axes_pixel_dims
        from . import render_basemap_raw

        # Panel ranges: plotnine passes a panel_view (x/y scale views with a
        # .range); older code paths used a dict.
        if hasattr(panel_params, "x") and hasattr(panel_params.x, "range"):
            x_range, y_range = panel_params.x.range, panel_params.y.range
        else:
            x_range = panel_params.get("x_range", panel_params.get("x.range", [0.0, 1.0]))
            y_range = panel_params.get("y_range", panel_params.get("y.range", [0.0, 1.0]))

        # plotnine has no spatial coordinate system, so the data CRS comes
        # from the layer (default EPSG:4326).
        crs = pyproj.CRS.from_user_input(self.crs)

        width_px, height_px = get_axes_pixel_dims(ax)

        raw = render_basemap_raw(
            bbox=[x_range[0], y_range[0], x_range[1], y_range[1]],
            crs=crs,
            width=width_px,
            height=height_px,
            style_input=self.style_url,
            zoom=self.zoom,
            layers=self.layers,
        )

        arr = np.frombuffer(raw, dtype=np.uint8).reshape(height_px, width_px, 4)

        # Make the panel background transparent so the basemap shows through
        # (ax.patch at zorder=1 would otherwise cover basemap at zorder=0).
        ax.set_facecolor("none")

        ax.imshow(
            arr,
            extent=[x_range[0], x_range[1], y_range[0], y_range[1]],
            origin="upper",
            zorder=0,
            interpolation="nearest",
            aspect="auto",
            alpha=self.alpha,
        )

        # Restore panel limits after imshow (which can reset them).
        ax.set_xlim(x_range[0], x_range[1])
        ax.set_ylim(y_range[0], y_range[1])
