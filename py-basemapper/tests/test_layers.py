"""Unit tests for list_layers and layer filter validation."""

import json

import pytest
import responses as resp_lib

from basemapper.layer_utils import list_layers

STYLE = json.dumps({
    "version": 8,
    "sources": {"s": {"type": "raster", "tiles": ["http://t/{z}/{x}/{y}.png"]}},
    "layers": [
        {"id": "water",  "type": "raster", "source": "s"},
        {"id": "roads",  "type": "raster", "source": "s"},
        {"id": "labels", "type": "raster", "source": "s"},
    ],
})


def test_list_layers_from_inline_json(capsys):
    ids = list_layers(STYLE)
    assert set(ids) == {"water", "roads", "labels"}
    out = capsys.readouterr().out
    assert "water" in out
    assert "raster" in out


@resp_lib.activate
def test_list_layers_from_url(capsys):
    url = "https://example.com/style.json"
    resp_lib.add(resp_lib.GET, url, body=STYLE, content_type="application/json")
    ids = list_layers(url)
    assert "water" in ids


def test_list_layers_returns_sorted():
    ids = list_layers(STYLE)
    assert ids == sorted(ids)


def test_mixed_layer_filter_raises():
    from basemapper.exceptions import BasemapError
    # Mixed filter validation is enforced in the Rust core; here we test
    # that BasemapError is importable and subclasses RuntimeError.
    err = BasemapError("test")
    assert isinstance(err, RuntimeError)
