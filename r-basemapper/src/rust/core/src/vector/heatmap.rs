//! Helpers for `heatmap` layers (drawn in render.rs): MapLibre's color ramp
//! texture and the half-float kernel density texture.

use super::expr::{eval_property, Color, Ctx, Kind, Val};
use serde_json::Value as Json;

/// MapLibre's default `heatmap-color`.
pub fn default_heatmap_color() -> Json {
    serde_json::json!([
        "interpolate", ["linear"], ["heatmap-density"],
        0, "rgba(0, 0, 255, 0)", 0.1, "royalblue", 0.3, "cyan", 0.5, "lime", 0.7, "yellow", 1, "red"
    ])
}

/// `expr` with every `[op]` (e.g. `["heatmap-density"]`, `["line-progress"]`)
/// replaced by the number `v`.
fn substitute(expr: &Json, op: &str, v: f64) -> Json {
    match expr {
        Json::Array(a) if a.len() == 1 && a[0] == op => serde_json::json!(v),
        Json::Array(a) => Json::Array(a.iter().map(|e| substitute(e, op, v)).collect()),
        Json::Object(o) => Json::Object(o.iter().map(|(k, e)| (k.clone(), substitute(e, op, v))).collect()),
        other => other.clone(),
    }
}

/// renderColorRamp: `expr` evaluated at `i / 255` of the global `op` for 256
/// texels, stored as RGBAImage#setPixel does (unpremultiplied, rounded), then
/// premultiplied on upload as Texture does. Premultiplied RGBA8 texels.
pub fn color_ramp(expr: &Json, op: &str, zoom: f64) -> Vec<[u8; 4]> {
    color_ramp_range(expr, op, zoom, 256, 0.0, 1.0)
}

/// renderColorRamp with clips: `n` texels, texel `i` evaluated at
/// `start + (end - start) * i / (n - 1)`.
pub fn color_ramp_range(expr: &Json, op: &str, zoom: f64, n: usize, start: f64, end: f64) -> Vec<[u8; 4]> {
    let ctx = Ctx { zoom, feature: None };
    (0..n)
        .map(|i| {
            let p = i as f64 / (n - 1) as f64;
            let e = substitute(expr, op, start * (1.0 - p) + end * p);
            let c = match eval_property(&e, Kind::Color, &ctx) {
                Val::Color(c) => c,
                _ => Color::TRANSPARENT,
            };
            let (r, g, b, a) = (c.r as f64, c.g as f64, c.b as f64, c.a as f64);
            // Math.round(NaN) stored in a Uint8Array is 0.
            let straight = |v: f64| if a > 0.0 { (v * 255.0 / a).round().clamp(0.0, 255.0) } else { 0.0 };
            let a8 = (a * 255.0).round().clamp(0.0, 255.0);
            let pm = |v: f64| (straight(v) * a8 / 255.0).round() as u8;
            [pm(r), pm(g), pm(b), a8 as u8]
        })
        .collect()
}

/// A ramp sampled `NEAREST` with `CLAMP_TO_EDGE` at `t`, as premultiplied
/// floats.
pub fn sample_ramp_nearest(ramp: &[[u8; 4]], t: f32) -> [f32; 4] {
    let n = ramp.len();
    let i = ((t * n as f32).floor().max(0.0) as usize).min(n - 1);
    ramp[i].map(|v| v as f32 / 255.0)
}

/// A ramp sampled `LINEAR` with `CLAMP_TO_EDGE` at `t`, as premultiplied
/// floats.
pub fn sample_ramp(ramp: &[[u8; 4]], t: f32) -> [f32; 4] {
    let n = ramp.len() as f32;
    let s = (t * n - 0.5).clamp(0.0, n - 1.0);
    let i0 = s.floor() as usize;
    let i1 = (i0 + 1).min(ramp.len() - 1);
    let f = s - i0 as f32;
    [0, 1, 2, 3].map(|k| (ramp[i0][k] as f32 * (1.0 - f) + ramp[i1][k] as f32 * f) / 255.0)
}

/// `x` rounded to the nearest IEEE half float (round to nearest, ties to
/// even), as an RGBA16F render target stores it.
pub fn to_f16(x: f32) -> f32 {
    let a = x.abs();
    if !a.is_finite() || a == 0.0 {
        return x;
    }
    if a >= 65520.0 {
        return f32::INFINITY.copysign(x);
    }
    if a < 6.103_515_6e-5 {
        // Subnormal halves: multiples of 2^-24.
        let s = x * 16_777_216.0;
        return s.round_ties_even() / 16_777_216.0;
    }
    let bits = x.to_bits();
    let shift = 13;
    let lsb = (bits >> shift) & 1;
    let rounded = bits + ((1 << (shift - 1)) - 1) + lsb;
    f32::from_bits(rounded & !((1u32 << shift) - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_float_rounding() {
        assert_eq!(to_f16(1.0), 1.0);
        // 1 + 2^-11 is a tie between 1 and 1 + 2^-10: rounds to even (1).
        assert_eq!(to_f16(1.0 + 1.0 / 2048.0), 1.0);
        assert_eq!(to_f16(1.0 + 3.0 / 2048.0), 1.0 + 2.0 / 1024.0);
        assert_eq!(to_f16(0.1), 0.099975586);
        assert_eq!(to_f16(1e-7), 1.0 / 16_777_216.0 * 2.0);
    }

    #[test]
    fn default_ramp_is_transparent_blue_to_red() {
        let ramp = color_ramp(&default_heatmap_color(), "heatmap-density", 0.0);
        assert_eq!(ramp[0], [0, 0, 0, 0]);
        assert_eq!(ramp[255], [255, 0, 0, 255]);
        // Halfway: lime.
        assert_eq!(ramp[128][1], 255);
    }
}
