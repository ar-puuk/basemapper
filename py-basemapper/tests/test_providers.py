"""Styles (implemented in the Rust core, shared with R)."""

import json
import warnings

import pytest

from basemapper import (
    basemap_provider, list_layers, list_providers, providers, tile_cache, tile_style, wms_style, wmts_style,
)
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
    assert list_layers(str(path)) == [{"id": "r", "type": "raster", "source_layer": None, "feature": "other"}]


def test_missing_style_is_a_clear_error():
    with pytest.raises(ValidationError, match="not a style"):
        tile_style("no/such/style.json")


def test_list_providers_is_a_table_of_forms():
    rows = list_providers()
    assert len(rows) >= 40
    assert set(rows[0]) == {"name", "vector", "raster", "api_key_env", "attribution"}
    by_name = {r["name"]: r for r in rows}
    assert by_name["Esri.WorldGrayCanvas"]["vector"] and by_name["Esri.WorldGrayCanvas"]["raster"]
    assert not by_name["OpenFreeMap.Liberty"]["raster"]
    assert by_name["MapTiler.Streets"]["api_key_env"] == "MAPTILER_API_KEY"
    assert by_name["CartoDB.Positron"]["api_key_env"] == "CARTO_API_KEY"
    assert by_name["OpenFreeMap.Liberty"]["api_key_env"] is None
    assert all(r["vector"] for r in list_providers("vector"))
    assert all(r["raster"] for r in list_providers("raster"))


def test_basemap_provider_resolves_every_form():
    for r in list_providers():
        for kind in ("vector", "raster"):
            if r[kind]:
                style = basemap_provider(r["name"], type=kind, api_key="KEY")
                # A basemap of stacked services (UGRC.Lite) gives its styles as a list.
                if r["name"] in ("UGRC.Lite", "UGRC.Dark", "UGRC.Hybrid", "UGRC.Outdoors"):
                    assert isinstance(style, list) and len(style) >= 2
                else:
                    assert isinstance(style, str)


def test_stacked_basemaps_expand_and_filter_as_one():
    lite = basemap_provider("UGRC.Lite")
    assert [s.split("/services/")[1].split("/")[0] for s in lite] == ["VectorHillshade", "LiteBase", "LiteLabels"]
    assert "UGRC.Hybrid" in [r["name"] for r in list_providers("vector")]
    with pytest.raises(ValidationError, match="only as vector"):
        basemap_provider("UGRC.Lite", type="raster")


def test_basemap_provider_forms_and_errors(monkeypatch):
    assert basemap_provider("OpenFreeMap.Positron").endswith("/styles/positron")
    assert basemap_provider("openfreemap.positron") == basemap_provider("OpenFreeMap.Positron")
    assert basemap_provider("Carto.Positron", api_key="K").endswith("positron-gl-style/style.json?key=K")
    raster = json.loads(basemap_provider("CartoDB.Positron", type="raster", api_key="K"))
    assert len(raster["sources"]["raster"]["tiles"]) == 4
    assert raster["sources"]["raster"]["attribution"].endswith("© CARTO")
    with pytest.raises(ValidationError, match='only as raster tiles'):
        basemap_provider("Esri.WorldImagery", type="vector")
    with pytest.raises(ValidationError, match="did you mean CartoDB.Positron"):
        basemap_provider("CartoDB.Positon")
    monkeypatch.delenv("MAPTILER_API_KEY", raising=False)
    with pytest.raises(ValidationError, match="MAPTILER_API_KEY"):
        basemap_provider("MapTiler.Streets")
    monkeypatch.setenv("MAPTILER_API_KEY", "ENVKEY")
    assert basemap_provider("MapTiler.Streets").endswith("key=ENVKEY")
    tiles = json.loads(basemap_provider("Esri.WorldGrayCanvas", type="raster", tiles="https://x/{z}/{x}/{y}.png"))
    assert tiles["sources"]["raster"]["tiles"] == ["https://x/{z}/{x}/{y}.png"]


