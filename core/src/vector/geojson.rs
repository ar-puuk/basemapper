//! GeoJSON sources, tiled as MapLibre does with geojson-vt: features are
//! projected to the unit Mercator square and simplified once (Douglas-Peucker
//! importance per vertex), world copies wrap at ±180°, and each tile is the
//! features clipped to the tile plus its buffer, with vertices kept where
//! their importance exceeds the tile zoom's tolerance, rounded to tile
//! coordinates. Tiles are encoded as MVT (one layer, `_geojsonTileLayer`, as
//! MapLibre's GeoJSONWrapper names it) so the vector pipeline renders them
//! like any other source.
//!
//! MapLibre's options (geojson_worker_source): extent 8192, buffer and
//! tolerance scaled by 8192 / 512 (defaults 128 and 0.375), maxzoom 18.
//! Clipping straight to a tile gives the same geometry as geojson-vt's
//! recursive splitting: every parent's clip lines lie outside the child's
//! buffered region.

use super::cluster::{js_round, ClusterIndex, ClusterOptions};
use super::mvt::{PbFeature, PbLayer, PbTile, PbValue, GEOJSON_LAYER};
use prost::Message;
use serde_json::Value as Json;

const EXTENT: f64 = 8192.0;

/// geojson-vt's lineMetrics feature properties (read by LineBucket).
pub const CLIP_START: &str = "geojsonvt_clip_start";
pub const CLIP_END: &str = "geojsonvt_clip_end";

#[derive(Clone, Debug)]
pub(super) struct Line {
    pts: Vec<[f64; 3]>,
    /// Area (polygon rings) or length (lines), in world units.
    size: f64,
    /// lineMetrics: where this slice starts and ends along the whole line
    /// (world units; 0 and `size` for an unclipped line).
    start: f64,
    end: f64,
}

#[derive(Clone, Debug)]
pub(super) enum Geom {
    Points(Vec<[f64; 3]>),
    Lines(Vec<Line>),
    /// Polygons, each a list of rings (outer first).
    Polygons(Vec<Vec<Line>>),
}

#[derive(Clone, Debug)]
pub(super) enum Prop {
    Str(String),
    Num(f64),
    Bool(bool),
}

#[derive(Clone, Debug)]
pub(super) struct Feat {
    pub(super) id: Option<u64>,
    pub(super) geom: Geom,
    pub(super) props: Vec<(String, Prop)>,
    min: (f64, f64),
    max: (f64, f64),
}

impl Feat {
    pub(super) fn point(id: Option<u64>, p: [f64; 2], props: Vec<(String, Prop)>) -> Feat {
        make_feat(id, Geom::Points(vec![[p[0], p[1], 0.0]]), props)
    }
}

/// Options of a GeoJSON source, in geojson-vt units.
#[derive(Clone, Debug)]
pub struct GeoJsonOptions {
    pub max_zoom: u8,
    /// Buffer in tile units (MapLibre: buffer px * 16).
    pub buffer: f64,
    /// Simplification tolerance in tile units (MapLibre: tolerance * 16).
    pub tolerance: f64,
    /// `lineMetrics`: lines carry their clip range for `line-gradient`.
    pub line_metrics: bool,
    /// `cluster: true`: points are clustered (other geometries dropped).
    pub cluster: Option<ClusterOptions>,
}

impl GeoJsonOptions {
    /// From a style source (`maxzoom`, `buffer`, `tolerance`).
    pub fn from_source(src: &Json) -> Self {
        let scale = EXTENT / 512.0;
        let max_zoom = src["maxzoom"].as_u64().map_or(18, |z| z.min(24) as u8);
        GeoJsonOptions {
            max_zoom,
            buffer: src["buffer"].as_f64().unwrap_or(128.0) * scale,
            tolerance: src["tolerance"].as_f64().unwrap_or(0.375) * scale,
            line_metrics: src["lineMetrics"].as_bool().unwrap_or(false),
            cluster: ClusterOptions::from_source(src, max_zoom),
        }
    }
}

/// A GeoJSON source prepared for tiling.
pub struct GeoJsonIndex {
    features: Vec<Feat>,
    cluster: Option<ClusterIndex>,
    opts: GeoJsonOptions,
}

