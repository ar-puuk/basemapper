"""Maps in a CRS other than Web Mercator (docs/design/api-review.md §4.2,
D18, D19, D20, D21): the map CRS rule, the warp's alignment and mesh
density, and clips. GeoPandas tests are skipped where it is not installed."""

from __future__ import annotations

import json
import math

import numpy as np
import pyproj
import pytest

import basemapper
from basemapper import ValidationError, basemap
from basemapper import _crs


def square_style(lon0, lat0, lon1, lat1, color="#ff0000") -> str:
    ring = [[lon0, lat0], [lon1, lat0], [lon1, lat1], [lon0, lat1], [lon0, lat0]]
    return json.dumps({
        "version": 8,
        "sources": {"sq": {"type": "geojson", "data": {"type": "Polygon", "coordinates": [ring]}}},
        "layers": [{"id": "bg", "type": "background", "paint": {"background-color": "#ffffff"}},
                   {"id": "sq", "type": "fill", "source": "sq",
                    "paint": {"fill-color": color, "fill-antialias": False}}],
    })


def red_centroid(img: np.ndarray) -> np.ndarray:
    rgb = img[..., :3].astype(int)
    rows, cols = np.nonzero((rgb[..., 0] > 200) & (rgb[..., 1] < 60) & (rgb[..., 2] < 60))
    assert rows.size, "no red pixels"
    return np.array([cols.mean() + 0.5, rows.mean() + 0.5])


def test_a_map_in_utm_puts_things_where_the_projection_does():
    # A small square at Salt Lake City, drawn in UTM 12N around it.
    style = square_style(-111.95, 40.72, -111.85, 40.80)
    m = basemap(style, center=(-111.9, 40.76), zoom=11, width=400, height=300, crs=32612)
    assert m.crs.to_epsg() == 32612 and m.size == (400, 300)
    r = m.render()
    assert r.crs.to_epsg() == 32612 and r.bounds == pytest.approx(m.bounds)
    # The square's centre in UTM, then in the image's pixels.
    to_utm = pyproj.Transformer.from_crs(4326, 32612, always_xy=True)
    corners = [to_utm.transform(x, y) for x, y in [(-111.95, 40.72), (-111.85, 40.72), (-111.85, 40.80), (-111.95, 40.80)]]
    cx, cy = np.mean([c[0] for c in corners]), np.mean([c[1] for c in corners])
    x0, y0, x1, y1 = r.bounds
    want = np.array([(cx - x0) / (x1 - x0) * r.width, (y1 - cy) / (y1 - y0) * r.height])
    got = red_centroid(r.to_numpy())
    assert np.hypot(*(got - want)) < 1.0, f"the square is {np.hypot(*(got - want)):.2f} px from where UTM puts it"


def test_the_scale_of_a_warped_map_is_its_centres():
    m = basemap(square_style(-112, 40, -111, 41), center=(-111.9, 40.76), zoom=11, width=64, height=64, crs=32612)
    r = m.render()
    # The scale comes from the zoom at the centre's latitude, not UTM northings.
    expected = 2 * 20_037_508.342_789_244 / (512 * 2**11) * math.cos(math.radians(40.76)) * 96 / 0.0254
    assert r.scale == pytest.approx(expected, rel=2e-3)


