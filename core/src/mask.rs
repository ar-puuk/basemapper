//! Clipping a map to polygons: `clip = layer` keeps the inside, `clip =
//! outside(layer)` the rest (docs/design/api-review.md §4.10, D20, D21).
//!
//! A clip is a mask on the finished image, in output pixels, applied after
//! any warp, so it works in every map CRS. Polygons arrive in the map's CRS
//! (the host transforms them); [`Shape::to_pixels`] maps them to the image
//! with its bounds. The mask is rasterised with the renderer's polygon
//! filler at 4 x 4 samples per pixel: each polygon is even-odd within itself
//! (its holes), and polygons are united, so overlapping polygons never
//! cancel. Geometry is simplified to a fraction of a pixel first, so a
//! detailed coastline costs what it shows.

use crate::vector::gl::{raster_polygon, Clip as Canvas};
use crate::BasemapError;

/// One polygon: its exterior ring, then its holes; each ring `(x, y)`
/// points, closed or not.
pub type Polygon = Vec<Vec<(f64, f64)>>;

/// Which side of the polygons a clip keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// The area inside the polygons (`clip = layer`).
    Inside,
    /// Everything but the polygons (`clip = outside(layer)`).
    Outside,
}

/// A clip: polygons in the map's CRS and the side kept.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub polygons: Vec<Polygon>,
    pub keep: Keep,
}

/// Samples per pixel along each axis.
const SS: usize = 4;
/// Simplification tolerance in output pixels: a quarter of one sample.
const TOLERANCE: f64 = 1.0 / (4.0 * SS as f64);

impl Shape {
    /// A clip, checked: at least one polygon, every ring at least three
    /// finite points.
    pub fn new(polygons: Vec<Polygon>, keep: Keep) -> Result<Shape, BasemapError> {
        if polygons.is_empty() {
            return Err(BasemapError::InvalidInput("clip: the layer has no polygons".into()));
        }
        for ring in polygons.iter().flatten() {
            if ring.len() < 3 || ring.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
                return Err(BasemapError::InvalidInput(
                    "clip: every polygon ring needs at least three finite points".into(),
                ));
            }
        }
        Ok(Shape { polygons, keep })
    }

    /// The bounding box of the polygons, `[xmin, ymin, xmax, ymax]`: the
    /// default extent of an inside clip (an outside clip has none).
    pub fn extent(&self) -> Option<[f64; 4]> {
        if self.keep == Keep::Outside {
            return None;
        }
        let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for &(x, y) in self.polygons.iter().flatten().flatten() {
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
        Some(b)
    }

    /// The polygons in pixels of a `width x height` image of `bounds` (in
    /// the polygons' CRS, y up). Web Mercator bounds across the date line
    /// (`xmin > xmax`) are unwrapped. Each ring is simplified to
    /// a sixteenth of a pixel.
    pub fn to_pixels(&self, bounds: [f64; 4], width: f64, height: f64, crs_3857: bool) -> Vec<Polygon> {
        const CIRC: f64 = 2.0 * 20_037_508.342_789_244;
        let across = crs_3857 && bounds[0] > bounds[2];
        let span_x = if across { bounds[2] + CIRC - bounds[0] } else { bounds[2] - bounds[0] };
        let (sx, sy) = (width / span_x, height / (bounds[3] - bounds[1]));
        let px = |&(x, y): &(f64, f64)| {
            let x = if across && x < bounds[0] { x + CIRC } else { x };
            ((x - bounds[0]) * sx, (bounds[3] - y) * sy)
        };
        self.polygons
            .iter()
            .map(|p| p.iter().map(|r| simplify(&r.iter().map(px).collect::<Vec<_>>(), TOLERANCE)).filter(|r| r.len() >= 3).collect())
            .filter(|p: &Polygon| !p.is_empty())
            .collect()
    }
}

/// `ring` without the points that move its outline less than `tolerance`
/// (Douglas-Peucker, iterative), its closing point dropped.
pub fn simplify(ring: &[(f64, f64)], tolerance: f64) -> Vec<(f64, f64)> {
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(ring.len());
    for &p in ring {
        // Repeated points first: cheap, and most of a detailed ring's points.
        if pts.last().is_none_or(|q: &(f64, f64)| (q.0 - p.0).abs() + (q.1 - p.1).abs() > tolerance / 4.0) {
            pts.push(p);
        }
    }
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    if pts.len() <= 4 {
        return pts;
    }
    let n = pts.len();
    let mut keep = vec![false; n];
    // Split the ring at its first point and the point farthest from it, so
    // both halves are open polylines.
    let far = (1..n).max_by(|&a, &b| dist2(pts[0], pts[a]).total_cmp(&dist2(pts[0], pts[b]))).unwrap_or(n - 1);
    keep[0] = true;
    keep[far] = true;
    let mut stack = vec![(0, far), (far, n)];
    let t2 = tolerance * tolerance;
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (pts[a], pts[b % n]);
        let mut best = (0.0, 0);
        for i in a + 1..b {
            let d = seg_dist2(pts[i], pa, pb);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > t2 {
            keep[best.1] = true;
            stack.push((a, best.1));
            stack.push((best.1, b));
        }
    }
    pts.into_iter().zip(keep).filter_map(|(p, k)| k.then_some(p)).collect()
}

