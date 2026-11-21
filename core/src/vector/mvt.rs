//! Mapbox Vector Tile (MVT 2.1) decoding.
//!
//! Decodes the protobuf with `prost` and expands geometry commands into tile-
//! space coordinates (0..extent, y down). Polygons are grouped into
//! exterior-ring-plus-holes using ring winding, as the spec requires.

use prost::Message;
use std::collections::HashMap;

// ── Protobuf schema (vector_tile.proto, MVT 2.1) ─────────────────────────────

#[derive(Clone, PartialEq, Message)]
struct PbTile {
    #[prost(message, repeated, tag = "3")]
    layers: Vec<PbLayer>,
}

#[derive(Clone, PartialEq, Message)]
struct PbLayer {
    #[prost(uint32, required, tag = "15")]
    version: u32,
    #[prost(string, required, tag = "1")]
    name: String,
    #[prost(message, repeated, tag = "2")]
    features: Vec<PbFeature>,
    #[prost(string, repeated, tag = "3")]
    keys: Vec<String>,
    #[prost(message, repeated, tag = "4")]
    values: Vec<PbValue>,
    #[prost(uint32, optional, tag = "5")]
    extent: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct PbFeature {
    #[prost(uint64, optional, tag = "1")]
    id: Option<u64>,
    #[prost(uint32, repeated, tag = "2")]
    tags: Vec<u32>,
    #[prost(int32, optional, tag = "3")]
    r#type: Option<i32>,
    #[prost(uint32, repeated, tag = "4")]
    geometry: Vec<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct PbValue {
    #[prost(string, optional, tag = "1")]
    string_value: Option<String>,
    #[prost(float, optional, tag = "2")]
    float_value: Option<f32>,
    #[prost(double, optional, tag = "3")]
    double_value: Option<f64>,
    #[prost(int64, optional, tag = "4")]
    int_value: Option<i64>,
    #[prost(uint64, optional, tag = "5")]
    uint_value: Option<u64>,
    #[prost(sint64, optional, tag = "6")]
    sint_value: Option<i64>,
    #[prost(bool, optional, tag = "7")]
    bool_value: Option<bool>,
}

// ── Decoded model ────────────────────────────────────────────────────────────

/// A feature property value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeomType {
    Point,
    LineString,
    Polygon,
}

/// Tile-space point (0..extent, y down).
pub type Point = (f32, f32);

#[derive(Clone, Debug)]
pub enum Geometry {
    /// One or more points.
    Points(Vec<Point>),
    /// One or more line strings.
    Lines(Vec<Vec<Point>>),
    /// Polygons, each an exterior ring followed by its holes.
    Polygons(Vec<Vec<Vec<Point>>>),
}

#[derive(Clone, Debug)]
pub struct Feature {
    pub id: Option<u64>,
    pub geom_type: GeomType,
    pub properties: HashMap<String, Value>,
    pub geometry: Geometry,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub extent: u32,
    pub features: Vec<Feature>,
}

#[derive(Clone, Debug, Default)]
pub struct Tile {
    pub layers: HashMap<String, Layer>,
}

/// Decode an MVT. Gzip-compressed input is accepted.
pub fn decode(bytes: &[u8]) -> Result<Tile, String> {
    let raw;
    let data = if bytes.starts_with(&[0x1f, 0x8b]) {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut out)
            .map_err(|e| format!("gzip: {e}"))?;
        raw = out;
        &raw[..]
    } else {
        bytes
    };

    let pb = PbTile::decode(data).map_err(|e| format!("protobuf: {e}"))?;
    let mut tile = Tile::default();
    for l in pb.layers {
        let values: Vec<Value> = l.values.iter().map(convert_value).collect();
        let mut features = Vec::with_capacity(l.features.len());
        for f in &l.features {
            let geom_type = match f.r#type.unwrap_or(0) {
                1 => GeomType::Point,
                2 => GeomType::LineString,
                3 => GeomType::Polygon,
                _ => continue, // UNKNOWN geometry: skip, as MapLibre does
            };
            let mut properties = HashMap::with_capacity(f.tags.len() / 2);
            for kv in f.tags.chunks_exact(2) {
                if let (Some(k), Some(v)) = (l.keys.get(kv[0] as usize), values.get(kv[1] as usize)) {
                    properties.insert(k.clone(), v.clone());
                }
            }
            let rings = decode_commands(&f.geometry);
            let geometry = match geom_type {
                GeomType::Point => Geometry::Points(rings.into_iter().flatten().collect()),
                GeomType::LineString => Geometry::Lines(rings),
                GeomType::Polygon => Geometry::Polygons(classify_rings(rings)),
            };
            features.push(Feature { id: f.id, geom_type, properties, geometry });
        }
        tile.layers.insert(
            l.name.clone(),
            Layer { name: l.name, extent: l.extent.unwrap_or(4096), features },
        );
    }
    Ok(tile)
}

