"""Bounding-box extraction and CRS reprojection utilities."""

from __future__ import annotations

import warnings
from typing import TYPE_CHECKING, Tuple

if TYPE_CHECKING:
    import matplotlib.axes
    import pyproj


def detect_crs_from_axes(ax: "matplotlib.axes.Axes") -> "pyproj.CRS":
    """Return the CRS attached to *ax*, or WGS-84 as a fallback.

    Args:
        ax: A matplotlib Axes object, optionally with a geographic projection.

    Returns:
        A pyproj.CRS instance.
    """
    import pyproj

    # cartopy axes carry the projection on ax.projection
    if hasattr(ax, "projection"):
        try:
            return pyproj.CRS.from_user_input(ax.projection)
        except Exception:
            pass

    # GeoDataFrame-backed axes may expose _gdf_crs
    if hasattr(ax, "_gdf_crs") and ax._gdf_crs is not None:
        try:
            return pyproj.CRS.from_user_input(ax._gdf_crs)
        except Exception:
            pass

    warnings.warn(
        "Could not detect a geographic CRS from the axes; assuming EPSG:4326 (WGS-84).",
        stacklevel=3,
    )
    return pyproj.CRS.from_epsg(4326)


def reproject_bbox_to_3857(
    xlim: Tuple[float, float],
    ylim: Tuple[float, float],
    crs: "pyproj.CRS",
) -> Tuple[float, float, float, float]:
    """Reproject axis limits to Web Mercator (EPSG:3857).

    Args:
        xlim: (xmin, xmax) in the source CRS units.
        ylim: (ymin, ymax) in the source CRS units.
        crs: Source coordinate reference system.

    Returns:
        (xmin_3857, ymin_3857, xmax_3857, ymax_3857)
    """
    import pyproj

    transformer = pyproj.Transformer.from_crs(crs, pyproj.CRS.from_epsg(3857), always_xy=True)
    xmin_3857, ymin_3857 = transformer.transform(xlim[0], ylim[0])
    xmax_3857, ymax_3857 = transformer.transform(xlim[1], ylim[1])
    return xmin_3857, ymin_3857, xmax_3857, ymax_3857


def get_axes_pixel_dims(ax: "matplotlib.axes.Axes") -> Tuple[int, int]:
    """Return the pixel dimensions of the axes panel.

    Forces a layout pass so that the renderer has committed to sizes.

    Args:
        ax: A matplotlib Axes object.

    Returns:
        (width_px, height_px) as integers.
    """
    fig = ax.figure
    try:
        fig.canvas.draw_idle()
        renderer = fig.canvas.get_renderer()
        bbox = ax.get_window_extent(renderer=renderer)
    except Exception:
        # Fallback: use figure size × DPI
        fig_w, fig_h = fig.get_size_inches()
        dpi = fig.dpi
        return int(fig_w * dpi), int(fig_h * dpi)

    return max(1, int(bbox.width)), max(1, int(bbox.height))
