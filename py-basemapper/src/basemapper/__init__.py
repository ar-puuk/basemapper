"""basemapper — render MapLibre GL basemaps headlessly, as images or SVG.

The API matches the R package and the Rust core: the same function and
argument names, with R's ``verb(m, ...)`` written ``m.verb(...)`` here.
"""

from .exceptions import BasemapError, NetworkError, StyleError, ValidationError
from ._basemap import Basemap, basemap
from ._render import render_basemap_raw
from .layer_utils import get_attribution, list_layers
from .matplotlib_integration import add_basemap
from .styles import basemap_provider, list_providers, providers, tile_cache, tile_style, wms_style, wmts_style

# plotnine integration is optional — base package usable without plotnine installed.
try:
    from .plotnine_integration import geom_basemap  # noqa: F401
    __all_plotnine__ = ["geom_basemap"]
except ImportError:
    __all_plotnine__ = []

__all__ = [
    "basemap",
    "Basemap",
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
    *__all_plotnine__,
    "BasemapError",
    "ValidationError",
    "StyleError",
    "NetworkError",
]
