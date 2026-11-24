"""Basemaps against the golden fixtures: stacks, dpi, SVG and plots rendering
natively at the export's dpi, compared with MapLibre GL JS."""

from __future__ import annotations

import io

import numpy as np
import pytest

import basemapper
from basemapper import basemap
from test_render_golden import BAD_CHANNEL_DIFF, GOLDEN, _reference, _serve, lonlat

pytestmark = pytest.mark.skipif(not GOLDEN.is_dir(), reason="golden fixtures need a repository checkout")

UGRC = ["ugrc_hillshade_z12_4", "ugrc_litebase_z12_4", "ugrc_litelabels_z12_4"]


def _bad_frac(ours: np.ndarray, ref: np.ndarray) -> float:
    return float((np.abs(ours.astype(np.int16) - ref).max(axis=2) > BAD_CHANNEL_DIFF).mean())


def _over(dst: np.ndarray, src: np.ndarray) -> np.ndarray:
    """Straight-alpha source-over, as the golden tests composite stacks."""
    s, d = src.astype(float) / 255, dst.astype(float) / 255
    sa, da = s[..., 3:], d[..., 3:]
    a = sa + da * (1 - sa)
    c = np.where(a > 0, (s[..., :3] * sa + d[..., :3] * da * (1 - sa)) / np.maximum(a, 1e-9), 0)
    return np.rint(np.concatenate([c, a], -1) * 255).astype(np.int16)


@pytest.mark.parametrize("ratio", [1, 2])
def test_a_stack_matches_maplibre_at_any_dpi(ratio):
    """UGRC's basemap (three services stacked) at zoom 12.4; at 192 dpi the
    same map with twice the pixels."""
    servers, styles, case = [], [], None
    for name in UGRC:
        server, style, case = _serve(name)
        servers.append(server)
        styles.append(style)
    try:
        m = basemap(styles, bounds=lonlat(case["bbox"]), width=case["width"] * ratio, height=case["height"] * ratio,
                    dpi=96 * ratio)
        assert m.zoom == pytest.approx(12.4, abs=1e-6)
        r = m.render()
    finally:
        for s in servers:
            s.shutdown()
    ours = r.to_numpy()
    assert ours.shape == (case["height"] * ratio, case["width"] * ratio, 4) and r.dpi == 96 * ratio
    suffix = "" if ratio == 1 else f"_x{ratio}"
    ref = None
    for name in UGRC:
        layer = _reference(name + suffix)
        ref = layer if ref is None else _over(ref, layer)
    assert _bad_frac(ours, ref) < 0.001

    import matplotlib.image as mpimg

    decoded = np.rint(mpimg.imread(io.BytesIO(r.to_png()), format="png") * 255).astype(np.uint8)
    assert np.array_equal(decoded, ours)


def test_render_svg_is_a_vector_document(tmp_path):
    server, style, case = _serve("carto_positron_z13")
    try:
        m = basemap(style, bounds=lonlat(case["bbox"]), width=case["width"], height=case["height"])
        out = tmp_path / "map.svg"
        svg = m.render_svg(out, embed_fonts="none")  # fonts by name: no downloads in tests
        paths = m.render_svg(text="paths")
        bitmap = m.render_svg(text="bitmap")
    finally:
        server.shutdown()
    assert svg.startswith("<svg") and svg.endswith("</svg>") and 'viewBox="0 0 800 600"' in svg
    assert out.read_text(encoding="utf-8") == svg
    assert "<text " in svg and 'xml:space="preserve"' in svg and "@font-face" not in svg
    assert "<text" not in paths and paths.count("<path") > svg.count("<path")
    assert "<text" not in bitmap and "<filter" in bitmap
    with pytest.raises(basemapper.ValidationError):
        m.render_svg(text="raster")


def test_a_plot_renders_natively_at_the_savefig_dpi():
    """savefig(dpi=192) re-renders the basemap at 2x, the same map as the
    96 dpi preview, matching MapLibre's 2x render."""
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    server, style, case = _serve("carto_positron_z13", empty_missing_tiles=True)
    try:
        fig = plt.figure(figsize=(800 / 96, 600 / 96), dpi=96)
        ax = fig.add_axes([0, 0, 1, 1])
        ax.set_xlim(case["bbox"][0], case["bbox"][2])
        ax.set_ylim(case["bbox"][1], case["bbox"][3])
        artist = basemap(style).add_to(ax, crs=3857, attribution=False)
        first = artist.rendering
        fig.savefig(io.BytesIO(), dpi=192, format="png")
        second = artist.rendering
        plt.close(fig)
    finally:
        server.shutdown()
    assert (first.width, first.height, first.dpi) == (800, 600, 96)
    assert (second.width, second.height, second.dpi) == (1600, 1200, 192)
    assert second.zoom == pytest.approx(first.zoom, abs=1e-9)
    assert _bad_frac(first.to_numpy(), _reference("carto_positron_z13")) < 0.001
    assert _bad_frac(second.to_numpy(), _reference("carto_positron_z13_x2")) < 0.001
