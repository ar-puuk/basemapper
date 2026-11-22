"""The basemap() view API and resolution-aware plotting, against the golden fixtures."""

from __future__ import annotations

import io

import numpy as np
import pytest

import basemapper
from basemapper import Basemap, basemap
from test_render_golden import BAD_CHANNEL_DIFF, GOLDEN, _reference, _serve

pytestmark = pytest.mark.skipif(not GOLDEN.is_dir(), reason="golden fixtures need a repository checkout")


def _bad_frac(ours: np.ndarray, ref: np.ndarray) -> float:
    return float((np.abs(ours.astype(np.int16) - ref).max(axis=2) > BAD_CHANNEL_DIFF).mean())


def _over(dst: np.ndarray, src: np.ndarray) -> np.ndarray:
    """Straight-alpha source-over, as the golden tests composite stacks."""
    s, d = src.astype(float) / 255, dst.astype(float) / 255
    sa, da = s[..., 3:], d[..., 3:]
    a = sa + da * (1 - sa)
    c = np.where(a > 0, (s[..., :3] * sa + d[..., :3] * da * (1 - sa)) / np.maximum(a, 1e-9), 0)
    return np.rint(np.concatenate([c, a], -1) * 255).astype(np.int16)


UGRC = ["ugrc_hillshade_z12_4", "ugrc_litebase_z12_4", "ugrc_litelabels_z12_4"]


def test_view_round_trips():
    m = basemap("{}", 800, 600, center=(-111.8887, 40.7556), zoom=12.4)
    assert isinstance(m, Basemap)
    b = m.get_bounds()
    # 800 CSS px at zoom 12.4 is 14.48 m/px (same value as the R test).
    assert (b[2] - b[0]) / 800 == pytest.approx(14.482108758, abs=1e-8)
    m2 = basemap("{}", 800, 600, bounds=b, crs=3857)
    assert m2.get_zoom() == pytest.approx(12.4, abs=1e-9)
    assert m2.get_center() == pytest.approx(m.get_center(), abs=1e-9)

    # set_view / set_bounds chain and keep what they are not given.
    assert m.set_view(zoom=10) is m
    assert m.get_zoom() == 10 and m.get_center() == pytest.approx((-111.8887, 40.7556))
    assert m.set_bounds(b, crs=3857).get_zoom() == pytest.approx(12.4, abs=1e-9)

    w, s, e, n = m.get_bounds(lonlat=True)
    assert w < -111.8887 < e and s < 40.7556 < n

    assert m.get_size() == (800, 600)
    assert basemap("{}", 800, 600, pixel_ratio=2).get_size() == (1600, 1200)
    # Rounded as the core rounds: halves away from zero.
    assert basemap("{}", 5, 3, pixel_ratio=1.5).get_size() == (8, 5)
    assert "800 x 600 @ 1x" in repr(m)
    with pytest.raises(ValueError):
        basemap("{}", center=(0, 0), zoom=1, bounds=[0, 0, 1, 1])
    with pytest.raises(ValueError):
        basemap("{}", pixel_ratio=0)
    with pytest.raises(ValueError):
        m.set_view(zoom=23)


@pytest.mark.parametrize("ratio", [1, 2])
def test_stack_matches_maplibre(ratio):
    """The full UGRC basemap (three services stacked) at zoom 12.4."""
    servers, styles, case = [], [], None
    for name in UGRC:
        server, style, case = _serve(name)
        servers.append(server)
        styles.append(style)
    try:
        m = basemap(styles, case["width"], case["height"], pixel_ratio=ratio, bounds=case["bbox"], crs=3857)
        assert m.get_zoom() == pytest.approx(12.4, abs=1e-6)
        ours = m.render_array()
        png = m.render_png()
    finally:
        for s in servers:
            s.shutdown()

    assert ours.shape == (case["height"] * ratio, case["width"] * ratio, 4)
    assert m.get_size() == (ours.shape[1], ours.shape[0])
    suffix = "" if ratio == 1 else f"_x{ratio}"
    ref = None
    for name in UGRC:
        r = _reference(name + suffix)
        ref = r if ref is None else _over(ref, r)
    assert _bad_frac(ours, ref) < 0.001

    import matplotlib.image as mpimg

    decoded = np.rint(mpimg.imread(io.BytesIO(png), format="png") * 255).astype(np.uint8)
    assert np.array_equal(decoded, ours)


def test_render_svg_is_a_vector_document(tmp_path):
    server, style, case = _serve("carto_positron_z13")
    try:
        m = basemap(style, case["width"], case["height"], bounds=case["bbox"], crs=3857)
        out = tmp_path / "map.svg"
        # Editable text (fonts by name only: no downloads in tests).
        svg = m.render_svg(str(out), embed_fonts="none")
        paths = m.render_svg(text="paths")
        bitmap = m.render_svg(text="bitmap")
    finally:
        server.shutdown()
    assert svg.startswith("<svg") and svg.endswith("</svg>")
    assert 'viewBox="0 0 800 600"' in svg
    assert out.read_text(encoding="utf-8") == svg
    # Labels are <text> with per-glyph positions and the style's fonts.
    assert "<text " in svg and 'xml:space="preserve"' in svg
    assert "font-family=\"'Montserrat'" in svg or "font-family=\"'Open Sans'" in svg
    assert "@font-face" not in svg
    # Traced outlines: no text elements, more paths.
    assert "<text" not in paths
    assert paths.count("<path") > svg.count("<path")
    # Bitmap glyphs: the style's SDF glyphs as images, thresholded by filters.
    assert "<text" not in bitmap and "<filter" in bitmap and "feComponentTransfer" in bitmap
    with pytest.raises(Exception):
        m.render_svg(text="raster")


def test_add_basemap_renders_natively_at_savefig_dpi():
    """savefig(dpi=192) re-renders the basemap at 2x instead of upscaling."""
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    server, style, case = _serve("carto_positron_z13", empty_missing_tiles=True)
    try:
        fig = plt.figure(figsize=(800 / 96, 600 / 96), dpi=96)
        ax = fig.add_axes([0, 0, 1, 1])
        ax.set_xlim(case["bbox"][0], case["bbox"][2])
        ax.set_ylim(case["bbox"][1], case["bbox"][3])
        first = basemapper.add_basemap(ax, style, crs=3857)
        img = ax.images[-1]
        assert first.attrs["pixel_ratio"] == pytest.approx(1.0)
        assert first.shape == (600, 800, 4)

        buf = io.BytesIO()
        fig.savefig(buf, dpi=192, format="png")
        r = img.last_render
        assert r["pixel_ratio"] == pytest.approx(2.0)
        assert (r["width"], r["height"]) == (1600, 1200)
        plt.close(fig)
    finally:
        server.shutdown()

    # At 96 dpi the basemap is the 1x render of the view; at 192 dpi the 2x.
    assert _bad_frac(first.values, _reference("carto_positron_z13")) < 0.001
    assert _bad_frac(r["array"], _reference("carto_positron_z13_x2")) < 0.001
