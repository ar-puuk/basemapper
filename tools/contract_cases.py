"""Write the view-resolution contract cases (core/tests/fixtures/contract/).

The cases pin the rules of docs/design/api-review.md §4.2 (the relation
between extent, zoom, printed size and symbol scale, with dpi turning the
printed size into pixels; the default order; conflicts; plots) and §4.2's
around(). The Rust, R and Python test suites all run
them, so the three languages resolve every view the same way.

Expected values come from the design's formulas, computed here
independently of the implementation they test:

    metres per CSS px at zoom z:  C / (512 * 2^z),  C = 2 * pi * 6378137
    CSS px = printed inches * 96 / symbol_scale = image px * 96 / (dpi * symbol_scale)
    the view is constrained: zoom >= log2(max(css w, css h) / 512)

Usage:  python tools/contract_cases.py   (rewrites cases.json)
"""

import json
import math
import pathlib

R = 6378137.0
C = 2 * math.pi * R
HALF = C / 2
SPHERE = 6371008.8  # mean Earth radius, for around()'s ground distances

OUT = pathlib.Path(__file__).resolve().parent.parent / "core/tests/fixtures/contract/cases.json"


def merc(lon, lat):
    return lon * math.pi / 180 * R, math.log(math.tan(math.pi / 4 + lat * math.pi / 360)) * R


def mpp(z):
    return C / (512 * 2**z)


def zoom_for(mpp_needed):
    return math.log2(C / (512 * mpp_needed))


def bounds_3857(lonlat):
    x0, y0 = merc(lonlat[0], lonlat[1])
    x1, y1 = merc(lonlat[2], lonlat[3])
    return [x0, y0, x1, y1]


def width_m(b):
    """Mercator width of a 3857 box, across the date line when xmin > xmax."""
    return b[2] - b[0] if b[2] >= b[0] else (HALF - b[0]) + (b[2] + HALF)


def fit(b, css_w, css_h):
    """Zoom at which box b fits a css_w x css_h view, and the view's box (the
    extent grows on one side to the view's shape), constrained."""
    m = max(width_m(b) / css_w, (b[3] - b[1]) / css_h)
    z = max(zoom_for(m), math.log2(max(css_w, css_h) / 512))
    return z, around_center(b, css_w * mpp(z), css_h * mpp(z))


def around_center(b, w, h):
    cx = (b[0] + b[2]) / 2 if b[2] >= b[0] else ((b[0] + b[2] + C) / 2 + HALF) % C - HALF
    cy = (b[1] + b[3]) / 2
    return [cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2]


SLC = [-112.1, 40.6, -111.7, 40.9]  # a Salt Lake Valley box, lon / lat
SLC_M = bounds_3857(SLC)
CENTER = [-111.89, 40.76]

cases = []


def case(name, doc, inputs, expect=None, error=None, context=None, tol=None):
    c = {"name": name, "doc": doc, "inputs": inputs}
    if context is not None:
        c["context"] = context
    if error is not None:
        c["error"] = error
    else:
        # The symbol scale is 1 unless a case gives or derives another.
        expect.setdefault("symbol_scale", 1.0)
        c["expect"] = expect
    c["tolerance"] = tol or {"zoom": 1e-9, "dpi": 1e-6, "px": 1, "metres": 1e-3}
    cases.append(c)


# ── On its own ───────────────────────────────────────────────────────────────

z, b = fit([-HALF, -HALF, HALF, HALF], 800, 600)
case("nothing_given", "§4.2 Defaults: the whole world, 800 x 600 px at 96 dpi, zoom derived (and constrained)",
     {}, {"width": 800, "height": 600, "dpi": 96, "zoom": z, "bounds_3857": [-HALF, b[1], HALF, b[3]],
          "provenance": {"extent": "default", "size": "default", "dpi": "default", "zoom": "derived"},
          "notes": []})

