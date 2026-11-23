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

# The plot integrations need optional packages (pip extras): they load when
# first used, so the package itself needs only numpy and pyproj.
_OPTIONAL = {
    "add_basemap": (".matplotlib_integration", "matplotlib"),
    "geom_basemap": (".plotnine_integration", "plotnine"),
}


def __getattr__(name):
    if name in _OPTIONAL:
        module, extra = _OPTIONAL[name]
        import importlib

        try:
            value = getattr(importlib.import_module(module, __name__), name)
        except ImportError as e:
            raise ImportError(
                f"basemapper.{name} needs {extra} (and xarray): pip install 'basemapper[{extra}]'"
                f" (or install {extra} and xarray); {e}"
            ) from e
        globals()[name] = value
        return value
    raise AttributeError(f"module 'basemapper' has no attribute {name!r}")


def _installed(*modules):
    import importlib.util

    return all(importlib.util.find_spec(m) is not None for m in modules)


__all_optional__ = [
    n for n, ok in (("add_basemap", _installed("matplotlib", "xarray")), ("geom_basemap", _installed("plotnine", "xarray"))) if ok
]

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
    *__all_optional__,
    "BasemapError",
    "ValidationError",
    "StyleError",
    "NetworkError",
]
