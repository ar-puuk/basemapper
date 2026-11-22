"""list_layers (implemented in the Rust core, shared with R)."""

import functools
import http.server
import json
import threading

import pytest

from basemapper import list_layers
from basemapper.exceptions import BasemapError

STYLE = json.dumps({
    "version": 8,
    "sources": {"s": {"type": "raster", "tiles": ["http://t/{z}/{x}/{y}.png"]}},
    "layers": [
        {"id": "water",  "type": "raster", "source": "s"},
        {"id": "roads",  "type": "raster", "source": "s"},
        {"id": "labels", "type": "background"},
    ],
})


def test_list_layers_from_inline_json():
    assert list_layers(STYLE) == [
        {"id": "water", "type": "raster", "source_layer": None, "feature": "other"},
        {"id": "roads", "type": "raster", "source_layer": None, "feature": "other"},
        {"id": "labels", "type": "background", "source_layer": None, "feature": "background"},
    ]


class _Style(http.server.BaseHTTPRequestHandler):
    def do_GET(self):  # noqa: N802
        body = STYLE.encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def test_list_layers_from_url():
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _Style)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        layers = list_layers(f"http://127.0.0.1:{server.server_address[1]}/style.json")
    finally:
        server.shutdown()
    assert [l["id"] for l in layers] == ["water", "roads", "labels"]


def test_list_layers_bad_style_raises():
    with pytest.raises(BasemapError):
        list_layers("{not json")


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


def test_list_layers_previews_filters():
    ids = lambda **kw: [r["id"] for r in list_layers(FILTER_STYLE, **kw)]  # noqa: E731
    assert list_layers(FILTER_STYLE)[1]["source_layer"] == "water"
    assert ids(layers=["road_*"]) == ["road_major", "road_name"]
    assert ids(layers=["-road_?ame"]) == ["bg", "water", "road_major"]
    assert ids(labels="only") == ["road_name"]
    assert ids(features=["-roads"]) == ["bg", "water"]
    assert ids(labels="only", features=["water"]) == []


def test_unmatched_filter_names_warn():
    import warnings

    from basemapper import basemap

    bg = json.dumps({"version": 8, "sources": {}, "layers": [
        {"id": "bg", "type": "background", "paint": {"background-color": "#336699"}}]})
    with pytest.warns(UserWarning, match='features: "water" matches no layer'):
        basemap(bg, 8, 8, features=["-water"]).render_array()
    with pytest.warns(UserWarning, match='layers: "-label_\\*" matches no layer'):
        basemap(bg, 8, 8, layers=["-label_*"]).render_array()
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        basemap(bg, 8, 8, features=["-water", "background"][1:]).render_array()
