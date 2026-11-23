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
            BBOX, 64, 64, '{"version": 7, "sources": {}, "layers": []}', crs=3857,
        )


def test_invalid_bbox_raises_validation_error():
    """xmin >= xmax must raise ValidationError before any network I/O."""
    import basemapper
    bad_bbox = [100.0, 4_500_000.0, -100.0, 4_600_000.0]  # xmin > xmax
    with pytest.raises(ValidationError):
        basemapper.render_basemap_raw(bad_bbox, 64, 64, GREYSCALE_STYLE, crs=3857)


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


# The functions shared with R (and the Rust core) use the same argument names;
# these tests pin them so the languages stay in step.
SHARED = {"style", "pixel_ratio", "layers", "auth_token", "fetch"}


def _params(fn):
    import inspect

    return list(inspect.signature(fn).parameters)


def test_basemap_has_the_shared_arguments():
    import basemapper

    p = _params(basemapper.basemap)
    assert p[:3] == ["style", "width", "height"]
    assert SHARED | {"center", "zoom", "bounds", "crs"} <= set(p)


def test_render_basemap_raw_has_the_shared_arguments():
    import basemapper

    p = _params(basemapper.render_basemap_raw)
    assert p[:4] == ["bounds", "width", "height", "style"]
    assert SHARED | {"crs", "tile_zoom"} <= set(p)


def test_plot_integrations_have_the_shared_arguments():
    import basemapper

    p = _params(basemapper.add_basemap)
    assert p[:2] == ["ax", "style"]
    assert SHARED | {"tile_zoom", "alpha"} <= set(p)
    pytest.importorskip("plotnine")
    p = _params(basemapper.geom_basemap.__init__)
    assert p[1] == "style"
    assert SHARED | {"tile_zoom", "alpha"} <= set(p)


def test_fetch_options_validate_and_map_to_the_core():
    from basemapper import FetchOptions, ValidationError

    assert FetchOptions()._core() == dict(
        tile_timeout_ms=10_000, max_tiles=256, tile_concurrency=16, fail_on_tile_error=False
    )
    assert FetchOptions(timeout=2.5, on_error="fail")._core()["tile_timeout_ms"] == 2500
    assert FetchOptions(on_error="fail")._core()["fail_on_tile_error"] is True
    for bad in (dict(timeout=0), dict(max_tiles=1.5), dict(concurrency=0), dict(on_error="ignore"),
                dict(max_tiles=True)):
        with pytest.raises(ValidationError):
            FetchOptions(**bad)
    with pytest.raises(Exception):
        FetchOptions().timeout = 3  # frozen


def test_replaced_fetch_arguments_still_work_with_a_deprecation_warning():
    import basemapper

    with pytest.warns(DeprecationWarning) as record:
        m = basemapper.basemap("x", tile_timeout_ms=2000, max_tiles=12)
    messages = [str(w.message) for w in record]
    assert len(messages) == 2
    assert "FetchOptions(timeout=...) (in seconds)" in messages[0]
    # The warning points at the caller's line, not basemapper's internals.
    assert record[0].filename == __file__
    assert m.fetch == basemapper.FetchOptions(timeout=2.0, max_tiles=12)
    with pytest.warns(DeprecationWarning):
        m = basemapper.Basemap("x", fail_on_tile_error=True, tile_concurrency=3)
    assert m.fetch.on_error == "fail" and m.fetch.concurrency == 3
    assert m._options()["tile_concurrency"] == 3


def test_unknown_keywords_and_bad_fetch_are_errors():
    import basemapper

    with pytest.raises(TypeError, match="zoo"):
        basemapper.basemap("x", zoo=3)
    with pytest.raises(basemapper.ValidationError):
        basemapper.basemap("x", fetch={"timeout": 1})
    with pytest.raises(TypeError):
        basemapper.render_basemap_raw([0, 0, 1, 1], 8, 8, "x", tile_zom=3)


def test_plot_integrations_load_lazily():
    import basemapper

    assert "add_basemap" in dir(basemapper) or callable(basemapper.add_basemap)
    with pytest.raises(AttributeError):
        basemapper.no_such_function  # noqa: B018


def test_view_methods_match_r():
    import basemapper

    for name in ("set_view", "set_bounds", "get_bounds", "get_center", "get_zoom", "get_size",
                 "render_png", "render_svg", "render_array"):
        assert callable(getattr(basemapper.Basemap, name))
    assert _params(basemapper.Basemap.render_png) == ["self", "path"]
    assert _params(basemapper.Basemap.render_svg)[:2] == ["self", "path"]


def _credited_style(credit="<b>&copy; Test</b>"):
    import json

    return json.dumps({"version": 8, "sources": {"s": {"type": "raster", "tiles": ["http://127.0.0.1:9/{z}/{x}/{y}.png"],
                       "attribution": credit}},
                       "layers": [{"id": "bg", "type": "background", "paint": {"background-color": "#dddddd"}},
                                  {"id": "r", "type": "raster", "source": "s"}]})


def test_add_basemap_draws_attribution_unless_disabled():
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    from basemapper import add_basemap

    def credits(**kw):
        fig, ax = plt.subplots(figsize=(3, 2))
        ax.set_xlim(-111.9, -111.8)
        ax.set_ylim(40.7, 40.8)
        add_basemap(ax, _credited_style(), crs="EPSG:4326", **kw)
        fig.canvas.draw()
        texts = [a.get_text() for a in ax.artists + ax.texts if a.get_gid() == "basemap_attribution"]
        plt.close(fig)
        return texts

    assert credits() == ["\u00a9 Test"]
    assert credits(attribution=False) == []
    assert credits(attribution="Mine") == ["Mine"]


def test_basemap_shows_itself_in_notebooks_at_css_size():
    import json

    import basemapper

    bg = json.dumps({"version": 8, "sources": {}, "layers": [
        {"id": "bg", "type": "background", "paint": {"background-color": "#336699"}}]})
    data, meta = basemapper.basemap(bg, 16, 8, pixel_ratio=2)._repr_png_()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    assert meta == {"width": 16, "height": 8}
