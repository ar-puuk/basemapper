"""Render integration tests matching quickstart.md scenarios 2, 4, 7, 8."""

import json

import pytest

from basemapper.exceptions import BasemapError, StyleError, ValidationError


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


def test_validation_error_is_value_error():
    """ValidationError must subclass both BasemapError and ValueError."""
    err = ValidationError("bad bbox")
    assert isinstance(err, BasemapError)
    assert isinstance(err, ValueError)


def test_malformed_style_raises_style_error():
    """Scenario 7: version != 8 must raise StyleError (not a generic Exception)."""
    import basemapper
    with pytest.raises(StyleError, match="version"):
        basemapper.render_basemap_raw(
            bbox=BBOX,
            crs=3857,
            width=64,
            height=64,
            style_input='{"version": 7, "sources": {}, "layers": []}',
        )


def test_invalid_bbox_raises_validation_error():
    """xmin >= xmax must raise ValidationError before any network I/O."""
    import basemapper
    bad_bbox = [100.0, 4_500_000.0, -100.0, 4_600_000.0]  # xmin > xmax
    with pytest.raises(ValidationError):
        basemapper.render_basemap_raw(
            bbox=bad_bbox,
            crs=3857,
            width=64,
            height=64,
            style_input=GREYSCALE_STYLE,
        )


def test_detect_crs_raises_validation_error_on_bare_axes():
    """A plain matplotlib Axes with no geographic CRS must raise ValidationError."""
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from basemapper.bbox_utils import detect_crs_from_axes

    fig, ax = plt.subplots()
    with pytest.raises(ValidationError, match="CRS"):
        detect_crs_from_axes(ax)
    plt.close(fig)


def test_import_without_plotnine(monkeypatch):
    """base import must not fail when plotnine is absent."""
    import sys

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
