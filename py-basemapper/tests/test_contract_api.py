"""The Python package against the API contract (docs/api-contract.json,
described in docs/api.md): every function and argument it lists, by name.
The R suite checks R against the same file, so the two packages cannot
drift apart."""

from __future__ import annotations

import inspect
import json
import pathlib

import pytest

import basemapper

CONTRACT = pathlib.Path(__file__).resolve().parents[2] / "docs" / "api-contract.json"
pytestmark = pytest.mark.skipif(not CONTRACT.is_file(), reason="the contract needs a repository checkout")


def _contract():
    return json.loads(CONTRACT.read_text(encoding="utf-8"))


def _target(name: str):
    obj = basemapper
    for part in name.split("."):
        obj = getattr(obj, part)
    return obj.__init__ if inspect.isclass(obj) else obj


def _args(fn, ignore):
    return [p for p in inspect.signature(fn).parameters if p not in ignore]


def test_every_function_has_the_contracts_arguments():
    c = _contract()
    ignore = set(c["language_only_arguments"]["python"])
    problems = []
    for f in c["functions"]:
        name = f.get("python")
        if not name:
            continue
        try:
            got = _args(_target(name), ignore)
        except AttributeError:
            problems.append(f"{name}: missing")
            continue
        want = f["args"] + f.get("python_also", [])
        if sorted(got) != sorted(want):
            problems.append(f"{name}: arguments {got}, the contract says {want}")
    assert not problems, "\n".join(problems)


def test_view_properties_and_rendering_conversions():
    c = _contract()
    for prop in c["view"]["python"]:
        assert isinstance(getattr(basemapper.Basemap, prop, None), property), f"Basemap.{prop} is not a property"
    for method in c["rendering"]["python"]:
        assert callable(getattr(basemapper.Rendering, method, None)), f"Rendering.{method} is missing"


def test_defunct_names_are_errors_that_point_to_the_migration_guide():
    for name in _contract()["defunct"]["python"]:
        with pytest.raises(AttributeError, match="migrating-to-0.3"):
            getattr(basemapper, name)


def test_nothing_public_is_left_out_of_the_contract():
    c = _contract()
    listed = {f["python"].split(".")[0] for f in c["functions"] if f.get("python")}
    # Values, errors and catalogue helpers the contract describes in prose.
    described = {"Basemap", "Rendering", "Note", "Style", "Source", "providers", "__version__", "BasemapError",
                 "ValidationError", "StyleError", "NetworkError", "BasemapWarning", "BasemapAdjustment",
                 "BasemapProblem"}
    unlisted = set(basemapper.__all__) - listed - described
    assert not unlisted, f"public but not in docs/api-contract.json: {sorted(unlisted)}"
