//! `hillshade` layers over `raster-dem` sources: a CPU port of MapLibre GL
//! JS's two-pass hillshading.
//!
//! 1. DEMData: each DEM tile image is decoded into a stride `dim + 2` buffer
//!    whose 1 px border repeats the edge pixels, then is backfilled from the
//!    neighbouring tiles that are loaded (MapLibre backfills only from tiles
//!    it has, i.e. the view's covering tiles).
//! 2. hillshade_prepare.fragment.glsl: per DEM pixel, the Sobel x / y slope
//!    from the 3 × 3 neighbourhood, vertically exaggerated below z15, stored
//!    in an RGBA8 texture as `deriv / 8 + 0.5`.
//! 3. hillshade.fragment.glsl: per screen pixel, the slope texture sampled
//!    `LINEAR` (or `NEAREST` with `resampling: nearest`), corrected for the
//!    Mercator scale at the pixel's latitude and shaded by `hillshade-method`
//!    (standard, basic, combined, igor, multidirectional), blended
//!    premultiplied. Tiles are placed with MapLibre's pixel-aligned raster
//!    matrix.

use super::expr::{eval_property, Color, Ctx, Kind, Val};
use super::gl::Framebuffer;
use crate::tile_fetcher::TileCoord;
use serde_json::Value as Json;
use std::collections::HashMap;
use std::f64::consts::PI;

const EARTH_HALF: f64 = 20_037_508.342_789_244;
const TILE_PX: f64 = 512.0;

/// DEM tiles of one `raster-dem` source at the zoom MapLibre covers the view
/// with, and the source's unpack vector (`redFactor`, `greenFactor`,
/// `blueFactor`, `baseShift`).
pub struct DemTiles {
    pub zoom: u8,
    pub tiles: Vec<(TileCoord, Vec<u8>)>,
    pub unpack: [f64; 4],
}

/// The unpack vector of a `raster-dem` source (DEMData's constructor).
pub fn unpack_vector(src: &Json) -> [f64; 4] {
    match src["encoding"].as_str() {
        Some("terrarium") => [256.0, 1.0, 1.0 / 256.0, 32768.0],
        Some("custom") => [
            src["redFactor"].as_f64().unwrap_or(1.0),
            src["greenFactor"].as_f64().unwrap_or(1.0),
            src["blueFactor"].as_f64().unwrap_or(1.0),
            src["baseShift"].as_f64().unwrap_or(0.0),
        ],
        _ => [6553.6, 25.6, 0.1, 10000.0],
    }
}

/// A decoded DEM tile: elevations (metres) with a 1 px border.
struct Dem {
    dim: usize,
    /// `(dim + 2)²` elevations, row-major, `(-1, -1)` first.
    ele: Vec<f64>,
}

impl Dem {
    fn decode(bytes: &[u8], unpack: [f64; 4]) -> Option<Dem> {
        let img = image::load_from_memory(bytes).map_err(|e| log::warn!("DEM tile: {e}")).ok()?.to_rgba8();
        if img.width() != img.height() || img.width() == 0 {
            log::warn!("DEM tiles must be square");
            return None;
        }
        let dim = img.width() as usize;
        let stride = dim + 2;
        let mut ele = vec![0.0; stride * stride];
        for (x, y, p) in img.enumerate_pixels() {
            ele[(y as usize + 1) * stride + x as usize + 1] =
                p[0] as f64 * unpack[0] + p[1] as f64 * unpack[1] + p[2] as f64 * unpack[2] - unpack[3];
        }
        let mut d = Dem { dim, ele };
        let n = dim as i64;
        for i in 0..n {
            d.set(-1, i, d.get(0, i));
            d.set(n, i, d.get(n - 1, i));
            d.set(i, -1, d.get(i, 0));
            d.set(i, n, d.get(i, n - 1));
        }
        d.set(-1, -1, d.get(0, 0));
        d.set(n, -1, d.get(n - 1, 0));
        d.set(-1, n, d.get(0, n - 1));
        d.set(n, n, d.get(n - 1, n - 1));
        Some(d)
    }

    #[inline]
    fn idx(&self, x: i64, y: i64) -> usize {
        ((y + 1) as usize) * (self.dim + 2) + (x + 1) as usize
    }
    #[inline]
    fn get(&self, x: i64, y: i64) -> f64 {
        self.ele[self.idx(x, y)]
    }
    #[inline]
    fn set(&mut self, x: i64, y: i64, v: f64) {
        let i = self.idx(x, y);
        self.ele[i] = v;
    }