impl GeoJsonIndex {
    pub fn new(data: &Json, opts: GeoJsonOptions) -> Self {
        let sq_tol = (opts.tolerance / ((1u64 << opts.max_zoom) as f64 * EXTENT)).powi(2);
        let mut features = Vec::new();
        convert(data, &mut features, sq_tol, None, opts.line_metrics);
        let features = wrap(features, opts.buffer / EXTENT, opts.line_metrics);
        if let Some(c) = &opts.cluster {
            let cluster = Some(ClusterIndex::new(&features, c.clone()));
            return GeoJsonIndex { features: Vec::new(), cluster, opts };
        }
        GeoJsonIndex { features, cluster: None, opts }
    }

    /// The tile (z, x, y) encoded as MVT.
    pub fn tile(&self, z: u8, x: u32, y: u32) -> Vec<u8> {
        let z2 = (1u64 << z) as f64;
        if let Some(c) = &self.cluster {
            return encode_tile(&c.tile_features(z, x, y), z2, x as f64, y as f64, 0.0, false);
        }
        let k = self.opts.buffer / EXTENT;
        let (x0, x1) = ((x as f64 - k) / z2, (x as f64 + 1.0 + k) / z2);
        let (y0, y1) = ((y as f64 - k) / z2, (y as f64 + 1.0 + k) / z2);
        let m = self.opts.line_metrics;
        let clipped = clip(&self.features, x0, x1, 0, m);
        let clipped = clip(&clipped, y0, y1, 1, m);
        let tol = if z >= self.opts.max_zoom { 0.0 } else { self.opts.tolerance / (z2 * EXTENT) };
        encode_tile(&clipped, z2, x as f64, y as f64, tol, m)
    }
}

// ── convert.js ───────────────────────────────────────────────────────────────

fn project_x(lng: f64) -> f64 {
    lng / 360.0 + 0.5
}

fn project_y(lat: f64) -> f64 {
    let s = lat.to_radians().sin();
    let y = 0.5 - 0.25 * ((1.0 + s) / (1.0 - s)).ln() / std::f64::consts::PI;
    y.clamp(0.0, 1.0)
}

fn convert(data: &Json, out: &mut Vec<Feat>, sq_tol: f64, inherit: Option<(&Json, Option<u64>)>, metrics: bool) {
    match data["type"].as_str() {
        Some("FeatureCollection") => {
            for f in data["features"].as_array().map(|v| v.as_slice()).unwrap_or(&[]) {
                convert(f, out, sq_tol, None, metrics);
            }
        }
        Some("Feature") => {
            let id = feature_id(&data["id"]);
            convert_geometry(&data["geometry"], &data["properties"], id, out, sq_tol, metrics);
        }
        Some(_) => {
            let (props, id) = inherit.unwrap_or((&Json::Null, None));
            convert_geometry(data, props, id, out, sq_tol, metrics);
        }
        None => {}
    }
}

/// GeoJSONWrapper: numeric ids (or numeric strings) become feature ids.
fn feature_id(j: &Json) -> Option<u64> {
    match j {
        Json::Number(n) => n.as_u64().or_else(|| n.as_f64().filter(|v| *v >= 0.0).map(|v| v as u64)),
        Json::String(s) => s.trim().parse::<f64>().ok().filter(|v| *v >= 0.0).map(|v| v as u64),
        _ => None,
    }
}

fn props_of(p: &Json) -> Vec<(String, Prop)> {
    let Some(obj) = p.as_object() else { return Vec::new() };
    obj.iter()
        .filter_map(|(k, v)| {
            let v = match v {
                Json::String(s) => Prop::Str(s.clone()),
                Json::Number(n) => Prop::Num(n.as_f64()?),
                Json::Bool(b) => Prop::Bool(*b),
                Json::Null => return None,
                other => Prop::Str(other.to_string()),
            };
            Some((k.clone(), v))
        })
        .collect()
}