z2, b2 = fit(SLC_M, 800, 600)
case("bounds_only", "§4.2: extent + image size + dpi give the zoom (automatic zoom); size and dpi default",
     {"bounds_lonlat": SLC}, {"width": 800, "height": 600, "dpi": 96, "zoom": z2, "bounds_3857": b2,
                              "provenance": {"extent": "given", "size": "default", "dpi": "default", "zoom": "derived"},
                              "notes": []})

w12 = round(width_m(SLC_M) / mpp(12))
h12 = round((SLC_M[3] - SLC_M[1]) / mpp(12))
case("bounds_and_zoom", "§4.2: extent + zoom + dpi give the image size: this area at this detail",
     {"bounds_lonlat": SLC, "zoom": 12}, {"width": w12, "height": h12, "dpi": 96, "zoom": 12, "bounds_3857": SLC_M,
                                         "provenance": {"extent": "given", "size": "derived", "dpi": "default", "zoom": "given"},
                                         "notes": []})

bcss_w, bcss_h = width_m(SLC_M) / mpp(12), (SLC_M[3] - SLC_M[1]) / mpp(12)
dpi_d = 96 * min(800 / bcss_w, 600 / bcss_h)
case("bounds_zoom_and_size", "§4.2: extent + zoom + image size give the dpi",
     {"bounds_lonlat": SLC, "zoom": 12, "width": 800, "height": 600},
     {"width": 800, "height": 600, "dpi": dpi_d, "zoom": 12,
      "bounds_3857": around_center(SLC_M, 800 * 96 / dpi_d * mpp(12), 600 * 96 / dpi_d * mpp(12)),
      "provenance": {"extent": "given", "size": "given", "dpi": "derived", "zoom": "given"},
      "notes": []})

# Physical units: the size fixes the map's layout (CSS px = inches x 96), so
# dpi only turns it into pixels; extent + zoom + a physical size is already
# over-determined.
zin, bin_ = fit(SLC_M, 6 * 96, 4 * 96)
case("inches_dpi_is_only_sharpness", "§4.2: with width / height in inches, dpi changes only the pixel count",
     {"bounds_lonlat": SLC, "width": 6, "height": 4, "units": "in", "dpi": 300},
     {"width": 1800, "height": 1200, "dpi": 300, "zoom": zin, "bounds_3857": bin_,
      "provenance": {"extent": "given", "size": "given", "dpi": "given", "zoom": "derived"}, "notes": []})
case("inches_at_default_dpi", "§4.2: the same layout at the default dpi: the same zoom, fewer pixels",
     {"bounds_lonlat": SLC, "width": 6, "height": 4, "units": "in"},
     {"width": 576, "height": 384, "dpi": 96, "zoom": zin, "bounds_3857": bin_,
      "provenance": {"extent": "given", "size": "given", "dpi": "default", "zoom": "derived"}, "notes": []})
# Extent + zoom + a printed size: the symbol scale follows (as in a plot).
s_in = 1 / min(6 * 96 / bcss_w, 4 * 96 / bcss_h)
case("inches_with_zoom_derive_symbol_scale", "§4.2: extent + zoom + a printed size give the symbol scale (a decided note); dpi only sets the pixels",
     {"bounds_lonlat": SLC, "zoom": 12, "width": 6, "height": 4, "units": "in"},
     {"width": 576, "height": 384, "dpi": 96, "zoom": 12, "symbol_scale": s_in,
      "bounds_3857": around_center(SLC_M, 6 * 96 / s_in * mpp(12), 4 * 96 / s_in * mpp(12)),
      "provenance": {"extent": "given", "size": "given", "dpi": "default", "zoom": "given"},
      "notes": [{"level": "decided", "kind": "symbol_scale"}]})

# Pixels at a dpi are a printed size: with extent + zoom, the symbol scale follows.
s_px = 1 / min(800 / bcss_w, 600 / bcss_h)
case("all_four_given_derive_symbol_scale", "§4.2: extent + zoom + pixels at a dpi (a printed size) give the symbol scale",
     {"bounds_lonlat": SLC, "zoom": 12, "width": 800, "height": 600, "dpi": 96},
     {"width": 800, "height": 600, "dpi": 96, "zoom": 12, "symbol_scale": s_px,
      "bounds_3857": around_center(SLC_M, 800 / s_px * mpp(12), 600 / s_px * mpp(12)),
      "provenance": {"extent": "given", "size": "given", "dpi": "given", "zoom": "given"},
      "notes": [{"level": "decided", "kind": "symbol_scale"}]})

