//! A software model of the parts of MapLibre GL JS's WebGL pipeline used to
//! draw fills, lines and circles, so CPU renders match it pixel-for-pixel.
//!
//! - Primitives are sampled at pixel centres with the top-left rule, like a
//!   GPU rasteriser; there is no other antialiasing.
//! - The framebuffer is 8-bit premultiplied RGBA; every fragment is blended
//!   with `src + dst * (1 - src.a)` and rounded to 8 bits, like WebGL.
//! - Line geometry is a port of `LineBucket.addLine` and the line / line-SDF
//!   shaders; fill outlines model `gl.LINES` with the `fill_outline` shader;
//!   dashes use a port of `LineAtlas`.
//!
//! References are to maplibre-gl-js `src/data/bucket/line_bucket.ts`,
//! `src/render/line_atlas.ts`, `src/render/draw_fill.ts` and `src/shaders/*`.

/// MapLibre loads all tile geometry into an 8192-unit extent.
pub const EXTENT: f64 = 8192.0;

// ── Framebuffer ──────────────────────────────────────────────────────────────

pub struct Framebuffer {
    pub w: usize,
    pub h: usize,
    /// Premultiplied RGBA8.
    pub px: Vec<[u8; 4]>,
}

/// Float → unorm8 as GPUs do it: float32 multiply, round half to even
/// (SIMD conversion). 0.3 * 255 is exactly 76.5 in float32 and stores as 76.
#[inline]
fn to_unorm8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0f32).round_ties_even() as u8
}

impl Framebuffer {
    pub fn new(w: usize, h: usize) -> Self {
        Framebuffer { w, h, px: vec![[0; 4]; w * h] }
    }

    /// Blend a premultiplied fragment (ONE, ONE_MINUS_SRC_ALPHA).
    #[inline]
    pub fn blend(&mut self, x: usize, y: usize, c: [f32; 4]) {
        let d = &mut self.px[y * self.w + x];
        let ia = 1.0 - c[3];
        for i in 0..4 {
            d[i] = to_unorm8(c[i] + d[i] as f32 / 255.0 * ia);
        }
    }

    /// Straight-alpha RGBA8, as a canvas readback (`toDataURL`) returns it.
    pub fn into_straight_rgba(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.w * self.h * 4);
        for p in self.px {
            let a = p[3] as u32;
            if a == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                for c in &p[..3] {
                    out.push(((*c as u32 * 255 + a / 2) / a).min(255) as u8);
                }
                out.push(p[3]);
            }
        }
        out
    }
}

/// Per-tile stencil clip: a pixel belongs to the tile if its centre is in
/// `[x0, x1) × [y0, y1)` (top-left rule on the tile quad).
#[derive(Clone, Copy, Debug)]
pub struct Clip {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Clip {
    /// Integer pixel range whose centres fall inside the clip and canvas.
    fn px_range(&self, w: usize, h: usize) -> (i64, i64, i64, i64) {
        let cx0 = ((self.x0 - 0.5).ceil() as i64).max(0);
        let cy0 = ((self.y0 - 0.5).ceil() as i64).max(0);
        let cx1 = ((self.x1 - 0.5).ceil() as i64).min(w as i64);
        let cy1 = ((self.y1 - 0.5).ceil() as i64).min(h as i64);
        (cx0, cy0, cx1, cy1)
    }
}

// ── Triangle rasterisation ───────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct Vert<const N: usize> {
    pub x: f64,
    pub y: f64,
    pub v: [f64; N],
}

#[inline]
fn edge(ax: f64, ay: f64, bx: f64, by: f64, px: f64, py: f64) -> f64 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Top-left rule for an edge a→b of a triangle with positive `edge()` area
/// (y down): top edges are horizontal going +x, left edges go up (-y).
#[inline]
fn is_top_left(ax: f64, ay: f64, bx: f64, by: f64) -> bool {
    let (dx, dy) = (bx - ax, by - ay);
    (dy == 0.0 && dx > 0.0) || dy < 0.0
}

