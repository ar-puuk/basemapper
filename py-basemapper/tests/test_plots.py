"""Basemaps in plots (docs/design/api-review.md §4.5, D14, D15, D16):
matplotlib ``m.add_to(ax)`` / ``add_basemap`` and plotnine ``ggplot() + m``
/ ``geom_basemap``, on the core's plot plan. Offline: inline styles."""

from __future__ import annotations

import io
import json

import numpy as np
import pytest

matplotlib = pytest.importorskip("matplotlib")
matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

import basemapper  # noqa: E402
from basemapper import ValidationError, basemap  # noqa: E402

# A background and an (empty) GeoJSON source that carries a credit line.
PLAIN = json.dumps({"version": 8,
                    "sources": {"g": {"type": "geojson", "data": {"type": "FeatureCollection", "features": []},
                                      "attribution": "<b>&copy; Test</b>"}},
                    "layers": [{"id": "bg", "type": "background", "paint": {"background-color": "#336699"}},
                               {"id": "d", "type": "circle", "source": "g"}]})


def axes(size=(4, 3), dpi=100, xlim=(-111.95, -111.85), ylim=(40.70, 40.80)):
    fig = plt.figure(figsize=size, dpi=dpi)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.set_xlim(*xlim)
    ax.set_ylim(*ylim)
    return fig, ax


def test_add_to_renders_for_the_axes_and_returns_the_artist():
    fig, ax = axes()
    artist = basemap(PLAIN).add_to(ax, crs="EPSG:4326", attribution=False)
    r = artist.rendering
    # A canvas that is no screen counts as an export at its dpi (D16).
    assert (r.width, r.height, r.dpi) == (400, 300, 100)
    assert r.crs.to_epsg() == 4326 and r.bounds == pytest.approx((-111.95, 40.70, -111.85, 40.80))
    fig.canvas.draw()
    px = np.asarray(fig.canvas.buffer_rgba())
    assert tuple(px[150, 200, :3]) == (0x33, 0x66, 0x99)
    plt.close(fig)


def test_savefig_renders_natively_at_the_export_dpi():
    fig, ax = axes()
    artist = basemap(PLAIN).add_to(ax, crs="EPSG:4326", attribution=False)
    zoom_screen = artist.rendering.zoom
    fig.savefig(io.BytesIO(), format="png", dpi=300)
    r = artist.rendering
    assert (r.width, r.height, r.dpi) == (1200, 900, 300)
    assert r.zoom == pytest.approx(zoom_screen, abs=1e-9), "the export shows the preview's map, sharper"
    fig.savefig(io.BytesIO(), format="pdf")
    pdf = artist.rendering
    assert (pdf.dpi, pdf.width, pdf.height) == (300, 1200, 900), "PDF has no dpi: 300 (D16)"
    assert any(n.kind == "default_dpi" for n in pdf.notes)
    plt.close(fig)


def test_a_chosen_zoom_gives_an_effective_dpi():
    fig, ax = axes()
    with pytest.warns(basemapper.BasemapAdjustment, match="effective") as caught:
        artist = basemap(PLAIN, zoom=14).add_to(ax, crs="EPSG:4326", attribution=False)
    assert caught[0].filename == __file__, "warnings point at the user's add_to()"
    assert artist.rendering.zoom == 14 and artist.rendering.dpi != 100
    plt.close(fig)


def test_the_plot_gives_extent_and_crs_d15():
    fig, ax = axes()
    with pytest.raises(ValidationError, match="takes its extent from the plot"):
        basemap(PLAIN, bounds=[-112, 40, -111, 41]).add_to(ax, crs=4326)
    with pytest.raises(ValidationError, match="takes its CRS from the plot"):
        basemap(PLAIN, bounds=[-112, 40, -111, 41], crs=4326).add_to(ax, crs=4326)
    with pytest.raises(ValidationError, match="size from the plot panel"):
        basemap(PLAIN, width=100, height=100).add_to(ax, crs=4326)
    with pytest.raises(ValidationError, match="carry no CRS"):
        basemap(PLAIN).add_to(ax)
    plt.close(fig)


