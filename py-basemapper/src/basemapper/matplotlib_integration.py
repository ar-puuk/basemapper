"""matplotlib integration: add_basemap()."""

from __future__ import annotations

from typing import TYPE_CHECKING, List, Optional

import numpy as np

from .bbox_utils import detect_crs_from_axes, get_axes_pixel_dims
from .exceptions import BasemapError

if TYPE_CHECKING:
    import matplotlib.axes
    import xarray as xr


def add_basemap(
    ax: "matplotlib.axes.Axes",
    style_url: str,
    zoom: Optional[int] = None,
    tile_timeout_ms: int = 10_000,
    max_tiles: int = 256,
    alpha: float = 1.0,
    layers: Optional[List[str]] = None,
) -> "xr.DataArray":
    """Render a basemap and composite it beneath existing axes content.

    Extracts the geographic bounding box and CRS directly from *ax*, delegates
    reprojection to :func:`render_basemap_raw`, and injects the result via
    imshow.

    Args:
        ax: An existing matplotlib Axes with a recognized geographic projection.
        style_url: MapLibre GL style URL or inline JSON string.
        zoom: Tile zoom level override (0–22). Auto-computed when None.
        tile_timeout_ms: Per-tile HTTP timeout in milliseconds.
        max_tiles: Maximum tile downloads per render.
        alpha: Opacity of the basemap layer (0.0–1.0).
        layers: Optional layer filter. Plain IDs keep only those layers;
                minus-prefixed IDs exclude those layers.

    Returns:
        xarray.DataArray of shape (height, width, 4) with spatial bounds and
        CRS stored in ``.attrs``. Note: this returns ``xarray.DataArray`` rather
        than a bare ``numpy.ndarray`` because ndarray cannot carry named spatial
        metadata without subclassing. Callers who need the underlying array can
        access it via ``.values`` on the returned DataArray.

    Raises:
        BasemapError: If CRS detection fails, the render fails, or the network
            is unreachable.
    """
    import xarray as xr

    from . import render_basemap_raw  # imported here to avoid circular import

    crs = detect_crs_from_axes(ax)
    xlim = ax.get_xlim()
    ylim = ax.get_ylim()
    width_px, height_px = get_axes_pixel_dims(ax)

    raw = render_basemap_raw(
        bbox=[xlim[0], ylim[0], xlim[1], ylim[1]],
        crs=crs,
        width=width_px,
        height=height_px,
        style_input=style_url,
        zoom=zoom,
        tile_timeout_ms=tile_timeout_ms,
        max_tiles=max_tiles,
        layers=layers,
    )

    arr = np.frombuffer(raw, dtype=np.uint8).reshape(height_px, width_px, 4)

    # Make the axes panel background transparent so the basemap shows through.
    # ax.patch (the white background rectangle) sits at zorder=1; imshow at
    # zorder=0 would be invisible behind it without this.
    ax.set_facecolor("none")

    ax.imshow(
        arr,
        extent=[xlim[0], xlim[1], ylim[0], ylim[1]],
        origin="upper",
        zorder=0,
        interpolation="nearest",
        aspect="auto",
        alpha=alpha,
    )

    # Restore axis limits: imshow resets them to the image extent in some
    # matplotlib versions.
    ax.set_xlim(xlim)
    ax.set_ylim(ylim)

    return xr.DataArray(
        arr.copy(),
        dims=["y", "x", "band"],
        attrs={
            "xmin": xlim[0],
            "ymin": ylim[0],
            "xmax": xlim[1],
            "ymax": ylim[1],
            "crs_epsg": crs.to_epsg(),
            "zoom": zoom,
            "width": width_px,
            "height": height_px,
        },
    )
