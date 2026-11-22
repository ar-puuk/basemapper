//! Point clustering for GeoJSON sources with `cluster: true`: a port of the
//! ClusterTileIndex (supercluster) bundled in MapLibre GL JS 5.24 and the
//! KDBush index it searches, faithful down to the Float32 kd-trees and the
//! order of points in them, so clusters, their centres and the order features
//! reach symbol placement match MapLibre.
//!
//! Points are projected to the unit square. From the cluster max zoom down,
//! every point not yet taken at a zoom gathers the untaken points within
//! `radius / (extent * 2^z)` of it; if they number at least `minPoints` they
//! become a cluster at their weighted centre, and the next zoom clusters
//! those. A tile at zoom z shows the points and clusters of tree z (the
//! points themselves past the max zoom). Only Point features cluster
//! (MultiPoints and other geometries are dropped). `clusterProperties`
//! aggregate with their map and reduce expressions.

use super::expr::{eval_expr, Ctx, Val};
use super::geojson::{Feat, Geom, Prop};
use super::mvt::{Feature, GeomType, Geometry, Value};
use serde_json::Value as Json;
use std::collections::HashMap;

const MAX_ZOOM: u32 = 30;
/// An item not processed at any zoom yet (supercluster's `Infinity`).
const NOT_PROCESSED: u32 = u32::MAX;

/// JavaScript's Math.round (halves toward +∞).
pub(super) fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

// ── KDBush ───────────────────────────────────────────────────────────────────

struct KdBush {
    ids: Vec<u32>,
    /// Float32Array, as supercluster creates its trees.
    coords: Vec<f32>,
    node_size: usize,
}

impl KdBush {
    fn new(coords: Vec<f32>, node_size: usize) -> Self {
        let n = coords.len() / 2;
        let mut t = KdBush {
            ids: (0..n as u32).collect(),
            coords,
            node_size,
        };
        if n > 0 {
            t.sort(0, n - 1, 0);
        }
        t
    }

    fn sort(&mut self, left: usize, right: usize, axis: usize) {
        if right - left <= self.node_size {
            return;
        }
        let m = (left + right) >> 1;
        self.select(m, left, right, axis);
        // m >= left + 1 here (right - left > node_size >= 2).
        self.sort(left, m - 1, 1 - axis);
        self.sort(m + 1, right, 1 - axis);
    }

    /// Floyd-Rivest selection, as kdbush's `select` (signed indices: `j` can
    /// step below `left`).
    fn select(&mut self, k: usize, left: usize, right: usize, axis: usize) {
        let (k, mut left, mut right) = (k as i64, left as i64, right as i64);
        while right > left {
            if right - left > 600 {
                let n = (right - left + 1) as f64;
                let m = (k - left + 1) as f64;
                let z = n.ln();
                let s = 0.5 * (2.0 * z / 3.0).exp();
                let sd =
                    0.5 * (z * s * (n - s) / n).sqrt() * if m - n / 2.0 < 0.0 { -1.0 } else { 1.0 };
                let new_left = left.max((k as f64 - m * s / n + sd).floor() as i64);
                let new_right = right.min((k as f64 + (n - m) * s / n + sd).floor() as i64);
                self.select(k as usize, new_left as usize, new_right as usize, axis);
            }
            let c = |t: &Self, i: i64| t.coords[2 * i as usize + axis];
            let t = c(self, k);
            let (mut i, mut j) = (left, right);
            self.swap(left, k);
            if c(self, right) > t {
                self.swap(left, right);
            }
            while i < j {
                self.swap(i, j);
                i += 1;
                j -= 1;
                while c(self, i) < t {
                    i += 1;
                }
                while c(self, j) > t {
                    j -= 1;
                }
            }
            if c(self, left) == t {
                self.swap(left, j);
            } else {
                j += 1;
                self.swap(j, right);
            }
            if j <= k {
                left = j + 1;
            }
            if k <= j {
                right = j - 1;
            }
        }
    }

    fn swap(&mut self, i: i64, j: i64) {
        let (i, j) = (i as usize, j as usize);
        self.ids.swap(i, j);
        self.coords.swap(2 * i, 2 * j);
        self.coords.swap(2 * i + 1, 2 * j + 1);
    }