fn convert_geometry(g: &Json, props: &Json, id: Option<u64>, out: &mut Vec<Feat>, sq_tol: f64, metrics: bool) {
    let coords = &g["coordinates"];
    let point = |c: &Json| -> Option<[f64; 3]> {
        let a = c.as_array()?;
        Some([project_x(a.first()?.as_f64()?), project_y(a.get(1)?.as_f64()?), 0.0])
    };
    let line = |c: &Json, polygon: bool| -> Option<Line> {
        let pts: Vec<[f64; 3]> = c.as_array()?.iter().filter_map(point).collect();
        if pts.is_empty() {
            return None;
        }
        Some(convert_line(pts, sq_tol, polygon))
    };
    let geom = match g["type"].as_str() {
        Some("Point") => point(coords).map(|p| Geom::Points(vec![p])),
        Some("MultiPoint") => Some(Geom::Points(coords.as_array().map(|a| a.iter().filter_map(point).collect()).unwrap_or_default())),
        Some("LineString") => line(coords, false).map(|l| Geom::Lines(vec![l])),
        Some("MultiLineString") if metrics => {
            // Exploded into LineStrings so each tracks its own metrics.
            for c in coords.as_array().map(|v| v.as_slice()).unwrap_or(&[]) {
                if let Some(l) = line(c, false) {
                    out.push(make_feat(id, Geom::Lines(vec![l]), props_of(props)));
                }
            }
            None
        }
        Some("MultiLineString") => {
            Some(Geom::Lines(coords.as_array().map(|a| a.iter().filter_map(|c| line(c, false)).collect()).unwrap_or_default()))
        }
        Some("Polygon") => {
            let rings: Vec<Line> = coords.as_array().map(|a| a.iter().filter_map(|c| line(c, true)).collect()).unwrap_or_default();
            Some(Geom::Polygons(vec![rings]))
        }
        Some("MultiPolygon") => Some(Geom::Polygons(
            coords
                .as_array()
                .map(|a| a.iter().map(|p| p.as_array().map(|r| r.iter().filter_map(|c| line(c, true)).collect()).unwrap_or_default()).collect())
                .unwrap_or_default(),
        )),
        Some("GeometryCollection") => {
            for sub in g["geometries"].as_array().map(|v| v.as_slice()).unwrap_or(&[]) {
                convert_geometry(sub, props, id, out, sq_tol, metrics);
            }
            None
        }
        _ => None,
    };
    if let Some(geom) = geom {
        out.push(make_feat(id, geom, props_of(props)));
    }
}

fn make_feat(id: Option<u64>, geom: Geom, props: Vec<(String, Prop)>) -> Feat {
    let mut f = Feat { id, geom, props, min: (f64::INFINITY, f64::INFINITY), max: (f64::NEG_INFINITY, f64::NEG_INFINITY) };
    let mut extend = |p: &[f64; 3]| {
        f.min = (f.min.0.min(p[0]), f.min.1.min(p[1]));
        f.max = (f.max.0.max(p[0]), f.max.1.max(p[1]));
    };
    match &f.geom {
        Geom::Points(pts) => pts.iter().for_each(&mut extend),
        Geom::Lines(ls) => ls.iter().flat_map(|l| l.pts.iter()).for_each(&mut extend),
        Geom::Polygons(ps) => ps.iter().flatten().flat_map(|l| l.pts.iter()).for_each(&mut extend),
    }
    f
}

fn convert_line(mut pts: Vec<[f64; 3]>, sq_tol: f64, polygon: bool) -> Line {
    let mut size = 0.0;
    for j in 1..pts.len() {
        let (a, b) = (pts[j - 1], pts[j]);
        if polygon {
            size += (a[0] * b[1] - b[0] * a[1]) / 2.0;
        } else {
            size += ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        }
    }
    let last = pts.len() - 1;
    pts[0][2] = 1.0;
    simplify(&mut pts, 0, last, sq_tol);
    pts[last][2] = 1.0;
    Line { pts, size: size.abs(), start: 0.0, end: size.abs() }
}

/// simplify.js: Douglas-Peucker, storing each kept vertex's importance
/// (squared distance) in its third coordinate.
fn simplify(c: &mut [[f64; 3]], first: usize, last: usize, sq_tol: f64) {
    let mut max_sq = sq_tol;
    let mid = (last - first) / 2 + first;
    let mut min_pos_to_mid = last - first;
    let mut index = None;
    let (a, b) = (c[first], c[last]);
    for i in first + 1..last {
        let d = sq_seg_dist(c[i][0], c[i][1], a[0], a[1], b[0], b[1]);
        if d > max_sq {
            index = Some(i);
            max_sq = d;
        } else if d == max_sq {
            // Prefer a pivot near the middle (geojson-vt's degenerate-input workaround).
            let pos = i.abs_diff(mid);
            if pos < min_pos_to_mid {
                index = Some(i);
                min_pos_to_mid = pos;
            }
        }
    }
    if max_sq > sq_tol {
        let index = index.unwrap();
        if index - first > 1 {
            simplify(c, first, index, sq_tol);
        }
        c[index][2] = max_sq;
        if last - index > 1 {
            simplify(c, index, last, sq_tol);
        }
    }
}

