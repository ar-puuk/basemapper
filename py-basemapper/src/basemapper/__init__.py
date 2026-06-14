"""basemapper — headless spatial basemap renderer."""

from .exceptions import BasemapError, NetworkError, StyleError, ValidationError
from .matplotlib_integration import add_basemap
from .layer_utils import list_layers
from .providers import EsriRasterProvider, EsriVectorProvider, RasterProvider, VectorProvider

# Core Rust FFI binding — thin Python wrapper so doc tools can do static analysis.
from ._render import render_basemap_raw

# plotnine integration is optional — base package usable without plotnine installed.
try:
    from .plotnine_integration import geom_basemap  # noqa: F401
    __all_plotnine__ = ["geom_basemap"]
except ImportError:
    __all_plotnine__ = []

__all__ = [
    "BasemapError",
    "ValidationError",
    "StyleError",
    "NetworkError",
    "add_basemap",
    "render_basemap_raw",
    "list_layers",
    "RasterProvider",
    "VectorProvider",
    "EsriVectorProvider",
    "EsriRasterProvider",
    *__all_plotnine__,
]
