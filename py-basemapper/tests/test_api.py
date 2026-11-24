"""The 0.3 model in Python (docs/design/api-review.md §4): basemaps as
immutable values, styles, renderings, notes, and errors. No network: every
style here is inline or local."""

from __future__ import annotations

import io
import json
import math
import warnings

import numpy as np
import pytest

import basemapper
from basemapper import BasemapAdjustment, BasemapProblem, ValidationError, basemap, style

HALF = 20_037_508.342_789_244


def plain(color: str = "#336699") -> str:
    return json.dumps({"version": 8, "sources": {},
                       "layers": [{"id": "bg", "type": "background", "paint": {"background-color": color}}]})


def with_labels(color: str = "#336699") -> str:
    return json.dumps({"version": 8,
                       "sources": {"g": {"type": "geojson", "data": {"type": "FeatureCollection", "features": []}}},
                       "layers": [{"id": "bg", "type": "background", "paint": {"background-color": color}},
                                  {"id": "names", "type": "symbol", "source": "g", "layout": {"text-field": "x"}}]})


# ── Errors and the package ───────────────────────────────────────────────────


def test_errors_are_typed():
    assert issubclass(basemapper.BasemapError, RuntimeError)
    assert issubclass(ValidationError, ValueError)
    assert issubclass(BasemapProblem, UserWarning) and issubclass(BasemapAdjustment, UserWarning)


def test_the_package_reports_its_version():
    import importlib.metadata

    assert basemapper.__version__ == importlib.metadata.version("basemapper")


def test_removed_names_point_to_the_migration_guide():
    for name in ("render_basemap_raw", "basemap_provider", "tile_style", "wms_style", "wmts_style"):
        with pytest.raises(AttributeError, match="removed in 0.3.*migrating-to-0.3"):
            getattr(basemapper, name)
    for arg, hint in [("pixel_ratio", "dpi"), ("tile_zoom", "zoom"), ("labels", "features"), ("auth_token", "key"),
                      ("max_tiles", "FetchOptions")]:
        with pytest.raises(TypeError, match=f"`{arg}` was removed in basemapper 0.3; .*{hint}"):
            basemap(plain(), **{arg: 1})
    with pytest.raises(TypeError, match="unexpected keyword argument 'nonsense'"):
        basemap(plain(), nonsense=1)
    with pytest.raises(TypeError):
        basemap(plain(), 800, 600)  # width and height are keywords


# ── The view: given, derived, defaulted ──────────────────────────────────────


def test_the_default_view_is_the_world_and_says_so():
    m = basemap(plain())
    assert m.size == (800, 600) and m.dpi == 96
    assert m.zoom == pytest.approx(math.log2(800 / 512))
    assert m.provenance == {"extent": "default", "size": "default", "dpi": "default", "zoom": "derived"}
    assert m.bounds[2] - m.bounds[0] == pytest.approx(2 * HALF, abs=1e-3)
    assert [n.kind for n in m.notes] == ["zoom_auto", "dpi_default", "size_default", "extent_default"]
    assert m.crs.to_epsg() == 3857


def test_any_three_give_the_fourth():
    slc = [-112.1, 40.6, -111.7, 40.9]
    auto = basemap(plain(), bounds=slc)
    assert auto.provenance["zoom"] == "derived" and auto.size == (800, 600)
    # Extent + zoom + dpi: the size follows.
    sized = basemap(plain(), bounds=slc, zoom=11)
    assert sized.provenance["size"] == "derived" and sized.zoom == 11
    # Extent + zoom + size: the dpi follows.
    dpi = basemap(plain(), bounds=slc, zoom=11, width=800, height=600)
    assert dpi.provenance["dpi"] == "derived"
    # dpi is sharpness: the same map, more pixels.
    sharp = basemap(plain(), bounds=slc, dpi=192)
    assert sharp.size == (1600, 1200) and sharp.zoom == pytest.approx(auto.zoom)
    # Physical units: 6 x 4 in at 300 dpi.
    print_ = basemap(plain(), bounds=slc, width=6, height=4, units="in", dpi=300)
    assert print_.size == (1800, 1200)


def test_conflicts_are_errors_that_say_what_to_drop():
    slc = [-112.1, 40.6, -111.7, 40.9]
    with pytest.raises(ValidationError, match="over-determine"):
        basemap(plain(), bounds=slc, zoom=11, width=800, height=600, dpi=96)
    with pytest.raises(ValidationError, match="bounds .* center"):
        basemap(plain(), bounds=slc, center=(-111.9, 40.7), zoom=11)
    with pytest.raises(ValidationError, match="around"):
        basemap(plain(), center=(-111.9, 40.7))
    with pytest.raises(ValidationError, match="width and height"):
        basemap(plain(), width=800)
    with pytest.raises(ValidationError, match="longitude / latitude"):
        basemap(plain(), bounds=[-12_500_000, 4_900_000, -12_400_000, 5_000_000])