case("all_five_given", "§4.2 Conflicts: extent, zoom, pixels at a dpi and a symbol scale over-determine the map",
     {"bounds_lonlat": SLC, "zoom": 12, "width": 800, "height": 600, "dpi": 96, "symbol_scale": 1},
     error={"kind": "validation", "contains": ["over-determine", "symbol_scale"]})

# The symbol scale on its own: dpi stays the sharpness.
zs, bs = fit(SLC_M, 800 / 0.5, 600 / 0.5)
case("symbol_scale_given", "§4.2: symbol_scale 0.5 halves lines and labels, so the same 800 x 600 px at 96 dpi fits twice the CSS px: more detail",
     {"bounds_lonlat": SLC, "width": 800, "height": 600, "symbol_scale": 0.5},
     {"width": 800, "height": 600, "dpi": 96, "zoom": zs, "symbol_scale": 0.5, "bounds_3857": bs,
      "provenance": {"extent": "given", "size": "given", "dpi": "default", "zoom": "derived"}, "notes": []})

zp6, bp6 = fit(SLC_M, 6 * 96 / 0.8, 4 * 96 / 0.8)
for d in (150, 300):
    case(f"printed_size_dpi_{d}", "§4.2: with a printed size and a symbol scale, dpi changes only the pixel count: the same zoom at any dpi",
         {"bounds_lonlat": SLC, "width": 6, "height": 4, "units": "in", "dpi": d, "symbol_scale": 0.8},
         {"width": 6 * d, "height": 4 * d, "dpi": d, "zoom": zp6, "symbol_scale": 0.8, "bounds_3857": bp6,
          "provenance": {"extent": "given", "size": "given", "dpi": "given", "zoom": "derived"}, "notes": []})

# 0.3's renders are unchanged: symbol_scale 1 is what 0.3 drew.
z03, b03 = fit(SLC_M, 1600 * 96 / 192, 1200 * 96 / 192)
for name, given in (("pixels_at_dpi_as_in_0_3", {}), ("pixels_at_dpi_symbol_scale_1", {"symbol_scale": 1})):
    case(name, "§4.2: 1600 x 1200 px at 192 dpi resolves as in 0.3 whether symbol_scale is left out or 1",
         {"bounds_lonlat": SLC, "width": 1600, "height": 1200, "dpi": 192, **given},
         {"width": 1600, "height": 1200, "dpi": 192, "zoom": z03, "bounds_3857": b03,
          "provenance": {"extent": "given", "size": "given", "dpi": "given", "zoom": "derived"}, "notes": []})

case("default_size_scales_with_dpi", "§4.2 Defaults: the default image size is 800 x 600 at 96 dpi, scaled with the dpi",
     {"bounds_lonlat": SLC, "dpi": 300}, {"width": 2500, "height": 1875, "dpi": 300, "zoom": z2, "bounds_3857": b2,
                                         "provenance": {"extent": "given", "size": "default", "dpi": "given", "zoom": "derived"},
                                         "notes": []})

z10, b10 = fit(SLC_M, 800 * 96 / 192, 600 * 96 / 192)
case("pixels_fixed_dpi_changes_zoom", "§4.2: with the image size fixed in pixels and no zoom, dpi decides how much detail fits",
     {"bounds_lonlat": SLC, "width": 800, "height": 600, "dpi": 192},
     {"width": 800, "height": 600, "dpi": 192, "zoom": z10, "bounds_3857": b10,
      "provenance": {"extent": "given", "size": "given", "dpi": "given", "zoom": "derived"},
      "notes": []})