    /// Item indices inside the box, in kdbush's traversal order.
    fn range(&self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Vec<u32> {
        self.search(
            |x, y| x >= min_x && x <= max_x && y >= min_y && y <= max_y,
            |axis, x, y| {
                if axis == 0 {
                    (min_x <= x, max_x >= x)
                } else {
                    (min_y <= y, max_y >= y)
                }
            },
        )
    }

    /// Item indices within `r` of (qx, qy), in kdbush's traversal order.
    fn within(&self, qx: f64, qy: f64, r: f64) -> Vec<u32> {
        let r2 = r * r;
        self.search(
            |x, y| (x - qx) * (x - qx) + (y - qy) * (y - qy) <= r2,
            |axis, x, y| {
                if axis == 0 {
                    (qx - r <= x, qx + r >= x)
                } else {
                    (qy - r <= y, qy + r >= y)
                }
            },
        )
    }

    fn search(
        &self,
        hit: impl Fn(f64, f64) -> bool,
        sides: impl Fn(usize, f64, f64) -> (bool, bool),
    ) -> Vec<u32> {
        let mut out = Vec::new();
        if self.ids.is_empty() {
            return out;
        }
        let at = |i: usize| (self.coords[2 * i] as f64, self.coords[2 * i + 1] as f64);
        // (left, right, axis); `right` may be left - 1 (an empty node).
        let mut stack: Vec<(i64, i64, usize)> = vec![(0, self.ids.len() as i64 - 1, 0)];
        while let Some((left, right, axis)) = stack.pop() {
            if right - left <= self.node_size as i64 {
                for i in left.max(0)..=right {
                    let (x, y) = at(i as usize);
                    if hit(x, y) {
                        out.push(self.ids[i as usize]);
                    }
                }
                continue;
            }
            let m = (left + right) >> 1;
            let (x, y) = at(m as usize);
            if hit(x, y) {
                out.push(self.ids[m as usize]);
            }
            let (lo, hi) = sides(axis, x, y);
            if lo {
                stack.push((left, m - 1, 1 - axis));
            }
            if hi {
                stack.push((m + 1, right, 1 - axis));
            }
        }
        out
    }
}

// ── ClusterTileIndex ─────────────────────────────────────────────────────────

/// A source's cluster options in geojson-vt units.
#[derive(Clone, Debug)]
pub struct ClusterOptions {
    pub max_zoom: u32,
    pub min_points: u32,
    /// Radius in tile units of `extent`.
    pub radius: f64,
    pub extent: f64,
    /// clusterProperties: (name, map expression, reduce expression with
    /// `["accumulated"]`).
    pub properties: Vec<(String, Json, Json)>,
}

impl ClusterOptions {
    /// geojson_source.ts: radius and extent in 8192 tile units, max zoom
    /// `clusterMaxZoom` (rounded) or the source's maxzoom - 1.
    pub fn from_source(src: &Json, source_max_zoom: u8) -> Option<Self> {
        if src["cluster"].as_bool() != Some(true) {
            return None;
        }
        let max_zoom = match src["clusterMaxZoom"].as_f64().filter(|z| *z != 0.0) {
            Some(z) => js_round(z).max(0.0) as u32,
            None => (source_max_zoom as u32).saturating_sub(1),
        };
        let radius = src["clusterRadius"]
            .as_f64()
            .filter(|r| *r != 0.0)
            .unwrap_or(50.0);
        let min_points = src["clusterMinPoints"]
            .as_f64()
            .filter(|p| *p != 0.0)
            .unwrap_or(2.0)
            .max(2.0);
        let properties = src["clusterProperties"]
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| {
                        let (op, map) = (v.get(0)?, v.get(1)?);
                        let reduce = match op {
                            Json::String(s) => serde_json::json!([s, ["accumulated"], ["get", k]]),
                            other => other.clone(),
                        };
                        Some((k.clone(), map.clone(), reduce))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(ClusterOptions {
            max_zoom: max_zoom.min(MAX_ZOOM),
            min_points: min_points as u32,
            radius: radius * 16.0,
            extent: 8192.0,
            properties,
        })
    }
}

/// A point or cluster of a tree.
#[derive(Clone, Copy, Debug)]
struct Item {
    x: f64,
    y: f64,
    /// The last zoom the item was processed at.
    zoom: u32,
    /// A point's index in `points`, or a cluster's id.
    id: i64,
    parent: i64,
    num: u32,
    /// Index into `cluster_props` (clusters with clusterProperties).
    prop: Option<usize>,
}

struct Tree {
    kd: KdBush,
    data: Vec<Item>,
}

impl Tree {
    fn new(data: Vec<Item>) -> Self {
        let coords = data.iter().flat_map(|d| [d.x as f32, d.y as f32]).collect();
        Tree {
            kd: KdBush::new(coords, 64),
            data,
        }
    }
}

struct Pt {
    id: Option<u64>,
    x: f64,
    y: f64,
    tags: Vec<(String, Prop)>,
}

pub(super) struct ClusterIndex {
    opts: ClusterOptions,
    /// Trees for zooms 0..=max_zoom + 1.
    trees: Vec<Tree>,
    points: Vec<Pt>,
    cluster_props: Vec<Vec<(String, Prop)>>,
}

impl ClusterIndex {
    /// initialize + createIndex over the source's (wrapped) features.
    pub(super) fn new(features: &[Feat], opts: ClusterOptions) -> Self {
        let mut points = Vec::new();
        for f in features {
            // A single position is a Point (MultiPoints are skipped).
            if let Geom::Points(pts) = &f.geom {
                if let [p] = pts.as_slice() {
                    points.push(Pt {
                        id: f.id,
                        x: p[0],
                        y: p[1],
                        tags: f.props.clone(),
                    });
                }
            }
        }
        let mut idx = ClusterIndex {
            opts,
            trees: Vec::new(),
            points,
            cluster_props: Vec::new(),
        };
        idx.create_index();
        idx
    }

