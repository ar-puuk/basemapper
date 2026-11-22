"""Layer discovery: :func:`list_layers`."""

from __future__ import annotations

from typing import Dict, List, Optional


def list_layers(style: str, auth_token: Optional[str] = None) -> List[Dict[str, str]]:
    """List a style's layers.

    The layers of a style in drawing order, with their types: the IDs are
    what the ``layers`` filter of :func:`basemap` and the renderers accepts
    (plain IDs keep, ``-`` prefixed IDs drop). The same function exists in R
    (``list_layers()``, returning a data frame).

    Args:
        style: A MapLibre GL style URL or JSON string.
        auth_token: Optional access token for the style URL.

    Returns:
        A list of ``{"id": ..., "type": ...}`` dicts (``pandas.DataFrame(...)``
        turns it into a table).

    Raises:
        BasemapError: If the style cannot be fetched or parsed.
    """
    from ._basemapper import list_layers as _impl

    return [{"id": i, "type": t} for i, t in _impl(str(style), auth_token)]
