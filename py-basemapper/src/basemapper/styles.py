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
from typing import Dict, List, Optional, Union

from . import _basemapper as _core


def basemap_provider(
    name: str,
    type: Optional[str] = None,
    api_key: Optional[str] = None,
    tiles: Optional[str] = None,
    labels: Union[bool, str] = True,
    features: Optional[List[str]] = None,
) -> Union[str, List[str]]:
    """A named basemap's style (styles, for a basemap of stacked services).

    Names follow leaflet-providers (``"CartoDB.Positron"``,
    ``"Esri.WorldImagery"``, ``"OpenStreetMap.HOT"``); see
    :func:`list_providers` or the :data:`providers` object. A name also works
    as ``style`` directly; this function picks the form, passes a key, or
    swaps in your own tiles.

    Args:
        name: Basemap name (case-insensitive).
        type: ``"vector"`` (a MapLibre style) or ``"raster"`` (tiles). By
            default the vector form when there is one, else raster. Asking
            for a form a basemap lacks is an error, never a silent fallback.
        api_key: API key for CARTO, MapTiler and Stadia, or the quad word
            for UGRC Discover's basemaps (``"UGRC.Terrain"``,
            ``"UGRC.Topo"``, ``"UGRC.Hillshade"``, ``"UGRC.Imagery"``, ...); by
            default read from ``CARTO_API_KEY``, ``MAPTILER_API_KEY``,
            ``STADIA_API_KEY`` or ``UGRC_QUAD_WORD``. The keys are free for
            moderate use (UGRC's for Utah's government and partners);
            :func:`list_providers` shows which basemaps need one.
        tiles: Your own tile data to draw with this basemap's style: a
            ``.pmtiles`` archive (URL or path), tile URL or TileJSON. It must
            use the same schema as the provider's tiles (OpenMapTiles for
            OpenFreeMap and Stadia).
        labels: ``True`` keeps every layer, ``False`` drops the labels,
            ``"only"`` keeps only the labels, to stack this basemap's labels
            over another map. Raster tiles switch to the provider's
            label-free or label-only tiles (an error when it has none).
        features: Vector form: features to keep (``["water", "roads"]``)
            or drop (``["-buildings"]``); see :func:`list_layers`.

    Returns:
        A style URL, or inline style JSON (the raster form, or with
        ``tiles``, ``labels`` or ``features``). A basemap published as
        separate services (``"UGRC.Lite"``: hillshade, base and labels)
        returns a list of styles, bottom to top, which :func:`basemap` and
        the plot layers take as a stack; ``labels`` and ``features`` apply
        to the stack as one basemap, leaving out a style they empty.

    Raises:
        ValidationError: For an unknown name (with suggestions), a form the
            basemap lacks, or a missing API key.
    """
    from ._render import _features_arg, labels_arg

    return _core.basemap_provider(
        name, type, api_key, None if tiles is None else str(tiles), labels_arg(labels), _features_arg(features)
    )


def list_providers(type: Optional[str] = None) -> List[Dict[str, object]]:
    """The basemaps :func:`basemap_provider` knows.

    Args:
        type: ``"vector"`` or ``"raster"`` to list only basemaps with that
            form; None lists all.

    Returns:
        One dict per basemap: ``name``, ``vector`` and ``raster`` (the forms
        it has), ``api_key_env`` (the environment variable its key is read
        from, or None when it needs none) and ``attribution``.
    """
    return [
        {"name": n, "vector": v, "raster": r, "api_key_env": k, "attribution": a}
        for n, v, r, k, a in _core.list_providers(type)
    ]


class _Group:
    """Basemap names of one provider, as attributes."""

    def __init__(self, name: str, names: Dict[str, str]):
        self._name = name
        self.__dict__.update(names)

    def __repr__(self) -> str:
        names = [v for k, v in self.__dict__.items() if not k.startswith("_")]
        return f"<{self._name}: {', '.join(names)}>"


def _providers_object() -> _Group:
    groups: Dict[str, Dict[str, str]] = {}
    top: Dict[str, object] = {}
    for p in _core.list_providers(None):
        name = p[0]
        family, _, variant = name.partition(".")
        if variant:
            groups.setdefault(family, {})[variant] = name
        else:
            top[name] = name
    for family, names in groups.items():
        top[family] = _Group(family, names)
    return _Group("providers", top)


providers = _providers_object()
"""Basemap names for autocompletion: ``providers.CartoDB.Positron`` is the
string ``"CartoDB.Positron"``, usable as ``style`` or with
:func:`basemap_provider`."""


def tile_cache(dir: Optional[str] = None, clear: bool = False) -> Optional[str]:
    """The disk cache for map tiles.

    Tiles are kept as long as the server's caching headers allow (a day when
    they say nothing), so repeated renders of an area do not download them
    again. The cache starts in ``BASEMAPPER_CACHE_DIR`` when set (``off``
    disables it), else in the platform cache directory.

    Args:
        dir: Move the cache to this directory; ``False`` or ``""`` turns it
            off. None leaves it as it is.
        clear: Delete every cached tile.

    Returns:
        The cache directory, or None when the cache is off.
    """
    if dir is False:
        dir = ""
    return _core.tile_cache(None if dir is None else str(dir), bool(clear))


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
