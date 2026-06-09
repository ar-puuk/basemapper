"""Python wrapper around the compiled Rust render core."""

from typing import List, Optional


def render_basemap_raw(
    bbox_3857: List[float],
    width: int,
    height: int,
    style_input: str,
    zoom: Optional[int] = None,
    tile_timeout_ms: int = 10_000,
    max_tiles: int = 256,
    layers: Optional[List[str]] = None,
) -> bytes:
    """Fetch and composite map tiles into a raw RGBA pixel buffer.

    Args:
        bbox_3857: Bounding box as ``[xmin, ymin, xmax, ymax]`` in EPSG:3857 metres.
        width: Output width in pixels.
        height: Output height in pixels.
        style_input: Tile style — a URL to a MapLibre GL Style JSON file, an
            inline JSON string, or a :class:`RasterProvider` / :class:`VectorProvider`
            / :class:`EsriVectorProvider` / :class:`EsriRasterProvider` object.
        zoom: Tile zoom level (0–22). Auto-computed from *bbox_3857* and *width*
            when ``None``.
        tile_timeout_ms: Per-tile HTTP timeout in milliseconds (default 10 000).
        max_tiles: Maximum number of tiles fetched per render call (default 256).
        layers: Optional layer filter. Plain layer IDs -> keep only those layers;
            minus-prefixed IDs (e.g. ``["-labels"]``) -> exclude those layers.
            Mixing inclusion and exclusion in one list raises :class:`BasemapError`.

    Returns:
        bytes: RGBA pixel data of length ``width * height * 4``.

    Raises:
        BasemapError: On any rendering or network failure.
    """
    from ._basemapper import render_basemap_raw as _impl

    return _impl(bbox_3857, width, height, style_input, zoom, tile_timeout_ms, max_tiles, layers)