fn sq_seg_dist(px: f64, py: f64, mut x: f64, mut y: f64, bx: f64, by: f64) -> f64 {
    let (mut dx, mut dy) = (bx - x, by - y);
    if dx != 0.0 || dy != 0.0 {
        let t = ((px - x) * dx + (py - y) * dy) / (dx * dx + dy * dy);
        if t > 1.0 {
            x = bx;
            y = by;
        } else if t > 0.0 {
            x += dx * t;
            y += dy * t;
        }
    }
    dx = px - x;
    dy = py - y;
    dx * dx + dy * dy
}

// ── wrap.js ──────────────────────────────────────────────────────────────────

fn wrap(features: Vec<Feat>, buffer: f64, metrics: bool) -> Vec<Feat> {
    let left = clip(&features, -1.0 - buffer, buffer, 0, metrics);
    let right = clip(&features, 1.0 - buffer, 2.0 + buffer, 0, metrics);
    if left.is_empty() && right.is_empty() {
        return features;
    }
    let mut merged: Vec<Feat> = left.into_iter().map(|f| shift(f, 1.0)).collect();
    merged.extend(clip(&features, -buffer, 1.0 + buffer, 0, metrics));
    merged.extend(right.into_iter().map(|f| shift(f, -1.0)));
    merged
}

fn shift(mut f: Feat, dx: f64) -> Feat {
    let s = |p: &mut [f64; 3]| p[0] += dx;
    match &mut f.geom {
        Geom::Points(pts) => pts.iter_mut().for_each(s),
        Geom::Lines(ls) => ls.iter_mut().flat_map(|l| l.pts.iter_mut()).for_each(s),
        Geom::Polygons(ps) => ps.iter_mut().flatten().flat_map(|l| l.pts.iter_mut()).for_each(s),
    }
    f.min.0 += dx;
    f.max.0 += dx;
    f
}

// ── clip.js ──────────────────────────────────────────────────────────────────

/// Features clipped to k1 <= coord <= k2 along `axis` (0 = x, 1 = y).
/// With `metrics`, every slice of a line becomes its own feature.
fn clip(features: &[Feat], k1: f64, k2: f64, axis: usize, metrics: bool) -> Vec<Feat> {
    let mut out = Vec::new();
    for f in features {
        let (min, max) = if axis == 0 { (f.min.0, f.max.0) } else { (f.min.1, f.max.1) };
        if min >= k1 && max < k2 {
            out.push(f.clone());
            continue;
        }
        if max < k1 || min >= k2 {
            continue;
        }
        let geom = match &f.geom {
            Geom::Points(pts) => {
                let kept: Vec<[f64; 3]> = pts.iter().filter(|p| p[axis] >= k1 && p[axis] <= k2).copied().collect();
                if kept.is_empty() { None } else { Some(Geom::Points(kept)) }
            }
            Geom::Lines(ls) => {
                let mut nl = Vec::new();
                for l in ls {
                    clip_line(l, &mut nl, k1, k2, axis, false, metrics);
                }
                if metrics {
                    for l in nl {
                        out.push(make_feat(f.id, Geom::Lines(vec![l]), f.props.clone()));
                    }
                    None
                } else if nl.is_empty() {
                    None
                } else {
                    Some(Geom::Lines(nl))
                }
            }
            Geom::Polygons(ps) => {
                let mut np = Vec::new();
                for p in ps {
                    let mut rings = Vec::new();
                    for r in p {
                        clip_line(r, &mut rings, k1, k2, axis, true, false);
                    }
                    if !rings.is_empty() {
                        np.push(rings);
                    }
                }
                if np.is_empty() { None } else { Some(Geom::Polygons(np)) }
            }
        };
        if let Some(g) = geom {
            out.push(make_feat(f.id, g, f.props.clone()));
        }
    }
    out
}

