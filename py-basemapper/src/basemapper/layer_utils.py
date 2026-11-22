"""Style inspection: :func:`list_layers` and :func:`get_attribution`."""

from __future__ import annotations

from typing import Any, Dict, List, Optional


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


def list_layers(style: str, auth_token: Optional[str] = None) -> List[Dict[str, str]]:
    """List a style's layers.

    The layers of a style in drawing order, with their types and features:
    the IDs are what the ``layers`` filter of :func:`basemap` and the
    renderers accepts (plain IDs keep, ``-`` prefixed IDs drop), the
    features what ``features`` accepts; ``symbol`` layers are the labels
    ``labels`` keeps or drops. The same function exists in R
    (``list_layers()``, returning a data frame).

    Args:
        style: A MapLibre GL style URL or JSON string.
        auth_token: Optional access token for the style URL.

    Returns:
        A list of ``{"id": ..., "type": ..., "feature": ...}`` dicts
        (``pandas.DataFrame(...)`` turns it into a table).

    Raises:
        BasemapError: If the style cannot be fetched or parsed.
    """
    from ._basemapper import list_layers as _impl

    return [{"id": i, "type": t, "feature": f} for i, t, f in _impl(str(style), auth_token)]
