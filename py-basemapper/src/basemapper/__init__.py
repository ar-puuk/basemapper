"""basemapper: MapLibre basemaps, rendered headlessly, as images, SVG and
plot layers.

The same model as the R package and the Rust core: a :func:`basemap` of
:func:`style` values, rendered to a :class:`Rendering` or placed in a plot
(``m.add_to(ax)``, ``ggplot(df) + m``).
"""

from ._basemap import MIGRATION, Basemap, basemap
from ._crs import around, outside
from ._fetch import FetchOptions
from ._notes import Note
from ._rendering import Rendering
from ._style import Source, Style, provider, style, tiles, wms, wmts
from .exceptions import (
    BasemapAdjustment,
    BasemapError,
    BasemapProblem,
    BasemapWarning,
    NetworkError,
    StyleError,
    ValidationError,
)
from .layer_utils import get_attribution, list_layers
from .styles import list_providers, providers, tile_cache

# The version R, Python and the Rust core share (the release tag is v + it).
try:
    from importlib.metadata import version as _version

    __version__ = _version("basemapper")
except Exception:  # not installed as a package (a source checkout)
    __version__ = "0.0.0"

# The plot integrations need optional packages (pip extras
# `basemapper[matplotlib]`, `[plotnine]`). Plain imports, so documentation
# tools see them; when the extra is missing, using the name says what to
# install.
try:
    from .matplotlib_integration import add_basemap
except ImportError:
    pass
try:
    from .plotnine_integration import geom_basemap
except ImportError:
    pass

_EXTRAS = {"add_basemap": "matplotlib", "geom_basemap": "plotnine"}

# 0.2 names that 0.3 replaced (D1: no shims), and their replacements.
_REMOVED = {
    "render_basemap_raw": "basemap(style, bounds=..., width=..., height=...).render().to_numpy()",
    "basemap_provider": "style(provider(name, form=..., tiles=...), features=..., key=...)",
    "tile_style": "style(tiles(source, ...))",
    "wms_style": "style(wms(url, layers, ...))",
    "wmts_style": "style(wmts(url, layer, ...))",
}


def __getattr__(name):
    if name in _EXTRAS:
        extra = _EXTRAS[name]
        raise ImportError(f"basemapper.{name} needs {extra}: pip install 'basemapper[{extra}]'")
    if name in _REMOVED:
        raise AttributeError(f"basemapper.{name} was removed in 0.3; use {_REMOVED[name]}. See {MIGRATION}")
    raise AttributeError(f"module 'basemapper' has no attribute {name!r}")


__all__ = [
    "__version__",
    "basemap",
    "Basemap",
    "style",
    "Style",
    "Source",
    "provider",
    "tiles",
    "wms",
    "wmts",
    "around",
    "outside",
    "Rendering",
    "Note",
    "FetchOptions",
    "list_layers",
    "get_attribution",
    "list_providers",
    "providers",
    "tile_cache",
    "add_basemap",
    "geom_basemap",
    "BasemapError",
    "ValidationError",
    "StyleError",
    "NetworkError",
    "BasemapWarning",
    "BasemapAdjustment",
    "BasemapProblem",
]
