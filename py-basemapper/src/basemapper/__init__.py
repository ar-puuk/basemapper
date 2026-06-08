"""basemapper — headless spatial basemap renderer."""

from .exceptions import BasemapError
from .matplotlib_integration import add_basemap
from .layer_utils import list_layers
from .providers import EsriRasterProvider, EsriVectorProvider, RasterProvider, VectorProvider

# Core Rust FFI binding (compiled by maturin).
from ._basemapper import render_basemap_raw  # noqa: E402  (compiled extension)

# plotnine integration is optional — base package usable without plotnine installed.
try:
    from .plotnine_integration import geom_basemap  # noqa: F401
    __all_plotnine__ = ["geom_basemap"]
except ImportError:
    __all_plotnine__ = []

__all__ = [
    "BasemapError",
    "add_basemap",
    "render_basemap_raw",
    "list_layers",
    "RasterProvider",
    "VectorProvider",
    "EsriVectorProvider",
    "EsriRasterProvider",
    *__all_plotnine__,
]