/// Rasterise a triangle, calling `frag(x, y, varyings)` for each covered pixel
/// centre inside `clip`. Varyings are interpolated linearly (no perspective).
pub fn raster_triangle<const N: usize>(
    w: usize,
    h: usize,
    tri: [Vert<N>; 3],
    clip: &Clip,
    mut frag: impl FnMut(usize, usize, &[f64; N]),
) {
    let [mut a, mut b, c] = tri;
    let mut area = edge(a.x, a.y, b.x, b.y, c.x, c.y);
    if area == 0.0 || !area.is_finite() {
        return;
    }
    if area < 0.0 {
        std::mem::swap(&mut a, &mut b);
        area = -area;
    }
    let (cx0, cy0, cx1, cy1) = clip.px_range(w, h);
    let minx = (a.x.min(b.x).min(c.x) - 0.5).ceil().max(cx0 as f64) as i64;
    let maxx = (a.x.max(b.x).max(c.x) - 0.5).floor().min(cx1 as f64 - 1.0) as i64;
    let miny = (a.y.min(b.y).min(c.y) - 0.5).ceil().max(cy0 as f64) as i64;
    let maxy = (a.y.max(b.y).max(c.y) - 0.5).floor().min(cy1 as f64 - 1.0) as i64;
    if minx > maxx || miny > maxy {
        return;
    }
    let tl = [
        is_top_left(b.x, b.y, c.x, c.y),
        is_top_left(c.x, c.y, a.x, a.y),
        is_top_left(a.x, a.y, b.x, b.y),
    ];
    let inv = 1.0 / area;
    let mut out = [0.0f64; N];
    for py in miny..=maxy {
        let yc = py as f64 + 0.5;
        for px in minx..=maxx {
            let xc = px as f64 + 0.5;
            let w0 = edge(b.x, b.y, c.x, c.y, xc, yc);
            let w1 = edge(c.x, c.y, a.x, a.y, xc, yc);
            let w2 = edge(a.x, a.y, b.x, b.y, xc, yc);
            let inside = (w0 > 0.0 || (w0 == 0.0 && tl[0]))
                && (w1 > 0.0 || (w1 == 0.0 && tl[1]))
                && (w2 > 0.0 || (w2 == 0.0 && tl[2]));
            if !inside {
                continue;
            }
            let (l0, l1, l2) = (w0 * inv, w1 * inv, w2 * inv);
            for i in 0..N {
                out[i] = a.v[i] * l0 + b.v[i] * l1 + c.v[i] * l2;
            }
            frag(px as usize, py as usize, &out);
        }
    }
}

// ── Fill interiors ───────────────────────────────────────────────────────────