def test_the_credit_line_is_drawn_unless_disabled():
    def credits(**kw):
        fig, ax = axes(size=(3, 2))
        basemapper.add_basemap(ax, PLAIN, crs="EPSG:4326", **kw)
        fig.canvas.draw()
        texts = [a.get_text() for a in ax.artists + ax.texts if a.get_gid() == "basemap_attribution"]
        plt.close(fig)
        return texts

    assert credits() == ["© Test"]
    assert credits(attribution=False) == []
    assert credits(attribution="Mine, " * 30) and "\n" in credits(attribution="Mine, " * 30)[0], "wrapped to the axes"


def test_add_basemap_is_an_alias():
    fig, ax = axes()
    a = basemapper.add_basemap(ax, PLAIN, crs=4326, zoom=13, attribution=False)
    assert a.rendering.zoom == 13
    m = basemap(PLAIN)
    with pytest.raises(TypeError, match="already has its arguments"):
        basemapper.add_basemap(ax, m, zoom=3)
    plt.close(fig)


def test_plotnine_ggplot_plus_a_basemap():
    pytest.importorskip("plotnine")
    import pandas as pd
    from plotnine import aes, geom_point, ggplot

    df = pd.DataFrame({"lon": [-111.95, -111.85], "lat": [40.70, 40.80]})
    for layer in (basemap(PLAIN), basemapper.geom_basemap(PLAIN, attribution=False)):
        fig = (ggplot(df, aes("lon", "lat")) + layer + geom_point(color="red")).draw()
        # plotnine's figure may have a bare FigureCanvasBase (CI): draw with Agg.
        from matplotlib.backends.backend_agg import FigureCanvasAgg
        canvas = FigureCanvasAgg(fig)
        canvas.draw()
        px = np.asarray(canvas.buffer_rgba())[..., :3].astype(int)
        share = (np.abs(px - [0x33, 0x66, 0x99]).max(axis=2) <= 2).mean()
        assert share > 0.2, f"basemap colour covers {share:.2%} of the figure"
        plt.close(fig)
    with pytest.raises(ValidationError, match="takes its CRS from the plot"):
        basemapper.geom_basemap(basemap(PLAIN, center=(0, 0), zoom=3, crs=32612))


def test_plotnine_reads_a_geodataframes_crs():
    pytest.importorskip("plotnine")
    gpd = pytest.importorskip("geopandas")
    from plotnine import aes, geom_point, ggplot
    from shapely.geometry import Point

    pts = gpd.GeoDataFrame({"x": [415_000.0, 435_000.0], "y": [4_500_000.0, 4_520_000.0]},
                           geometry=[Point(415_000, 4_500_000), Point(435_000, 4_520_000)], crs=32612)
    layer = basemapper.geom_basemap(PLAIN, attribution=False)
    fig = (ggplot(pts, aes("x", "y")) + layer + geom_point()).draw()
    fig.canvas.draw()
    img = [a for a in fig.axes[0].images][-1]
    assert img.rendering.crs.to_epsg() == 32612
    plt.close(fig)


# The WFRC tract view's Web Mercator limits: a 720 x 1000 px panel.
TRACT = (-12_469_979.0, 5_026_396.805555556, -12_466_414.0, 5_031_348.194444444)


def line_style(x_3857: float, width_css: float) -> str:
    """A white map with a black vertical line at Web Mercator ``x``."""
    lon = x_3857 / 20_037_508.342_789_244 * 180
    return json.dumps({"version": 8,
                       "sources": {"l": {"type": "geojson", "data": {"type": "LineString", "coordinates": [[lon, -80], [lon, 80]]}}},
                       "layers": [{"id": "bg", "type": "background", "paint": {"background-color": "#ffffff"}},
                                  {"id": "l", "type": "line", "source": "l", "paint": {"line-color": "#000000", "line-width": width_css}}]})


