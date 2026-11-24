"""The catalogue of named basemaps, and the tile cache."""

from __future__ import annotations

from typing import Dict, List, Optional

from . import _basemapper as _core


def list_providers(type: Optional[str] = None) -> List[Dict[str, object]]:
    """The named basemaps: their names work as styles (``"CartoDB.Positron"``),
    and with :func:`provider` for a chosen form.

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

    def __init__(self, name: str, names: Dict[str, object]):
        self._name = name
        self.__dict__.update(names)

    def __repr__(self) -> str:
        names = [v if isinstance(v, str) else k for k, v in self.__dict__.items() if not k.startswith("_")]
        return f"<{self._name}: {', '.join(names)}>"


def _providers_object() -> _Group:
    groups: Dict[str, Dict[str, object]] = {}
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
string ``"CartoDB.Positron"``, usable as a style."""


def tile_cache(dir: Optional[str] = None, clear: bool = False, max_size: Optional[float] = None) -> Optional[str]:
    """The disk cache for map tiles.

    Tiles are kept as long as the server's caching headers allow (a day when
    they say nothing), so repeated renders of an area do not download them
    again. The cache starts in ``BASEMAPPER_CACHE_DIR`` when set (``off``
    disables it), else in the platform cache directory. Past its size limit
    (250 MB, or ``BASEMAPPER_CACHE_MAX_SIZE`` in MB) the least recently used
    tiles go first. R shares it.

    Args:
        dir: Move the cache to this directory; ``False`` or ``""`` turns it
            off. None leaves it as it is.
        clear: Delete every cached tile.
        max_size: The size limit in megabytes; None leaves it as it is.

    Returns:
        The cache directory, or None when the cache is off.
    """
    if dir is False:
        dir = ""
    return _core.tile_cache(None if dir is None else str(dir), bool(clear), None if max_size is None else float(max_size))
