"""End-to-end render tests: Python -> Rust core -> HTTP -> renderer -> numpy.

Each test serves one of the repository's golden fixtures
(``core/tests/fixtures/golden/<case>``: style, tiles, glyphs, sprite) from a
local HTTP server, renders it through :func:`basemapper.basemap`,
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

import basemapper

GOLDEN = pathlib.Path(__file__).resolve().parents[2] / "core" / "tests" / "fixtures" / "golden"
BAD_CHANNEL_DIFF = 24

pytestmark = pytest.mark.skipif(not GOLDEN.is_dir(), reason="golden fixtures need a repository checkout")


class _FixtureHandler(http.server.SimpleHTTPRequestHandler):
    """Serves /tiles, /fonts and /sprites from the fixture's directories."""

    roots: dict = {}
    # When set, tiles outside the fixture are answered with an empty tile (a
    # valid, empty MVT), so a plot at any size and zoom still renders.
    empty_missing_tiles = False

    def do_GET(self):
        path = pathlib.Path(self.translate_path(self.path))
        if self.empty_missing_tiles and "/tiles/" in self.path and not path.is_file():
            self.send_response(200)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        super().do_GET()

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


def _serve(name: str, empty_missing_tiles: bool = False):
    case = _case(name)
    base = GOLDEN / name
    fixture = GOLDEN / case["fixture_from"] if case.get("fixture_from") else base
    roots = {
        "tiles": GOLDEN / case["tiles_from"] if case.get("tiles_from") else fixture,
        "fonts": GOLDEN / case["fonts_from"] if case.get("fonts_from") else fixture,
        "sprites": fixture,
    }
    handler = type("Handler", (_FixtureHandler,), {"roots": roots, "empty_missing_tiles": empty_missing_tiles})
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    style = (fixture / "style.json").read_text(encoding="utf-8")
    style = style.replace("{{BASE}}", f"http://127.0.0.1:{server.server_address[1]}")
    return server, style, case


_TO_LONLAT = None


def lonlat(bbox_3857):
    """A fixture's Web Mercator box as lon/lat numbers (plain numbers are
    lon/lat; the round trip is exact to nanometres)."""
    global _TO_LONLAT
    if _TO_LONLAT is None:
        import pyproj

        _TO_LONLAT = pyproj.Transformer.from_crs(3857, 4326, always_xy=True)
    xmin, ymin, xmax, ymax = bbox_3857
    return [*_TO_LONLAT.transform(xmin, ymin), *_TO_LONLAT.transform(xmax, ymax)]


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
        s = basemapper.style(style, layers=case.get("layers"))
        ours = basemapper.basemap(s, bounds=lonlat(case["bbox"]), width=w, height=h).render().to_numpy()
    finally:
        server.shutdown()

    assert ours.shape == (h, w, 4) and ours.dtype == np.uint8
    ours = ours.astype(np.int16)
    ref = _reference(name)
    assert ref.shape == ours.shape

    bad = (np.abs(ours - ref).max(axis=2) > BAD_CHANNEL_DIFF).mean()
    limit = case.get("max_bad_pixel_frac", 0.0)
    assert bad <= limit + 1e-9, f"{name}: {bad:.5%} of pixels differ from MapLibre (limit {limit:.5%})"


def test_add_basemap_with_explicit_crs():
    """add_basemap on plain matplotlib axes with crs= (axes carry no CRS):
    the fixture's view, matching MapLibre's render of it."""
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    from basemapper import add_basemap

    server, style, case = _serve("carto_positron_z13")
    try:
        # Axes filling an 800 x 600 px figure: the fixture's view and tiles.
        # 96 dpi: the fixture's view (D16: the plot's dpi sets the zoom).
        fig = plt.figure(figsize=(case["width"] / 96, case["height"] / 96), dpi=96)
        ax = fig.add_axes([0, 0, 1, 1])
        xmin, ymin, xmax, ymax = case["bbox"]
        ax.set_xlim(xmin, xmax)
        ax.set_ylim(ymin, ymax)
        artist = add_basemap(ax, style, crs="EPSG:3857")
        plt.close(fig)
    finally:
        server.shutdown()
    values = artist.rendering.to_numpy()
    assert values.shape == (case["height"], case["width"], 4)
    bad = (np.abs(values.astype(np.int16) - _reference("carto_positron_z13")).max(axis=2) > BAD_CHANNEL_DIFF).mean()
    assert bad < 0.001, f"{bad:.5%} of pixels differ from MapLibre"


def test_plotnine_geom_basemap_renders():
    """geom_basemap draws under a plotnine plot (lon/lat data, default CRS)."""
    pytest.importorskip("plotnine")
    import matplotlib

    matplotlib.use("Agg")
    import pandas as pd
    import pyproj
    import matplotlib.pyplot as plt
    from plotnine import aes, geom_point, ggplot

    from basemapper import geom_basemap

    server, style, case = _serve("carto_positron_z13", empty_missing_tiles=True)
    try:
        to_ll = pyproj.Transformer.from_crs(3857, 4326, always_xy=True)
        xmin, ymin, xmax, ymax = case["bbox"]
        (lon0, lat0), (lon1, lat1) = to_ll.transform(xmin, ymin), to_ll.transform(xmax, ymax)
        df = pd.DataFrame({"lon": [lon0, lon1], "lat": [lat0, lat1]})
        plot = ggplot(df, aes("lon", "lat")) + geom_basemap(style) + geom_point(color="red")
        fig = plot.draw()
        from matplotlib.backends.backend_agg import FigureCanvasAgg

        FigureCanvasAgg(fig).draw()
        rgba = np.asarray(fig.canvas.buffer_rgba())
        plt.close(fig)
    finally:
        server.shutdown()
    # Positron's background colour (#fafaf8) must appear in the panel.
    near_bg = (np.abs(rgba[..., :3].astype(int) - [250, 250, 248]).max(axis=2) <= 2).mean()
    assert near_bg > 0.05, f"basemap not visible (background fraction {near_bg:.3f})"
