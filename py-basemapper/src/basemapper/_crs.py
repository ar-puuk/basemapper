"""Coordinates and CRSs, the Python side (docs/design/api-review.md §4.2,
D18, D19): what needs pyproj or GeoPandas, and nothing else. The view's
arithmetic is the core's.

- Plain numbers are longitude / latitude; spatial objects (GeoDataFrame,
  GeoSeries, shapely geometries with a ``crs``, :func:`around`) bring their
  CRS.
- The map's CRS is ``crs`` if given, else a spatial extent layer's when that
  CRS is projected, else Web Mercator.
- A map in another CRS: the core needs a frame (a point, its Web Mercator
  position, the local scale) and a mesh of the image's pixels to Web
  Mercator; both are computed here with pyproj.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Sequence, Tuple

import numpy as np

from .exceptions import ValidationError

WEB_MERCATOR = "EPSG:3857"
LONLAT = "EPSG:4326"


def _pyproj():
    import pyproj

    return pyproj


def crs_of(value: Any):
    """A ``pyproj.CRS`` from anything naming one: a code, a string, a
    ``pyproj.CRS``, or an object with a ``crs`` (GeoDataFrame, ...)."""
    pyproj = _pyproj()
    if value is None:
        return None
    inner = getattr(value, "crs", None)
    if inner is not None and not isinstance(value, (str, int)):
        value = inner
    try:
        return pyproj.CRS.from_user_input(value)
    except Exception as e:  # pyproj raises CRSError
        raise ValidationError(f"not a CRS: {value!r} ({e})") from None


def is_web_mercator(crs) -> bool:
    return crs is not None and crs.to_epsg() == 3857


def is_lonlat(crs) -> bool:
    """WGS 84 longitude / latitude, in either axis order."""
    pyproj = _pyproj()
    return crs is not None and crs.equals(pyproj.CRS.from_epsg(4326), ignore_axis_order=True)


def crs_name(crs) -> str:
    """A short name for a CRS: ``"EPSG:32612"``, else its PROJ string."""
    code = crs.to_epsg()
    if code is not None:
        return f"EPSG:{code}"
    return crs.to_string()


# ── Extents ──────────────────────────────────────────────────────────────────


@dataclass(frozen=True)
class Around:
    """An area of a ground distance around a point: see :func:`around`."""

    center: Tuple[float, float]
    span: Tuple[float, float]
    units: str
    bounds: Tuple[float, float, float, float]

    def __repr__(self) -> str:
        return f"around(({self.center[0]:g}, {self.center[1]:g}), ({self.span[0]:g}, {self.span[1]:g}), units={self.units!r})"


def around(center: Sequence[float], span: Any, units: str = "m") -> Around:
    """An area of a true ground distance around a point, for ``bounds``.

    Computed on the sphere (half the span each way), so it needs no
    projection library, and accurate to well under 1% for spans up to a few
    hundred kilometres.

    Args:
        center: ``(longitude, latitude)``.
        span: ``(width, height)`` on the ground, or one number for a square.
        units: The unit of ``span``: ``"m"`` (default), ``"km"``, ``"mi"`` or
            ``"ft"``.

    Returns:
        An area in longitude / latitude, for ``basemap(bounds=...)``.

    Example:
        >>> import basemapper
        >>> m = basemapper.basemap("OpenFreeMap.Liberty", bounds=basemapper.around((-111.9, 40.76), (20, 15), units="km"))
    """
    from . import _basemapper as _core

    c = _pair(center, "center")
    s = (float(span), float(span)) if isinstance(span, (int, float)) else _pair(span, "span")
    return Around(c, s, units, tuple(_core.around(c, s, units)))


@dataclass(frozen=True)
class Outside:
    """The area outside a layer's polygons, for ``clip``: see :func:`outside`."""

    layer: Any

    def __repr__(self) -> str:
        return f"outside({type(self.layer).__name__})"


def outside(layer: Any) -> Outside:
    """The area outside a layer's polygons, for ``clip``: the basemap
    everywhere except inside them.

    ``outside()`` only marks the layer; the polygon-only rule applies as for
    ``clip = layer``. Unlike an inside clip, it sets no default extent or CRS.

    Args:
        layer: Polygons: a GeoDataFrame, GeoSeries, or shapely geometry with a
            ``crs`` attribute.

    Example:
        >>> m = basemapper.basemap("OpenFreeMap.Liberty", bounds=county, clip=basemapper.outside(lakes))
    """
    if isinstance(layer, Outside):
        return layer
    _spatial_crs(layer, "outside()")
    return Outside(layer)


