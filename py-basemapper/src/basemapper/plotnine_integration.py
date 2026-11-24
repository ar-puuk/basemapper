"""plotnine: ``ggplot(df) + m`` and its alias :func:`geom_basemap`
(docs/design/api-review.md §4.5, D14).

The layer draws the same artist as matplotlib's ``m.add_to(ax)``, rendering
at draw time through the core's plot plan, so a plotnine plot and a
matplotlib one (and R's ggplot2) show the same map.
"""

from __future__ import annotations

from typing import Any, Optional

from plotnine import aes
from plotnine.geoms.geom import geom


class geom_basemap(geom):
    """A basemap under a plotnine plot's layers: ``ggplot(df) + m``, or this
    alias, which builds the basemap from :func:`basemap`'s arguments.

    The plot gives the extent, the printed size and the sharpness (the
    figure's dpi; a basemap's ``dpi`` is an error here). What the map shows
    is set by ``zoom`` or ``symbol_scale`` (see :meth:`Basemap.add_to`). The
    same layer exists in R (``ggplot(nc) + basemap(s)``, alias
    ``geom_basemap()``).

    Args:
        style: A style, a list of styles, or a :class:`Basemap`.
        crs: The plot's CRS. plotnine has no spatial coordinate system: when
            the plot's data is a GeoDataFrame its CRS is used, else this,
            else ``"EPSG:4326"`` (longitude / latitude).
        alpha: The basemap's opacity (0-1).
        attribution: The providers' credit line in the bottom-right corner:
            True (their own), False (none), or your text.
        **kwargs: :func:`basemap`'s arguments that apply in a plot (``zoom``
            or ``symbol_scale``, ``clip``, ``fetch``), and plotnine's layer
            arguments.

    Example:
        >>> import pandas as pd, basemapper
        >>> from plotnine import ggplot, aes, geom_point
        >>> df = pd.DataFrame({"lon": [-111.95, -111.80], "lat": [40.68, 40.80]})
        >>> (ggplot(df, aes("lon", "lat")) + basemapper.basemap("OpenFreeMap.Positron") + geom_point())
    """

    REQUIRED_AES: set = set()
    DEFAULT_AES = aes()
    NON_MISSING_AES: set = set()
    # plotnine >= 0.15 reads stat/position from DEFAULT_PARAMS.
    DEFAULT_PARAMS = {"stat": "identity", "position": "identity", "na_rm": False}
    GROUP = -1

    _BASEMAP_ARGS = ("zoom", "symbol_scale", "dpi", "clip", "fetch")

    def __init__(self, style: Any, *, crs: Any = None, alpha: float = 1.0, attribution: Any = True, **kwargs: Any) -> None:
        from ._basemap import Basemap, basemap

        basemap_args = {k: kwargs.pop(k) for k in list(kwargs) if k in self._BASEMAP_ARGS}
        if isinstance(style, Basemap):
            if basemap_args:
                raise TypeError("geom_basemap(m): a Basemap already has its arguments; change them with m.set_view(...)")
            self.basemap = style
        else:
            self.basemap = basemap(style, **basemap_args)
        self.basemap._plot_parts()  # a basemap that cannot go in a plot is an error now
        self.crs = crs
        self.plot_data_crs = None
        self.alpha = alpha
        self.attribution = attribution
        super().__init__(mapping=None, data=None, **kwargs)

    def __radd__(self, plot: Any):
        # The plot's data may say its CRS (a GeoDataFrame).
        self.plot_data_crs = getattr(getattr(plot, "data", None), "crs", None)
        return super().__radd__(plot)

    def draw_panel(self, data, panel_params, coord, ax, **kwargs):  # type: ignore[override]
        """Add the basemap artist, rendering at draw time over the panel."""
        from . import _crs
        from .matplotlib_integration import BasemapImage, add_attribution, attribution_text

        if hasattr(panel_params, "x") and hasattr(panel_params.x, "range"):
            x_range, y_range = panel_params.x.range, panel_params.y.range
        else:
            x_range = panel_params.get("x_range", panel_params.get("x.range", [0.0, 1.0]))
            y_range = panel_params.get("y_range", panel_params.get("y.range", [0.0, 1.0]))
        crs = _crs.crs_of(self.plot_data_crs or self.crs or _crs.LONLAT)
        ax.set_xlim(x_range[0], x_range[1])
        ax.set_ylim(y_range[0], y_range[1])
        img = BasemapImage(ax, self.basemap, crs, alpha=self.alpha)
        img.set_extent((x_range[0], x_range[1], y_range[0], y_range[1]))
        ax.add_image(img)
        add_attribution(ax, attribution_text(self.attribution, self.basemap.styles))
        # The panel's background would cover the basemap: make it clear.
        ax.set_facecolor("none")
        ax.set_xlim(x_range[0], x_range[1])
        ax.set_ylim(y_range[0], y_range[1])