def test_basemaps_are_immutable_values():
    m = basemap(plain(), center=(-111.89, 40.76), zoom=12)
    n = m.set_view(zoom=10)
    assert (m.zoom, n.zoom) == (12, 10) and n is not m
    assert n.center == pytest.approx((-111.89, 40.76))
    with pytest.raises(Exception):
        m.zoom_arg = 3  # frozen
    b = m.set_bounds([-112.1, 40.6, -111.7, 40.9])
    assert b.provenance["extent"] == "given" and b.center_arg is None
    # The zoom given before stays: extent + zoom give the size.
    assert b.zoom == 12 and b.provenance["size"] == "derived"
    c = b.set_view(center=(-111.5, 40.5))
    assert c.bounds_arg is None and c.center == pytest.approx((-111.5, 40.5))
    assert "zoom 12.00" in repr(m) and "(auto)" not in repr(b)
    assert "(auto)" in repr(b.set_view(zoom=None))


def test_around_is_ground_distance_in_its_own_units():
    a = basemapper.around((-111.9, 40.76), (20, 15), units="km")
    m = basemap(plain(), bounds=a)
    assert m.crs.to_epsg() == 3857
    with pytest.raises(ValidationError, match="units"):
        basemapper.around((0, 0), 1, units="furlongs")


# ── Styles ───────────────────────────────────────────────────────────────────


def test_styles_are_checked_when_made():
    s = style("OpenFreeMap.Liberty", features=["roads", "labels"])
    assert s.features == ("roads", "labels") and style(s, key="k").features == s.features
    with pytest.raises(ValidationError, match="unknown feature"):
        style(plain(), features="highways")
    with pytest.raises(ValidationError, match='both "labels" and "-labels"'):
        style(plain(), features=["labels", "-labels"])
    with pytest.raises(ValidationError, match="only as raster"):
        style(basemapper.provider("Esri.WorldImagery", form="vector"))
    with pytest.raises(ValidationError, match="did you mean CartoDB.Positron"):
        style(basemapper.provider("CartoDB.Positon"))


def test_a_missing_catalogue_key_is_an_error_at_once(monkeypatch):
    monkeypatch.delenv("MAPTILER_API_KEY", raising=False)
    with pytest.raises(ValidationError, match="MAPTILER_API_KEY"):
        style("MapTiler.Streets")
    assert style("MapTiler.Streets", key="K").key == "K"
    assert "K" not in repr(style("MapTiler.Streets", key="K")), "keys stay out of reprs"


def test_each_style_of_a_stack_has_its_own_filters():
    base, top = style(plain("#ff0000")), style(with_labels("#0000ff"), features="labels")
    img = basemap([base, top], center=(0, 0), zoom=2, width=64, height=48).render().to_numpy()
    assert tuple(img[0, 0]) == (255, 0, 0, 255), "only the top style's labels: the base shows through"
    with pytest.raises(basemapper.BasemapError, match="nothing of this style of the stack"):
        basemap([style(plain(), features="labels"), with_labels()], center=(0, 0), zoom=2, width=8, height=8).render()


# ── Renderings ───────────────────────────────────────────────────────────────


def test_a_rendering_is_one_render_with_many_conversions(tmp_path):
    m = basemap(plain("#123456"), center=(-111.89, 40.76), zoom=12, width=40, height=20, dpi=192)
    r = m.render()
    assert (r.width, r.height, r.dpi, r.zoom) == (40, 20, 192, 12)
    assert r.crs.to_epsg() == 3857 and r.bounds == pytest.approx(m.bounds)
    a = r.to_numpy()
    assert a.shape == (20, 40, 4) and tuple(a[0, 0]) == (0x12, 0x34, 0x56, 255)
    assert not a.flags.writeable, "a view of the rendering's memory, not a copy"
    path = tmp_path / "map.png"
    r.save(path)
    assert path.read_bytes()[:8] == b"\x89PNG\r\n\x1a\n" and r.to_png() == path.read_bytes()
    assert r.scale == pytest.approx(m.scale)
    assert r.css_size() == (20, 10)
    data, meta = r._repr_png_()
    assert meta == {"width": 20, "height": 10}
    assert "40 x 20 px" in repr(r) and "EPSG:3857" in repr(r)
    # render(dpi=) is set_view(dpi=).render(). With the size defaulted, the
    # same map, sharper; with the pixels fixed (and a zoom), the same pixels
    # hold less of the map (§4.2: dpi changes content only then).
    world = basemap(plain(), bounds=[-112.1, 40.6, -111.7, 40.9])
    sharp = world.render(dpi=192)
    assert (sharp.width, sharp.height) == (1600, 1200) and sharp.zoom == pytest.approx(world.zoom)
    fixed = m.render(dpi=384)
    assert fixed.width == 40 and fixed.zoom == 12


