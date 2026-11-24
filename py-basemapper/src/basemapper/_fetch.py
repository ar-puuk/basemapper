"""How basemaps download their data: :class:`FetchOptions`."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict

from .exceptions import ValidationError

__all__ = ["FetchOptions"]


@dataclass(frozen=True)
class FetchOptions:
    """Settings for downloading a basemap's data.

    Give one as ``fetch=`` to :func:`basemap`. The defaults suit most maps;
    change them for slow servers, very large maps, or to stop on missing
    data. R: ``fetch_options()``; Rust: ``FetchOptions``.

    Args:
        timeout: Seconds to wait for each download.
        max_tiles: Most tiles fetched per style. A view needing more is an
            error for vector tiles; raster tiles are fetched one zoom level
            coarser instead (an *adjusted* note).
        concurrency: Downloads at once.
        on_error: ``"skip"`` draws what there is, each gap a
            :class:`BasemapProblem` warning; ``"fail"`` raises an error for a
            failed tile or any other problem.

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

    def _spec(self) -> Dict[str, Any]:
        """As the core reads it."""
        return {"timeout": float(self.timeout), "max_tiles": self.max_tiles, "concurrency": self.concurrency,
                "on_error": self.on_error}


def fetch_arg(fetch: Any) -> FetchOptions:
    """``fetch`` checked (None is the defaults)."""
    if fetch is None:
        return FetchOptions()
    if not isinstance(fetch, FetchOptions):
        raise ValidationError(f"fetch must be a basemapper.FetchOptions, not {type(fetch).__name__}")
    return fetch