fn dist2(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
}

/// Squared distance from `p` to the segment `a`-`b`.
fn seg_dist2(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return dist2(p, a);
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
    dist2(p, (a.0 + t * dx, a.1 + t * dy))
}

/// The alpha mask (0-255 per pixel) that `polygons` (in pixels) cut from a
/// `width x height` image, keeping `keep`.
pub fn alpha_mask(polygons: &[Polygon], width: u32, height: u32, keep: Keep) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    // Each pixel's 16 samples as bits: a polygon sets the samples it covers;
    // OR across polygons is their union.
    let mut bits = vec![0u16; w * h];
    let canvas = Canvas { x0: 0.0, y0: 0.0, x1: (w * SS) as f64, y1: (h * SS) as f64 };
    for polygon in polygons {
        let rings: Vec<Vec<(f64, f64)>> =
            polygon.iter().map(|r| r.iter().map(|&(x, y)| (x * SS as f64, y * SS as f64)).collect()).collect();
        raster_polygon(w * SS, h * SS, &rings, &canvas, |sx, sy| {
            bits[(sy / SS) * w + sx / SS] |= 1 << ((sy % SS) * SS + sx % SS);
        });
    }
    let full = (SS * SS) as u32;
    bits.into_iter()
        .map(|b| {
            let covered = b.count_ones();
            let kept = if keep == Keep::Inside { covered } else { full - covered };
            ((kept * 255 + full / 2) / full) as u8
        })
        .collect()
}

/// Multiply the alpha of straight-alpha RGBA `pixels` by `mask`.
pub fn apply(pixels: &mut [u8], mask: &[u8]) {
    for (px, &m) in pixels.chunks_exact_mut(4).zip(mask) {
        if m != 255 {
            px[3] = ((px[3] as u32 * m as u32 + 127) / 255) as u8;
        }
    }
}

