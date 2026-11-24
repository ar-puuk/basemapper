"""Inspecting styles: :func:`list_layers` and :func:`get_attribution`."""

from __future__ import annotations

from typing import Any, Dict, List, Optional


def _styles_of(x: Any):
    from ._basemap import Basemap
    from ._style import styles_arg

    return x.styles if isinstance(x, Basemap) else styles_arg(x)


def list_layers(style: Any) -> List[Dict[str, Optional[str]]]:
    """The layers a style draws, in drawing order, with its filters applied.

    Each layer's ``id`` (what a style's ``layers`` matches), ``type``
    (``symbol`` layers are the labels), ``source_layer`` (the vector tile
    layer drawn; None for background and raster layers) and ``feature`` (what
    ``features`` matches). A style with filters lists the layers it keeps,
    so this previews a filter: ``list_layers(style(s, features="roads"))``.
    A catalogue stack lists every part's layers, bottom to top. The same
    function exists in R.

    A layer's feature comes from its source layer, with tables for the
    OpenMapTiles, Protomaps and Shortbread schemas and keywords for others:
    a convenience to check here, with ``layers`` for exact control.

    Args:
        style: A style (anything :func:`style` takes).

    Returns:
        A list of ``{"id", "type", "source_layer", "feature"}`` dicts
        (``pandas.DataFrame(...)`` makes it a table).
    """
    from . import _basemapper as _core
    from ._style import style_arg

    rows = _core.list_layers(style_arg(style)._spec())
    return [{"id": i, "type": t, "source_layer": sl, "feature": f} for i, t, sl, f in rows]


def get_attribution(x: Any) -> str:
    """The credit line a basemap's providers ask maps to show.

    Every source the styles draw from contributes its attribution (from the
    style, its TileJSON or PMTiles metadata), as plain text without
    duplicates, joined with ``" | "``. The plot layers draw it by default.

    Args:
        x: A :class:`Basemap`, a style, or a list of styles.

    Returns:
        The attribution; an empty string when the styles give none.
    """
    from . import _basemapper as _core

    return _core.attribution([s._spec() for s in _styles_of(x)])
