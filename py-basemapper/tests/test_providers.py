"""Styles (implemented in the Rust core, shared with R)."""

import json
import warnings

import pytest

from basemapper import basemap_provider, list_layers, list_providers, tile_style, wms_style, wmts_style
from basemapper.exceptions import ValidationError


def test_tile_style_raster_template():
    url = "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
    style = json.loads(tile_style(url, maxzoom=19, attribution="© OSM"))
    src = style["sources"]["tiles"]
    assert style["version"] == 8
    assert src == {"type": "raster", "tiles": [url], "tileSize": 256, "maxzoom": 19, "attribution": "© OSM"}
    assert style["layers"] == [{"id": "raster", "type": "raster", "source": "tiles"}]


def test_tile_style_tms_and_tile_size():
    src = json.loads(tile_style("https://t/{z}/{x}/{y}.jpg", scheme="tms", tile_size=512))["sources"]["tiles"]
    assert src["scheme"] == "tms" and src["tileSize"] == 512


def test_tile_style_arcgis_mapserver():
    base = "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer"
    src = json.loads(tile_style(base + "/"))["sources"]["tiles"]
    assert src["tiles"] == [base + "/tile/{z}/{y}/{x}"]
    with pytest.raises(ValidationError, match="raster"):
        tile_style(base, type="vector")


def test_tile_style_vector_default_style_and_paint():
    style = json.loads(tile_style(
        "https://x.com/{z}/{x}/{y}.mvt", source_layers=["land", "roads"],
        paint={"fill-color": "#e8e0d8", "line-width": 2},
    ))
    ids = [l["id"] for l in style["layers"]]
    assert ids == ["land-fill", "roads-fill", "land-line", "roads-line", "land-circle", "roads-circle"]
    assert style["layers"][0]["paint"]["fill-color"] == "#e8e0d8"
    assert style["layers"][2]["paint"]["line-width"] == 2
    assert style["layers"][0]["source-layer"] == "land"
    assert "background" not in [l["type"] for l in style["layers"]]  # transparent, stacks


def test_tile_style_unknown_paint_key_warns():
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        tile_style("https://x.com/{z}/{x}/{y}.pbf", source_layers=["land"], paint={"text-color": "#000"})
        assert any("text-color" in str(x.message) for x in w)


def test_tile_style_type_is_validated():
    with pytest.raises(ValidationError, match="raster"):
        tile_style("https://x.com/{z}/{x}/{y}.png", type="imagery")


def test_local_style_file_gets_absolute_urls(tmp_path):
    style = {
        "version": 8,
        "sources": {"s": {"type": "raster", "tiles": ["tiles/{z}/{x}/{y}.png"]}},
        "layers": [{"id": "r", "type": "raster", "source": "s"}],
    }
    path = tmp_path / "style.json"
    path.write_text(json.dumps(style))
    tile = json.loads(tile_style(str(path)))["sources"]["s"]["tiles"][0]
    assert tile.startswith("file://") and tile.endswith("/tiles/{z}/{x}/{y}.png")
    # The same file works anywhere a style does.
    assert list_layers(str(path)) == [{"id": "r", "type": "raster"}]


def test_missing_style_is_a_clear_error():
    with pytest.raises(ValidationError, match="not a style"):
        tile_style("no/such/style.json")


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


def test_ogc_styles():
    tile = next(iter(json.loads(wms_style("https://x.com/wms", layers="a b"))["sources"].values()))["tiles"][0]
    assert "LAYERS=a%20b" in tile and "BBOX={bbox-epsg-3857}" in tile and "&CRS=EPSG:3857" in tile
    tile = next(iter(json.loads(wms_style("https://x.com/wms", "a", version="1.1.1"))["sources"].values()))["tiles"][0]
    assert "&SRS=EPSG:3857" in tile
    tile = next(iter(json.loads(wmts_style("https://x.com/wmts", "lyr"))["sources"].values()))["tiles"][0]
    assert "LAYER=lyr" in tile
