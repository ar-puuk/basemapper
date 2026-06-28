"""Provider Generator Helpers — convert bare tile URLs to MapLibre GL Style JSON."""

from __future__ import annotations

import json
import warnings
from typing import Dict, Optional

_FILL_KEYS = {"fill-color", "fill-opacity", "fill-outline-color"}
_LINE_KEYS = {"line-color", "line-width", "line-opacity"}
_KNOWN_KEYS = _FILL_KEYS | _LINE_KEYS


class RasterProvider:
    """Generate a MapLibre GL Style JSON string for an XYZ raster tile source.

    Cast to ``str`` to obtain the inline MapLibre GL Style JSON.

    Args:
        url_template: Slippy-map tile URL with ``{z}``, ``{x}``, ``{y}`` placeholders.
        tile_size: Tile size in pixels (default 256).
    """

    def __init__(self, url_template: str, tile_size: int = 256) -> None:
        self.url_template = url_template
        self.tile_size = tile_size

    def to_style_json(self) -> str:
        """Return a MapLibre GL Style JSON string."""
        style = {
            "version": 8,
            "sources": {
                "raster-source": {
                    "type": "raster",
                    "tiles": [self.url_template],
                    "tileSize": self.tile_size,
                }
            },
            "layers": [
                {
                    "id": "raster-layer",
                    "type": "raster",
                    "source": "raster-source",
                }
            ],
        }
        return json.dumps(style)

    def __str__(self) -> str:
        return self.to_style_json()


class VectorProvider:
    """Generate a MapLibre GL Style JSON string for an MVT vector tile source.

    Accepted paint keys: ``fill-color``, ``fill-opacity``, ``fill-outline-color``,
    ``line-color``, ``line-width``, ``line-opacity``. Unrecognised keys are dropped
    with a warning. Omitting ``paint`` applies a light-grey default style.

    Args:
        url_template: MVT tile URL with ``{z}``, ``{x}``, ``{y}`` placeholders.
        source_layer: The name of the layer within each MVT tile to render.
            This is tile-server specific (e.g. ``"water"``, ``"roads"``); it
            maps to the MapLibre GL ``"source-layer"`` property and is required
            for vector sources.
        paint: Optional dict of MapLibre GL paint properties.
    """

    def __init__(self, url_template: str, source_layer: str, paint: Optional[Dict[str, object]] = None) -> None:
        self.url_template = url_template
        self.source_layer = source_layer
        self.paint = paint or {}

    def to_style_json(self) -> str:
        paint = dict(self.paint)
        unknown = set(paint.keys()) - _KNOWN_KEYS
        for key in unknown:
            warnings.warn(
                f"VectorProvider: unrecognised paint key '{key}' dropped.",
                stacklevel=2,
            )
            del paint[key]

        fill_paint: Dict[str, object] = {
            k: paint[k] for k in _FILL_KEYS if k in paint
        }
        line_paint: Dict[str, object] = {
            k: paint[k] for k in _LINE_KEYS if k in paint
        }

        if "fill-color" not in fill_paint:
            fill_paint["fill-color"] = "#e8e0d8"
        if "fill-opacity" not in fill_paint:
            fill_paint["fill-opacity"] = 1
        if "line-color" not in line_paint:
            line_paint["line-color"] = "#aaaaaa"
        if "line-width" not in line_paint:
            line_paint["line-width"] = 1

        style = {
            "version": 8,
            "sources": {
                "vector-source": {
                    "type": "vector",
                    "tiles": [self.url_template],
                }
            },
            "layers": [
                {
                    "id": "vector-fill",
                    "type": "fill",
                    "source": "vector-source",
                    "source-layer": self.source_layer,
                    "paint": fill_paint,
                },
                {
                    "id": "vector-line",
                    "type": "line",
                    "source": "vector-source",
                    "source-layer": self.source_layer,
                    "paint": line_paint,
                },
            ],
        }
        return json.dumps(style)

    def __str__(self) -> str:
        return self.to_style_json()


class EsriVectorProvider:
    """Return the root.json style endpoint URL for an ArcGIS VectorTileServer.

    The Rust core fetches this URL to obtain the full, ESRI-published MapLibre
    GL style JSON. Cast to ``str`` to get the URL string (not inline JSON).

    Args:
        base_url: Base ArcGIS VectorTileServer URL (trailing slashes stripped).
    """

    def __init__(self, base_url: str) -> None:
        self.base_url = base_url.rstrip("/")

    def __str__(self) -> str:
        return f"{self.base_url}/resources/styles/root.json"


class EsriRasterProvider:
    """Generate a MapLibre GL Style JSON string for an ArcGIS MapServer raster source.

    Args:
        url_template: ArcGIS MapServer tile URL (e.g., ``.../MapServer/tile/{z}/{y}/{x}``).
        tile_size: Tile size in pixels (default 256).
    """

    def __init__(self, url_template: str, tile_size: int = 256) -> None:
        self.url_template = url_template
        self.tile_size = tile_size

    def to_style_json(self) -> str:
        style = {
            "version": 8,
            "sources": {
                "esri-raster-source": {
                    "type": "raster",
                    "tiles": [self.url_template],
                    "tileSize": self.tile_size,
                }
            },
            "layers": [
                {
                    "id": "esri-raster-layer",
                    "type": "raster",
                    "source": "esri-raster-source",
                }
            ],
        }
        return json.dumps(style)

    def __str__(self) -> str:
        return self.to_style_json()


