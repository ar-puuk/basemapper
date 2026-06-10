"""Bounding-box extraction and CRS reprojection utilities."""

from __future__ import annotations

import warnings
from typing import TYPE_CHECKING, Any, Optional, Tuple

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


def _maybe_warn_projected(xmin: float, ymin: float, xmax: float, ymax: float) -> None:
    """Warn when coordinates appear to exceed WGS-84 geographic bounds."""
    if xmin < -180 or xmax > 180 or ymin < -90 or ymax > 90:
        warnings.warn(
            "bbox coordinates appear to be outside WGS-84 bounds; "
            "they may be in a projected CRS (e.g. EPSG:3857). "
            "Pass crs= explicitly to suppress this warning.",
            UserWarning,
            stacklevel=4,
        )


def normalize_bbox(
    bbox: Any,
    crs: Optional[Any] = None,
) -> Tuple[float, float, float, float, "pyproj.CRS"]:
    """Normalise a bbox input to ``(xmin, ymin, xmax, ymax, pyproj.CRS)``.

    Accepted forms:

    * A sequence of 4 floats ``[xmin, ymin, xmax, ymax]`` in the CRS given by
      *crs* (defaults to EPSG:4326 / WGS-84 when *crs* is omitted).
    * A ``dict`` with keys ``"xmin"``, ``"ymin"``, ``"xmax"``, ``"ymax"``, and
      an optional ``"crs"`` key.
    * A ``geopandas.GeoDataFrame`` or ``GeoSeries``; CRS is read from ``.crs``.

    Args:
        bbox: Bounding box input in one of the forms above.
        crs: CRS override — EPSG integer, WKT/PROJ string, or
            :class:`pyproj.CRS`.  Overrides a CRS embedded in a dict.
            Ignored for GeoDataFrame/GeoSeries when *crs* is ``None``.

    Returns:
        ``(xmin, ymin, xmax, ymax, source_crs)`` in the input CRS.

    Raises:
        ValueError: If the bbox cannot be parsed or has no discernible CRS.
        TypeError: If *bbox* is not a recognised type.
    """
    import pyproj

    # geopandas (optional dependency)
    try:
        import geopandas as gpd
        if isinstance(bbox, (gpd.GeoDataFrame, gpd.GeoSeries)):
            bounds = bbox.total_bounds  # [xmin, ymin, xmax, ymax]
            source_crs = bbox.crs if crs is None else pyproj.CRS.from_user_input(crs)
            if source_crs is None:
                raise ValueError(
                    "GeoDataFrame/GeoSeries has no CRS; pass crs= explicitly."
                )
            return (
                float(bounds[0]), float(bounds[1]),
                float(bounds[2]), float(bounds[3]),
                source_crs,
            )
    except ImportError:
        pass

    if isinstance(bbox, dict):
        try:
            xmin = float(bbox["xmin"])
            ymin = float(bbox["ymin"])
            xmax = float(bbox["xmax"])
            ymax = float(bbox["ymax"])
        except KeyError as exc:
            raise ValueError(f"bbox dict missing required key: {exc}") from exc
        dict_crs = bbox.get("crs")
        if crs is not None:
            source_crs = pyproj.CRS.from_user_input(crs)
        elif dict_crs is not None:
            source_crs = pyproj.CRS.from_user_input(dict_crs)
        else:
            source_crs = pyproj.CRS.from_epsg(4326)
            _maybe_warn_projected(xmin, ymin, xmax, ymax)
        return xmin, ymin, xmax, ymax, source_crs

    # Plain sequence
    try:
        vals = [float(v) for v in bbox]
    except (TypeError, ValueError) as exc:
        raise TypeError(
            f"bbox must be a sequence of 4 floats, a dict, or a GeoDataFrame; "
            f"got {type(bbox).__name__}."
        ) from exc
    if len(vals) != 4:
        raise ValueError(
            f"bbox must have exactly 4 elements (xmin, ymin, xmax, ymax), got {len(vals)}."
        )
    xmin, ymin, xmax, ymax = vals
    if crs is not None:
        source_crs = pyproj.CRS.from_user_input(crs)
    else:
        source_crs = pyproj.CRS.from_epsg(4326)
        _maybe_warn_projected(xmin, ymin, xmax, ymax)
    return xmin, ymin, xmax, ymax, source_crs


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
