"""basemapper — render MapLibre GL basemaps headlessly, as images or SVG.

The API matches the R package and the Rust core: the same function and
argument names, with R's ``verb(m, ...)`` written ``m.verb(...)`` here.
"""

from .exceptions import BasemapError, NetworkError, StyleError, ValidationError
from ._basemap import Basemap, basemap
from ._fetch import FetchOptions
from ._render import render_basemap_raw
from .layer_utils import get_attribution, list_layers
from .styles import basemap_provider, list_providers, providers, tile_cache, tile_style, wms_style, wmts_style

# The version R, Python and the Rust core share (the release tag is v + it).
try:
    from importlib.metadata import version as _version

    __version__ = _version("basemapper")
except Exception:  # not installed as a package (a source checkout)
    __version__ = "0.0.0"

# The plot integrations need optional packages (pip extras
# `basemapper[matplotlib]`, `[plotnine]`); the package itself needs only
# numpy and pyproj. Plain imports (not lazy ones) so documentation tools
# see them; when the extra is missing, using the name says what to install.
try:
    from .matplotlib_integration import add_basemap
except ImportError:
    pass
try:
    from .plotnine_integration import geom_basemap
except ImportError:
    pass

_EXTRAS = {"add_basemap": "matplotlib", "geom_basemap": "plotnine"}


def __getattr__(name):
    # Reached only when the import above failed.
    if name in _EXTRAS:
        extra = _EXTRAS[name]
        raise ImportError(
            f"basemapper.{name} needs {extra} and xarray: pip install 'basemapper[{extra}]'"
            f" (or install {extra} and xarray)"
        )
    raise AttributeError(f"module 'basemapper' has no attribute {name!r}")


__all__ = [
    "__version__",
    "basemap",
    "Basemap",
    "FetchOptions",
    "render_basemap_raw",
    "list_layers",
    "get_attribution",
    "basemap_provider",
    "list_providers",
    "providers",
    "tile_cache",
    "tile_style",
    "wms_style",
    "wmts_style",
    "add_basemap",
    "geom_basemap",
    "BasemapError",
    "ValidationError",
    "StyleError",
    "NetworkError",
]
