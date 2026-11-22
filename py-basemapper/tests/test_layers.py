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
        {"id": "water", "type": "raster"},
        {"id": "roads", "type": "raster"},
        {"id": "labels", "type": "background"},
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
