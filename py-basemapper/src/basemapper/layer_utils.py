"""Style inspection: :func:`list_layers` and :func:`get_attribution`."""

from __future__ import annotations

from typing import Any, Dict, List, Optional, Union


def get_attribution(style: Any, auth_token: Optional[str] = None) -> str:
    """The attribution text a style asks maps to show.

    Every source the style's layers draw from contributes its attribution
    (from the style, the source's TileJSON or PMTiles metadata), as MapLibre's
    attribution control collects it, as plain text without duplicates, joined
    with ``" | "``. Use it for a caption or a credit line, e.g.
    ``ax.annotate(get_attribution(style), ...)``. :meth:`Basemap.get_attribution`
    gives a view's. The same function exists in R (``get_attribution()``).

    Args:
        style: A style (anything :func:`basemap` takes), or a list of
            styles stacked bottom to top.
        auth_token: Optional access token for the tile service.

    Returns:
        The attribution; an empty string when the styles give none.
    """
    from ._basemap import _style_strings
    from ._basemapper import attribution

    return attribution(_style_strings(style), auth_token)


def list_layers(
    style: Any,
    auth_token: Optional[str] = None,
    layers: Optional[List[str]] = None,
    labels: Union[bool, str] = True,
    features: Optional[List[str]] = None,
) -> List[Dict[str, Optional[str]]]:
    """List a style's layers, or preview a filter.

    The layers of a style in drawing order: their ``id`` (what ``layers``
    matches), ``type`` (``symbol`` layers are the labels ``labels`` keeps or
    drops), ``source_layer`` (the vector tile layer drawn, None for
    background and raster layers) and ``feature`` (what ``features``
    matches). With ``layers``, ``labels`` or ``features``, the layers a
    render with those filters draws; an empty list when they leave nothing.
    The same function exists in R (``list_layers()``, returning a data
    frame).

    A layer's feature comes from its source layer, with tables for the
    OpenMapTiles (OpenFreeMap, Stadia, MapTiler), Protomaps (CARTO) and
    Shortbread (VersaTiles) schemas, and keywords for others (Esri's
    descriptive names, layer ids). It is a convenience: check it here, and
    use ``layers`` for exact control.

    Args:
        style: A style (anything :func:`basemap` takes).
        auth_token: Optional access token for the style URL.
        layers: Layer filter to preview: plain IDs keep, ``-`` prefixed IDs
            drop; ``*`` and ``?`` are wildcards.
        labels: ``True``, ``False`` or ``"only"``, as the renderers take it.
        features: Feature filter to preview, as the renderers take it.

    Returns:
        A list of ``{"id", "type", "source_layer", "feature"}`` dicts
        (``pandas.DataFrame(...)`` turns it into a table).

    Raises:
        BasemapError: If the style cannot be fetched or parsed, or a filter
            is invalid.
    """
    from ._basemapper import list_layers as _impl
    from ._render import _features_arg, labels_arg

    rows = _impl(str(style), auth_token, layers, labels_arg(labels), _features_arg(features))
    return [{"id": i, "type": t, "source_layer": sl, "feature": f} for i, t, sl, f in rows]
