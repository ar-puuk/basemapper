//! Helpers for SVG output: number/colour formatting, path data, contour
//! tracing of SDF bitmaps (glyph and SDF-icon outlines), and embedded images.

use super::expr::Color;
use std::fmt::Write as _;

/// Format a coordinate compactly (2 decimals, trailing zeros trimmed).
pub fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.to_owned() }
}

/// Format an opacity (4 decimals: 2 would shift translucent fills by up to
/// 1/255 per layer).
pub fn alpha(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.to_owned() }
}

/// A premultiplied colour (times an opacity) as SVG `#rrggbb` + opacity.
pub fn paint(c: Color, opacity: f64) -> Option<(String, f64)> {
    let a = c.a as f64 * opacity;
    if a <= 0.0 {
        return None;
    }
    let ca = c.a.max(1e-9) as f64;
    let ch = |v: f32| ((v as f64 / ca).clamp(0.0, 1.0) * 255.0).round() as u8;
    Some((format!("#{:02x}{:02x}{:02x}", ch(c.r), ch(c.g), ch(c.b)), a.min(1.0)))
}

/// Path data for closed rings / open polylines.
pub fn path_data(rings: &[Vec<(f64, f64)>], close: bool) -> String {
    let mut d = String::new();
    for r in rings {
        let mut it = r.iter();
        let Some(&(x, y)) = it.next() else { continue };
        let _ = write!(d, "M{} {}", num(x), num(y));
        for &(x, y) in it {
            let _ = write!(d, "L{} {}", num(x), num(y));
        }
        if close {
            d.push('Z');
        }
    }
    d
}

/// Escape text for XML attribute values.
pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Iso-contours of a scalar field sampled at texel centres (value(i, j) for
/// texel (i, j), centre at (i + 0.5, j + 0.5)), at `threshold`, as closed
/// rings in texel coordinates. Outside the grid the field is 0, so every
/// contour closes. Linear interpolation along cell edges matches bilinear
/// texture filtering at the threshold crossing.
pub fn trace_contours(w: usize, h: usize, value: impl Fn(usize, usize) -> f64, threshold: f64) -> Vec<Vec<(f64, f64)>> {
    let v = |i: i64, j: i64| -> f64 {
        if i < 0 || j < 0 || i >= w as i64 || j >= h as i64 { 0.0 } else { value(i as usize, j as usize) }
    };
    // Edge identifiers: horizontal edge between (i,j)-(i+1,j) and vertical
    // edge between (i,j)-(i,j+1), on the grid of texel centres incl. padding.
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    enum E {
        H(i64, i64),
        V(i64, i64),
    }
    let point = |e: E| -> (f64, f64) {
        let (i0, j0, i1, j1) = match e {
            E::H(i, j) => (i, j, i + 1, j),
            E::V(i, j) => (i, j, i, j + 1),
        };
        let (a, b) = (v(i0, j0), v(i1, j1));
        let t = if (b - a).abs() < 1e-12 { 0.5 } else { ((threshold - a) / (b - a)).clamp(0.0, 1.0) };
        let x = i0 as f64 + (i1 - i0) as f64 * t + 0.5;
        let y = j0 as f64 + (j1 - j0) as f64 * t + 0.5;
        (x, y)
    };
    let mut segs: Vec<(E, E)> = Vec::new();
    for j in -1..h as i64 {
        for i in -1..w as i64 {
            let (a, b, c, d) = (v(i, j), v(i + 1, j), v(i + 1, j + 1), v(i, j + 1)); // tl tr br bl
            let idx = (a >= threshold) as u8 | ((b >= threshold) as u8) << 1 | ((c >= threshold) as u8) << 2 | ((d >= threshold) as u8) << 3;
            let sides = [E::H(i, j), E::V(i + 1, j), E::H(i, j + 1), E::V(i, j)]; // top right bottom left
            let inside = [a >= threshold, b >= threshold, c >= threshold, d >= threshold]; // tl tr br bl
            let centre = (a + b + c + d) / 4.0 >= threshold;
            const T: usize = 0;
            const R: usize = 1;
            const B: usize = 2;
            const L: usize = 3;
            let mut push = |s1: usize, s2: usize| {
                let (s1, s2) = orient(s1, s2, inside);
                segs.push((sides[s1], sides[s2]));
            };
            match idx {
                0 | 15 => {}
                1 | 14 => push(L, T),
                2 | 13 => push(T, R),
                3 | 12 => push(L, R),
                4 | 11 => push(R, B),
                6 | 9 => push(T, B),
                7 | 8 => push(L, B),
                5 => {
                    if centre {
                        push(L, B);
                        push(T, R);
                    } else {
                        push(L, T);
                        push(R, B);
                    }
                }
                10 => {
                    if centre {
                        push(L, T);
                        push(R, B);
                    } else {
                        push(L, B);
                        push(T, R);
                    }
                }
                _ => {}
            }
        }
    }
    // Link segments into rings (each edge point joins exactly two segments).
    // Segments are directed, so each edge point starts exactly one segment.
    let mut starts: std::collections::HashMap<E, Vec<usize>> = std::collections::HashMap::new();
    for (k, (p, _)) in segs.iter().enumerate() {
        starts.entry(*p).or_default().push(k);
    }
    let mut used = vec![false; segs.len()];
    let mut rings = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let (first, mut cur) = segs[start];
        let mut ring = vec![point(first), point(cur)];
        while cur != first {
            let Some(&next) = starts.get(&cur).and_then(|l| l.iter().find(|&&k| !used[k])) else { break };
            used[next] = true;
            cur = segs[next].1;
            ring.push(point(cur));
        }
        if ring.len() >= 4 {
            ring.pop(); // closing point duplicates the first
            rings.push(ring);
        }
    }
    rings
}

