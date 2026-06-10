"""matplotlib integration: add_basemap()."""

from __future__ import annotations

from typing import TYPE_CHECKING, List, Optional

import numpy as np

from .bbox_utils import detect_crs_from_axes, get_axes_pixel_dims, reproject_bbox_to_3857
from .exceptions import BasemapError

if TYPE_CHECKING:
    import matplotlib.axes


def add_basemap(
    ax: "matplotlib.axes.Axes",
    style_url: str,
    zoom: Optional[int] = None,
    tile_timeout_ms: int = 10_000,
    max_tiles: int = 256,
    alpha: float = 1.0,
    layers: Optional[List[str]] = None,
) -> np.ndarray:
    """Render a basemap and composite it beneath existing axes content.

    Extracts the geographic bounding box and CRS directly from *ax*, reprojects
    to EPSG:3857, calls the Rust core, and injects the result via imshow.

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
        numpy.ndarray of shape (height, width, 4) with spatial bounds in .attrs.

    Raises:
        BasemapError: If CRS detection fails, the render fails, or the network
            is unreachable.
    """
    from . import render_basemap_raw  # imported here to avoid circular import

    crs = detect_crs_from_axes(ax)
    xlim = ax.get_xlim()
    ylim = ax.get_ylim()
    bbox_3857 = reproject_bbox_to_3857(xlim, ylim, crs)
    width_px, height_px = get_axes_pixel_dims(ax)

    raw = render_basemap_raw(
        bbox_3857=list(bbox_3857),
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

    # Attach spatial metadata as attrs for downstream use.
    result = arr.copy()
    result.attrs = {  # type: ignore[attr-defined]
        "xmin": bbox_3857[0],
        "ymin": bbox_3857[1],
        "xmax": bbox_3857[2],
        "ymax": bbox_3857[3],
        "crs_epsg": 3857,
        "zoom": zoom,
        "width": width_px,
        "height": height_px,
    }
    return result