cx, cy = merc(*CENTER)
case("camera", "§4.2: center + zoom + image size + dpi give the area shown (the MapLibre camera)",
     {"center": CENTER, "zoom": 12},
     {"width": 800, "height": 600, "dpi": 96, "zoom": 12,
      "bounds_3857": [cx - 400 * mpp(12), cy - 300 * mpp(12), cx + 400 * mpp(12), cy + 300 * mpp(12)],
      "provenance": {"extent": "derived", "size": "default", "dpi": "default", "zoom": "given"},
      "notes": []})

case("center_without_zoom", "§4.2 Conflicts: a centre says neither how much to show nor how detailed",
     {"center": CENTER}, error={"kind": "validation", "contains": ["zoom", "around"]})

case("bounds_and_center", "§4.2 Conflicts: two extents",
     {"bounds_lonlat": SLC, "center": CENTER, "zoom": 12}, error={"kind": "validation", "contains": ["bounds", "center"]})

# Near a pole: the requested centre moves so the view stops at the pole.
zc = 3
half_h = 300 * mpp(zc)
case("camera_near_pole", "§4.2 / constrain_view: the view stops at the pole; an adjusted note",
     {"center": [10, 84], "zoom": zc},
     {"width": 800, "height": 600, "dpi": 96, "zoom": zc,
      "bounds_3857": [merc(10, 0)[0] - 400 * mpp(zc), HALF - 2 * half_h, merc(10, 0)[0] + 400 * mpp(zc), HALF],
      "provenance": {"extent": "derived", "size": "default", "dpi": "default", "zoom": "given"},
      "notes": [{"level": "adjusted", "kind": "view_constrained"}]})

DL = [170, -10, -170, 10]
DL_M = bounds_3857(DL)
zd, bd = fit(DL_M, 800, 600)
case("date_line_bounds", "F4: a box crossing the date line (xmin > xmax) is a valid extent",
     {"bounds_lonlat": DL}, {"width": 800, "height": 600, "dpi": 96, "zoom": zd,
                             "bounds_3857": [bd[0] if bd[0] >= -HALF else bd[0] + C, bd[1],
                                             bd[2] if bd[2] <= HALF else bd[2] - C, bd[3]],
                             "provenance": {"extent": "given", "size": "default", "dpi": "default", "zoom": "derived"},
                             "notes": []})

# ── In a plot ────────────────────────────────────────────────────────────────

# A 6 x 4 inch panel over the SLC box's width (the plot's extent already has
# the panel's shape).
ext = [SLC_M[0], 0.0, SLC_M[2], 0.0]
ph = width_m(SLC_M) * 4 / 6
mid = (SLC_M[1] + SLC_M[3]) / 2
ext[1], ext[3] = mid - ph / 2, mid + ph / 2
zp = zoom_for(width_m(SLC_M) / (6 * 96))
plot = lambda px, dpi, kind, ratio=1: {"plot": {"extent_3857": ext, "panel_px": px, "device": kind, "dpi": dpi, "pixel_ratio": ratio}}

case("plot_raster_export", "§4.2 In a plot: extent and size from the plot, the device's dpi, zoom derived",
     {}, {"width": 1800, "height": 1200, "dpi": 300, "zoom": zp, "bounds_3857": ext,
          "provenance": {"extent": "context", "size": "context", "dpi": "context", "zoom": "derived"}, "notes": []},
     context=plot([1800, 1200], 300, "raster_export"))

case("plot_screen_hidpi", "§4.2 In a plot (D16): on a screen 96 x its pixel ratio, so the preview derives the export's zoom",
     {}, {"width": 1152, "height": 768, "dpi": 192, "zoom": zp, "bounds_3857": ext,
          "provenance": {"extent": "context", "size": "context", "dpi": "context", "zoom": "derived"}, "notes": []},
     context=plot([1152, 768], None, "screen", 2))

case("plot_vector_export", "§4.2 In a plot (D16): PDF / SVG have no dpi, 300 is the default",
     {}, {"width": 1800, "height": 1200, "dpi": 300, "zoom": zp, "bounds_3857": ext,
          "provenance": {"extent": "context", "size": "context", "dpi": "default", "zoom": "derived"},
          "notes": [{"level": "adjusted", "kind": "default_dpi"}]},
     context=plot([1800, 1200], None, "vector_export"))

