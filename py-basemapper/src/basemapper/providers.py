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
        paint: Optional dict of MapLibre GL paint properties.
    """

    def __init__(self, url_template: str, paint: Optional[Dict[str, object]] = None) -> None:
        self.url_template = url_template
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
                    "paint": fill_paint,
                },
                {
                    "id": "vector-line",
                    "type": "line",
                    "source": "vector-source",
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