/// Pixel-centre coverage of one polygon (exterior + holes, even-odd), the
/// same pixels earcut triangles cover under the top-left rule.
pub fn raster_polygon(
    w: usize,
    h: usize,
    rings: &[Vec<(f64, f64)>],
    clip: &Clip,
    mut frag: impl FnMut(usize, usize),
) {
    let mut edges: Vec<(f64, f64, f64, f64)> = Vec::new();
    let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
    for ring in rings {
        let n = ring.len();
        if n < 3 {
            continue;
        }
        for i in 0..n {
            let p = ring[i];
            let q = ring[(i + 1) % n];
            if p.1 != q.1 {
                edges.push((p.0, p.1, q.0, q.1));
            }
            ymin = ymin.min(p.1);
            ymax = ymax.max(p.1);
        }
    }
    if edges.is_empty() {
        return;
    }
    let (cx0, cy0, cx1, cy1) = clip.px_range(w, h);
    let row0 = ((ymin - 0.5).ceil() as i64).max(cy0);
    let row1 = ((ymax - 0.5).ceil() as i64).min(cy1);
    let mut xs: Vec<f64> = Vec::new();
    for py in row0..row1 {
        let yc = py as f64 + 0.5;
        xs.clear();
        for &(x0, y0, x1, y1) in &edges {
            let (lo, hi) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
            if yc >= lo && yc < hi {
                xs.push(x0 + (yc - y0) * (x1 - x0) / (y1 - y0));
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.chunks_exact(2) {
            let s = ((pair[0] - 0.5).ceil() as i64).max(cx0);
            let e = ((pair[1] - 0.5).ceil() as i64).min(cx1);
            for px in s..e {
                frag(px as usize, py as usize);
            }
        }
    }
}

// ── 1 px GL lines (fill outlines) ────────────────────────────────────────────

/// Fragments of a 1 px aliased `gl.LINES` segment (diamond-exit style: start
/// inclusive, end exclusive along the major axis), with the perpendicular
/// distance from each pixel centre to the line, which is what the
/// `fill_outline` shader's `length(v_pos - gl_FragCoord.xy)` evaluates to.
pub fn raster_gl_line(
    w: usize,
    h: usize,
    p0: (f64, f64),
    p1: (f64, f64),
    clip: &Clip,
    mut frag: impl FnMut(usize, usize, f64),
) {
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return;
    }
    let (cx0, cy0, cx1, cy1) = clip.px_range(w, h);
    let mut emit = |px: i64, py: i64| {
        if px < cx0 || px >= cx1 || py < cy0 || py >= cy1 {
            return;
        }
        let (xc, yc) = (px as f64 + 0.5, py as f64 + 0.5);
        let d = ((xc - p0.0) * dy - (yc - p0.1) * dx).abs() / len;
        frag(px as usize, py as usize, d);
    };
    // Note: a strict diamond-exit start rule (emitting the start pixel when
    // the segment begins past its centre) matched SwiftShader's output worse,
    // so the column/row-centre walk below is kept as is.
    if dx.abs() >= dy.abs() {
        // x-major: one fragment per column whose centre the segment spans.
        let (k0, k1) = if dx > 0.0 {
            ((p0.0 - 0.5).ceil() as i64, (p1.0 - 0.5).ceil() as i64)
        } else {
            ((p1.0 - 0.5).floor() as i64 + 1, (p0.0 - 0.5).floor() as i64 + 1)
        };
        for k in k0..k1 {
            let xc = k as f64 + 0.5;
            let y = p0.1 + (xc - p0.0) * dy / dx;
            emit(k, y.floor() as i64);
        }
    } else {
        let (k0, k1) = if dy > 0.0 {
            ((p0.1 - 0.5).ceil() as i64, (p1.1 - 0.5).ceil() as i64)
        } else {
            ((p1.1 - 0.5).floor() as i64 + 1, (p0.1 - 0.5).floor() as i64 + 1)
        };
        for k in k0..k1 {
            let yc = k as f64 + 0.5;
            let x = p0.0 + (yc - p0.1) * dx / dy;
            emit(x.floor() as i64, k);
        }
    }
}

#[inline]
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ── Line tessellation (port of LineBucket.addLine) ───────────────────────────

const EXTRUDE_SCALE: f64 = 63.0;
const SHARP_CORNER_OFFSET: f64 = 15.0;
const DEG_PER_TRIANGLE: f64 = 20.0;
const LINE_DISTANCE_SCALE: f64 = 0.5;
const MAX_LINE_DISTANCE: f64 = 32768.0; // 2^(15-1) / LINE_DISTANCE_SCALE

/// JavaScript `Math.round` (halves round toward +∞).
#[inline]
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// JavaScript ToUint8 (modulo 256 after truncation), as typed-array stores do.
#[inline]
fn to_uint8(x: f64) -> u8 {
    (x.trunc() as i64).rem_euclid(256) as u8
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct P {
    x: f64,
    y: f64,
}

impl P {
    fn sub(self, o: P) -> P {
        P { x: self.x - o.x, y: self.y - o.y }
    }
    fn add(self, o: P) -> P {
        P { x: self.x + o.x, y: self.y + o.y }
    }
    fn mult(self, k: f64) -> P {
        P { x: self.x * k, y: self.y * k }
    }
    fn mag(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    fn unit(self) -> P {
        let m = self.mag();
        P { x: self.x / m, y: self.y / m }
    }
    fn perp(self) -> P {
        P { x: -self.y, y: self.x }
    }
    fn round(self) -> P {
        P { x: js_round(self.x), y: js_round(self.y) }
    }
    fn dist(self, o: P) -> f64 {
        self.sub(o).mag()
    }
}

/// One line vertex as the line shaders see it after attribute decoding.
#[derive(Clone, Copy, Debug)]
pub struct LineVertex {
    /// Position in tile units (8192 extent).
    pub x: f64,
    pub y: f64,
    /// Extrusion vector, quantised to 1/63 as stored in `a_data.xy`.
    pub ex: f64,
    pub ey: f64,
    pub round: bool,
    pub up: bool,
    /// -1 / 0 / 1 (`a_direction`).
    pub dir: f64,
    /// `v_linesofar` in tile units, quantised as the shader decodes it.
    pub linesofar: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Join {
    Miter,
    Bevel,
    Round,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CurJoin {
    Miter,
    Bevel,
    FlipBevel,
    FakeRound,
    Round,
    Butt,
    Square,
}

pub struct LineBuilder {
    pub verts: Vec<LineVertex>,
    pub tris: Vec<[u32; 3]>,
    distance: f64,
    e1: i64,
    e2: i64,
    overscaling: f64,
}

impl LineBuilder {
    pub fn new(overscaling: f64) -> Self {
        LineBuilder { verts: Vec::new(), tris: Vec::new(), distance: 0.0, e1: -1, e2: -1, overscaling }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_line(
        &mut self,
        vertices: &[(f64, f64)],
        is_polygon: bool,
        join: Join,
        cap: Cap,
        miter_limit: f64,
        round_limit: f64,
    ) {
        let v: Vec<P> = vertices.iter().map(|&(x, y)| P { x, y }).collect();
        self.distance = 0.0;
        let mut len = v.len();
        while len >= 2 && v[len - 1] == v[len - 2] {
            len -= 1;
        }
        let mut first = 0;
        while first < len.saturating_sub(1) && v[first] == v[first + 1] {
            first += 1;
        }
        if len < if is_polygon { 3 } else { 2 } {
            return;
        }
        let miter_limit = if join == Join::Bevel { 1.05 } else { miter_limit };
        let sharp_corner_offset = if self.overscaling <= 16.0 {
            SHARP_CORNER_OFFSET * EXTENT / (512.0 * self.overscaling)
        } else {
            0.0
        };
        let cos_half_sharp_corner = (75.0f64 / 2.0).to_radians().cos();

        let mut current: Option<P> = None;
        let mut prev_vertex: Option<P>;
        let mut prev_normal: Option<P> = None;
        let mut next_normal: Option<P> = None;
        self.e1 = -1;
        self.e2 = -1;

        if is_polygon {
            current = Some(v[len - 2]);
            next_normal = Some(v[first].sub(v[len - 2]).unit().perp());
        }
        prev_vertex = None;

        let mut i = first;
        while i < len {
            let next_vertex = if i == len - 1 {
                if is_polygon { Some(v[first + 1]) } else { None }
            } else {
                Some(v[i + 1])
            };
            if let Some(nv) = next_vertex {
                if v[i] == nv {
                    i += 1;
                    continue;
                }
            }
            if next_normal.is_some() {
                prev_normal = next_normal;
            }
            if current.is_some() {
                prev_vertex = current;
            }
            let mut cur = v[i];
            current = Some(cur);

            next_normal = match next_vertex {
                Some(nv) => Some(nv.sub(cur).unit().perp()),
                None => prev_normal,
            };
            let pn = prev_normal.or(next_normal).unwrap();
            prev_normal = Some(pn);
            let nn = next_normal.unwrap();

            let mut join_normal = pn.add(nn);
            if join_normal.x != 0.0 || join_normal.y != 0.0 {
                join_normal = join_normal.unit();
            }
            let cos_angle = pn.x * nn.x + pn.y * nn.y;
            let cos_half_angle = join_normal.x * nn.x + join_normal.y * nn.y;
            let miter_length = if cos_half_angle != 0.0 { 1.0 / cos_half_angle } else { f64::INFINITY };
            let approx_angle = 2.0 * (2.0 - 2.0 * cos_half_angle).sqrt();
            let is_sharp_corner =
                cos_half_angle < cos_half_sharp_corner && prev_vertex.is_some() && next_vertex.is_some();
            let line_turns_left = pn.x * nn.y - pn.y * nn.x > 0.0;

            if is_sharp_corner && i > first {
                let pv = prev_vertex.unwrap();
                let prev_seg_len = cur.dist(pv);
                if prev_seg_len > 2.0 * sharp_corner_offset {
                    let new_prev = cur.sub(cur.sub(pv).mult(sharp_corner_offset / prev_seg_len).round());
                    self.distance += pv.dist(new_prev);
                    self.add_current_vertex(new_prev, pn, 0.0, 0.0, false);
                    prev_vertex = Some(new_prev);
                }
            }

            let middle = prev_vertex.is_some() && next_vertex.is_some();
            let mut cj = if middle {
                match join {
                    Join::Miter => CurJoin::Miter,
                    Join::Bevel => CurJoin::Bevel,
                    Join::Round => CurJoin::Round,
                }
            } else if is_polygon {
                CurJoin::Butt
            } else {
                match cap {
                    Cap::Butt => CurJoin::Butt,
                    Cap::Round => CurJoin::Round,
                    Cap::Square => CurJoin::Square,
                }
            };
            if middle && cj == CurJoin::Round {
                if miter_length < round_limit {
                    cj = CurJoin::Miter;
                } else if miter_length <= 2.0 {
                    cj = CurJoin::FakeRound;
                }
            }
            if cj == CurJoin::Miter && miter_length > miter_limit {
                cj = CurJoin::Bevel;
            }
            if cj == CurJoin::Bevel {
                if miter_length > 2.0 {
                    cj = CurJoin::FlipBevel;
                }
                if miter_length < miter_limit {
                    cj = CurJoin::Miter;
                }
            }

            if let Some(pv) = prev_vertex {
                self.distance += pv.dist(cur);
            }

            match cj {
                CurJoin::Miter => {
                    let jn = join_normal.mult(miter_length);
                    self.add_current_vertex(cur, jn, 0.0, 0.0, false);
                }
                CurJoin::FlipBevel => {
                    let jn = if miter_length > 100.0 {
                        nn.mult(-1.0)
                    } else {
                        let bevel_length = miter_length * pn.add(nn).mag() / pn.sub(nn).mag();
                        join_normal.perp().mult(bevel_length * if line_turns_left { -1.0 } else { 1.0 })
                    };
                    self.add_current_vertex(cur, jn, 0.0, 0.0, false);
                    self.add_current_vertex(cur, jn.mult(-1.0), 0.0, 0.0, false);
                }
                CurJoin::Bevel | CurJoin::FakeRound => {
                    let offset = -(miter_length * miter_length - 1.0).sqrt();
                    let offset_a = if line_turns_left { offset } else { 0.0 };
                    let offset_b = if line_turns_left { 0.0 } else { offset };
                    if prev_vertex.is_some() {
                        self.add_current_vertex(cur, pn, offset_a, offset_b, false);
                    }
                    if cj == CurJoin::FakeRound {
                        let n = js_round(approx_angle * 180.0 / std::f64::consts::PI / DEG_PER_TRIANGLE) as i64;
                        for m in 1..n {
                            let mut t = m as f64 / n as f64;
                            if t != 0.5 {
                                let t2 = t - 0.5;
                                let a = 1.0904 + cos_angle * (-3.2452 + cos_angle * (3.55645 - cos_angle * 1.43519));
                                let b = 0.848013 + cos_angle * (-1.06021 + cos_angle * 0.215638);
                                t = t + t * t2 * (t - 1.0) * (a * t2 * t2 + b);
                            }
                            let extrude = nn
                                .sub(pn)
                                .mult(t)
                                .add(pn)
                                .unit()
                                .mult(if line_turns_left { -1.0 } else { 1.0 });
                            self.add_half_vertex(cur, extrude.x, extrude.y, false, line_turns_left, 0.0);
                        }
                    }
                    if next_vertex.is_some() {
                        self.add_current_vertex(cur, nn, -offset_a, -offset_b, false);
                    }
                }
                CurJoin::Butt => self.add_current_vertex(cur, join_normal, 0.0, 0.0, false),
                CurJoin::Square => {
                    let o = if prev_vertex.is_some() { 1.0 } else { -1.0 };
                    self.add_current_vertex(cur, join_normal, o, o, false);
                }
                CurJoin::Round => {
                    if prev_vertex.is_some() {
                        self.add_current_vertex(cur, pn, 0.0, 0.0, false);
                        self.add_current_vertex(cur, pn, 1.0, 1.0, true);
                    }
                    if next_vertex.is_some() {
                        self.add_current_vertex(cur, nn, -1.0, -1.0, true);
                        self.add_current_vertex(cur, nn, 0.0, 0.0, false);
                    }
                }
            }

            if is_sharp_corner && i < len - 1 {
                let nv = next_vertex.unwrap();
                let next_seg_len = cur.dist(nv);
                if next_seg_len > 2.0 * sharp_corner_offset {
                    let new_cur = cur.add(nv.sub(cur).mult(sharp_corner_offset / next_seg_len).round());
                    self.distance += cur.dist(new_cur);
                    self.add_current_vertex(new_cur, nn, 0.0, 0.0, false);
                    cur = new_cur;
                    current = Some(cur);
                }
            }
            i += 1;
        }
    }

    fn add_current_vertex(&mut self, p: P, normal: P, end_left: f64, end_right: f64, round: bool) {
        let lx = normal.x + normal.y * end_left;
        let ly = normal.y - normal.x * end_left;
        let rx = -normal.x + normal.y * end_right;
        let ry = -normal.y - normal.x * end_right;
        self.add_half_vertex(p, lx, ly, round, false, end_left);
        self.add_half_vertex(p, rx, ry, round, true, -end_right);
        if self.distance > MAX_LINE_DISTANCE / 2.0 {
            self.distance = 0.0;
            self.add_current_vertex(p, normal, end_left, end_right, round);
        }
    }

    fn add_half_vertex(&mut self, p: P, ex: f64, ey: f64, round: bool, up: bool, dir: f64) {
        // Attribute encoding round trip (a_data is 4 x u8).
        let qx = to_uint8(js_round(EXTRUDE_SCALE * ex) + 128.0) as f64 - 128.0;
        let qy = to_uint8(js_round(EXTRUDE_SCALE * ey) + 128.0) as f64 - 128.0;
        let scaled = (self.distance * LINE_DISTANCE_SCALE).trunc() as i64;
        let z = to_uint8((((if dir == 0.0 { 0 } else if dir < 0.0 { -1 } else { 1 }) + 1) | ((scaled & 0x3F) << 2)) as f64);
        let wv = to_uint8((scaled >> 6) as f64);
        let linesofar = ((z as f64 / 4.0).floor() + wv as f64 * 64.0) * 2.0;
        let a_dir = (z as f64 % 4.0) - 1.0;
        self.verts.push(LineVertex {
            x: p.x,
            y: p.y,
            ex: qx / EXTRUDE_SCALE,
            ey: qy / EXTRUDE_SCALE,
            round,
            up,
            dir: a_dir,
            linesofar,
        });
        let e = (self.verts.len() - 1) as i64;
        if self.e1 >= 0 && self.e2 >= 0 {
            self.tris.push([self.e1 as u32, self.e2 as u32, e as u32]);
        }
        if up {
            self.e2 = e;
        } else {
            self.e1 = e;
        }
    }
}

// ── Dash atlas (port of LineAtlas) ───────────────────────────────────────────

pub const ATLAS_W: usize = 256;

pub struct Dash {
    /// Rows of signed-distance bytes, `2n + 1` rows of `ATLAS_W`.
    pub rows: Vec<Vec<u8>>,
    pub n: usize,
    /// Sum of the dasharray (`dashEntry.width`).
    pub width: f64,
}

struct Range {
    left: f64,
    right: f64,
    is_dash: bool,
    zero_length: bool,
}

fn dash_ranges(dasharray: &[f64], stretch: f64) -> Vec<Range> {
    let odd = dasharray.len() % 2 == 1;
    let mut ranges = Vec::new();
    let left = if odd { -dasharray[dasharray.len() - 1] * stretch } else { 0.0 };
    let right = dasharray[0] * stretch;
    let mut is_dash = true;
    ranges.push(Range { left, right, is_dash, zero_length: dasharray[0] == 0.0 });
    let mut current = dasharray[0];
    for &d in &dasharray[1..] {
        is_dash = !is_dash;
        let left = current * stretch;
        current += d;
        let right = current * stretch;
        ranges.push(Range { left, right, is_dash, zero_length: d == 0.0 });
    }
    ranges
}

impl Dash {
    pub fn new(dasharray: &[f64], round: bool) -> Dash {
        let n = if round { 7 } else { 0 };
        let width: f64 = dasharray.iter().sum();
        let mut rows = vec![vec![0u8; ATLAS_W]; 2 * n + 1];
        if width != 0.0 {
            let stretch = ATLAS_W as f64 / width;
            let mut ranges = dash_ranges(dasharray, stretch);
            if round {
                let half = stretch / 2.0;
                for (ri, yy) in (-(n as i64)..=n as i64).enumerate() {
                    let mut ci = 0;
                    for x in 0..ATLAS_W {
                        let xf = x as f64;
                        if xf / ranges[ci].right > 1.0 && ci + 1 < ranges.len() {
                            ci += 1;
                        }
                        let r = &ranges[ci];
                        let min_dist = (xf - r.left).abs().min((xf - r.right).abs());
                        let dist_middle = yy as f64 / n as f64 * (half + 1.0);
                        let sd = if r.is_dash {
                            let dist_edge = half - dist_middle.abs();
                            (min_dist * min_dist + dist_edge * dist_edge).sqrt()
                        } else {
                            half - (min_dist * min_dist + dist_middle * dist_middle).sqrt()
                        };
                        rows[ri][x] = to_uint8((sd + 128.0).clamp(0.0, 255.0));
                    }
                }
            } else {
                // Collapse zero-length ranges and merge neighbours of the same type.
                let mut i = ranges.len() as i64 - 1;
                while i >= 0 {
                    let iu = i as usize;
                    if ranges[iu].zero_length {
                        ranges.remove(iu);
                    } else if iu + 1 < ranges.len() && ranges[iu + 1].is_dash == ranges[iu].is_dash {
                        ranges[iu + 1].left = ranges[iu].left;
                        ranges.remove(iu);
                    }
                    i -= 1;
                }
                if !ranges.is_empty() {
                    let last = ranges.len() - 1;
                    if ranges[0].is_dash == ranges[last].is_dash {
                        let (fl, lr) = (ranges[last].left, ranges[0].right);
                        ranges[0].left = fl - ATLAS_W as f64;
                        ranges[last].right = lr + ATLAS_W as f64;
                    }
                    let mut ci = 0;
                    for x in 0..ATLAS_W {
                        let xf = x as f64;
                        if xf / ranges[ci].right > 1.0 && ci + 1 < ranges.len() {
                            ci += 1;
                        }
                        let r = &ranges[ci];
                        let min_dist = (xf - r.left).abs().min((xf - r.right).abs());
                        let sd = if r.is_dash { min_dist } else { -min_dist };
                        rows[0][x] = to_uint8((sd + 128.0).clamp(0.0, 255.0));
                    }
                }
            }
        }
        Dash { rows, n, width }
    }

    /// Bilinear sample (REPEAT in s) at texture s and local row coordinate
    /// `r` in `[0, 2n]` (row centres at integers). Returns 0..=1.
    pub fn sample(&self, s: f64, r: f64) -> f64 {
        let x = s * ATLAS_W as f64 - 0.5;
        let x0 = x.floor();
        let fx = x - x0;
        let i0 = (x0 as i64).rem_euclid(ATLAS_W as i64) as usize;
        let i1 = (i0 + 1) % ATLAS_W;
        let row = |ri: usize| {
            let rr = &self.rows[ri];
            rr[i0] as f64 * (1.0 - fx) + rr[i1] as f64 * fx
        };
        let v = if self.n == 0 {
            row(0)
        } else {
            let r = r.clamp(0.0, (2 * self.n) as f64);
            let r0 = r.floor() as usize;
            let r1 = (r0 + 1).min(2 * self.n);
            let fr = r - r0 as f64;
            row(r0) * (1.0 - fr) + row(r1) * fr
        };
        v / 255.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_left_rule_shares_edges_exactly_once() {
        // Two triangles forming a 4x4 square; every pixel centre hit once.
        let clip = Clip { x0: -100.0, y0: -100.0, x1: 100.0, y1: 100.0 };
        let mut hits = vec![0u8; 16];
        let v = |x, y| Vert::<0> { x, y, v: [] };
        for tri in [[v(0.0, 0.0), v(4.0, 0.0), v(4.0, 4.0)], [v(0.0, 0.0), v(4.0, 4.0), v(0.0, 4.0)]] {
            raster_triangle(4, 4, tri, &clip, |x, y, _| hits[y * 4 + x] += 1);
        }
        assert!(hits.iter().all(|&h| h == 1), "{hits:?}");
    }

    #[test]
    fn polygon_scanline_matches_square() {
        let clip = Clip { x0: 0.0, y0: 0.0, x1: 8.0, y1: 8.0 };
        let mut n = 0;
        let ring = vec![(1.0, 1.0), (5.0, 1.0), (5.0, 5.0), (1.0, 5.0)];
        raster_polygon(8, 8, &[ring], &clip, |_, _| n += 1);
        assert_eq!(n, 16);
    }

    #[test]
    fn straight_line_builds_quad() {
        let mut b = LineBuilder::new(1.0);
        b.add_line(&[(0.0, 0.0), (100.0, 0.0)], false, Join::Miter, Cap::Butt, 2.0, 1.05);
        assert_eq!(b.verts.len(), 4);
        assert_eq!(b.tris.len(), 2);
        // Left extrude of an eastward line is +y (south) in tile space.
        assert_eq!((b.verts[0].ex, b.verts[0].ey), (0.0, 1.0));
        assert!(!b.verts[0].up && b.verts[1].up);
        assert_eq!(b.verts[2].linesofar, 100.0);
    }

    #[test]
    fn dash_atlas_regular_pattern() {
        let d = Dash::new(&[2.0, 2.0], false);
        assert_eq!(d.width, 4.0);
        // Middle of the first dash is far inside (> 0.5), middle of gap is outside.
        assert!(d.sample(0.125, 0.0) > 0.5);
        assert!(d.sample(0.625, 0.0) < 0.5);
    }
}
