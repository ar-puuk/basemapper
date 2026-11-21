"""End-to-end render tests: Python -> Rust core -> HTTP -> renderer -> numpy.

Each test serves one of the repository's golden fixtures
(``core/tests/fixtures/golden/<case>``: style, tiles, glyphs, sprite) from a
local HTTP server, renders it through :func:`basemapper.render_basemap_raw`,
and compares the pixels with MapLibre GL JS's render of the same view
(``reference-maplibre-gl.png``) within the case's own fidelity limit.

The fixtures live outside the Python package, so these tests are skipped when
the package is tested outside a repository checkout.
"""

from __future__ import annotations

import functools
import http.server
import json
import pathlib
import threading
import urllib.parse

import numpy as np
import pytest

from basemapper import render_basemap_raw

GOLDEN = pathlib.Path(__file__).resolve().parents[2] / "core" / "tests" / "fixtures" / "golden"
BAD_CHANNEL_DIFF = 24

pytestmark = pytest.mark.skipif(not GOLDEN.is_dir(), reason="golden fixtures need a repository checkout")


class _FixtureHandler(http.server.SimpleHTTPRequestHandler):
    """Serves /tiles, /fonts and /sprites from the fixture's directories."""

    roots: dict = {}

    def translate_path(self, path: str) -> str:
        parts = [urllib.parse.unquote(p) for p in urllib.parse.urlsplit(path).path.split("/") if p]
        if not parts or parts[0] not in self.roots:
            return str(GOLDEN / "__missing__")
        return str(self.roots[parts[0]].joinpath(*parts))

    def log_message(self, *args):  # keep test output quiet
        pass


@functools.lru_cache(maxsize=None)
def _case(name: str) -> dict:
    return json.loads((GOLDEN / name / "case.json").read_text(encoding="utf-8"))


def _serve(name: str):
    case = _case(name)
    base = GOLDEN / name
    fixture = GOLDEN / case["fixture_from"] if case.get("fixture_from") else base
    roots = {
        "tiles": GOLDEN / case["tiles_from"] if case.get("tiles_from") else fixture,
        "fonts": GOLDEN / case["fonts_from"] if case.get("fonts_from") else fixture,
        "sprites": fixture,
    }
    handler = type("Handler", (_FixtureHandler,), {"roots": roots})
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    style = (fixture / "style.json").read_text(encoding="utf-8")
    style = style.replace("{{BASE}}", f"http://127.0.0.1:{server.server_address[1]}")
    return server, style, case


def _reference(name: str) -> np.ndarray:
    import matplotlib.image as mpimg

    ref = mpimg.imread(GOLDEN / name / "reference-maplibre-gl.png")  # float 0..1
    return np.rint(ref * 255).astype(np.int16)


@pytest.mark.parametrize(
    "name",
    [
        "osm_raster_slc",  # raster tiles
        "carto_positron_z13",  # vector: fills, lines, point + line labels
        "carto_dark_matter_z7",  # vector at low zoom, icons
        "ugrc_litebase_slc",  # ESRI style, dashes, line offsets
    ],
)
def test_render_matches_maplibre(name):
    server, style, case = _serve(name)
    try:
        w, h = case["width"], case["height"]
        raw = render_basemap_raw(
            bbox=case["bbox"],
            crs=3857,
            width=w,
            height=h,
            style_input=style,
            layers=case.get("layers"),
            fail_on_tile_error=False,
        )
    finally:
        server.shutdown()

    assert len(raw) == w * h * 4
    ours = np.frombuffer(raw, dtype=np.uint8).reshape(h, w, 4).astype(np.int16)
    ref = _reference(name)
    assert ref.shape == ours.shape

    bad = (np.abs(ours - ref).max(axis=2) > BAD_CHANNEL_DIFF).mean()
    limit = case.get("max_bad_pixel_frac", 0.0)
    assert bad <= limit + 1e-9, f"{name}: {bad:.5%} of pixels differ from MapLibre (limit {limit:.5%})"


def test_add_basemap_with_explicit_crs():
    """add_basemap on plain matplotlib axes needs crs= (axes carry no CRS)."""
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    from basemapper import add_basemap

    server, style, case = _serve("carto_positron_z13")
    try:
        # Axes filling an 800 x 600 px figure: the fixture's view and tiles.
        fig = plt.figure(figsize=(case["width"] / 100, case["height"] / 100), dpi=100)
        ax = fig.add_axes([0, 0, 1, 1])
        xmin, ymin, xmax, ymax = case["bbox"]
        ax.set_xlim(xmin, xmax)
        ax.set_ylim(ymin, ymax)
        arr = add_basemap(ax, style, crs="EPSG:3857")
        plt.close(fig)
    finally:
        server.shutdown()
    values = np.asarray(arr)
    assert values.ndim == 3 and values.shape[2] == 4
    assert values[..., 3].max() == 255  # something opaque was drawn