    /// DEMData#backfillBorder: copy the border strip facing `other`, the
    /// neighbour at offset (dx, dy).
    fn backfill(&mut self, other: &Dem, dx: i64, dy: i64) {
        if self.dim != other.dim {
            return;
        }
        let n = self.dim as i64;
        let (mut x0, mut x1, mut y0, mut y1) = (dx * n, dx * n + n, dy * n, dy * n + n);
        match dx {
            -1 => x0 = x1 - 1,
            1 => x1 = x0 + 1,
            _ => {}
        }
        match dy {
            -1 => y0 = y1 - 1,
            1 => y1 = y0 + 1,
            _ => {}
        }
        let (ox, oy) = (-dx * n, -dy * n);
        for y in y0..y1 {
            for x in x0..x1 {
                let v = other.get(x + ox, y + oy);
                self.set(x, y, v);
            }
        }
    }

    /// hillshade_prepare.fragment.glsl: the slope texture (R, G), `dim²`.
    fn prepare(&self, z: u8) -> Vec<[u8; 2]> {
        let n = self.dim as i64;
        let zf = z as f64;
        let factor = if zf < 2.0 { 0.4 } else if zf < 4.5 { 0.35 } else { 0.3 };
        let exaggeration = if zf < 15.0 { (zf - 15.0) * factor } else { 0.0 };
        let k = self.dim as f64 / 2f64.powf(exaggeration + (28.2562 - zf));
        let unorm = |v: f64| ((v / 8.0 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
        let mut out = Vec::with_capacity(self.dim * self.dim);
        for y in 0..n {
            for x in 0..n {
                let e = |dx: i64, dy: i64| self.get(x + dx, y + dy);
                let (a, b, c) = (e(-1, -1), e(0, -1), e(1, -1));
                let (d, f) = (e(-1, 0), e(1, 0));
                let (g, h, i) = (e(-1, 1), e(0, 1), e(1, 1));
                let dx = ((c + f + f + i) - (a + d + d + g)) * k;
                let dy = ((g + h + h + i) - (a + b + b + c)) * k;
                out.push([unorm(dx), unorm(dy)]);
            }
        }
        out
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Method {
    Standard,
    Basic,
    Combined,
    Igor,
    Multidirectional,
}

/// The evaluated hillshade paint properties (uniforms).
struct Shading {
    method: Method,
    exaggeration: f64,
    accent: [f64; 4],
    /// Per illumination source: azimuth (rad), altitude (rad), shadow, highlight.
    azimuths: Vec<f64>,
    altitudes: Vec<f64>,
    shadows: Vec<[f64; 4]>,
    highlights: Vec<[f64; 4]>,
    nearest: bool,
}

fn c4(c: Color) -> [f64; 4] {
    [c.r as f64, c.g as f64, c.b as f64, c.a as f64]
}

/// A number-or-array property (`hillshade-illumination-direction`, ...).
fn numbers(p: &Json, key: &str, ctx: &Ctx, default: f64) -> Vec<f64> {
    let Some(j) = p.get(key).filter(|j| !j.is_null()) else { return vec![default] };
    let v = match j {
        Json::Array(a) if a.first().map_or(false, |f| f.is_number()) => Val::from_json(j),
        _ => eval_property(j, Kind::Array, ctx),
    };
    let out: Vec<f64> = match v {
        Val::Num(n) => vec![n],
        Val::Array(a) => a.iter().filter_map(|v| if let Val::Num(n) = v { Some(*n) } else { None }).collect(),
        _ => vec![],
    };
    if out.is_empty() { vec![default] } else { out }
}

/// A color-or-array property (`hillshade-shadow-color`, ...).
fn colors(p: &Json, key: &str, ctx: &Ctx, default: &str) -> Vec<[f64; 4]> {
    let fallback = || vec![c4(Color::parse(default).unwrap())];
    let Some(j) = p.get(key).filter(|j| !j.is_null()) else { return fallback() };
    // A literal array of color strings (its head is a color, not an operator).
    if let Json::Array(a) = j {
        let parsed: Option<Vec<[f64; 4]>> = a.iter().map(|s| s.as_str().and_then(Color::parse).map(c4)).collect();
        if let Some(cs) = parsed.filter(|c| !c.is_empty()) {
            return cs;
        }
    }
    let out: Vec<[f64; 4]> = match eval_property(j, Kind::Color, ctx) {
        Val::Color(c) => vec![c4(c)],
        Val::Array(a) => a
            .iter()
            .filter_map(|v| match v {
                Val::Color(c) => Some(c4(*c)),
                Val::Str(s) => Color::parse(s).map(c4),
                _ => None,
            })
            .collect(),
        Val::Str(s) => Color::parse(&s).map(c4).into_iter().collect(),
        _ => vec![],
    };
    if out.is_empty() { fallback() } else { out }
}

impl Shading {
    fn new(layer: &Json, zoom: f64) -> Shading {
        let ctx = Ctx { zoom, feature: None };
        let p = &layer["paint"];
        let num = |k: &str, d: f64| match p.get(k).filter(|j| !j.is_null()) {
            Some(j) => match eval_property(j, Kind::Number, &ctx) {
                Val::Num(n) => n,
                _ => d,
            },
            None => d,
        };
        let enum_ = |k: &str, d: &str| match p.get(k).filter(|j| !j.is_null()) {
            Some(j) => match eval_property(j, Kind::Enum, &ctx) {
                Val::Str(s) => s,
                _ => d.to_owned(),
            },
            None => d.to_owned(),
        };
        let method = match enum_("hillshade-method", "standard").as_str() {
            "basic" => Method::Basic,
            "combined" => Method::Combined,
            "igor" => Method::Igor,
            "multidirectional" => Method::Multidirectional,
            _ => Method::Standard,
        };
        let mut dir = numbers(p, "hillshade-illumination-direction", &ctx, 335.0);
        let mut alt = numbers(p, "hillshade-illumination-altitude", &ctx, 45.0);
        let mut hi = colors(p, "hillshade-highlight-color", &ctx, "#FFFFFF");
        let mut sh = colors(p, "hillshade-shadow-color", &ctx, "#000000");
        // getIlluminationProperties: pad every list with its last value.
        let n = dir.len().max(alt.len()).max(hi.len()).max(sh.len());
        fn pad<T: Copy>(v: &mut Vec<T>, n: usize) {
            let last = *v.last().unwrap();
            v.resize(n, last);
        }
        pad(&mut dir, n);
        pad(&mut alt, n);
        pad(&mut hi, n);
        pad(&mut sh, n);
        // The bearing is always 0 here, so `hillshade-illumination-anchor`
        // (viewport or map) does not change the azimuth.
        let accent = colors(p, "hillshade-accent-color", &ctx, "#000000")[0];
        Shading {
            method,
            exaggeration: num("hillshade-exaggeration", 0.5),
            accent,
            azimuths: dir.iter().map(|d| d.to_radians()).collect(),
            altitudes: alt.iter().map(|a| a.to_radians()).collect(),
            shadows: sh,
            highlights: hi,
            nearest: enum_("resampling", "linear") == "nearest",
        }
    }

    /// hillshade.fragment.glsl's methods: premultiplied RGBA for `deriv`.
    fn shade(&self, deriv: [f64; 2]) -> [f64; 4] {
        let add = |a: [f64; 4], b: [f64; 4]| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]];
        let mul = |a: [f64; 4], k: f64| [a[0] * k, a[1] * k, a[2] * k, a[3] * k];
        let len = (deriv[0] * deriv[0] + deriv[1] * deriv[1]).sqrt();
        let aspect = |d: [f64; 2]| if d[0] != 0.0 { d[1].atan2(-d[0]) } else { PI / 2.0 * if d[1] > 0.0 { 1.0 } else { -1.0 } };
        // GLSL mod: x - y * floor(x / y).
        let gmod = |x: f64, y: f64| x - y * (x / y).floor();
        let cang_of = |d: [f64; 2], cos_az: f64, sin_az: f64, alt: f64| {
            let (cos_alt, sin_alt) = (alt.cos(), alt.sin());
            (sin_alt - (d[1] * cos_az * cos_alt - d[0] * sin_az * cos_alt)) / (1.0 + d[0] * d[0] + d[1] * d[1]).sqrt()
        };
        match self.method {
            Method::Standard => {
                let azimuth = self.azimuths[0] + PI;
                let slope = (0.625 * len).atan();
                let intensity = self.exaggeration;
                let base = 1.875 - intensity * 1.75;
                let max_value = 0.5 * PI;
                let scaled = if intensity != 0.5 {
                    ((base.powf(slope) - 1.0) / (base.powf(max_value) - 1.0)) * max_value
                } else {
                    slope
                };
                let accent = scaled.cos();
                let k = (intensity * 2.0).clamp(0.0, 1.0);
                let accent_color = mul(self.accent, (1.0 - accent) * k);
                let shade = (gmod((aspect(deriv) + azimuth) / PI + 0.5, 2.0) - 1.0).abs();
                let (s, h) = (self.shadows[0], self.highlights[0]);
                let mix = [0, 1, 2, 3].map(|i| s[i] + (h[i] - s[i]) * shade);
                let shade_color = mul(mix, scaled.sin() * k);
                add(mul(accent_color, 1.0 - shade_color[3]), shade_color)
            }
            Method::Basic => {
                let d = [deriv[0] * self.exaggeration * 2.0, deriv[1] * self.exaggeration * 2.0];
                let azimuth = self.azimuths[0] + PI;
                let shade = cang_of(d, azimuth.cos(), azimuth.sin(), self.altitudes[0]).clamp(0.0, 1.0);
                if shade > 0.5 {
                    mul(self.highlights[0], 2.0 * shade - 1.0)
                } else {
                    mul(self.shadows[0], 1.0 - 2.0 * shade)
                }
            }
            Method::Multidirectional => {
                let d = [deriv[0] * self.exaggeration * 2.0, deriv[1] * self.exaggeration * 2.0];
                let n = self.azimuths.len() as f64;
                let mut out = [0.0; 4];
                for i in 0..self.azimuths.len() {
                    let az = self.azimuths[i];
                    let shade = cang_of(d, -az.cos(), -az.sin(), self.altitudes[i]).clamp(0.0, 1.0);
                    out = if shade > 0.5 {
                        add(out, mul(self.highlights[i], (2.0 * shade - 1.0) / n))
                    } else {
                        add(out, mul(self.shadows[i], (1.0 - 2.0 * shade) / n))
                    };
                }
                out
            }
            Method::Combined => {
                let d = [deriv[0] * self.exaggeration * 2.0, deriv[1] * self.exaggeration * 2.0];
                let azimuth = self.azimuths[0] + PI;
                let cang = cang_of(d, azimuth.cos(), azimuth.sin(), self.altitudes[0]).clamp(-1.0, 1.0).acos().clamp(0.0, PI / 2.0);
                let l = (d[0] * d[0] + d[1] * d[1]).sqrt().atan();
                let shade = cang * l * 4.0 / PI / PI;
                let highlight = (PI / 2.0 - cang) * l * 4.0 / PI / PI;
                add(mul(self.shadows[0], shade), mul(self.highlights[0], highlight))
            }
            Method::Igor => {
                let d = [deriv[0] * self.exaggeration * 2.0, deriv[1] * self.exaggeration * 2.0];
                let azimuth = self.azimuths[0] + PI;
                let slope = (d[0] * d[0] + d[1] * d[1]).sqrt().atan() * 2.0 / PI;
                let aspect_strength = 1.0 - (gmod((aspect(d) + azimuth) / PI + 0.5, 2.0) - 1.0).abs();
                add(mul(self.shadows[0], slope * aspect_strength), mul(self.highlights[0], slope * (1.0 - aspect_strength)))
            }
        }
    }
}

/// Latitude (degrees) of the Mercator y in [0, 1].
fn lat_of(y: f64) -> f64 {
    let y2 = 180.0 - y * 360.0;
    360.0 / PI * (y2 * PI / 180.0).exp().atan() - 90.0
}

/// Whether the slope textures of `source` are sampled `NEAREST`. MapLibre
/// prepares one texture per DEM tile, shared by every hillshade layer of the
/// source, and sets its filter when the first layer (in style order) with an
/// offscreen pass creates it: that layer's `resampling` applies to all.
pub fn source_nearest(style: &Json, source: &str, zoom: f64) -> bool {
    let layers = style["layers"].as_array().map(|v| v.as_slice()).unwrap_or(&[]);
    layers
        .iter()
        .find(|l| {
            l["type"] == "hillshade"
                && l["source"] == source
                && super::symbol::layer_visible(l, zoom)
                && Shading::new(l, zoom).exaggeration != 0.0
        })
        .map_or(false, |l| Shading::new(l, zoom).nearest)
}

/// Draw a `hillshade` layer from `dem` into `fb` (device pixels) for the view
/// `bbox` at `width × height` CSS pixels, scale zoom `zoom`. `nearest`: the
/// source's texture filter (see [`source_nearest`]).
#[allow(clippy::too_many_arguments)]
pub fn draw_layer(
    fb: &mut Framebuffer,
    layer: &Json,
    zoom: f64,
    dem: &DemTiles,
    bbox: [f64; 4],
    width: u32,
    height: u32,
    ratio: f64,
    nearest: bool,
) {
    let mut sh = Shading::new(layer, zoom);
    sh.nearest = nearest;
    // hasOffscreenPass: nothing is prepared (or drawn) at exaggeration 0.
    if sh.exaggeration == 0.0 {
        return;
    }
    // Decode, then backfill borders from loaded neighbours.
    let mut dems: HashMap<(u32, u32), Dem> = HashMap::new();
    for (c, bytes) in &dem.tiles {
        if let Some(d) = Dem::decode(bytes, dem.unpack) {
            dems.insert((c.x, c.y), d);
        }
    }
    let z = dem.zoom;
    let n = 1i64 << z;
    let keys: Vec<(u32, u32)> = dems.keys().copied().collect();
    for &(x, y) in &keys {
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let ny = y as i64 + dy;
                if ny < 0 || ny >= n {
                    continue;
                }
                let nx = (x as i64 + dx).rem_euclid(n);
                let Some(other) = dems.get(&(nx as u32, ny as u32)) else { continue };
                // Borrow juggling: the neighbour is read while this tile is written.
                let other = Dem { dim: other.dim, ele: other.ele.clone() };
                dems.get_mut(&(x, y)).unwrap().backfill(&other, dx, dy);
            }
        }
    }

