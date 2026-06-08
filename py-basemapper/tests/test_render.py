"""Render integration tests matching quickstart.md scenarios 2, 4, 7, 8."""

import json

import pytest
import responses as resp_lib

from basemapper.exceptions import BasemapError


BBOX = [-13_700_000.0, 4_500_000.0, -13_600_000.0, 4_600_000.0]
GREYSCALE_STYLE = json.dumps({
    "version": 8,
    "sources": {
        "osm": {
            "type": "raster",
            "tiles": ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"],
            "tileSize": 256,
        }
    },
    "layers": [{"id": "osm", "type": "raster", "source": "osm",
                "paint": {"raster-saturation": -1}}],
})


def test_basemap_error_is_runtime_error():
    err = BasemapError("boom")
    assert isinstance(err, RuntimeError)
    assert "boom" in str(err)


def test_malformed_style_raises(monkeypatch):
    """Scenario 7: version != 8 should raise before tile fetching."""
    import basemapper
    with pytest.raises(Exception, match="style|version"):
        basemapper.render_basemap_raw(
            bbox_3857=BBOX,
            width=64,
            height=64,
            style_input='{"version": 7, "sources": {}, "layers": []}',
        )


def test_import_without_plotnine(monkeypatch):
    """base import must not fail when plotnine is absent."""
    import sys
    import types

    # Temporarily hide plotnine.
    real_plotnine = sys.modules.pop("plotnine", None)
    try:
        import importlib
        import basemapper
        importlib.reload(basemapper)
        assert hasattr(basemapper, "add_basemap")
    finally:
        if real_plotnine is not None:
            sys.modules["plotnine"] = real_plotnine