fn convert_value(v: &PbValue) -> Value {
    if let Some(s) = &v.string_value {
        Value::String(s.clone())
    } else if let Some(x) = v.float_value {
        Value::Number(x as f64)
    } else if let Some(x) = v.double_value {
        Value::Number(x)
    } else if let Some(x) = v.int_value {
        Value::Number(x as f64)
    } else if let Some(x) = v.uint_value {
        Value::Number(x as f64)
    } else if let Some(x) = v.sint_value {
        Value::Number(x as f64)
    } else if let Some(b) = v.bool_value {
        Value::Bool(b)
    } else {
        Value::String(String::new())
    }
}

fn zigzag(n: u32) -> i32 {
    ((n >> 1) as i32) ^ -((n & 1) as i32)
}

/// Expand MoveTo / LineTo / ClosePath commands into point sequences. Each
/// MoveTo starts a new sequence; ClosePath closes the current ring.
fn decode_commands(cmds: &[u32]) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = Vec::new();
    let (mut x, mut y) = (0i32, 0i32);
    let mut i = 0;
    while i < cmds.len() {
        let cmd = cmds[i] & 0x7;
        let count = (cmds[i] >> 3) as usize;
        i += 1;
        match cmd {
            1 | 2 => {
                for _ in 0..count {
                    // Truncated geometry: keep what decoded cleanly.
                    let (Some(&dx), Some(&dy)) = (cmds.get(i), cmds.get(i + 1)) else {
                        return out;
                    };
                    i += 2;
                    x += zigzag(dx);
                    y += zigzag(dy);
                    if cmd == 1 {
                        out.push(Vec::new());
                    }
                    if let Some(cur) = out.last_mut() {
                        cur.push((x as f32, y as f32));
                    }
                }
            }
            7 => {
                if let Some(cur) = out.last_mut() {
                    if let Some(&first) = cur.first() {
                        cur.push(first);
                    }
                }
            }
            _ => return out, // malformed: stop rather than misread
        }
    }
    out
}

/// Signed area (shoelace). In MVT's y-down tile space, exterior rings have
/// positive area and holes negative.
fn signed_area(ring: &[Point]) -> f64 {
    let mut a = 0.0f64;
    for w in ring.windows(2) {
        a += (w[0].0 as f64) * (w[1].1 as f64) - (w[1].0 as f64) * (w[0].1 as f64);
    }
    a / 2.0
}

/// Group rings into polygons: a positive-area ring starts a polygon, negative
/// rings are its holes. Zero-area rings are dropped. If the first ring is
/// negative (non-conforming encoder), winding is inverted for the whole
/// feature, matching MapLibre's `classifyRings`.
fn classify_rings(rings: Vec<Vec<Point>>) -> Vec<Vec<Vec<Point>>> {
    let mut polygons: Vec<Vec<Vec<Point>>> = Vec::new();
    let mut ccw: Option<bool> = None;
    for ring in rings {
        let area = signed_area(&ring);
        if area == 0.0 {
            continue;
        }
        let is_neg = area < 0.0;
        let exterior_is_neg = *ccw.get_or_insert(is_neg);
        if is_neg == exterior_is_neg {
            polygons.push(vec![ring]);
        } else if let Some(p) = polygons.last_mut() {
            p.push(ring);
        }
    }
    polygons
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zigzag_decodes() {
        assert_eq!(zigzag(0), 0);
        assert_eq!(zigzag(1), -1);
        assert_eq!(zigzag(2), 1);
        assert_eq!(zigzag(3), -2);
    }

    #[test]
    fn commands_square_polygon() {
        // MoveTo(1) (3,6); LineTo(3) (8,12) (20,34) ; ClosePath — from the MVT spec.
        let cmds = [9, 6, 12, 26, 10, 12, 24, 44, 23, 33, 15];
        let rings = decode_commands(&cmds);
        assert_eq!(rings.len(), 1);
        assert_eq!(rings[0][0], (3.0, 6.0));
        assert_eq!(rings[0][1], (8.0, 12.0));
        assert_eq!(rings[0][2], (20.0, 34.0));
        assert_eq!(rings[0].last(), rings[0].first());
    }

    #[test]
    fn rings_with_hole_group_into_one_polygon() {
        let outer = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)];
        let hole = vec![(2.0, 2.0), (2.0, 8.0), (8.0, 8.0), (8.0, 2.0), (2.0, 2.0)];
        assert!(signed_area(&outer) > 0.0);
        assert!(signed_area(&hole) < 0.0);
        let polys = classify_rings(vec![outer, hole]);
        assert_eq!(polys.len(), 1);
        assert_eq!(polys[0].len(), 2);
    }
}