fn clip_line(geom: &Line, out: &mut Vec<Line>, k1: f64, k2: f64, axis: usize, polygon: bool, metrics: bool) {
    let new_slice = || Line { pts: Vec::new(), size: geom.size, start: geom.start, end: geom.end };
    let mut slice = new_slice();
    let intersect = |s: &mut Line, a: [f64; 3], b: [f64; 3], k: f64| -> f64 {
        if axis == 0 {
            let t = (k - a[0]) / (b[0] - a[0]);
            s.pts.push([k, a[1] + (b[1] - a[1]) * t, 1.0]);
            t
        } else {
            let t = (k - a[1]) / (b[1] - a[1]);
            s.pts.push([a[0] + (b[0] - a[0]) * t, k, 1.0]);
            t
        }
    };
    // lineMetrics: distance along the whole line at the segment start.
    let mut len = geom.start;
    let n = geom.pts.len();
    for i in 0..n.saturating_sub(1) {
        let (pa, pb) = (geom.pts[i], geom.pts[i + 1]);
        let (a, b) = (pa[axis], pb[axis]);
        let seg = if metrics { ((pa[0] - pb[0]).powi(2) + (pa[1] - pb[1]).powi(2)).sqrt() } else { 0.0 };
        let mut exited = false;
        let mut t = 0.0;
        if a < k1 {
            if b > k1 {
                t = intersect(&mut slice, pa, pb, k1);
                slice.start = len + seg * t;
            }
        } else if a > k2 {
            if b < k2 {
                t = intersect(&mut slice, pa, pb, k2);
                slice.start = len + seg * t;
            }
        } else {
            slice.pts.push(pa);
        }
        if b < k1 && a >= k1 {
            t = intersect(&mut slice, pa, pb, k1);
            exited = true;
        }
        if b > k2 && a <= k2 {
            t = intersect(&mut slice, pa, pb, k2);
            exited = true;
        }
        if !polygon && exited {
            slice.end = len + seg * t;
            out.push(std::mem::replace(&mut slice, new_slice()));
        }
        len += seg;
    }
    if let Some(&last) = geom.pts.last() {
        if last[axis] >= k1 && last[axis] <= k2 {
            slice.pts.push(last);
        }
    }
    // Close polygon rings whose ends differ after clipping.
    if polygon && slice.pts.len() >= 2 {
        let (f, l) = (slice.pts[0], *slice.pts.last().unwrap());
        if f[0] != l[0] || f[1] != l[1] {
            slice.pts.push(f);
        }
    }
    if !slice.pts.is_empty() {
        out.push(slice);
    }
}

// ── tile.js + transform, encoded as MVT ──────────────────────────────────────

