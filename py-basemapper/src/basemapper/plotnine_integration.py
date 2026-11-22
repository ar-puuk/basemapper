"""plotnine integration: geom_basemap."""

from __future__ import annotations

from typing import Any, List, Optional

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
    (natively at ``save(dpi=...)``), like :func:`add_basemap`. The same
    layer exists in R (ggplot2 ``geom_basemap()``), with the same arguments.

    Args:
        style: A MapLibre GL style URL or JSON string (e.g. from
            :func:`basemap_provider`), or a list of those stacked bottom to
            top.
        tile_zoom: Tile detail override: the tile zoom on the 256 px grid
            (vector sources use one less), or None to pick tiles as MapLibre
            does for the view.
        alpha: Opacity of the basemap layer (0.0–1.0).
        layers: Optional layer filter list — plain IDs keep only those layers;
            minus-prefixed IDs drop those layers.
        pixel_ratio: Device pixels per CSS pixel; default output dpi / 96.
        auth_token: Optional access token for the tile service.
        tile_timeout_ms: Per-tile HTTP timeout in milliseconds.
        max_tiles: Maximum tile downloads per style.
        crs: CRS of the plot's x/y data (anything ``pyproj.CRS`` accepts).
            plotnine has no spatial coordinate system to read it from, so it
            defaults to ``"EPSG:4326"`` (longitude/latitude). Pass e.g.
            ``crs=gdf.crs`` or ``crs="EPSG:3857"`` for projected data.

    Example:
        >>> import pandas as pd
        >>> from plotnine import ggplot, aes, geom_point
        >>> from basemapper import basemap_provider, geom_basemap
        >>> df = pd.DataFrame({"lon": [-111.95, -111.80], "lat": [40.68, 40.80]})
        >>> (ggplot(df, aes("lon", "lat"))
        ...  + geom_basemap(basemap_provider("Carto.Positron"))
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
        style: Any,
        tile_zoom: Optional[int] = None,
        alpha: float = 1.0,
        layers: Optional[List[str]] = None,
        pixel_ratio: Optional[float] = None,
        auth_token: Optional[str] = None,
        tile_timeout_ms: int = 10_000,
        max_tiles: int = 256,
        crs: object = "EPSG:4326",
        **kwargs: object,
    ) -> None:
        self.style = style
        self.tile_zoom = tile_zoom
        self.alpha = alpha
        self.layers = layers
        self.pixel_ratio = pixel_ratio
        self.auth_token = auth_token
        self.tile_timeout_ms = tile_timeout_ms
        self.max_tiles = max_tiles
        self.crs = crs
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
            ax, self.style, crs, self.pixel_ratio, self.tile_zoom,
            dict(layers=self.layers, auth_token=self.auth_token,
                 tile_timeout_ms=self.tile_timeout_ms, max_tiles=self.max_tiles),
            alpha=self.alpha,
        )
        img.set_extent((x_range[0], x_range[1], y_range[0], y_range[1]))
        ax.add_image(img)

        # Make the panel background transparent so the basemap shows through
        # (ax.patch at zorder=1 would otherwise cover basemap at zorder=0).
        ax.set_facecolor("none")
        ax.set_xlim(x_range[0], x_range[1])
        ax.set_ylim(y_range[0], y_range[1])
