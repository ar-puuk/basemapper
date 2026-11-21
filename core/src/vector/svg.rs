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
            let (top, right, bottom, left) = (E::H(i, j), E::V(i + 1, j), E::H(i, j + 1), E::V(i, j));
            let centre = (a + b + c + d) / 4.0 >= threshold;
            match idx {
                0 | 15 => {}
                1 | 14 => segs.push((left, top)),
                2 | 13 => segs.push((top, right)),
                3 | 12 => segs.push((left, right)),
                4 | 11 => segs.push((right, bottom)),
                6 | 9 => segs.push((top, bottom)),
                7 | 8 => segs.push((left, bottom)),
                5 => {
                    if centre {
                        segs.push((left, bottom));
                        segs.push((top, right));
                    } else {
                        segs.push((left, top));
                        segs.push((right, bottom));
                    }
                }
                10 => {
                    if centre {
                        segs.push((left, top));
                        segs.push((right, bottom));
                    } else {
                        segs.push((left, bottom));
                        segs.push((top, right));
                    }
                }
                _ => {}
            }
        }
    }
    // Link segments into rings (each edge point joins exactly two segments).
    let mut at: std::collections::HashMap<E, Vec<usize>> = std::collections::HashMap::new();
    for (k, (p, q)) in segs.iter().enumerate() {
        at.entry(*p).or_default().push(k);
        at.entry(*q).or_default().push(k);
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
            let Some(&next) = at.get(&cur).and_then(|l| l.iter().find(|&&k| !used[k])) else { break };
            used[next] = true;
            let (p, q) = segs[next];
            cur = if p == cur { q } else { p };
            ring.push(point(cur));
        }
        if ring.len() >= 4 {
            ring.pop(); // closing point duplicates the first
            rings.push(ring);
        }
    }
    rings
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

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}