/// An SVG `<clipPath>` with `id` for `polygons` (in the SVG's user units),
/// keeping `keep` of a `width x height` document: the polygons for inside,
/// the frame with the polygons as holes (even-odd) for outside.
pub fn svg_clip_path(id: &str, polygons: &[Polygon], width: f64, height: f64, keep: Keep) -> String {
    use std::fmt::Write;
    let mut d = String::new();
    if keep == Keep::Outside {
        let _ = write!(d, "M0 0H{width}V{height}H0Z");
    }
    for ring in polygons.iter().flatten() {
        for (i, (x, y)) in ring.iter().enumerate() {
            let _ = write!(d, "{}{:.2} {:.2}", if i == 0 { "M" } else { "L" }, x, y);
        }
        d.push('Z');
    }
    // Inside: nonzero would also unite overlapping polygons, but holes need
    // even-odd; outside needs even-odd for the frame's holes.
    format!(r#"<clipPath id="{id}"><path clip-rule="evenodd" d="{d}"/></clipPath>"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<(f64, f64)> {
        vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    }

    fn at(mask: &[u8], w: usize, x: usize, y: usize) -> u8 {
        mask[y * w + x]
    }

    #[test]
    fn inside_outside_and_edges() {
        // A square from pixel 2 to 6, with a half-covered column at x = 6.
        let p = vec![vec![square(2.0, 2.0, 6.5, 6.0)]];
        let m = alpha_mask(&p, 10, 10, Keep::Inside);
        assert_eq!((at(&m, 10, 3, 3), at(&m, 10, 0, 0), at(&m, 10, 8, 3)), (255, 0, 0));
        assert_eq!(at(&m, 10, 6, 3), 128, "half the samples of an edge pixel");
        let o = alpha_mask(&p, 10, 10, Keep::Outside);
        assert!(m.iter().zip(&o).all(|(a, b)| *a as u16 + *b as u16 == 255 || (*a == 128 && *b == 128)));
    }

    #[test]
    fn holes_cut_and_overlaps_unite() {
        // A square with a hole: the hole is not kept.
        let donut = vec![vec![square(0.0, 0.0, 10.0, 10.0), square(3.0, 3.0, 7.0, 7.0)]];
        let m = alpha_mask(&donut, 10, 10, Keep::Inside);
        assert_eq!((at(&m, 10, 1, 1), at(&m, 10, 5, 5)), (255, 0));
        // Two overlapping polygons: the overlap is kept, not cancelled.
        let two = vec![vec![square(0.0, 0.0, 6.0, 6.0)], vec![square(4.0, 4.0, 10.0, 10.0)]];
        let m = alpha_mask(&two, 10, 10, Keep::Inside);
        assert_eq!((at(&m, 10, 5, 5), at(&m, 10, 8, 2)), (255, 0));
        // Outside a donut: the frame and the hole.
        let m = alpha_mask(&donut, 12, 12, Keep::Outside);
        assert_eq!((at(&m, 12, 11, 11), at(&m, 12, 5, 5), at(&m, 12, 1, 1)), (255, 255, 0));
    }

    #[test]
    fn map_coordinates_to_pixels() {
        // Bounds 0..100 x 0..50 on a 200 x 100 image: y up becomes y down.
        let s = Shape::new(vec![vec![square(25.0, 25.0, 75.0, 50.0)]], Keep::Inside).unwrap();
        assert_eq!(s.extent(), Some([25.0, 25.0, 75.0, 50.0]));
        let p = s.to_pixels([0.0, 0.0, 100.0, 50.0], 200.0, 100.0, false);
        assert_eq!(p[0][0], vec![(50.0, 50.0), (150.0, 50.0), (150.0, 0.0), (50.0, 0.0)]);
        // Across the date line in Web Mercator: east of it continues the
        // frame rightwards.
        let half = 20_037_508.342_789_244;
        let s = Shape::new(vec![vec![square(half - 10.0, 0.0, -half + 10.0, 10.0)]], Keep::Inside).unwrap();
        let p = s.to_pixels([half - 20.0, 0.0, -half + 20.0, 40.0], 40.0, 40.0, true);
        let xs: Vec<f64> = p[0][0].iter().map(|q| (q.0 * 1e6).round() / 1e6).collect();
        assert_eq!(xs, [10.0, 30.0, 30.0, 10.0]);
        assert_eq!(Shape::new(vec![vec![square(0.0, 0.0, 1.0, 1.0)]], Keep::Outside).unwrap().extent(), None);
        assert!(Shape::new(vec![], Keep::Inside).is_err());
        assert!(Shape::new(vec![vec![vec![(0.0, 0.0), (1.0, 1.0)]]], Keep::Inside).is_err());
    }

    #[test]
    fn detail_below_a_pixel_is_dropped_without_changing_the_mask() {
        // A circle of 20 000 points, 30 px across.
        let ring: Vec<(f64, f64)> = (0..20_000)
            .map(|i| {
                let a = i as f64 / 20_000.0 * std::f64::consts::TAU;
                (20.0 + 15.0 * a.cos(), 20.0 + 15.0 * a.sin())
            })
            .collect();
        let simple = simplify(&ring, TOLERANCE);
        assert!(simple.len() < 400, "{} points left", simple.len());
        let exact = alpha_mask(&[vec![ring]], 40, 40, Keep::Inside);
        let fast = alpha_mask(&[vec![simple]], 40, 40, Keep::Inside);
        let worst = exact.iter().zip(&fast).map(|(a, b)| (*a as i16 - *b as i16).abs()).max().unwrap();
        assert!(worst <= 16, "at most one sample of 16 differs: {worst}");
    }

    #[test]
    fn applying_a_mask_scales_alpha() {
        let mut px = vec![10, 20, 30, 255, 10, 20, 30, 200, 1, 2, 3, 255];
        apply(&mut px, &[255, 128, 0]);
        assert_eq!((px[3], px[7], px[11]), (255, 100, 0));
        assert_eq!(&px[..3], &[10, 20, 30], "colour is untouched (straight alpha)");
    }

    #[test]
    fn svg_clip_paths() {
        let p = vec![vec![square(1.0, 1.0, 3.0, 3.0)]];
        let inside = svg_clip_path("c", &p, 10.0, 10.0, Keep::Inside);
        assert!(inside.starts_with(r#"<clipPath id="c">"#) && inside.contains("M1.00 1.00L3.00 1.00") && !inside.contains("H10"));
        let outside = svg_clip_path("c", &p, 10.0, 10.0, Keep::Outside);
        assert!(outside.contains(r#"clip-rule="evenodd" d="M0 0H10V10H0ZM1.00"#), "{outside}");
    }
}
