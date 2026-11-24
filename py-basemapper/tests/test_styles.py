"""Styles and their inspection: list_layers, get_attribution, the catalogue,
the tile cache (implemented in the Rust core, shared with R). Offline."""

from __future__ import annotations

import http.server
import json
import threading

import pytest

import basemapper
from basemapper import BasemapError, ValidationError, get_attribution, list_layers, style

FILTER_STYLE = json.dumps({
    "version": 8,
    "sources": {"s": {"type": "vector", "tiles": ["https://t/{z}/{x}/{y}.pbf"]}},
    "layers": [
        {"id": "bg", "type": "background", "paint": {"background-color": "#336699"}},
        {"id": "water", "type": "fill", "source": "s", "source-layer": "water"},
        {"id": "road_major", "type": "line", "source": "s", "source-layer": "transportation"},
        {"id": "road_name", "type": "symbol", "source": "s", "source-layer": "transportation_name"},
    ],
})


def ids(s):
    return [r["id"] for r in list_layers(s)]


def test_list_layers_and_previewing_a_styles_filters():
    rows = list_layers(FILTER_STYLE)
    assert rows[1] == {"id": "water", "type": "fill", "source_layer": "water", "feature": "water"}
    assert ids(style(FILTER_STYLE, layers="road_*")) == ["road_major", "road_name"]
    assert ids(style(FILTER_STYLE, layers="-road_?ame")) == ["bg", "water", "road_major"]
    assert ids(style(FILTER_STYLE, features="labels")) == ["road_name"]
    assert ids(style(FILTER_STYLE, features="-roads")) == ["bg", "water"]
    assert ids(style(FILTER_STYLE, features=["roads", "labels"])) == ["road_name"], "the road labels"
    assert ids(style(FILTER_STYLE, features=["water", "labels"])) == []


class _Style(http.server.BaseHTTPRequestHandler):
    def do_GET(self):  # noqa: N802
        body = FILTER_STYLE.encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def test_list_layers_from_a_url_and_errors():
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _Style)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        assert ids(f"http://127.0.0.1:{server.server_address[1]}/style.json")[0] == "bg"
    finally:
        server.shutdown()
    with pytest.raises(BasemapError):
        list_layers("{not json")


def test_a_local_style_file_works_anywhere_a_style_does(tmp_path):
    path = tmp_path / "style.json"
    path.write_text(json.dumps({"version": 8, "sources": {"s": {"type": "raster", "tiles": ["tiles/{z}/{x}/{y}.png"]}},
                                "layers": [{"id": "r", "type": "raster", "source": "s"}]}))
    assert list_layers(str(path)) == [{"id": "r", "type": "raster", "source_layer": None, "feature": "other"}]


def test_source_helpers_build_styles():
    # Vector tile data with a built default style and paint overrides.
    t = style(basemapper.tiles("https://x.com/{z}/{x}/{y}.mvt", source_layers=["land", "roads"],
                               paint={"fill-color": "#e8e0d8"}))
    assert ids(t) == ["land-fill", "roads-fill", "land-line", "roads-line", "land-circle", "roads-circle"]
    assert ids(basemapper.tiles("https://x.com/{z}/{x}/{y}.png")) == ["raster"]
    assert ids(basemapper.wms("https://x.com/wms", "a b")) == ["wms"] or len(ids(basemapper.wms("https://x.com/wms", "a"))) == 1
    assert len(ids(basemapper.wmts("https://x.com/wmts", "lyr"))) == 1
    # A catalogue basemap's raster form is built locally.
    assert ids(basemapper.provider("Esri.WorldImagery")) == ["raster"]
    with pytest.raises(ValidationError, match="raster"):
        style(basemapper.tiles("https://x.com/{z}/{x}/{y}.png", type="imagery"))


class _NotFound(http.server.BaseHTTPRequestHandler):
    def do_GET(self):  # noqa: N802
        self.send_response(404)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, *args):
        pass


def test_unmatched_paint_keys_are_problems():
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _NotFound)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        url = f"http://127.0.0.1:{server.server_address[1]}/{{z}}/{{x}}/{{y}}.pbf"
        t = style(basemapper.tiles(url, source_layers=["land"], paint={"text-color": "#000"}))
        with pytest.warns(basemapper.BasemapProblem, match="text-color"):
            basemapper.basemap(t, center=(0, 0), zoom=1, width=8, height=8).render()
    finally:
        server.shutdown()


def test_the_catalogue():
    rows = basemapper.list_providers()
    assert len(rows) >= 40 and set(rows[0]) == {"name", "vector", "raster", "api_key_env", "attribution"}
    by = {r["name"]: r for r in rows}
    assert by["Esri.WorldGrayCanvas"]["vector"] and by["Esri.WorldGrayCanvas"]["raster"]
    assert not by["OpenFreeMap.Liberty"]["raster"] and by["OpenFreeMap.Liberty"]["api_key_env"] is None
    assert by["MapTiler.Streets"]["api_key_env"] == "MAPTILER_API_KEY"
    assert all(r["vector"] for r in basemapper.list_providers("vector"))
    assert basemapper.providers.CartoDB.Positron == "CartoDB.Positron"
    assert "Positron" in dir(basemapper.providers.CartoDB)
    # A catalogue stack is one style of several services.
    assert len(list_layers("UGRC.Lite")) > len(list_layers(basemapper.provider("Esri.WorldImagery")))


def test_get_attribution_takes_styles_and_basemaps():
    credited = json.dumps({"version": 8, "sources": {"a": {"type": "raster", "tiles": ["https://t/{z}/{x}/{y}.png"],
                           "attribution": "<a href='x'>&copy; A</a>"}},
                           "layers": [{"id": "a", "type": "raster", "source": "a"}]})
    assert get_attribution("Esri.WorldImagery").startswith("Tiles © Esri")
    assert get_attribution([credited, credited]) == "© A"
    m = basemapper.basemap([credited, "Esri.WorldImagery"])
    assert get_attribution(m).startswith("© A | Tiles © Esri")
    assert get_attribution(json.dumps({"version": 8, "sources": {}, "layers": []})) == ""


def test_tile_cache(tmp_path):
    before = basemapper.tile_cache()
    try:
        assert basemapper.tile_cache(tmp_path) == str(tmp_path)
        assert basemapper.tile_cache(False) is None
        assert basemapper.tile_cache(tmp_path, clear=True) == str(tmp_path)
    finally:
        basemapper.tile_cache(before or False)