def _pair(value: Any, name: str) -> Tuple[float, float]:
    try:
        a, b = value
        a, b = float(a), float(b)
    except (TypeError, ValueError):
        raise ValidationError(f"{name} must be two numbers, not {value!r}") from None
    if not (math.isfinite(a) and math.isfinite(b)):
        raise ValidationError(f"{name} must be finite, not {value!r}")
    return a, b


def is_spatial(value: Any) -> bool:
    """Whether ``value`` is a spatial object with bounds and a CRS."""
    return hasattr(value, "total_bounds") or (hasattr(value, "bounds") and hasattr(value, "crs") and not isinstance(value, Around))


def _spatial_crs(value: Any, what: str):
    crs = getattr(value, "crs", None)
    if crs is None:
        raise ValidationError(f"{what}: the layer has no CRS; set one (e.g. gdf.set_crs(...))")
    return crs_of(crs)


@dataclass(frozen=True)
class Extent:
    """An extent as given: a box in its CRS, and whether a spatial layer
    gave it (only those infer the map's CRS)."""

    box: Tuple[float, float, float, float]
    crs: Any
    from_layer: bool


def extent_of(value: Any) -> Extent:
    """``bounds`` as an :class:`Extent`: numbers are lon/lat (west > east
    across the date line), spatial objects and :func:`around` bring their CRS."""
    if isinstance(value, Around):
        return Extent(value.bounds, crs_of(LONLAT), False)
    if hasattr(value, "total_bounds"):
        b = tuple(float(v) for v in value.total_bounds)
        return Extent(b, _spatial_crs(value, "bounds"), True)
    if hasattr(value, "bounds") and hasattr(value, "crs") and not isinstance(value, (list, tuple, dict)):
        b = tuple(float(v) for v in value.bounds)
        return Extent(b, _spatial_crs(value, "bounds"), True)
    try:
        b = tuple(float(v) for v in value)
    except (TypeError, ValueError):
        raise ValidationError(
            "bounds must be (west, south, east, north) in longitude / latitude, around(...), or a spatial layer "
            f"(GeoDataFrame, GeoSeries); not {type(value).__name__}"
        ) from None
    if len(b) != 4:
        raise ValidationError(f"bounds must have 4 numbers (west, south, east, north), not {len(b)}")
    if not all(math.isfinite(v) for v in b):
        raise ValidationError("bounds must be finite")
    if not (-90 <= b[1] < b[3] <= 90):
        raise ValidationError(
            "bounds numbers are longitude / latitude: south < north, within -90 and 90. "
            "For projected coordinates, give a spatial layer (it carries its CRS)"
        )
    return Extent(b, crs_of(LONLAT), False)


def map_crs(crs: Any, extent: Optional[Extent]):
    """The map's CRS (D18, D19): ``crs`` if given; else the extent layer's
    when projected; else Web Mercator."""
    if crs is not None:
        return crs_of(crs)
    if extent is not None and extent.from_layer and extent.crs.is_projected:
        return extent.crs
    return crs_of(WEB_MERCATOR)


def _transformer(src, dst):
    pyproj = _pyproj()
    return pyproj.Transformer.from_crs(src, dst, always_xy=True)


def box_in(extent: Extent, crs) -> Tuple[float, float, float, float]:
    """``extent``'s box in ``crs``: corner by corner for lon/lat numbers into
    Web Mercator (keeping west > east across the date line), else the
    envelope of the densified edges."""
    if extent.crs.equals(crs):
        return extent.box
    t = _transformer(extent.crs, crs)
    if is_lonlat(extent.crs) and is_web_mercator(crs):
        x0, y0 = t.transform(extent.box[0], extent.box[1])
        x1, y1 = t.transform(extent.box[2], extent.box[3])
        return (x0, y0, x1, y1)
    return tuple(t.transform_bounds(*extent.box, densify_pts=21))


def lonlat_to_3857(lon: float, lat: float) -> Tuple[float, float]:
    return _transformer(crs_of(LONLAT), crs_of(WEB_MERCATOR)).transform(lon, lat)


def mercator_to_lonlat(x: float, y: float) -> Tuple[float, float]:
    return _transformer(crs_of(WEB_MERCATOR), crs_of(LONLAT)).transform(x, y)


# ── A map in another CRS ─────────────────────────────────────────────────────


