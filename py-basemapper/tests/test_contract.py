"""The view-resolution contract (core/tests/fixtures/contract/cases.json),
run from Python: the same cases the Rust and R suites run, so the languages
resolve every view the same way."""

from __future__ import annotations

import json
import pathlib

import pyproj
import pytest

import basemapper
from basemapper import _basemapper as core
from basemapper.exceptions import ValidationError

CASES = pathlib.Path(__file__).resolve().parents[2] / "core" / "tests" / "fixtures" / "contract" / "cases.json"
pytestmark = pytest.mark.skipif(not CASES.is_file(), reason="the contract needs a repository checkout")
TO_3857 = pyproj.Transformer.from_crs(4326, 3857, always_xy=True)


def _cases():
    return json.loads(CASES.read_text(encoding="utf-8")) if CASES.is_file() else {"views": [], "around": []}


def _inputs(c):
    i = c["inputs"]
    out = {k: i[k] for k in ("center", "zoom", "width", "height", "units", "dpi") if k in i}
    if "bounds_lonlat" in i:
        w, s, e, n = i["bounds_lonlat"]
        out["bounds"] = [*TO_3857.transform(w, s), *TO_3857.transform(e, n)]
    return out


def _resolve(c):
    ctx = c.get("context")
    if ctx is None:
        return core.resolve(_inputs(c))
    p = ctx["plot"]
    device = {"screen": {"kind": "screen", "pixel_ratio": p.get("pixel_ratio") or 1.0},
              "raster_export": {"kind": "raster_export", "dpi": p.get("dpi")},
              "vector_export": {"kind": "vector_export"}}[p["device"]]
    panel = {"limits": p["extent_3857"], "crs": "EPSG:3857", "px": tuple(p["panel_px"]), "device": device}
    return core.plan(_inputs(c), panel)


@pytest.mark.parametrize("case", _cases()["views"], ids=lambda c: c["name"])
def test_view_cases(case):
    if "error" in case:
        with pytest.raises(ValidationError) as e:
            _resolve(case)
        for word in case["error"]["contains"]:
            assert word in str(e.value)
        return
    v, want, tol = _resolve(case), case["expect"], case["tolerance"]
    assert v["width"] == pytest.approx(want["width"], abs=tol["px"])
    assert v["height"] == pytest.approx(want["height"], abs=tol["px"])
    assert v["dpi"] == pytest.approx(want["dpi"], abs=tol["dpi"])
    assert v["zoom"] == pytest.approx(want["zoom"], abs=tol["zoom"])
    assert v["bounds"] == pytest.approx(want["bounds_3857"], abs=tol["metres"])
    assert v["provenance"] == want["provenance"]
    assert [(n[0], n[1]) for n in v["notes"]] == [(n["level"], n["kind"]) for n in want["notes"]]


@pytest.mark.parametrize("case", _cases()["around"], ids=lambda c: c["name"])
def test_around_cases(case):
    span = case["span"] if len(case["span"]) == 2 else case["span"][0]
    a = basemapper.around(case["center"], span, units=case.get("units", "m"))
    assert a.bounds == pytest.approx(case["expect_lonlat"], abs=1e-9)
