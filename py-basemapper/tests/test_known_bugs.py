"""Known bugs of 0.2, fixed by the 0.3.0 redesign (docs/design/api-review.md,
implementation-plan.md).

Each test states the correct behaviour and is marked ``xfail(strict=True)``:
it is expected to fail while the bug is there, and the suite fails once the
bug is fixed, as a reminder to remove the marker and keep the test.
"""

import numpy as np
import pytest

SQUARE = '{"type":"Polygon","coordinates":[[[10,58],[12,58],[12,60],[10,60],[10,58]]]}'
SQUARE_STYLE = (
    '{"version":8,"sources":{"sq":{"type":"geojson","data":' + SQUARE + '}},"layers":['
    '{"id":"bg","type":"background","paint":{"background-color":"#ffffff"}},'
    '{"id":"sq","type":"fill","source":"sq","paint":{"fill-color":"#ff0000","fill-antialias":false}}]}'
)


def _centroid(rgba, is_colour):
    rgb = rgba[..., :3].astype(float) / 255
    rows, cols = np.nonzero(is_colour(rgb[..., 0], rgb[..., 1], rgb[..., 2]))
    assert rows.size > 0, "no pixels of that colour"
    return np.array([cols.mean(), rows.mean()])


def _figure():
    matplotlib = pytest.importorskip("matplotlib")
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig = plt.figure(figsize=(6, 6), dpi=100)
    ax = fig.add_axes([0, 0, 1, 1])
    # Europe, in longitude / latitude: the square sits far from the centre.
    ax.set_xlim(-10, 30)
    ax.set_ylim(35, 71)
    ax.set_axis_off()
    return fig, ax


def _pixels(fig):
    fig.canvas.draw()
    return np.asarray(fig.canvas.buffer_rgba()).copy()


@pytest.mark.xfail(strict=True, reason="F1: the Web Mercator image is stretched over a lon/lat axes, not warped (phase 3)")
def test_f1_add_basemap_lines_up_with_the_data_on_lon_lat_axes():
    import basemapper

    fig, ax = _figure()
    basemapper.add_basemap(ax, SQUARE_STYLE, crs="EPSG:4326", attribution=False)
    red = _centroid(_pixels(fig), lambda r, g, b: (r > 0.8) & (g < 0.3) & (b < 0.3))

    fig, ax = _figure()
    ax.fill([10, 12, 12, 10], [58, 58, 60, 60], color="#0000ff", linewidth=0)
    blue = _centroid(_pixels(fig), lambda r, g, b: (b > 0.8) & (r < 0.3) & (g < 0.3))

    offset = float(np.hypot(*(red - blue)))
    assert offset < 2, f"basemap offset {offset:.1f} px from the data"


@pytest.mark.xfail(strict=True, reason="F4: Python's bbox checks refuse a box crossing the date line (phase 3)")
def test_f4_box_crossing_the_date_line_renders():
    import basemapper

    bg = '{"version":8,"sources":{},"layers":[{"id":"bg","type":"background"}]}'
    a = basemapper.render_basemap_raw([19_000_000, -1e6, -19_000_000, 1e6], 100, 50, bg, crs=3857)
    assert a.shape[2] == 4