    fn create_index(&mut self) {
        let max_zoom = self.opts.max_zoom;
        let data: Vec<Item> = self
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| Item {
                x: p.x as f32 as f64,
                y: p.y as f32 as f64,
                zoom: NOT_PROCESSED,
                id: i as i64,
                parent: -1,
                num: 1,
                prop: None,
            })
            .collect();
        let mut trees = vec![Tree::new(data)];
        for z in (0..=max_zoom).rev() {
            let next = self.cluster(trees.last_mut().unwrap(), z);
            trees.push(Tree::new(next));
        }
        trees.reverse();
        self.trees = trees;
    }

    fn cluster(&mut self, tree: &mut Tree, zoom: u32) -> Vec<Item> {
        let o = &self.opts;
        let r = o.radius / (o.extent * 2f64.powi(zoom as i32));
        let n_points = self.points.len() as i64;
        let reduce = !o.properties.is_empty();
        let min_points = o.min_points;
        let mut next = Vec::with_capacity(tree.data.len());
        for index in 0..tree.data.len() {
            if tree.data[index].zoom <= zoom {
                continue;
            }
            tree.data[index].zoom = zoom;
            let (x, y) = (tree.data[index].x, tree.data[index].y);
            let neighbors = tree.kd.within(x, y, r);
            let num_origin = tree.data[index].num;
            let mut num_points = num_origin;
            for &nb in &neighbors {
                let d = &tree.data[nb as usize];
                if d.zoom > zoom {
                    num_points += d.num;
                }
            }
            if num_points > num_origin && num_points >= min_points {
                let mut wx = x * num_origin as f64;
                let mut wy = y * num_origin as f64;
                let mut props: Option<usize> = None;
                let id = ((index as i64) << 5) + (zoom as i64 + 1) + n_points;
                for &nb in &neighbors {
                    let nb = nb as usize;
                    if tree.data[nb].zoom <= zoom {
                        continue;
                    }
                    tree.data[nb].zoom = zoom;
                    let num2 = tree.data[nb].num as f64;
                    wx += tree.data[nb].x * num2;
                    wy += tree.data[nb].y * num2;
                    tree.data[nb].parent = id;
                    if reduce {
                        let pi = match props {
                            Some(pi) => pi,
                            None => {
                                let p = self.map(&tree.data[index]);
                                self.cluster_props.push(p);
                                let pi = self.cluster_props.len() - 1;
                                props = Some(pi);
                                pi
                            }
                        };
                        let other = self.map(&tree.data[nb]);
                        self.reduce(pi, &other);
                    }
                }
                tree.data[index].parent = id;
                let n = num_points as f64;
                next.push(Item {
                    x: wx / n,
                    y: wy / n,
                    zoom: NOT_PROCESSED,
                    id,
                    parent: -1,
                    num: num_points,
                    prop: props,
                });
            } else {
                next.push(tree.data[index]);
                if num_points > 1 {
                    for &nb in &neighbors {
                        let nb = nb as usize;
                        if tree.data[nb].zoom <= zoom {
                            continue;
                        }
                        tree.data[nb].zoom = zoom;
                        next.push(tree.data[nb]);
                    }
                }
            }
        }
        next
    }