def test_providers_object_and_names_as_style():
    assert providers.CartoDB.Positron == "CartoDB.Positron"
    assert providers.OpenTopoMap == "OpenTopoMap"
    assert "Positron" in dir(providers.CartoDB)
    assert list_layers(providers.Esri.WorldImagery)[0]["id"] == "raster"


def test_tile_cache(tmp_path):
    before = tile_cache()
    try:
        assert tile_cache(tmp_path) == str(tmp_path)
        assert tile_cache(False) is None
        assert tile_cache(tmp_path, clear=True) == str(tmp_path)
    finally:
        tile_cache(before or False)


def test_ogc_styles():
    tile = next(iter(json.loads(wms_style("https://x.com/wms", layers="a b"))["sources"].values()))["tiles"][0]
    assert "LAYERS=a%20b" in tile and "BBOX={bbox-epsg-3857}" in tile and "&CRS=EPSG:3857" in tile
    tile = next(iter(json.loads(wms_style("https://x.com/wms", "a", version="1.1.1"))["sources"].values()))["tiles"][0]
    assert "&SRS=EPSG:3857" in tile
    tile = next(iter(json.loads(wmts_style("https://x.com/wmts", "lyr"))["sources"].values()))["tiles"][0]
    assert "LAYER=lyr" in tile


def test_list_layers_reports_features():
    rows = list_layers("Esri.WorldImagery")
    assert rows == [{"id": "raster", "type": "raster", "source_layer": None, "feature": "other"}]


def test_basemap_provider_labels_pick_raster_tiles():
    def tiles(name, labels):
        style = json.loads(basemap_provider(name, type="raster", api_key="K", labels=labels))
        return style["sources"]["raster"]["tiles"][0]

    assert "/light_nolabels/" in tiles("CartoDB.Positron", False)
    assert "/light_only_labels/" in tiles("CartoDB.Positron", "only")
    assert "World_Boundaries_and_Places" in tiles("Esri.WorldImagery", "only")
    with pytest.raises(ValidationError, match="labels drawn in"):
        tiles("OpenStreetMap.Mapnik", False)
    with pytest.raises(ValueError, match="labels must be"):
        basemap_provider("Esri.WorldImagery", labels="some")


def test_labels_and_features_filter_a_style():
    from basemapper import BasemapError, basemap

    style = json.dumps({"version": 8, "sources": {}, "layers": [{"id": "bg", "type": "background",
                        "paint": {"background-color": "#ff0000"}}]})
    a = basemap(style, 8, 8, labels=False).render_array()
    assert tuple(a[0, 0]) == (255, 0, 0, 255)
    with pytest.raises(BasemapError, match="its features are background"):
        basemap(style, 8, 8, labels="only").render_array()
    with pytest.raises(BasemapError, match="unknown feature"):
        basemap(style, 8, 8, features=["highways"]).render_array()
    b = basemap(style, 8, 8, features=["background"]).render_array()
    assert tuple(b[0, 0]) == (255, 0, 0, 255)


def test_get_attribution():
    from basemapper import basemap, get_attribution

    assert get_attribution("Esri.WorldImagery").startswith("Tiles © Esri")
    style = json.dumps({"version": 8, "sources": {"a": {"type": "raster", "tiles": ["https://t/{z}/{x}/{y}.png"],
                        "attribution": "<a href='x'>&copy; A</a>"}}, "layers": [{"id": "a", "type": "raster", "source": "a"}]})
    assert get_attribution([style, style]) == "© A"
    assert basemap([style, "Esri.WorldImagery"], 8, 8).get_attribution().startswith("© A | Tiles © Esri")
    empty = json.dumps({"version": 8, "sources": {}, "layers": []})
    assert get_attribution(empty) == ""


def test_the_package_reports_its_version():
    import importlib.metadata

    import basemapper

    assert basemapper.__version__ == importlib.metadata.version("basemapper")
    assert basemapper.__version__.count(".") >= 2
