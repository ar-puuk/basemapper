//! Warping a Web Mercator rendering into another CRS (docs/design/api-review.md
//! D7, §4.5): the host (R with `sf`, Python with `pyproj`) computes a coarse
//! [`Mesh`] of output pixel positions to Web Mercator coordinates; the core
//! resamples the image through it. No GDAL is needed.
//!
//! Each output pixel's centre is located in the mesh, its Web Mercator
//! position interpolated bilinearly between the four nodes around it, and the
//! source sampled bilinearly there, in premultiplied alpha (so transparent
//! areas do not darken edges). Rows are resampled in parallel.

use crate::BasemapError;

const HALF: f64 = 20_037_508.342_789_244;
const CIRC: f64 = 2.0 * HALF;

/// Web Mercator coordinates at a grid of output pixel positions: `nx * ny`
/// nodes, row-major from the top-left corner, node `(i, j)` at pixel position
/// `(i * width / (nx - 1), j * height / (ny - 1))` (pixel edges, so the
/// corners of the image). A node outside the CRS's valid area is NaN; pixels
/// next to one are left transparent.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub nx: usize,
    pub ny: usize,
    pub nodes: Vec<(f64, f64)>,
}

impl Mesh {
    /// A mesh, checked: at least 2 x 2 nodes, `nx * ny` of them.
    pub fn new(nx: usize, ny: usize, nodes: Vec<(f64, f64)>) -> Result<Mesh, BasemapError> {
        if nx < 2 || ny < 2 || nodes.len() != nx * ny {
            return Err(BasemapError::InvalidInput(format!(
                "a warp mesh needs at least 2 x 2 nodes, nx * ny of them (got {nx} x {ny}, {} nodes)",
                nodes.len()
            )));
        }
        Ok(Mesh { nx, ny, nodes })
    }

    /// A mesh from a function of output pixel positions to Web Mercator.
    pub fn from_fn(nx: usize, ny: usize, width: u32, height: u32, f: impl Fn(f64, f64) -> (f64, f64)) -> Result<Mesh, BasemapError> {
        let mut nodes = Vec::with_capacity(nx * ny);
        for j in 0..ny {
            for i in 0..nx {
                let x = i as f64 * width as f64 / (nx - 1).max(1) as f64;
                let y = j as f64 * height as f64 / (ny - 1).max(1) as f64;
                nodes.push(f(x, y));
            }
        }
        Mesh::new(nx, ny, nodes)
    }

    /// The Web Mercator position of output pixel position `(x, y)` in a
    /// `width x height` image, or `None` near an invalid node.
    fn at(&self, x: f64, y: f64, width: f64, height: f64) -> Option<(f64, f64)> {
        let gx = (x / width * (self.nx - 1) as f64).clamp(0.0, (self.nx - 1) as f64);
        let gy = (y / height * (self.ny - 1) as f64).clamp(0.0, (self.ny - 1) as f64);
        let (i, j) = ((gx as usize).min(self.nx - 2), (gy as usize).min(self.ny - 2));
        let (fx, fy) = (gx - i as f64, gy - j as f64);
        let n = |di: usize, dj: usize| self.nodes[(j + dj) * self.nx + i + di];
        let (a, b, c, d) = (n(0, 0), n(1, 0), n(0, 1), n(1, 1));
        if [a, b, c, d].iter().any(|p| !p.0.is_finite() || !p.1.is_finite()) {
            return None;
        }
        // Nodes either side of the date line: unwrap them next to the first.
        let unwrap = |p: (f64, f64)| {
            let dx = p.0 - a.0;
            (if dx > HALF { p.0 - CIRC } else if dx < -HALF { p.0 + CIRC } else { p.0 }, p.1)
        };
        let (b, c, d) = (unwrap(b), unwrap(c), unwrap(d));
        let lerp = |p: (f64, f64), q: (f64, f64), t: f64| (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
        Some(lerp(lerp(a, b, fx), lerp(c, d, fx), fy))
    }
}

/// A Web Mercator image to warp: straight-alpha RGBA and the extent it
/// covers (`xmin > xmax` across the date line).
pub struct Source<'a> {
    pub pixels: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub bounds: [f64; 4],
}