    /// The (mapped) properties an item contributes to a cluster.
    fn map(&self, item: &Item) -> Vec<(String, Prop)> {
        if item.num > 1 {
            return item
                .prop
                .map(|p| self.cluster_props[p].clone())
                .unwrap_or_default();
        }
        let feature = prop_feature(&self.points[item.id as usize].tags);
        let ctx = Ctx {
            zoom: 0.0,
            feature: Some(&feature),
        };
        self.opts
            .properties
            .iter()
            .filter_map(|(k, map, _)| Some((k.clone(), val_prop(eval_expr(map, &ctx))?)))
            .collect()
    }

    fn reduce(&mut self, acc: usize, other: &[(String, Prop)]) {
        let feature = prop_feature(other);
        let ctx = Ctx {
            zoom: 0.0,
            feature: Some(&feature),
        };
        for (k, _, reduce) in &self.opts.properties {
            let props = &mut self.cluster_props[acc];
            let cur = props
                .iter()
                .find(|(n, _)| n == k)
                .map_or(Json::Null, |(_, v)| prop_literal(v));
            let v = val_prop(eval_expr(&substitute(reduce, &cur), &ctx));
            props.retain(|(n, _)| n != k);
            if let Some(v) = v {
                props.push((k.clone(), v));
            }
        }
    }