def dark_centre(row: np.ndarray, lo: int, hi: int) -> float:
    d = 255.0 - row[lo:hi].astype(float)
    return float((d * np.arange(lo, hi)).sum() / d.sum())


def test_a_web_mercator_panel_shows_the_rendering_pixel_for_pixel():
    # A panel of whole device pixels in Web Mercator: no warp, no resample.
    # The image is the rendering, and a line one device pixel wide at the
    # centre of column 300 is there, to a tenth of a pixel.
    x0, y0, x1, y1 = TRACT
    mx = (x1 - x0) / 720
    style = line_style(x0 + 300.5 * mx, 1 / 1.75)
    fig = plt.figure(figsize=(7.2, 10), dpi=100)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.set_axis_off()
    ax.set_xlim(x0, x1)
    ax.set_ylim(y0, y1)
    artist = basemap(style, dpi=168).add_to(ax, crs=3857, attribution=False)
    fig.canvas.draw()
    r = artist.rendering
    assert (r.width, r.height, r.dpi, r.crs.to_epsg()) == (720, 1000, 168, 3857)
    assert r.bounds == pytest.approx(TRACT, abs=1e-6)
    drawn = np.asarray(fig.canvas.buffer_rgba())[..., :3]
    assert np.array_equal(drawn, r.to_numpy()[..., :3]), "the panel shows the rendering's pixels"
    for y in (10, 500, 990):
        assert abs(dark_centre(drawn[y, :, 0], 290, 311) - 300) < 0.1
    plt.close(fig)


def test_a_plotnine_web_mercator_panel_shows_the_rendering_pixel_for_pixel():
    pytest.importorskip("plotnine")
    gpd = pytest.importorskip("geopandas")
    from matplotlib.backends.backend_agg import FigureCanvasAgg
    from plotnine import aes, coord_cartesian, geom_blank, ggplot, scale_x_continuous, scale_y_continuous, theme, theme_void
    from shapely.geometry import Point

    x0, y0, x1, y1 = TRACT
    mx = (x1 - x0) / 720
    layer = basemapper.geom_basemap(line_style(x0 + 300.5 * mx, 1 / 1.75), dpi=168, attribution=False)
    df = gpd.GeoDataFrame({"x": [x0, x1], "y": [y0, y1]}, geometry=[Point(x0, y0), Point(x1, y1)], crs=3857)
    p = (ggplot(df, aes("x", "y")) + layer + geom_blank()
         + scale_x_continuous(expand=(0, 0)) + scale_y_continuous(expand=(0, 0))
         + coord_cartesian(xlim=(x0, x1), ylim=(y0, y1), expand=False)
         + theme_void() + theme(figure_size=(7.2, 10), dpi=100, plot_margin=0))
    fig = p.draw()
    canvas = FigureCanvasAgg(fig)
    canvas.draw()
    drawn = np.asarray(canvas.buffer_rgba())[..., :3]
    ax = fig.axes[0]
    bb = ax.get_window_extent(canvas.get_renderer())
    img = next(a for a in ax.get_images())
    r = img.rendering
    assert (r.width, r.height) == (round(bb.width), round(bb.height)), "the image is the panel's size"
    # The panel's pixels in the figure (from the top-left).
    left, top = int(round(bb.x0)), int(round(drawn.shape[0] - bb.y1))
    panel = drawn[top:top + r.height, left:left + r.width]
    assert np.array_equal(panel, r.to_numpy()[..., :3]), "the panel shows the rendering's pixels"
    col = (300.5 * 720 / r.width) if r.width != 720 else 300.5
    for y in (10, r.height // 2, r.height - 10):
        assert abs(dark_centre(panel[y, :, 0], int(col) - 10, int(col) + 11) - (col - 0.5)) < 0.1
    plt.close(fig)