def test_to_xarray_is_georeferenced():
    pytest.importorskip("xarray")
    r = basemap(plain(), bounds=[-1, -1, 1, 1], width=20, height=20).render()
    x = r.to_xarray()
    assert x.dims == ("y", "x", "band") and list(x.band.values) == ["r", "g", "b", "a"]
    assert float(x.x[0]) == pytest.approx(r.bounds[0] + (r.bounds[2] - r.bounds[0]) / 40)
    assert float(x.y[0]) > float(x.y[-1]), "rows run north to south"
    assert "3857" in x.attrs["crs"]


def test_svg_is_a_vector_document_in_web_mercator(tmp_path):
    m = basemap(plain(), center=(0, 0), zoom=3, width=64, height=32)
    out = tmp_path / "map.svg"
    svg = m.render_svg(out)
    assert svg.startswith("<svg") and 'viewBox="0 0 64 32"' in svg and out.read_text(encoding="utf-8") == svg
    utm = basemap(plain(), center=(-111.9, 40.7), zoom=10, crs=32612)
    with pytest.raises(ValidationError, match="SVG output is in Web Mercator only"):
        utm.render_svg()


# ── Notes ────────────────────────────────────────────────────────────────────


def test_notes_are_surfaced_by_level(caplog):
    import logging

    # Decided: on the logger, never a warning.
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        with caplog.at_level(logging.INFO, logger="basemapper"):
            basemap(plain(), width=8, height=8).render()
    assert any("dpi (default)" in m for m in caplog.messages)
    # Adjusted: the view kept inside the world, attributed to this line.
    with pytest.warns(BasemapAdjustment, match="kept inside the map's world") as caught:
        basemap(plain(), center=(0, 89), zoom=1, width=64, height=64).render()
    assert caught[0].filename == __file__, "warnings point at the user's call"
    # Problem: a filter name that matches nothing, every time.
    m = basemap(style(plain(), layers=["-label_*"]), width=8, height=8)
    for _ in range(2):
        with pytest.warns(BasemapProblem, match="matches no layer"):
            m.render()


def test_on_error_fail_turns_problems_into_errors():
    video = json.dumps({"version": 8,
                        "sources": {"g": {"type": "geojson", "data": {"type": "FeatureCollection", "features": []}},
                                    "v": {"type": "video", "urls": []}},
                        "layers": [{"id": "bg", "type": "background"}, {"id": "d", "type": "circle", "source": "g"},
                                   {"id": "m", "type": "raster", "source": "v"}]})
    with pytest.warns(BasemapProblem, match="video"):
        r = basemap(video, width=8, height=8).render()
    assert r.notes_at("problem")
    strict = basemap(video, width=8, height=8, fetch=basemapper.FetchOptions(on_error="fail"))
    with pytest.raises(basemapper.BasemapError, match="on_error"):
        strict.render()


def test_fetch_options_are_checked():
    with pytest.raises(ValidationError):
        basemapper.FetchOptions(timeout=0)
    with pytest.raises(ValidationError):
        basemapper.FetchOptions(on_error="retry")
    with pytest.raises(ValidationError, match="FetchOptions"):
        basemap(plain(), fetch={"timeout": 3})


def test_the_package_imports_without_its_plot_extras(monkeypatch):
    import importlib
    import sys

    for name in [m for m in sys.modules if m == "basemapper" or m.startswith("basemapper.")]:
        if name != "basemapper._basemapper":
            monkeypatch.delitem(sys.modules, name)
    # Block the extras and every submodule already loaded (a cached
    # submodule would import without its package).
    for name in [m for m in sys.modules if m.split(".")[0] in ("plotnine", "matplotlib")] + ["plotnine", "matplotlib"]:
        monkeypatch.setitem(sys.modules, name, None)
    fresh = importlib.import_module("basemapper")
    assert fresh.basemap is not None
    with pytest.raises(ImportError, match=r"pip install 'basemapper\[plotnine\]'"):
        fresh.geom_basemap
    with pytest.raises(ImportError, match=r"basemapper\[matplotlib\]"):
        fresh.add_basemap