@pytest.mark.parametrize("crs, box", [
    (32612, (380_000.0, 4_450_000.0, 520_000.0, 4_560_000.0)),  # UTM 12N, ~140 km
    (5070, (-1_900_000.0, 1_700_000.0, -1_300_000.0, 2_200_000.0)),  # CONUS Albers, ~600 km
    (5070, (-2_300_000.0, 300_000.0, 2_200_000.0, 3_100_000.0)),  # CONUS Albers, the whole country
])
def test_the_warp_mesh_is_within_half_a_pixel(crs, box):
    """The mesh (a node every 32 px) interpolated bilinearly against the
    exact transform at every pixel centre, in output pixels."""
    w, h = 800, 600
    nx, ny, nodes = _crs.mesh(_crs.crs_of(crs), box, w, h)
    grid = np.array(nodes).reshape(ny, nx, 2)
    px, py = np.meshgrid(np.arange(w) + 0.5, np.arange(h) + 0.5)
    gx, gy = px / w * (nx - 1), py / h * (ny - 1)
    i, j = np.minimum(gx.astype(int), nx - 2), np.minimum(gy.astype(int), ny - 2)
    fx, fy = (gx - i)[..., None], (gy - j)[..., None]
    interp = (grid[j, i] * (1 - fx) * (1 - fy) + grid[j, i + 1] * fx * (1 - fy)
              + grid[j + 1, i] * (1 - fx) * fy + grid[j + 1, i + 1] * fx * fy)
    xmin, ymin, xmax, ymax = box
    t = pyproj.Transformer.from_crs(crs, 3857, always_xy=True)
    ex, ey = t.transform(xmin + px / w * (xmax - xmin), ymax - py / h * (ymax - ymin))
    exact = np.stack([ex, ey], axis=-1)
    # Web Mercator metres per output pixel, locally: the distance between
    # neighbouring pixel centres.
    local = np.hypot(*(exact[:, 1:] - exact[:, :-1]).transpose(2, 0, 1))
    err = np.hypot(*(interp - exact).transpose(2, 0, 1))[:, :-1] / local
    assert err.max() < 0.5, f"worst interpolation error {err.max():.3f} px"


# ── With GeoPandas ───────────────────────────────────────────────────────────


@pytest.fixture
def gpd():
    return pytest.importorskip("geopandas")


def _county(gpd, crs=32612):
    from shapely.geometry import box

    # Salt Lake City's area in UTM 12N.
    return gpd.GeoDataFrame(geometry=[box(415_000, 4_500_000, 435_000, 4_520_000)], crs=crs)


def test_the_map_crs_rule(gpd):
    utm = _county(gpd)
    assert basemap(square_style(0, 0, 1, 1), bounds=utm).crs.to_epsg() == 32612, "a projected layer gives its CRS (D18)"
    lonlat = utm.to_crs(4326)
    assert basemap(square_style(0, 0, 1, 1), bounds=lonlat).crs.to_epsg() == 3857, "a lon/lat layer falls back (D19)"
    assert basemap(square_style(0, 0, 1, 1), bounds=lonlat, crs=4326).crs.to_epsg() == 4326, "crs given wins"
    assert basemap(square_style(0, 0, 1, 1), bounds=utm, crs=3857).crs.to_epsg() == 3857
    m = basemap(square_style(0, 0, 1, 1), bounds=utm, width=200, height=200)
    b = m.bounds
    assert b[0] <= 415_000 and b[2] >= 435_000 and b[1] <= 4_500_000 and b[3] >= 4_520_000


def test_clips_inside_outside_and_their_extent(gpd):
    utm = _county(gpd)
    style = square_style(-120, 30, -100, 50, color="#00ff00")
    inside = basemap(style, clip=utm, width=100, height=100)
    assert inside.crs.to_epsg() == 32612 and inside.provenance["extent"] == "given", "an inside clip frames the map"
    img = inside.render().to_numpy()
    assert img[50, 50, 3] == 255
    outside = basemap(style, bounds=utm.buffer(10_000), clip=basemapper.outside(utm), width=100, height=100)
    o = outside.render().to_numpy()
    assert o[50, 50, 3] == 0 and o[1, 1, 3] == 255, "outside: a hole where the layer is"
    no_extent = basemap(style, clip=basemapper.outside(utm))
    assert no_extent.provenance["extent"] == "default" and no_extent.crs.to_epsg() == 3857, "outside() sets no extent or CRS"


def test_clips_need_polygons(gpd):
    from shapely.geometry import Point

    points = gpd.GeoDataFrame(geometry=[Point(-111.9, 40.7)], crs=4326)
    with pytest.raises(ValidationError, match="buffer"):
        basemap(square_style(0, 0, 1, 1), bounds=[-112, 40, -111, 41], clip=points)
    with pytest.raises(ValidationError, match="no CRS"):
        basemapper.outside(gpd.GeoDataFrame(geometry=[Point(0, 0).buffer(1)]))