def basemap_provider(name: str, api_key: str | None = None) -> str:
    """Return a style URL or inline MapLibre GL JSON for a named basemap provider.

    Args:
        name: Provider name, e.g. ``"OpenFreeMap.liberty"`` or
            ``"Carto.DarkMatter"``.  See the list below for all available names.
        api_key: API key required by some providers (e.g. MapTiler).
            Ignored for providers that do not use a key.  Stadia providers
            work without a key at low volume; supply one to lift rate limits.

    Returns:
        str: A MapLibre GL style URL or inline JSON string, ready to pass to
            :func:`render_basemap_raw`.

    Available provider names:

    * **Vector GL styles** (no key): ``"OpenFreeMap.liberty"``,
      ``"OpenFreeMap.bright"``, ``"OpenFreeMap.positron"``,
      ``"Carto.Positron"``, ``"Carto.DarkMatter"``, ``"Carto.Voyager"``,
      ``"ESRI.WorldBasemap"``
    * **MapTiler** (require ``api_key``): ``"MapTiler.Streets"``,
      ``"MapTiler.Satellite"``, ``"MapTiler.Topo"``, ``"MapTiler.Outdoor"``
    * **ESRI raster** (no key): ``"ESRI.WorldImagery"``,
      ``"ESRI.WorldStreetMap"``, ``"ESRI.WorldTopoMap"``,
      ``"ESRI.WorldShadedRelief"``, ``"ESRI.WorldGrayCanvas"``,
      ``"ESRI.NatGeoWorldMap"``
    * **OSM raster** (no key): ``"OSM.Standard"``, ``"OSM.CyclOSM"``
    * **Stadia/Stamen** (key optional): ``"Stadia.AlidadeSmooth"``,
      ``"Stadia.AlidadeSmoothDark"``, ``"Stadia.OSMBright"``,
      ``"Stadia.StamenToner"``, ``"Stadia.StamenTerrain"``,
      ``"Stadia.StamenWatercolor"``

    Raises:
        ValueError: For unknown provider names or when a required ``api_key``
            is missing.
    """
    # --- Vector GL styles (return style URL directly) ---
    _PLAIN_URLS = {
        "OpenFreeMap.liberty":   "https://tiles.openfreemap.org/styles/liberty",
        "OpenFreeMap.bright":    "https://tiles.openfreemap.org/styles/bright",
        "OpenFreeMap.positron":  "https://tiles.openfreemap.org/styles/positron",
        "Carto.Positron":        "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json",
        "Carto.DarkMatter":      "https://basemaps.cartocdn.com/gl/dark-matter-gl-style/style.json",
        "Carto.Voyager":         "https://basemaps.cartocdn.com/gl/voyager-gl-style/style.json",
    }
    if name in _PLAIN_URLS:
        return _PLAIN_URLS[name]

    # --- MapTiler GL styles (require api_key; embed key in URL) ---
    _MAPTILER_BASE = {
        "MapTiler.Streets":   "https://api.maptiler.com/maps/streets-v2/style.json?key=",
        "MapTiler.Satellite": "https://api.maptiler.com/maps/satellite/style.json?key=",
        "MapTiler.Topo":      "https://api.maptiler.com/maps/topo-v2/style.json?key=",
        "MapTiler.Outdoor":   "https://api.maptiler.com/maps/outdoor-v2/style.json?key=",
    }
    if name in _MAPTILER_BASE:
        if api_key is None:
            raise ValueError(f"Provider '{name}' requires an api_key argument")
        return _MAPTILER_BASE[name] + api_key

    # --- ESRI raster tile services ---
    _ESRI_RASTER = {
        "ESRI.WorldImagery":      "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
        "ESRI.WorldStreetMap":    "https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{z}/{y}/{x}",
        "ESRI.WorldTopoMap":      "https://server.arcgisonline.com/ArcGIS/rest/services/World_Topo_Map/MapServer/tile/{z}/{y}/{x}",
        "ESRI.WorldShadedRelief": "https://server.arcgisonline.com/ArcGIS/rest/services/World_Shaded_Relief/MapServer/tile/{z}/{y}/{x}",
        "ESRI.WorldGrayCanvas":   "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Base/MapServer/tile/{z}/{y}/{x}",
        "ESRI.NatGeoWorldMap":    "https://server.arcgisonline.com/ArcGIS/rest/services/NatGeo_World_Map/MapServer/tile/{z}/{y}/{x}",
    }
    if name in _ESRI_RASTER:
        return str(EsriRasterProvider(_ESRI_RASTER[name]))

    # --- ESRI vector tile services ---
    if name == "ESRI.WorldBasemap":
        return str(EsriVectorProvider("https://basemaps.arcgis.com/arcgis/rest/services/World_Basemap_v2/VectorTileServer"))

    # --- OSM raster ---
    _OSM = {
        "OSM.Standard": "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
        "OSM.CyclOSM":  "https://tile-cyclosm.openstreetmap.fr/cyclosm/{z}/{x}/{y}.png",
    }
    if name in _OSM:
        return str(RasterProvider(_OSM[name]))

    # --- Stadia (Stamen successors; key optional for low-volume use) ---
    _STADIA = {
        "Stadia.AlidadeSmooth":     "alidade_smooth",
        "Stadia.AlidadeSmoothDark": "alidade_smooth_dark",
        "Stadia.OSMBright":         "osm_bright",
        "Stadia.StamenToner":       "stamen_toner",
        "Stadia.StamenTerrain":     "stamen_terrain",
        "Stadia.StamenWatercolor":  "stamen_watercolor",
    }
    if name in _STADIA:
        tile_name = _STADIA[name]
        url = f"https://tiles.stadiamaps.com/tiles/{tile_name}/{{z}}/{{x}}/{{y}}.png"
        if api_key is not None:
            url = f"{url}?api_key={api_key}"
        return str(RasterProvider(url))

    raise ValueError(
        f"Unknown provider: '{name}'. See help(basemap_provider) for available names."
    )