fn encode_tile(features: &[Feat], z2: f64, tx: f64, ty: f64, tol: f64, metrics: bool) -> Vec<u8> {
    let sq_tol = tol * tol;
    let transform = |p: &[f64; 3]| -> (i64, i64) {
        (js_round(EXTENT * (p[0] * z2 - tx)) as i64, js_round(EXTENT * (p[1] * z2 - ty)) as i64)
    };
    // Rings/lines kept at this zoom (addLine), as tile coordinates.
    let keep = |l: &Line, polygon: bool, outer: bool| -> Option<Vec<(i64, i64)>> {
        if tol > 0.0 && l.size < if polygon { sq_tol } else { tol } {
            return None;
        }
        let mut ring: Vec<(i64, i64)> = l.pts.iter().filter(|p| tol == 0.0 || p[2] > sq_tol).map(transform).collect();
        if polygon {
            rewind(&mut ring, outer);
        }
        Some(ring)
    };
    let mut keys: Vec<String> = Vec::new();
    let mut values: Vec<(u8, String)> = Vec::new(); // (kind, canonical text) for dedup
    let mut pb_values: Vec<PbValue> = Vec::new();
    let mut pb_features = Vec::new();
    for f in features {
        let (kind, geometry): (i32, Vec<u32>) = match &f.geom {
            Geom::Points(pts) => {
                let pts: Vec<(i64, i64)> = pts.iter().map(transform).collect();
                (1, encode_points(&pts))
            }
            Geom::Lines(ls) => {
                let lines: Vec<Vec<(i64, i64)>> = ls.iter().filter_map(|l| keep(l, false, false)).filter(|l| l.len() >= 2).collect();
                if lines.is_empty() {
                    continue;
                }
                (2, encode_lines(&lines, false))
            }
            Geom::Polygons(ps) => {
                let mut rings = Vec::new();
                for p in ps {
                    for (i, r) in p.iter().enumerate() {
                        if let Some(ring) = keep(r, true, i == 0) {
                            if ring.len() >= 2 {
                                rings.push(ring);
                            }
                        }
                    }
                }
                if rings.is_empty() {
                    continue;
                }
                (3, encode_lines(&rings, true))
            }
        };
        let mut tags = Vec::new();
        // tile.js addLineTileFeature: the slice's range as fractions of the line.
        let clip_props: Vec<(String, Prop)> = match &f.geom {
            Geom::Lines(ls) if metrics && ls.len() == 1 => vec![
                (CLIP_START.to_owned(), Prop::Num(ls[0].start / ls[0].size)),
                (CLIP_END.to_owned(), Prop::Num(ls[0].end / ls[0].size)),
            ],
            _ => Vec::new(),
        };
        for (k, v) in f.props.iter().chain(clip_props.iter()) {
            let ki = keys.iter().position(|x| x == k).unwrap_or_else(|| {
                keys.push(k.clone());
                keys.len() - 1
            });
            let canon = match v {
                Prop::Str(s) => (0u8, s.clone()),
                Prop::Num(n) => (1, n.to_bits().to_string()),
                Prop::Bool(b) => (2, b.to_string()),
            };
            let vi = values.iter().position(|x| *x == canon).unwrap_or_else(|| {
                values.push(canon);
                let mut pv = PbValue::default();
                match v {
                    Prop::Str(s) => pv.string_value = Some(s.clone()),
                    Prop::Num(n) => pv.double_value = Some(*n),
                    Prop::Bool(b) => pv.bool_value = Some(*b),
                }
                pb_values.push(pv);
                values.len() - 1
            });
            tags.push(ki as u32);
            tags.push(vi as u32);
        }
        pb_features.push(PbFeature { id: f.id, tags, r#type: Some(kind), geometry });
    }
    let layer = PbLayer { version: 2, name: GEOJSON_LAYER.to_owned(), features: pb_features, keys, values: pb_values, extent: Some(EXTENT as u32) };
    PbTile { layers: vec![layer] }.encode_to_vec()
}

/// tile.js rewind: outer rings clockwise, holes counter-clockwise (y down).
fn rewind(ring: &mut [(i64, i64)], clockwise: bool) {
    let n = ring.len();
    if n < 3 {
        return;
    }
    let mut area = 0.0;
    let mut j = n - 1;
    for i in 0..n {
        area += (ring[i].0 - ring[j].0) as f64 * (ring[i].1 + ring[j].1) as f64;
        j = i;
    }
    if (area > 0.0) == clockwise {
        ring.reverse();
    }
}

fn zigzag(v: i64) -> u32 {
    ((v << 1) ^ (v >> 63)) as u32
}

fn command(id: u32, count: u32) -> u32 {
    (id & 7) | (count << 3)
}

fn encode_points(pts: &[(i64, i64)]) -> Vec<u32> {
    let mut g = vec![command(1, pts.len() as u32)];
    let (mut cx, mut cy) = (0, 0);
    for &(x, y) in pts {
        g.push(zigzag(x - cx));
        g.push(zigzag(y - cy));
        cx = x;
        cy = y;
    }
    g
}

/// MoveTo + LineTo per line; rings drop their repeated closing point and end
/// with ClosePath (the decoder restores it).
fn encode_lines(lines: &[Vec<(i64, i64)>], closed: bool) -> Vec<u32> {
    let mut g = Vec::new();
    let (mut cx, mut cy) = (0, 0);
    for l in lines {
        let mut pts: &[(i64, i64)] = l;
        if closed && pts.len() > 1 && pts.first() == pts.last() {
            pts = &pts[..pts.len() - 1];
        }
        if pts.is_empty() {
            continue;
        }
        g.push(command(1, 1));
        g.push(zigzag(pts[0].0 - cx));
        g.push(zigzag(pts[0].1 - cy));
        cx = pts[0].0;
        cy = pts[0].1;
        if pts.len() > 1 {
            g.push(command(2, (pts.len() - 1) as u32));
            for &(x, y) in &pts[1..] {
                g.push(zigzag(x - cx));
                g.push(zigzag(y - cy));
                cx = x;
                cy = y;
            }
        }
        if closed {
            g.push(command(7, 1));
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn point_and_polygon_tile_round_trip_through_mvt() {
        let data = json!({"type": "FeatureCollection", "features": [
            {"type": "Feature", "id": 7, "properties": {"name": "A", "n": 3},
             "geometry": {"type": "Point", "coordinates": [0.0, 0.0]}},
            {"type": "Feature", "properties": {"kind": "park"},
             "geometry": {"type": "Polygon", "coordinates": [[[-10.0, -10.0], [10.0, -10.0], [10.0, 10.0], [-10.0, 10.0], [-10.0, -10.0]]]}}
        ]});
        let idx = GeoJsonIndex::new(&data, GeoJsonOptions { max_zoom: 18, buffer: 2048.0, tolerance: 6.0, line_metrics: false, cluster: None });
        let bytes = idx.tile(1, 1, 1);
        let tile = super::super::mvt::decode(&bytes).unwrap();
        let layer = &tile.layers[GEOJSON_LAYER];
        assert_eq!(layer.features.len(), 2);
        let p = &layer.features[0];
        assert_eq!(p.id, Some(7));
        // Lon/lat 0,0 is the top-left corner of tile 1/1/1.
        match &p.geometry {
            super::super::mvt::Geometry::Points(pts) => assert_eq!(pts[0], (0.0, 0.0)),
            g => panic!("{g:?}"),
        }
        match &layer.features[1].geometry {
            super::super::mvt::Geometry::Polygons(polys) => {
                assert_eq!(polys.len(), 1);
                // Clipped to the tile plus its 2048-unit buffer on the left/top.
                let xs: Vec<f32> = polys[0][0].iter().map(|p| p.0).collect();
                assert!(xs.iter().cloned().fold(f32::INFINITY, f32::min) >= -2048.0);
            }
            g => panic!("{g:?}"),
        }
    }

    #[test]
    fn simplification_drops_vertices_at_low_zoom_only() {
        let coords: Vec<Json> = (0..=200).map(|i| json!([i as f64 * 0.01, (i as f64 * 0.3).sin() * 0.0001])).collect();
        let data = json!({"type": "LineString", "coordinates": coords});
        // maxzoom 4: the whole 2-degree line lies in one tile at z <= 4.
        let idx = GeoJsonIndex::new(&data, GeoJsonOptions { max_zoom: 4, buffer: 2048.0, tolerance: 6.0, line_metrics: false, cluster: None });
        let count = |z: u8| {
            let n = 1u32 << z;
            let (x, y) = ((0.5 + 1.0 / 360.0) * n as f64, 0.5 * n as f64);
            let t = super::super::mvt::decode(&idx.tile(z, x as u32, y as u32)).unwrap();
            match &t.layers[GEOJSON_LAYER].features[0].geometry {
                super::super::mvt::Geometry::Lines(ls) => ls.iter().map(|l| l.len()).sum::<usize>(),
                g => panic!("{g:?}"),
            }
        };
        assert!(count(2) < 10, "below maxzoom the line is simplified");
        assert_eq!(count(4), 201, "at maxzoom every vertex is kept");
    }

    #[test]
    fn line_metrics_give_each_slice_its_clip_range() {
        // Along the equator from x = 0.25 to 0.75; at z1 the buffer is 1/8
        // of the world, so tile 0 clips at x = 0.625 and tile 1 at 0.375.
        let data = json!({"type": "Feature", "properties": {"k": 1},
            "geometry": {"type": "LineString", "coordinates": [[-90.0, 0.0], [0.0, 0.0], [90.0, 0.0]]}});
        let idx = GeoJsonIndex::new(&data, GeoJsonOptions { max_zoom: 18, buffer: 2048.0, tolerance: 6.0, line_metrics: true, cluster: None });
        let range = |x: u32| {
            let t = super::super::mvt::decode(&idx.tile(1, x, 1)).unwrap();
            let f = &t.layers[GEOJSON_LAYER].features[0];
            let num = |k: &str| match f.properties.get(k) {
                Some(super::super::mvt::Value::Number(v)) => *v,
                v => panic!("{k}: {v:?}"),
            };
            (num(CLIP_START), num(CLIP_END))
        };
        let (s0, e0) = range(0);
        let (s1, e1) = range(1);
        assert_eq!(s0, 0.0);
        assert!((e0 - 0.75).abs() < 1e-12, "{e0}");
        assert!((s1 - 0.25).abs() < 1e-12, "{s1}");
        assert_eq!(e1, 1.0);
        // Without lineMetrics no clip properties are added.
        let plain = GeoJsonIndex::new(&data, GeoJsonOptions { max_zoom: 18, buffer: 2048.0, tolerance: 6.0, line_metrics: false, cluster: None });
        let t = super::super::mvt::decode(&plain.tile(1, 0, 1)).unwrap();
        assert!(!t.layers[GEOJSON_LAYER].features[0].properties.contains_key(CLIP_START));
    }
}