    // MapLibre's pixel-aligned raster projection (alignedProjMatrix).
    let world_px = TILE_PX * 2f64.powf(zoom);
    let cx = ((bbox[0] + bbox[2]) / 2.0 + EARTH_HALF) / (2.0 * EARTH_HALF) * world_px;
    let cy = (EARTH_HALF - (bbox[1] + bbox[3]) / 2.0) / (2.0 * EARTH_HALF) * world_px;
    let align = |c: f64, size: u32| {
        let shift = (size % 2) as f64 / 2.0;
        let d = c - c.round() + shift;
        c - if d > 0.5 { d - 1.0 } else { d }
    };
    let view_x0 = align(cx, width) - width as f64 / 2.0;
    let view_y0 = align(cy, height) - height as f64 / 2.0;
    let tile_px = world_px / n as f64;

    let mut keys = keys;
    keys.sort();
    for (tx, ty) in keys {
        let d = &dems[&(tx, ty)];
        let tex = d.prepare(z);
        let dim = d.dim;
        let lat0 = lat_of(ty as f64 / n as f64);
        let lat1 = lat_of((ty + 1) as f64 / n as f64);
        let ox = (tx as f64 * tile_px - view_x0) * ratio;
        let oy = (ty as f64 * tile_px - view_y0) * ratio;
        let size = tile_px * ratio;
        // Pixels whose centres fall inside the tile.
        let px0 = (ox - 0.5).ceil().max(0.0) as i64;
        let py0 = (oy - 0.5).ceil().max(0.0) as i64;
        let px1 = ((ox + size - 0.5).ceil() as i64).min(fb.w as i64);
        let py1 = ((oy + size - 0.5).ceil() as i64).min(fb.h as i64);
        let texel = |x: i64, y: i64| {
            let x = x.clamp(0, dim as i64 - 1) as usize;
            let y = y.clamp(0, dim as i64 - 1) as usize;
            let t = tex[y * dim + x];
            [t[0] as f64 / 255.0, t[1] as f64 / 255.0]
        };
        for py in py0..py1 {
            let v = (py as f64 + 0.5 - oy) / size;
            let scale_factor = ((lat0 - lat1) * (1.0 - v) + lat1).to_radians().cos();
            for px in px0..px1 {
                let u = (px as f64 + 0.5 - ox) / size;
                let rg = if sh.nearest {
                    texel((u * dim as f64).floor() as i64, (v * dim as f64).floor() as i64)
                } else {
                    let (s, t) = (u * dim as f64 - 0.5, v * dim as f64 - 0.5);
                    let (i0, j0) = (s.floor(), t.floor());
                    let (fx, fy) = (s - i0, t - j0);
                    let (i0, j0) = (i0 as i64, j0 as i64);
                    let (a, b, c, e) = (texel(i0, j0), texel(i0 + 1, j0), texel(i0, j0 + 1), texel(i0 + 1, j0 + 1));
                    [0, 1].map(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + e[k] * fx) * fy)
                };
                let deriv = [(rg[0] * 8.0 - 4.0) / scale_factor, (rg[1] * 8.0 - 4.0) / scale_factor];
                let c = sh.shade(deriv).map(|v| v.clamp(0.0, 1.0) as f32);
                if c[3] > 0.0 || c[0] > 0.0 || c[1] > 0.0 || c[2] > 0.0 {
                    fb.blend(px as usize, py as usize, c);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut img = image::RgbaImage::new(w, w);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let c = f(x, y);
            *p = image::Rgba([c[0], c[1], c[2], 255]);
        }
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    #[test]
    fn flat_dem_has_zero_slope_and_draws_nothing() {
        let unpack = unpack_vector(&serde_json::json!({"encoding": "terrarium"}));
        let d = Dem::decode(&png(8, |_, _| [128, 0, 0]), unpack).unwrap();
        assert_eq!(d.get(3, 3), 0.0);
        // deriv 0 → 0.5 → 128 (rounded).
        assert!(d.prepare(10).iter().all(|t| *t == [128, 128]));
        let sh = Shading::new(&serde_json::json!({"paint": {}}), 10.0);
        // A flat pixel (128/255 → deriv ≈ 0.0157) is almost transparent.
        let c = sh.shade([0.0, 0.0]);
        assert!(c[3].abs() < 1e-9, "{c:?}");
    }

    #[test]
    fn east_facing_slope_is_shaded_by_the_northwest_light() {
        // Elevation falling to the east (mapbox encoding: blue steps of 0.1 m).
        let d = Dem::decode(&png(16, |x, _| [1, 134, (255 - x * 10) as u8]), unpack_vector(&Json::Null)).unwrap();
        let tex = d.prepare(12);
        assert!(tex[5 * 16 + 5][0] < 128, "x slope negative: {:?}", tex[5 * 16 + 5]);
        assert_eq!(tex[5 * 16 + 5][1], 128);
        // Shadow side under the default light from 335°.
        let sh = Shading::new(&serde_json::json!({"paint": {}}), 12.0);
        let c = sh.shade([-1.0, 0.0]);
        assert!(c[3] > 0.0 && c[0] < c[3], "{c:?}");
    }

    #[test]
    fn backfill_copies_the_neighbour_edge() {
        let unpack = unpack_vector(&serde_json::json!({"encoding": "terrarium"}));
        let mut a = Dem::decode(&png(4, |_, _| [128, 0, 0]), unpack).unwrap();
        let b = Dem::decode(&png(4, |x, _| [128, x as u8 * 10, 0]), unpack).unwrap();
        a.backfill(&b, 1, 0);
        // a's right border is b's first column (x = 0 → 0 m).
        assert_eq!(a.get(4, 2), 0.0);
        a.backfill(&b, -1, 0);
        // a's left border is b's last column (x = 3 → 30 m).
        assert_eq!(a.get(-1, 2), 30.0);
    }

    #[test]
    fn literal_arrays_pad_to_the_longest() {
        let layer = serde_json::json!({"paint": {
            "hillshade-method": "multidirectional",
            "hillshade-illumination-direction": [270, 315, 0],
            "hillshade-highlight-color": ["#ff0000", "#00ff00"],
        }});
        let sh = Shading::new(&layer, 10.0);
        assert_eq!(sh.azimuths.len(), 3);
        assert_eq!(sh.highlights.len(), 3);
        assert_eq!(sh.highlights[2], [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(sh.altitudes, vec![45f64.to_radians(); 3]);
        assert!(sh.method == Method::Multidirectional);
    }
}