/// `source` resampled into a `width x height` image through `mesh`
/// (straight-alpha RGBA). Output pixels whose position falls outside the
/// source are transparent.
pub fn warp(source: &Source, mesh: &Mesh, width: u32, height: u32) -> Result<Vec<u8>, BasemapError> {
    let (sw, sh) = (source.width as usize, source.height as usize);
    if source.pixels.len() != sw * sh * 4 || sw == 0 || sh == 0 {
        return Err(BasemapError::InvalidInput("warp: the source pixels do not match its size".into()));
    }
    let b = source.bounds;
    let span_x = if b[0] > b[2] { b[2] + CIRC - b[0] } else { b[2] - b[0] };
    let (kx, ky) = (sw as f64 / span_x, sh as f64 / (b[3] - b[1]));
    let whole_world = span_x >= CIRC - 1e-6;
    let rows: Vec<u32> = (0..height).collect();
    let out = crate::vector::memo::par_map(&rows, |&py| {
        let mut row = vec![0u8; width as usize * 4];
        for px in 0..width {
            let Some((mx, my)) = mesh.at(px as f64 + 0.5, py as f64 + 0.5, width as f64, height as f64) else { continue };
            // Into the source's pixels, x measured east from its west edge
            // (any longitude, across the date line).
            let dx = if whole_world { (mx - b[0]).rem_euclid(CIRC) } else { wrap_east(mx - b[0]) };
            if dx > span_x {
                continue;
            }
            let (sx, sy) = (dx * kx - 0.5, (b[3] - my) * ky - 0.5);
            if let Some(rgba) = sample(source.pixels, sw, sh, sx, sy, whole_world) {
                row[px as usize * 4..px as usize * 4 + 4].copy_from_slice(&rgba);
            }
        }
        row
    });
    Ok(out.concat())
}

/// `dx` (metres east of a west edge) folded into `(-HALF, CIRC - HALF]`, so
/// a point just west of the edge stays negative (outside) rather than
/// wrapping to the far east.
fn wrap_east(dx: f64) -> f64 {
    let d = dx.rem_euclid(CIRC);
    if d > CIRC - HALF { d - CIRC } else { d }
}

