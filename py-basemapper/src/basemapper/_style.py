"""Styles as values: :func:`style` and the source helpers :func:`provider`,
:func:`tiles`, :func:`wms` and :func:`wmts` (docs/design/api-review.md §4.3).

A style is what to draw: a source plus its own filters and credential. A
plain string works wherever a style is expected and means ``style(string)``.
Every style is checked by the core when it is made, so a bad filter or
source fails here, not at the first render.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple, Union

from .exceptions import ValidationError


@dataclass(frozen=True)
class Source:
    """Where a style's map comes from, with that source's own options. Made by
    :func:`provider`, :func:`tiles`, :func:`wms` or :func:`wmts`."""

    kind: str
    options: Tuple[Tuple[str, Any], ...] = ()

    def _spec(self) -> Dict[str, Any]:
        return {"kind": self.kind, **dict(self.options)}


def _source(kind: str, **options: Any) -> Source:
    return Source(kind, tuple((k, v) for k, v in options.items() if v is not None))


def provider(name: str, form: Optional[str] = None, tiles: Optional[str] = None) -> Source:
    """A catalogue basemap, in a chosen form or with your own tile data.

    A catalogue name also works as a style on its own (``"CartoDB.Positron"``);
    this helper picks the form or swaps the tiles. See :func:`list_providers`.

    Args:
        name: The basemap's name (case-insensitive), e.g. ``"CartoDB.Positron"``.
        form: ``"vector"`` (a MapLibre style) or ``"raster"`` (tiles). By
            default the vector form when there is one. A form the basemap
            lacks is an error, never a silent fallback.
        tiles: Your own tile data drawn with this basemap's style (a
            ``.pmtiles`` archive, tile URL or TileJSON, by URL or path), in
            the same schema as the provider's tiles.
    """
    return _source("provider", name=str(name), form=form, tiles=None if tiles is None else str(tiles))


def tiles(
    input: str,
    *,
    type: Optional[str] = None,
    tile_size: Optional[int] = None,
    scheme: Optional[str] = None,
    minzoom: Optional[int] = None,
    maxzoom: Optional[int] = None,
    attribution: Optional[str] = None,
    source_layers: Optional[Sequence[str]] = None,
    paint: Optional[Mapping[str, Any]] = None,
) -> Source:
    """Tile data, with a style built for it and overrides.

    Tile data also works as a style on its own; this helper overrides what is
    detected or styles vector tiles.

    Args:
        input: A tile URL template (``{z}/{x}/{y}``, ``{quadkey}``,
            ``{bbox-epsg-3857}``, WMTS ``{TileMatrix}/{TileRow}/{TileCol}``),
            a TileJSON, a PMTiles archive (URL or path) or an ArcGIS
            ``.../MapServer`` or ``.../VectorTileServer``.
        type: ``"raster"`` or ``"vector"``; detected when None.
        tile_size: Raster tile size in px (default 256).
        scheme: ``"xyz"`` or ``"tms"`` (rows counted from the south).
        minzoom: Minimum zoom of the tiles.
        maxzoom: Maximum zoom of the tiles (enlarged beyond).
        attribution: Attribution text for the source.
        source_layers: Vector tiles: the source layers to draw (default all).
        paint: Vector tiles: paint properties for the generated ``fill-*``,
            ``line-*`` and ``circle-*`` layers, e.g. ``{"fill-color": "#ccc"}``.
            A key that matches no layer is a :class:`BasemapProblem` warning.
    """
    return _source(
        "tiles", input=str(input), type=type, tile_size=tile_size, scheme=scheme, minzoom=minzoom, maxzoom=maxzoom,
        attribution=attribution, source_layers=None if source_layers is None else [str(s) for s in source_layers],
        paint=None if paint is None else json.dumps(dict(paint)),
    )


def wms(
    url: str,
    layers: str,
    *,
    styles: str = "",
    version: str = "1.3.0",
    format: str = "image/png",
    transparent: bool = False,
    extra_params: Optional[str] = None,
) -> Source:
    """An OGC WMS service, drawn as 256 px tiles in Web Mercator.

    Args:
        url: The service URL, without query parameters.
        layers: The ``LAYERS`` parameter.
        styles: The ``STYLES`` parameter.
        version: The service version (1.1.x sends ``SRS``, 1.3.0 ``CRS``).
        format: The image MIME type.
        transparent: Request transparent images.
        extra_params: More query parameters, appended verbatim.
    """
    return _source("wms", url=str(url), layers=str(layers), styles=styles, version=version, format=format,
                   transparent=bool(transparent), extra_params=extra_params)


def wmts(
    url: str,
    layer: str,
    *,
    tilematrixset: str = "GoogleMapsCompatible",
    format: str = "image/png",
    style: str = "default",
    version: str = "1.0.0",
    extra_params: Optional[str] = None,
) -> Source:
    """An OGC WMTS service (KVP GetTile). A RESTful WMTS template
    (``{TileMatrix}/{TileRow}/{TileCol}``) needs no helper: give it as the
    style.

    Args:
        url: The service URL, without query parameters.
        layer: The ``LAYER`` parameter.
        tilematrixset: A Web Mercator tile matrix set.
        format: The image MIME type.
        style: The ``STYLE`` parameter.
        version: The service version.
        extra_params: More query parameters, appended verbatim.
    """
    return _source("wmts", url=str(url), layer=str(layer), tilematrixset=tilematrixset, format=format, style=style,
                   version=version, extra_params=extra_params)


def _strings(value: Union[None, str, Sequence[str]], name: str) -> Optional[Tuple[str, ...]]:
    if value is None:
        return None
    if isinstance(value, str):
        return (value,)
    try:
        return tuple(str(v) for v in value)
    except TypeError:
        raise ValidationError(f"{name} must be a string or a list of strings, not {type(value).__name__}") from None


@dataclass(frozen=True)
class Style:
    """A style: what to draw, with its own filters and credential. Made by
    :func:`style`; a plain string means ``style(string)``."""

    source: Union[str, Source]
    layers: Optional[Tuple[str, ...]] = None
    features: Optional[Tuple[str, ...]] = None
    key: Optional[str] = field(default=None, repr=False)

    def _spec(self) -> Dict[str, Any]:
        source = {"kind": "auto", "value": self.source} if isinstance(self.source, str) else self.source._spec()
        spec: Dict[str, Any] = {"source": source}
        if self.layers is not None:
            spec["layers"] = list(self.layers)
        if self.features is not None:
            spec["features"] = list(self.features)
        if self.key is not None:
            spec["key"] = self.key
        return spec


def style(
    source: Union[str, Source, Style],
    *,
    layers: Union[None, str, Sequence[str]] = None,
    features: Union[None, str, Sequence[str]] = None,
    key: Optional[str] = None,
) -> Style:
    """A style: a source and what to draw of it.

    Args:
        source: A catalogue name (``"OpenFreeMap.Liberty"``; see
            :func:`list_providers`), a MapLibre style URL, file or JSON, tile
            data (a tile URL, TileJSON, PMTiles archive, ArcGIS service), or
            a source helper: :func:`provider`, :func:`tiles`, :func:`wms`,
            :func:`wmts`.
        layers: Layer ids to keep, or (``-``-prefixed) to drop; ``*`` and
            ``?`` are wildcards (``"-label_*"``). See :func:`list_layers`.
        features: Feature classes to keep or (``-``-prefixed) drop:
            background, water, landcover, parks, buildings, roads, rail,
            boundaries, places, poi, terrain, other, and ``"labels"``
            (``"labels"`` keeps only the labels, ``"-labels"`` drops them;
            ``["roads", "labels"]`` keeps the road labels).
        key: The credential: an API key for catalogue basemaps (default from
            their environment variable), else a token sent only to the
            style's own server.

    Returns:
        A :class:`Style`, checked: a bad filter or source is an error now.

    Example:
        >>> import basemapper
        >>> imagery = basemapper.style("Esri.WorldImagery")
        >>> labels = basemapper.style("OpenFreeMap.Liberty", features="labels")
        >>> m = basemapper.basemap([imagery, labels], center=(-111.89, 40.76), zoom=13)
    """
    if isinstance(source, Style):
        base = source
        source = base.source
        layers = base.layers if layers is None else layers
        features = base.features if features is None else features
        key = base.key if key is None else key
    elif not isinstance(source, (str, Source)):
        source = str(source)
    s = Style(source, _strings(layers, "layers"), _strings(features, "features"), key)
    from . import _basemapper as _core

    _core.check_style(s._spec())
    return s


def style_arg(value: Any) -> Style:
    """A style, or anything :func:`style` takes."""
    return value if isinstance(value, Style) else style(value)


def styles_arg(value: Any) -> Tuple[Style, ...]:
    """One style or a list of them (stacked bottom to top)."""
    items: List[Any] = list(value) if isinstance(value, (list, tuple)) else [value]
    if not items:
        raise ValidationError("a basemap needs at least one style")
    return tuple(style_arg(v) for v in items)