    /// getTile: the points and clusters of the tile's tree within the tile
    /// plus the radius, as features in world coordinates.
    pub(super) fn tile_features(&self, z: u8, x: u32, y: u32) -> Vec<Feat> {
        let tz = (z as u32).min(self.opts.max_zoom + 1) as usize;
        let t = &self.trees[tz];
        let z2 = 2f64.powi(z as i32);
        let p = self.opts.radius / self.opts.extent;
        let (xf, yf) = (x as f64, y as f64);
        let top = (yf - p) / z2;
        let bottom = (yf + 1.0 + p) / z2;
        let mut out = Vec::new();
        let mut add = |ids: Vec<u32>, dx: f64| {
            for i in ids {
                let item = &t.data[i as usize];
                let (id, px, py, tags) = if item.num > 1 {
                    let mut tags = item
                        .prop
                        .map(|p| self.cluster_props[p].clone())
                        .unwrap_or_default();
                    let n = item.num;
                    let abbrev = if n >= 10000 {
                        Prop::Str(format!("{}k", js_round(n as f64 / 1000.0)))
                    } else if n >= 1000 {
                        Prop::Str(format!("{}k", js_round(n as f64 / 100.0) / 10.0))
                    } else {
                        Prop::Num(n as f64)
                    };
                    tags.retain(|(k, _)| {
                        !matches!(
                            k.as_str(),
                            "cluster" | "cluster_id" | "point_count" | "point_count_abbreviated"
                        )
                    });
                    tags.push(("cluster".into(), Prop::Bool(true)));
                    tags.push(("cluster_id".into(), Prop::Num(item.id as f64)));
                    tags.push(("point_count".into(), Prop::Num(n as f64)));
                    tags.push(("point_count_abbreviated".into(), abbrev));
                    (u64::try_from(item.id).ok(), item.x, item.y, tags)
                } else {
                    let pt = &self.points[item.id as usize];
                    (pt.id, pt.x, pt.y, pt.tags.clone())
                };
                // Wrapped copies for x = 0 / z2 - 1 shift by a world.
                out.push(Feat::point(id, [px + dx, py], tags));
            }
        };
        add(
            t.kd.range((xf - p) / z2, top, (xf + 1.0 + p) / z2, bottom),
            0.0,
        );
        if x == 0 {
            add(t.kd.range(1.0 - p / z2, top, 1.0, bottom), -1.0);
        }
        if x as f64 == z2 - 1.0 {
            add(t.kd.range(0.0, top, p / z2, bottom), 1.0);
        }
        out
    }
}

fn prop_feature(props: &[(String, Prop)]) -> Feature {
    let properties: HashMap<String, Value> = props
        .iter()
        .map(|(k, v)| {
            let v = match v {
                Prop::Str(s) => Value::String(s.clone()),
                Prop::Num(n) => Value::Number(*n),
                Prop::Bool(b) => Value::Bool(*b),
            };
            (k.clone(), v)
        })
        .collect();
    Feature {
        id: None,
        geom_type: GeomType::Point,
        properties,
        geometry: Geometry::Points(Vec::new()),
    }
}

fn val_prop(v: Val) -> Option<Prop> {
    match v {
        Val::Num(n) => Some(Prop::Num(n)),
        Val::Str(s) => Some(Prop::Str(s)),
        Val::Bool(b) => Some(Prop::Bool(b)),
        _ => None,
    }
}

fn prop_literal(p: &Prop) -> Json {
    match p {
        Prop::Num(n) => serde_json::json!(n),
        Prop::Str(s) => serde_json::json!(s),
        Prop::Bool(b) => serde_json::json!(b),
    }
}

/// `["accumulated"]` replaced by the accumulated value.
fn substitute(expr: &Json, v: &Json) -> Json {
    match expr {
        Json::Array(a) if a.len() == 1 && a[0] == "accumulated" => v.clone(),
        Json::Array(a) => Json::Array(a.iter().map(|e| substitute(e, v)).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kdbush_queries_match_brute_force() {
        let mut seed = 1u64;
        let mut rnd = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as f32 / (1u64 << 31) as f32
        };
        let coords: Vec<f32> = (0..3000).map(|_| rnd()).collect();
        let kd = KdBush::new(coords.clone(), 16);
        let at = |i: usize| (coords[2 * i] as f64, coords[2 * i + 1] as f64);
        let mut got = kd.within(0.4, 0.6, 0.1);
        got.sort();
        let want: Vec<u32> = (0..1500)
            .filter(|&i| {
                let (x, y) = at(i);
                (x - 0.4).powi(2) + (y - 0.6).powi(2) <= 0.01
            })
            .map(|i| i as u32)
            .collect();
        assert_eq!(got, want);
        let mut got = kd.range(0.1, 0.2, 0.3, 0.5);
        got.sort();
        let want: Vec<u32> = (0..1500)
            .filter(|&i| {
                let (x, y) = at(i);
                (0.1..=0.3).contains(&x) && (0.2..=0.5).contains(&y)
            })
            .map(|i| i as u32)
            .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn clusters_conserve_points_and_aggregate_properties() {
        let features: Vec<Json> = (0..200)
            .map(|i| {
                let (lng, lat) = (
                    -111.9 + (i % 20) as f64 * 0.002,
                    40.7 + (i / 20) as f64 * 0.002,
                );
                serde_json::json!({"type": "Feature", "properties": {"v": 1},
                    "geometry": {"type": "Point", "coordinates": [lng, lat]}})
            })
            .collect();
        let src = serde_json::json!({"type": "geojson", "cluster": true,
            "clusterProperties": {"total": ["+", ["get", "v"]]},
            "data": {"type": "FeatureCollection", "features": features}});
        let opts = ClusterOptions::from_source(&src, 18).unwrap();
        assert_eq!(
            (opts.max_zoom, opts.min_points, opts.radius),
            (17, 2, 800.0)
        );
        let mut feats = Vec::new();
        for f in src["data"]["features"].as_array().unwrap() {
            let c = &f["geometry"]["coordinates"];
            let (x, y) = (c[0].as_f64().unwrap() / 360.0 + 0.5, {
                let s = (c[1].as_f64().unwrap() * std::f64::consts::PI / 180.0).sin();
                0.5 - 0.25 * ((1.0 + s) / (1.0 - s)).ln() / std::f64::consts::PI
            });
            feats.push(Feat::point(
                None,
                [x, y],
                vec![("v".into(), Prop::Num(1.0))],
            ));
        }
        let idx = ClusterIndex::new(&feats, opts);
        let tile = idx.tile_features(0, 0, 0);
        let num = |f: &Feat, k: &str| {
            f.props
                .iter()
                .find(|(n, _)| n == k)
                .and_then(|(_, v)| match v {
                    Prop::Num(n) => Some(*n),
                    _ => None,
                })
        };
        assert_eq!(tile.len(), 1);
        assert_eq!(num(&tile[0], "point_count"), Some(200.0));
        assert_eq!(num(&tile[0], "total"), Some(200.0));
        // Past the cluster max zoom every point is its own feature.
        assert!(idx.trees[18].data.iter().all(|d| d.num == 1));
        assert_eq!(idx.trees[18].data.len(), 200);
    }
}
