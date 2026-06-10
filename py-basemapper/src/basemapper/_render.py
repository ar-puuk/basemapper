"""Python wrapper around the compiled Rust render core."""

from __future__ import annotations

from typing import Any, List, Optional


def render_basemap_raw(
    bbox: Any,
    width: int,
    height: int,
    style_input: str,
    crs: Optional[Any] = None,
    zoom: Optional[int] = None,
    tile_timeout_ms: int = 10_000,
    max_tiles: int = 256,
    layers: Optional[List[str]] = None,
) -> bytes:
    """Fetch and composite map tiles into a raw RGBA pixel buffer.

    Args:
        bbox: Bounding box — one of:

            * A ``[xmin, ymin, xmax, ymax]`` list or tuple in the CRS given by
              *crs* (defaults to EPSG:4326 / WGS-84 when *crs* is omitted).
            * A ``dict`` with keys ``"xmin"``, ``"ymin"``, ``"xmax"``,
              ``"ymax"``, and an optional ``"crs"`` key.
            * A ``geopandas.GeoDataFrame`` or ``GeoSeries``; CRS is read from
              ``.crs`` automatically.

        width: Output width in pixels.
        height: Output height in pixels.
        style_input: Tile style — a URL to a MapLibre GL Style JSON file, an
            inline JSON string, or a :class:`RasterProvider` /
            :class:`VectorProvider` / :class:`EsriVectorProvider` /
            :class:`EsriRasterProvider` object.
        crs: CRS of the input *bbox* — an EPSG integer, WKT/PROJ string, or
            :class:`pyproj.CRS` object.  Ignored when *bbox* carries its own
            CRS.  Defaults to EPSG:4326 (WGS-84) with a warning if coordinates
            appear projected.
        zoom: Tile zoom level (0–22). Auto-computed from *bbox* and *width*
            when ``None``.
        tile_timeout_ms: Per-tile HTTP timeout in milliseconds (default 10 000).
        max_tiles: Maximum number of tiles fetched per render call (default 256).
        layers: Optional layer filter. Plain layer IDs -> keep only those
            layers; minus-prefixed IDs (e.g. ``["-labels"]``) -> exclude those
            layers.  Mixing inclusion and exclusion raises :class:`BasemapError`.

    Returns:
        bytes: RGBA pixel data of length ``width * height * 4``.

    Raises:
        BasemapError: On any rendering or network failure.
    """
    import pyproj

    from .bbox_utils import normalize_bbox, reproject_bbox_to_3857
    from ._basemapper import render_basemap_raw as _impl

    xmin, ymin, xmax, ymax, source_crs = normalize_bbox(bbox, crs)

    crs_3857 = pyproj.CRS.from_epsg(3857)
    if source_crs == crs_3857:
        bbox_3857 = [xmin, ymin, xmax, ymax]
    else:
        bbox_3857 = list(reproject_bbox_to_3857((xmin, xmax), (ymin, ymax), source_crs))

    return _impl(bbox_3857, width, height, style_input, zoom, tile_timeout_ms, max_tiles, layers)
