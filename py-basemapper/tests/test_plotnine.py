"""Tests for the plotnine geom_basemap integration (US7)."""

import sys

import pytest


def test_import_basemapper_without_plotnine():
    """Scenario T057: base package importable when plotnine is absent."""
    saved = sys.modules.pop("plotnine", None)
    try:
        import importlib
        import basemapper
        importlib.reload(basemapper)
        assert hasattr(basemapper, "add_basemap")
        assert hasattr(basemapper, "render_basemap_raw")
    finally:
        if saved is not None:
            sys.modules["plotnine"] = saved


def test_geom_basemap_importable_when_plotnine_available():
    pytest.importorskip("plotnine")
    from basemapper import geom_basemap
    assert geom_basemap is not None


def test_geom_basemap_has_correct_attributes():
    pytest.importorskip("plotnine")
    from basemapper import geom_basemap
    g = geom_basemap(style_url="https://example.com/style.json", alpha=0.5)
    assert g.alpha == 0.5
    assert g.style_url == "https://example.com/style.json"


def test_geom_basemap_layers_parameter():
    pytest.importorskip("plotnine")
    from basemapper import geom_basemap
    g = geom_basemap(style_url="https://example.com/style.json", layers=["water"])
    assert g.layers == ["water"]
