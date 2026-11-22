"""Provider styles (implemented in the Rust core, shared with R)."""

import json
import warnings

import pytest

from basemapper import (
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
from basemapper.exceptions import ValidationError


def test_raster_provider_valid_json():
    url = "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
    style = json.loads(raster_provider(url))
    assert style["version"] == 8
    assert style["sources"]["raster-source"]["tiles"] == [url]
    assert style["layers"][0]["type"] == "raster"


def test_esri_vector_provider_constructs_url():
    base = "https://example.com/VectorTileServer"
    assert esri_vector_provider(base) == f"{base}/resources/styles/root.json"


def test_esri_vector_provider_strips_trailing_slash():
    result = esri_vector_provider("https://example.com/VectorTileServer/")
    assert result == "https://example.com/VectorTileServer/resources/styles/root.json"


def test_vector_provider_with_paint():
    paint = {"fill-color": "#e8e0d8", "line-color": "#aaa", "line-width": 1}
    style = json.loads(vector_provider("https://example.com/{z}/{x}/{y}.mvt", "land", paint=paint))
    fill_layer = next(l for l in style["layers"] if l["type"] == "fill")
    line_layer = next(l for l in style["layers"] if l["type"] == "line")
    assert fill_layer["paint"]["fill-color"] == "#e8e0d8"
    assert line_layer["paint"]["line-color"] == "#aaa"
    assert fill_layer["source-layer"] == "land"
    assert line_layer["source-layer"] == "land"


def test_vector_provider_default_grey():
    style = json.loads(vector_provider("https://example.com/{z}/{x}/{y}.mvt", "land"))
    fill_layer = next(l for l in style["layers"] if l["type"] == "fill")
    assert "fill-color" in fill_layer["paint"]


def test_vector_provider_unknown_key_warns():
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        s = vector_provider("https://x.com/{z}/{x}/{y}.mvt", "land", paint={"circle-radius": 5})
        assert any("circle-radius" in str(x.message) for x in w)
    for layer in json.loads(s)["layers"]:
        assert "circle-radius" not in layer.get("paint", {})


def test_esri_raster_provider_valid_json():
    url = "https://example.com/MapServer/tile/{z}/{y}/{x}"
    style = json.loads(esri_raster_provider(url))
    assert style["version"] == 8
    assert style["sources"]["esri-raster-source"]["tileSize"] == 256
    assert style["sources"]["esri-raster-source"]["tiles"] == [url]


def test_basemap_provider_resolves_every_listed_provider():
    names = list_providers()
    assert len(names) >= 20
    assert {"OpenFreeMap.liberty", "Carto.Positron", "ESRI.WorldImagery"} <= set(names)
    for name in names:
        key = "KEY" if name.startswith("MapTiler.") else None
        assert isinstance(basemap_provider(name, api_key=key), str)
    assert "positron-gl-style" in basemap_provider("Carto.Positron")
    with pytest.raises(ValidationError, match="api_key"):
        basemap_provider("MapTiler.Streets")
    with pytest.raises(ValidationError, match="list_providers"):
        basemap_provider("Nope")


def test_ogc_and_pmtiles_providers():
    tile = next(iter(json.loads(wms_provider("https://x.com/wms", layers="a b"))["sources"].values()))["tiles"][0]
    assert "LAYERS=a%20b" in tile and "BBOX={bbox-epsg-3857}" in tile
    tile = next(iter(json.loads(wmts_provider("https://x.com/wmts", "lyr"))["sources"].values()))["tiles"][0]
    assert "LAYER=lyr" in tile
    src = next(iter(json.loads(pmtiles_provider("https://x.com/a.pmtiles"))["sources"].values()))
    assert src["tiles"] == ["pmtiles://https://x.com/a.pmtiles"]
