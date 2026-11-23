"""How basemaps download tiles: :class:`FetchOptions`."""

from __future__ import annotations

import warnings
from dataclasses import dataclass
from typing import Any, Dict, Optional

from .exceptions import ValidationError

__all__ = ["FetchOptions"]


@dataclass(frozen=True)
class FetchOptions:
    """Settings for downloading a basemap's tiles.

    Give one as ``fetch=`` to :func:`basemap`, :func:`render_basemap_raw`,
    :func:`add_basemap` and :func:`geom_basemap`. The defaults suit most
    maps; change them for slow servers, very large maps, or to stop on a
    missing tile. R: ``fetch_options()``.

    Args:
        timeout: Seconds to wait for each tile.
        max_tiles: Most tiles fetched per style. A view needing more is an
            error for vector tiles; raster tiles are fetched one zoom level
            coarser instead.
        concurrency: Tiles downloaded at once.
        on_error: What a tile that fails to download does: ``"skip"`` leaves
            it empty (the map is drawn without it), ``"fail"`` raises an
            error.

    Example:
        >>> import basemapper
        >>> slow = basemapper.FetchOptions(timeout=30, on_error="fail")
        >>> m = basemapper.basemap("OpenFreeMap.Liberty", center=(-111.89, 40.76), zoom=12, fetch=slow)
    """

    timeout: float = 10.0
    max_tiles: int = 256
    concurrency: int = 16
    on_error: str = "skip"

    def __post_init__(self) -> None:
        if isinstance(self.timeout, bool) or not isinstance(self.timeout, (int, float)) or not self.timeout > 0:
            raise ValidationError(f"timeout must be a number of seconds > 0, not {self.timeout!r}")
        for name in ("max_tiles", "concurrency"):
            v = getattr(self, name)
            if isinstance(v, bool) or not isinstance(v, int) or v < 1:
                raise ValidationError(f"{name} must be a whole number of at least 1, not {v!r}")
        if self.on_error not in ("skip", "fail"):
            raise ValidationError(f'on_error must be "skip" or "fail", not {self.on_error!r}')

    def _core(self) -> Dict[str, Any]:
        """The Rust core's names for these settings."""
        return dict(
            tile_timeout_ms=min(int(round(self.timeout * 1000)), 2**31 - 1),
            max_tiles=self.max_tiles,
            tile_concurrency=self.concurrency,
            fail_on_tile_error=self.on_error == "fail",
        )


# The keyword arguments `fetch` replaces, and the FetchOptions field each maps to.
_DEPRECATED = {
    "tile_timeout_ms": "timeout",
    "max_tiles": "max_tiles",
    "tile_concurrency": "concurrency",
    "fail_on_tile_error": "on_error",
}


def _resolve_fetch(fetch: Optional[FetchOptions], fn: str, deprecated: Dict[str, Any], stacklevel: int = 3) -> FetchOptions:
    """``fetch`` (default :class:`FetchOptions`) with the replaced keyword
    arguments in ``deprecated`` applied, each with a DeprecationWarning;
    any other keyword is a TypeError, as Python raises for unknown ones."""
    if fetch is None:
        fetch = FetchOptions()
    elif not isinstance(fetch, FetchOptions):
        raise ValidationError(f"fetch must be a basemapper.FetchOptions, not {type(fetch).__name__}")
    unknown = [k for k in deprecated if k not in _DEPRECATED]
    if unknown:
        raise TypeError(f"{fn}() got an unexpected keyword argument {unknown[0]!r}")
    if not deprecated:
        return fetch
    changes: Dict[str, Any] = {}
    for old, value in deprecated.items():
        new = _DEPRECATED[old]
        hint = f"fetch=basemapper.FetchOptions({new}=...)" + (" (in seconds)" if old == "tile_timeout_ms" else "")
        warnings.warn(f"{fn}({old}=) is deprecated; use {hint}", DeprecationWarning, stacklevel=stacklevel)
        if old == "tile_timeout_ms":
            changes[new] = value / 1000
        elif old == "fail_on_tile_error":
            changes[new] = "fail" if value else "skip"
        else:
            changes[new] = value
    from dataclasses import replace

    return replace(fetch, **changes)