/// Direct a cell segment between sides `s1` and `s2` (0 top, 1 right,
/// 2 bottom, 3 left) so the inside of the contour is always on the same side.
/// Rings then wind consistently: outer boundaries one way, holes the other,
/// so overlapping shapes can be filled with the nonzero rule.
fn orient(s1: usize, s2: usize, inside: [bool; 4]) -> (usize, usize) {
    const MID: [(f64, f64); 4] = [(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)];
    const CORNER: [(f64, f64); 4] = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    // The corner the segment cuts off when the sides are adjacent (the only
    // reliable witness in saddle cells); for opposite sides any corner works.
    let k = match (s1.min(s2), s1.max(s2)) {
        (0, 3) => 0,
        (0, 1) => 1,
        (1, 2) => 2,
        (2, 3) => 3,
        _ => 0,
    };
    let (p, q) = (MID[s1], MID[s2]);
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    let (rx, ry) = (CORNER[k].0 - p.0, CORNER[k].1 - p.1);
    let left_of_travel = dx * ry - dy * rx < 0.0;
    if left_of_travel == inside[k] { (s1, s2) } else { (s2, s1) }
}

/// Standard base64 (for data: URIs).
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Encode straight-alpha RGBA as a PNG data URI.
pub fn png_data_uri(w: u32, h: u32, rgba: &[u8]) -> String {
    let mut buf = Vec::new();
    let enc = image::codecs::png::PngEncoder::new(&mut buf);
    use image::ImageEncoder;
    let _ = enc.write_image(rgba, w, h, image::ExtendedColorType::Rgba8);
    format!("data:image/png;base64,{}", base64(&buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contour_of_a_square_blob_is_one_ring() {
        let (w, h) = (6, 6);
        let rings = trace_contours(w, h, |i, j| if (2..4).contains(&i) && (2..4).contains(&j) { 1.0 } else { 0.0 }, 0.5);
        assert_eq!(rings.len(), 1);
        assert!(rings[0].len() >= 4);
    }

    /// Signed area (shoelace) of a ring.
    fn area(r: &[(f64, f64)]) -> f64 {
        (0..r.len()).map(|k| {
            let (a, b) = (r[k], r[(k + 1) % r.len()]);
            a.0 * b.1 - b.0 * a.1
        }).sum::<f64>() / 2.0
    }

    #[test]
    fn holes_wind_opposite_to_outer_rings() {
        // A 6x6 ring of ones around a 2x2 hole of zeros.
        let rings = trace_contours(8, 8, |i, j| {
            let on = (1..7).contains(&i) && (1..7).contains(&j);
            let hole = (3..5).contains(&i) && (3..5).contains(&j);
            if on && !hole { 1.0 } else { 0.0 }
        }, 0.5);
        assert_eq!(rings.len(), 2);
        let (a0, a1) = (area(&rings[0]), area(&rings[1]));
        assert!(a0 * a1 < 0.0, "outer and hole must wind oppositely: {a0} {a1}");
    }

    #[test]
    fn saddle_blobs_wind_the_same_way() {
        // Two diagonal blobs touching at a saddle: both outer rings.
        let rings = trace_contours(4, 4, |i, j| if (i < 2) == (j < 2) { 1.0 } else { 0.0 }, 0.6);
        assert!(rings.len() >= 2);
        let signs: Vec<bool> = rings.iter().map(|r| area(r) > 0.0).collect();
        assert!(signs.iter().all(|&s| s == signs[0]), "{signs:?}");
    }

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}
