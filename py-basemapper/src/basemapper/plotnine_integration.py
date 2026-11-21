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

    The basemap renders when the plot is drawn, at the output resolution
    (natively at ``save(dpi=...)``), like :func:`add_basemap`.

    Args:
        style_url: MapLibre GL style URL or inline JSON string, a provider
            object, or a list of those stacked bottom to top.
        zoom: Tile zoom level override (0–22). Auto-computed when None.
        alpha: Opacity of the basemap layer (0.0–1.0).
        layers: Optional layer filter list — plain IDs keep only those layers;
                minus-prefixed IDs exclude those layers.
        crs: CRS of the plot's x/y data (anything ``pyproj.CRS`` accepts).
            plotnine has no spatial coordinate system to read it from, so it
            defaults to ``"EPSG:4326"`` (longitude/latitude). Pass e.g.
            ``crs=gdf.crs`` or ``crs="EPSG:3857"`` for projected data.
        ratio: Device pixels per CSS pixel; default output dpi / 96.

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
        ratio: Optional[float] = None,
        **kwargs: object,
    ) -> None:
        self.style_url = style_url
        self.zoom = zoom
        self.alpha = alpha
        self.layers = layers
        self.crs = crs
        self.ratio = ratio
        super().__init__(mapping=None, data=None, **kwargs)

    def draw_panel(self, data, panel_params, coord, ax, **kwargs):  # type: ignore[override]
        """Add a basemap artist that renders at draw time.

        The panel background is made transparent so the basemap shows
        through. Axis limits are kept at the panel ranges.
        """
        import pyproj

        from .matplotlib_integration import BasemapImage

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

        ax.set_xlim(x_range[0], x_range[1])
        ax.set_ylim(y_range[0], y_range[1])
        img = BasemapImage(
            ax, self.style_url, crs, self.ratio, self.zoom,
            dict(layers=self.layers), alpha=self.alpha,
        )
        img.set_extent((x_range[0], x_range[1], y_range[0], y_range[1]))
        ax.add_image(img)

        # Make the panel background transparent so the basemap shows through
        # (ax.patch at zorder=1 would otherwise cover basemap at zorder=0).
        ax.set_facecolor("none")
        ax.set_xlim(x_range[0], x_range[1])
        ax.set_ylim(y_range[0], y_range[1])
