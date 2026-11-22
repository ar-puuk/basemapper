"""Styles: named basemaps, tile data and OGC services.

Anything a style is built from — a tile URL, TileJSON, PMTiles archive or
ArcGIS service — is also accepted as ``style`` directly; :func:`tile_style`
builds that style with overrides. All of it is implemented once in the Rust
core, so Python and R (``tile_style()``, ...) return the same style for the
same call.
"""

from __future__ import annotations

import json
import warnings
from typing import Dict, List, Optional

from . import _basemapper as _core


def basemap_provider(name: str, api_key: Optional[str] = None) -> str:
    """A named basemap's style.

    Args:
        name: Provider name, e.g. ``"OpenFreeMap.liberty"`` or
            ``"Carto.Positron"``; see :func:`list_providers`.
        api_key: API key, required by MapTiler; optional for Stadia (lifts
            rate limits); ignored by the others.

    Returns:
        A style URL or inline style JSON.

    Raises:
        ValidationError: For an unknown name, or a MapTiler style without a key.
    """
    return _core.basemap_provider(name, api_key)


def list_providers() -> List[str]:
    """The names :func:`basemap_provider` accepts."""
    return list(_core.list_providers())


def tile_style(
    source: str,
    type: Optional[str] = None,
    tile_size: Optional[int] = None,
    scheme: Optional[str] = None,
    minzoom: Optional[int] = None,
    maxzoom: Optional[int] = None,
    attribution: Optional[str] = None,
    source_layers: Optional[List[str]] = None,
    paint: Optional[Dict[str, object]] = None,
    auth_token: Optional[str] = None,
) -> str:
    """A style for tile data, with overrides.

    ``source`` is what you could pass as ``style`` directly; use this function
    to override what is detected, or to get the style JSON to edit:

    * a tile URL template (``{z}/{x}/{y}``, ``{quadkey}``,
      ``{bbox-epsg-3857}``, WMTS ``{TileMatrix}/{TileRow}/{TileCol}``);
    * a TileJSON URL or file;
    * a PMTiles archive: URL, local path or ``pmtiles://`` URL;
    * an ArcGIS ``.../MapServer`` (raster tiles) or ``.../VectorTileServer``
      (its published style);
    * a style URL or file (returned with relative URLs made absolute).

    Raster tiles get one raster layer. Vector tiles get a default style
    drawing every source layer (polygons as translucent fills, lines and
    outlines, points as circles, a colour per source layer) over a
    transparent background.

    Args:
        source: The tile data, as above.
        type: ``"raster"`` or ``"vector"``; detected when None (from the
            archive header, the TileJSON, the URL's extension, or one fetched
            tile).
        tile_size: Raster tile size in px (default 256 for tile URLs and
            archives).
        scheme: ``"xyz"`` or ``"tms"`` (rows counted from the south).
        minzoom: Minimum zoom of the tiles.
        maxzoom: Maximum zoom of the tiles (overzoomed beyond).
        attribution: Attribution text for the source.
        source_layers: Vector tiles: the source layers to draw (default all).
        paint: Vector tiles: paint properties for the generated ``fill-*``,
            ``line-*`` and ``circle-*`` layers, e.g. ``{"fill-color": "#ccc"}``.
            Others are dropped with a warning.
        auth_token: Optional access token for the tile service.

    Returns:
        The style JSON.

    Raises:
        ValidationError: If the type cannot be detected or does not match the
            data, or ``source`` is not found.
    """
    style, dropped = _core.tile_style(
        str(source), type, tile_size, scheme, minzoom, maxzoom, attribution, source_layers,
        json.dumps(paint or {}), auth_token,
    )
    if dropped:
        warnings.warn(
            f"tile_style: paint key(s) that match no generated layer dropped: {', '.join(dropped)}",
            UserWarning,
            stacklevel=2,
        )
    return style


def wms_style(
    url: str,
    layers: str,
    styles: str = "",
    version: str = "1.3.0",
    format: str = "image/png",
    transparent: bool = False,
    extra_params: Optional[str] = None,
) -> str:
    """Style for an OGC WMS GetMap endpoint, as 256 px tiles in EPSG:3857.

    Args:
        url: Base service URL without query parameters.
        layers: ``LAYERS`` parameter.
        styles: ``STYLES`` parameter.
        version: Service version (1.1.x sends ``SRS``, 1.3.0 ``CRS``).
        format: Image MIME type.
        transparent: Request transparent images.
        extra_params: Additional query parameters appended verbatim.
    """
    return _core.wms_style(url, layers, styles, version, format, bool(transparent), extra_params)


def wmts_style(
    url: str,
    layer: str,
    tilematrixset: str = "GoogleMapsCompatible",
    format: str = "image/png",
    style: str = "default",
    version: str = "1.0.0",
    extra_params: Optional[str] = None,
) -> str:
    """Style for an OGC WMTS GetTile (KVP) endpoint. A RESTful WMTS
    template (``{TileMatrix}/{TileRow}/{TileCol}``) needs no builder: pass it
    as ``style``.

    Args:
        url: Base service URL without query parameters.
        layer: ``LAYER`` parameter.
        tilematrixset: Tile matrix set (a Web Mercator one).
        format: Image MIME type.
        style: ``STYLE`` parameter.
        version: Service version.
        extra_params: Additional query parameters appended verbatim.
    """
    return _core.wmts_style(url, layer, tilematrixset, format, style, version, extra_params)