def frame(crs, origin: Tuple[float, float]) -> Dict[str, Any]:
    """The core's frame for ``crs`` at ``origin`` (in ``crs``): its Web
    Mercator position and the Web Mercator metres per CRS unit there (the
    geometric mean of the x and y scales)."""
    t = _transformer(crs, crs_of(WEB_MERCATOR))
    ox, oy = origin
    d = 0.01 if crs.is_geographic else 100.0
    m = t.transform(ox, oy)
    xa, xb = t.transform(ox - d, oy), t.transform(ox + d, oy)
    ya, yb = t.transform(ox, oy - d), t.transform(ox, oy + d)
    sx = math.dist(xa, xb) / (2 * d)
    sy = math.dist(ya, yb) / (2 * d)
    scale = math.sqrt(sx * sy)
    if not (all(math.isfinite(v) for v in m) and math.isfinite(scale) and scale > 0):
        raise ValidationError(f"the map's centre has no position in Web Mercator in {crs_name(crs)}")
    return {"name": crs_name(crs), "origin": (ox, oy), "origin_3857": m, "scale": scale}


def mesh(crs, bounds: Sequence[float], width: int, height: int) -> Tuple[int, int, List[Tuple[float, float]]]:
    """The mesh of a ``width x height`` image of ``bounds`` (in ``crs``) to
    Web Mercator: a node every 32 px (at least 2, at most 129 a side), NaN
    where a point has no Web Mercator position."""
    nx = min(max(width // 32 + 2, 2), 129)
    ny = min(max(height // 32 + 2, 2), 129)
    xmin, ymin, xmax, ymax = bounds
    px = np.linspace(0, width, nx)
    py = np.linspace(0, height, ny)
    gx, gy = np.meshgrid(xmin + px / width * (xmax - xmin), ymax - py / height * (ymax - ymin))
    t = _transformer(crs, crs_of(WEB_MERCATOR))
    with np.errstate(all="ignore"):
        mx, my = t.transform(gx.ravel(), gy.ravel(), errcheck=False)
    mx, my = np.asarray(mx, dtype=float), np.asarray(my, dtype=float)
    bad = ~(np.isfinite(mx) & np.isfinite(my))
    mx[bad] = np.nan
    my[bad] = np.nan
    return nx, ny, list(zip(mx.tolist(), my.tolist()))


# ── Clips ────────────────────────────────────────────────────────────────────


def clip_rings(layer: Any, crs) -> List[List[List[Tuple[float, float]]]]:
    """A polygon layer's polygons in ``crs``, each its exterior and holes as
    rings. Points and lines are an error: buffer them (the distance is yours)."""
    if isinstance(layer, Outside):
        layer = layer.layer
    src = _spatial_crs(layer, "clip")
    geoms = _geometries(layer)
    if not crs.equals(src):
        geoms = _reproject(geoms, src, crs)
    polygons: List[List[List[Tuple[float, float]]]] = []
    for g in geoms:
        if g is None or g.is_empty:
            continue
        parts = list(getattr(g, "geoms", [g]))
        for p in parts:
            if p.geom_type != "Polygon":
                raise ValidationError(
                    f"clip needs polygons, not {p.geom_type}: buffer points or lines first "
                    "(GeoSeries.buffer(distance), in a projected CRS)"
                )
            rings = [list(p.exterior.coords)] + [list(r.coords) for r in p.interiors]
            polygons.append([[(float(x), float(y)) for x, y, *_ in r] for r in rings])
    if not polygons:
        raise ValidationError("clip: the layer has no polygons")
    return polygons


def _geometries(layer: Any) -> list:
    if hasattr(layer, "geometry") and hasattr(layer.geometry, "__iter__") and hasattr(layer, "total_bounds"):
        return list(layer.geometry)
    if hasattr(layer, "total_bounds"):
        return list(layer)
    return [layer]


def _reproject(geoms: list, src, dst) -> list:
    """``geoms`` from ``src`` to ``dst``, every coordinate at once (shapely 2)."""
    import shapely

    t = _transformer(src, dst)

    def project(xy: np.ndarray) -> np.ndarray:
        x, y = t.transform(xy[:, 0], xy[:, 1])
        return np.column_stack([x, y])

    return [None if g is None else shapely.transform(g, project) for g in geoms]


# ── Plot panels ──────────────────────────────────────────────────────────────


def panel_spec(limits: Sequence[float], crs, px: Tuple[int, int], device: Dict[str, Any]):
    """The core's panel for a plot, and its mesh when the core cannot build
    it (a CRS other than Web Mercator and lon/lat)."""
    w, h = px
    if is_web_mercator(crs):
        name, m = WEB_MERCATOR, None
    elif is_lonlat(crs):
        name, m = LONLAT, None
    else:
        name, m = crs_name(crs), mesh(crs, limits, w, h)
    return {"limits": list(limits), "crs": name, "px": (w, h), "device": device}, m