case("plot_zoom_given", "§4.2 In a plot: a zoom sets what is drawn, and so the size of lines and labels (a decided note); the device keeps its dpi",
     {"zoom": zp + 1}, {"width": 1800, "height": 1200, "dpi": 300, "zoom": zp + 1, "symbol_scale": 0.5, "bounds_3857": ext,
                        "provenance": {"extent": "context", "size": "context", "dpi": "context", "zoom": "given"},
                        "notes": [{"level": "decided", "kind": "symbol_scale"}]},
     context=plot([1800, 1200], 300, "raster_export"))

case("plot_symbol_scale_given", "§4.2 In a plot: symbol_scale 0.5 draws lines and labels at half their designed size, so twice the CSS pixels and one zoom more, at the device's dpi",
     {"symbol_scale": 0.5}, {"width": 1800, "height": 1200, "dpi": 300, "zoom": zp + 1, "symbol_scale": 0.5, "bounds_3857": ext,
                      "provenance": {"extent": "context", "size": "context", "dpi": "context", "zoom": "derived"}, "notes": []},
     context=plot([1800, 1200], 300, "raster_export"))

case("plot_dpi_given", "§4.2 Conflicts: in a plot the device sets the dpi; a basemap's dpi is an error naming symbol_scale",
     {"dpi": 150}, error={"kind": "validation", "contains": ["device sets the dpi", "symbol_scale"]},
     context=plot([1800, 1200], 300, "raster_export"))

case("plot_screen_plain", "§4.2 In a plot (D16): a screen without a pixel ratio is 96 dpi",
     {}, {"width": 576, "height": 384, "dpi": 96, "zoom": zp, "bounds_3857": ext,
          "provenance": {"extent": "context", "size": "context", "dpi": "context", "zoom": "derived"}, "notes": []},
     context=plot([576, 384], None, "screen"))

case("plot_zoom_and_symbol_scale", "§4.2 Conflicts: in a plot the extent and printed size are fixed, so zoom and symbol_scale each set the other",
     {"zoom": 12, "symbol_scale": 0.8}, error={"kind": "validation", "contains": ["zoom", "symbol_scale"]},
     context=plot([1800, 1200], 300, "raster_export"))

case("plot_with_size", "§4.2 Conflicts: in a plot the panel gives the image size",
     {"width": 800, "height": 600}, error={"kind": "validation", "contains": ["width", "plot"]},
     context=plot([1800, 1200], 300, "raster_export"))

case("plot_with_bounds", "§4.2 Conflicts / D15: a basemap with its own extent in a plot",
     {"bounds_lonlat": SLC}, error={"kind": "validation", "contains": ["plot"]},
     context=plot([1800, 1200], 300, "raster_export"))

# ── around() ─────────────────────────────────────────────────────────────────

arounds = []
for name, center, span, units, metres in [
    ("km", CENTER, [20, 15], "km", [20000, 15000]),
    ("metres_default", CENTER, [20000, 15000], None, [20000, 15000]),
    ("square_miles", CENTER, [5], "mi", [5 * 1609.344, 5 * 1609.344]),
]:
    dlat = math.degrees(metres[1] / 2 / SPHERE)
    dlon = math.degrees(metres[0] / 2 / (SPHERE * math.cos(math.radians(center[1]))))
    a = {"name": name, "center": center, "span": span}
    if units:
        a["units"] = units
    a["expect_lonlat"] = [center[0] - dlon, center[1] - dlat, center[0] + dlon, center[1] + dlat]
    arounds.append(a)

OUT.parent.mkdir(parents=True, exist_ok=True)
OUT.write_text(
    json.dumps({"version": 1, "about": __doc__.strip().splitlines()[0], "views": cases, "around": arounds}, indent=2) + "\n",
    encoding="utf-8",
)
print(f"wrote {OUT} ({len(cases)} view cases, {len(arounds)} around() cases)")
