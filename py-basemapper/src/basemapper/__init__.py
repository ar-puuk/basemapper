"""basemapper — render MapLibre GL basemaps headlessly, as images or SVG.

The API matches the R package and the Rust core: the same function and
argument names, with R's ``verb(m, ...)`` written ``m.verb(...)`` here.
"""

from .exceptions import BasemapError, NetworkError, StyleError, ValidationError
from ._basemap import Basemap, basemap
from ._render import render_basemap_raw
from .layer_utils import list_layers
from .matplotlib_integration import add_basemap
from .providers import (
    basemap_provider,
    esri_raster_provider,
    esri_vector_provider,
    list_providers,
    pmtiles_provider,
    raster_provider,
    vector_provider,
    wms_provider,
    wmts_provider,
)

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
    "basemap_provider",
    "list_providers",
    "raster_provider",
    "vector_provider",
    "esri_raster_provider",
    "esri_vector_provider",
    "wms_provider",
    "wmts_provider",
    "pmtiles_provider",
    "add_basemap",
    *__all_plotnine__,
    "BasemapError",
    "ValidationError",
    "StyleError",
    "NetworkError",
]
