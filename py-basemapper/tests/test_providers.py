"""Unit tests for Provider Generator Helpers (US4)."""

import json
import warnings

import pytest

from basemapper.providers import (
    EsriRasterProvider,
    EsriVectorProvider,
    RasterProvider,
    VectorProvider,
)


def test_raster_provider_valid_json():
    url = "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
    style = json.loads(str(RasterProvider(url)))
    assert style["version"] == 8
    assert style["sources"]["raster-source"]["tiles"] == [url]
    assert style["layers"][0]["type"] == "raster"


def test_esri_vector_provider_constructs_url():
    base = "https://example.com/VectorTileServer"
    assert str(EsriVectorProvider(base)) == f"{base}/resources/styles/root.json"


def test_esri_vector_provider_strips_trailing_slash():
    base = "https://example.com/VectorTileServer/"
    result = str(EsriVectorProvider(base))
    assert result == "https://example.com/VectorTileServer/resources/styles/root.json"
    assert "//" not in result.split("://")[1]


def test_vector_provider_with_paint():
    paint = {"fill-color": "#e8e0d8", "line-color": "#aaa", "line-width": 1}
    style = json.loads(str(VectorProvider("https://example.com/{z}/{x}/{y}.mvt", paint=paint)))
    fill_layer = next(l for l in style["layers"] if l["type"] == "fill")
    line_layer = next(l for l in style["layers"] if l["type"] == "line")
    assert fill_layer["paint"]["fill-color"] == "#e8e0d8"
    assert line_layer["paint"]["line-color"] == "#aaa"


def test_vector_provider_default_grey():
    style = json.loads(str(VectorProvider("https://example.com/{z}/{x}/{y}.mvt")))
    fill_layer = next(l for l in style["layers"] if l["type"] == "fill")
    assert "fill-color" in fill_layer["paint"]


def test_vector_provider_unknown_key_warns():
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        style_str = str(VectorProvider("https://x.com/{z}/{x}/{y}.mvt", paint={"circle-radius": 5}))
        assert any("circle-radius" in str(warning.message) for warning in w)
    style = json.loads(style_str)
    for layer in style["layers"]:
        assert "circle-radius" not in layer.get("paint", {})


def test_esri_raster_provider_valid_json():
    url = "https://example.com/MapServer/tile/{z}/{y}/{x}"
    style = json.loads(str(EsriRasterProvider(url)))
    assert style["version"] == 8
    assert style["sources"]["esri-raster-source"]["tileSize"] == 256
    assert style["sources"]["esri-raster-source"]["tiles"] == [url]
