"""Styles for common tile services.

Each function returns a style for :func:`basemap` and the other renderers'
``style`` argument: a style URL (named GL styles, ArcGIS vector tile servers)
or inline MapLibre GL style JSON. They are implemented once in the Rust core,
so Python and R (``raster_provider()``, ...) return the same style for the
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


def raster_provider(url_template: str, tile_size: int = 256) -> str:
    """Style for an XYZ raster tile URL with ``{z}``, ``{x}``, ``{y}``
    placeholders."""
    return _core.raster_provider(url_template, int(tile_size))


def vector_provider(url_template: str, source_layer: str, paint: Optional[Dict[str, object]] = None) -> str:
    """Style drawing one layer of an MVT tile URL as a fill and a line layer.

    Args:
        url_template: MVT tile URL with ``{z}``, ``{x}``, ``{y}`` placeholders.
        source_layer: Layer within each vector tile to draw (MapLibre's
            ``source-layer``), e.g. ``"water"``.
        paint: Paint properties: ``fill-color``, ``fill-opacity``,
            ``fill-outline-color``, ``line-color``, ``line-width``,
            ``line-opacity``. Others are dropped with a warning; unset ones
            default to a light grey style.
    """
    style, dropped = _core.vector_provider(url_template, source_layer, json.dumps(paint or {}))
    if dropped:
        warnings.warn(
            f"vector_provider: unrecognised paint key(s) dropped: {', '.join(dropped)}",
            UserWarning,
            stacklevel=2,
        )
    return style


def esri_raster_provider(url_template: str, tile_size: int = 256) -> str:
    """Style for ArcGIS MapServer raster tiles
    (``.../MapServer/tile/{z}/{y}/{x}``)."""
    return _core.esri_raster_provider(url_template, int(tile_size))


def esri_vector_provider(base_url: str) -> str:
    """The published style URL of an ArcGIS VectorTileServer."""
    return _core.esri_vector_provider(base_url)


def wms_provider(
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
        version: Service version.
        format: Image MIME type.
        transparent: Request transparent images.
        extra_params: Additional query parameters appended verbatim.
    """
    return _core.wms_provider(url, layers, styles, version, format, bool(transparent), extra_params)


def wmts_provider(
    url: str,
    layer: str,
    tilematrixset: str = "GoogleMapsCompatible",
    format: str = "image/png",
    style: str = "default",
    version: str = "1.0.0",
    extra_params: Optional[str] = None,
) -> str:
    """Style for an OGC WMTS GetTile (KVP) endpoint.

    Args:
        url: Base service URL without query parameters.
        layer: ``LAYER`` parameter.
        tilematrixset: Tile matrix set.
        format: Image MIME type.
        style: ``STYLE`` parameter.
        version: Service version.
        extra_params: Additional query parameters appended verbatim.
    """
    return _core.wmts_provider(url, layer, tilematrixset, format, style, version, extra_params)


def pmtiles_provider(url: str) -> str:
    """Style for a raster PMTiles archive served over HTTP."""
    return _core.pmtiles_provider(url)