/// Bilinear sample of straight-alpha `pixels` at pixel-centre coordinates
/// (`(0, 0)` the centre of the first pixel), in premultiplied alpha; `None`
/// outside the image. Edge pixels extend half a pixel; `wrap_x` wraps
/// columns (a whole-world image).
fn sample(pixels: &[u8], w: usize, h: usize, x: f64, y: f64, wrap_x: bool) -> Option<[u8; 4]> {
    if y < -0.5 || y > h as f64 - 0.5 || (!wrap_x && (x < -0.5 || x > w as f64 - 0.5)) {
        return None;
    }
    let (x, y) = (if wrap_x { x } else { x.clamp(0.0, (w - 1) as f64) }, y.clamp(0.0, (h - 1) as f64));
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let col = |c: f64| if wrap_x { (c as i64).rem_euclid(w as i64) as usize } else { (c as usize).min(w - 1) };
    let (c0, c1) = (col(x0), col(x0 + 1.0));
    let (r0, r1) = (y0 as usize, (y0 as usize + 1).min(h - 1));
    let mut acc = [0.0f64; 4];
    for (r, c, wt) in [(r0, c0, (1.0 - fx) * (1.0 - fy)), (r0, c1, fx * (1.0 - fy)), (r1, c0, (1.0 - fx) * fy), (r1, c1, fx * fy)] {
        if wt == 0.0 {
            continue;
        }
        let p = &pixels[(r * w + c) * 4..(r * w + c) * 4 + 4];
        let a = p[3] as f64;
        acc[0] += p[0] as f64 * a * wt;
        acc[1] += p[1] as f64 * a * wt;
        acc[2] += p[2] as f64 * a * wt;
        acc[3] += a * wt;
    }
    if acc[3] <= 0.0 {
        return Some([0, 0, 0, 0]);
    }
    let un = |v: f64| (v / acc[3]).round().clamp(0.0, 255.0) as u8;
    Some([un(acc[0]), un(acc[1]), un(acc[2]), acc[3].round().clamp(0.0, 255.0) as u8])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test image: each pixel's colour encodes its position.
    fn gradient(w: u32, h: u32) -> Vec<u8> {
        (0..h).flat_map(|y| (0..w).flat_map(move |x| [(x * 7 % 256) as u8, (y * 11 % 256) as u8, 128, 255])).collect()
    }

    #[test]
    fn the_identity_mesh_reproduces_the_image() {
        let (w, h) = (40, 30);
        let px = gradient(w, h);
        let b = [-4000.0, -3000.0, 4000.0, 3000.0];
        let mesh = Mesh::from_fn(5, 4, w, h, |x, y| (b[0] + x / w as f64 * 8000.0, b[3] - y / h as f64 * 6000.0)).unwrap();
        let out = warp(&Source { pixels: &px, width: w, height: h, bounds: b }, &mesh, w, h).unwrap();
        assert_eq!(out, px);
    }

    #[test]
    fn a_shifted_mesh_moves_the_image_and_leaves_the_rest_transparent() {
        let (w, h) = (20, 10);
        let px = gradient(w, h);
        let b = [0.0, 0.0, 20.0, 10.0];
        // The output starts 5 source pixels west of the source.
        let mesh = Mesh::from_fn(2, 2, w, h, |x, y| (x - 5.0, 10.0 - y)).unwrap();
        let out = warp(&Source { pixels: &px, width: w, height: h, bounds: b }, &mesh, w, h).unwrap();
        let at = |v: &[u8], x: u32, y: u32| v[((y * w + x) * 4) as usize..((y * w + x) * 4 + 4) as usize].to_vec();
        assert_eq!(at(&out, 2, 3)[3], 0, "west of the source: transparent");
        assert_eq!(at(&out, 7, 3), at(&px, 2, 3));
    }

    #[test]
    fn transparency_does_not_darken_edges() {
        // Red next to transparent black: halfway between, still pure red.
        let px = vec![255, 0, 0, 255, 0, 0, 0, 0];
        let mesh = Mesh::from_fn(2, 2, 1, 1, |_, _| (1.0, 0.5)).unwrap();
        let out = warp(&Source { pixels: &px, width: 2, height: 1, bounds: [0.0, 0.0, 2.0, 1.0] }, &mesh, 1, 1).unwrap();
        assert_eq!(out, [255, 0, 0, 128]);
    }

    #[test]
    fn invalid_nodes_and_the_date_line() {
        let px = gradient(8, 4);
        let src = Source { pixels: &px, width: 8, height: 4, bounds: [HALF - 4.0, 0.0, -HALF + 4.0, 4.0] };
        // Across the date line: a point just east of it is in the image's
        // east half.
        let mesh = Mesh::from_fn(2, 2, 1, 1, |_, _| (-HALF + 2.5, 1.5)).unwrap();
        let out = warp(&src, &mesh, 1, 1).unwrap();
        assert_eq!(&out[..4], &px[(2 * 8 + 6) * 4..(2 * 8 + 6) * 4 + 4]);
        // A node outside the CRS's area leaves its pixels transparent.
        let mesh = Mesh::new(2, 2, vec![(f64::NAN, 0.0), (1.0, 1.0), (1.0, 1.0), (1.0, 1.0)]).unwrap();
        assert_eq!(warp(&src, &mesh, 1, 1).unwrap(), [0, 0, 0, 0]);
        assert!(Mesh::new(1, 2, vec![(0.0, 0.0); 2]).is_err());
        assert!(Mesh::new(2, 2, vec![(0.0, 0.0); 3]).is_err());
    }
}
