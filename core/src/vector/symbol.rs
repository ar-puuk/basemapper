//! Point text labels: a port of MapLibre GL JS's symbol pipeline.
//!
//! - Glyph PBFs (`style/parse_glyph_pbf.ts`): SDF bitmaps with a 3 px border.
//! - Shaping (`symbol/shaping.ts`, `symbol/tagged_string.ts`): line breaking
//!   by least badness, justification, anchor alignment, letter spacing.
//! - Quads (`symbol/quads.ts`) with corner offsets stored at 1/32 precision
//!   (`symbol_bucket.ts addVertex`).
//! - Anchors (`symbol/symbol_layout.ts addFeature`): points, the first vertex
//!   of lines, and the pole of inaccessibility of polygons; anchors outside
//!   the tile are dropped.
//! - Placement (`symbol/placement.ts`, `symbol/collision_index.ts`): layers
//!   top-down, tiles by (x, y), features by sort key; boxes in a viewport grid
//!   padded by 100 px.
//! - Drawing (`webgl/draw/draw_symbol.ts`, `symbol_sdf.*.glsl`): bilinear SDF
//!   sampling, EDGE_GAMMA, one- or two-pass halos.
//!
//! Not yet: line placement, icons, variable anchors, vertical text.

use super::expr::{eval_filter, eval_property, is_data_driven, Color, Ctx, Kind, Val};
use super::gl::{raster_triangle, smoothstep, Clip, Framebuffer, Vert, EXTENT};
use super::mvt::{Feature, Geometry, Value};
use prost::Message;
use serde_json::Value as Json;
use std::collections::{HashMap, HashSet};

const ONE_EM: f64 = 24.0;
const GLYPH_PBF_BORDER: f64 = 3.0;
const ATLAS_PADDING: i64 = 1;
const SHAPING_DEFAULT_OFFSET: f64 = -17.0;
const VIEWPORT_PADDING: f64 = 100.0;
const SDF_PX: f64 = 8.0;
const EDGE_GAMMA: f64 = 0.105;

// ── Glyph PBFs ───────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Message)]
struct PbGlyphs {
    #[prost(message, repeated, tag = "1")]
    stacks: Vec<PbFontstack>,
}

#[derive(Clone, PartialEq, Message)]
struct PbFontstack {
    #[prost(string, optional, tag = "1")]
    name: Option<String>,
    #[prost(string, optional, tag = "2")]
    range: Option<String>,
    #[prost(message, repeated, tag = "3")]
    glyphs: Vec<PbGlyph>,
}

#[derive(Clone, PartialEq, Message)]
struct PbGlyph {
    #[prost(uint32, optional, tag = "1")]
    id: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    bitmap: Option<Vec<u8>>,
    #[prost(uint32, optional, tag = "3")]
    width: Option<u32>,
    #[prost(uint32, optional, tag = "4")]
    height: Option<u32>,
    #[prost(sint32, optional, tag = "5")]
    left: Option<i32>,
    #[prost(sint32, optional, tag = "6")]
    top: Option<i32>,
    #[prost(uint32, optional, tag = "7")]
    advance: Option<u32>,
}

/// A glyph as laid out in MapLibre's atlas: the SDF bitmap (glyph + 3 px
/// border) surrounded by 1 px of zero padding.
#[derive(Clone, Debug)]
pub struct Glyph {
    pub width: f64,
    pub height: f64,
    pub left: f64,
    pub top: f64,
    pub advance: f64,
    /// Atlas cell size (bitmap + 2 * padding).
    pub rect_w: usize,
    pub rect_h: usize,
    /// Padded cell pixels, `rect_w * rect_h`.
    pub cell: Vec<u8>,
}

impl Glyph {
    /// Bilinear sample (GL_LINEAR) at atlas-cell coordinates; zero outside,
    /// as the neighbouring atlas texels are padding.
    fn sample(&self, tx: f64, ty: f64) -> f64 {
        let x = tx - 0.5;
        let y = ty - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let px = |ix: i64, iy: i64| -> f64 {
            if ix < 0 || iy < 0 || ix >= self.rect_w as i64 || iy >= self.rect_h as i64 {
                0.0
            } else {
                self.cell[iy as usize * self.rect_w + ix as usize] as f64
            }
        };
        let (ix, iy) = (x0 as i64, y0 as i64);
        let top = px(ix, iy) * (1.0 - fx) + px(ix + 1, iy) * fx;
        let bot = px(ix, iy + 1) * (1.0 - fx) + px(ix + 1, iy + 1) * fx;
        (top * (1.0 - fy) + bot * fy) / 255.0
    }
}

/// Glyphs by font stack (comma-joined, as in the glyph URL) and code point.
#[derive(Default)]
pub struct GlyphMap {
    pub stacks: HashMap<String, HashMap<u32, Glyph>>,
}

impl GlyphMap {
    pub fn add_pbf(&mut self, fontstack: &str, bytes: &[u8]) -> Result<(), String> {
        let pb = PbGlyphs::decode(bytes).map_err(|e| format!("glyph pbf: {e}"))?;
        let stack = self.stacks.entry(fontstack.to_owned()).or_default();
        for fs in pb.stacks {
            for g in fs.glyphs {
                let (w, h) = (g.width.unwrap_or(0) as usize, g.height.unwrap_or(0) as usize);
                let bw = w + 2 * GLYPH_PBF_BORDER as usize;
                let bh = h + 2 * GLYPH_PBF_BORDER as usize;
                let bitmap = g.bitmap.unwrap_or_default();
                let pad = ATLAS_PADDING as usize;
                let (rw, rh) = (bw + 2 * pad, bh + 2 * pad);
                let mut cell = vec![0u8; rw * rh];
                if bitmap.len() >= bw * bh {
                    for y in 0..bh {
                        cell[(y + pad) * rw + pad..(y + pad) * rw + pad + bw]
                            .copy_from_slice(&bitmap[y * bw..y * bw + bw]);
                    }
                }
                stack.insert(
                    g.id.unwrap_or(0),
                    Glyph {
                        width: w as f64,
                        height: h as f64,
                        left: g.left.unwrap_or(0) as f64,
                        top: g.top.unwrap_or(0) as f64,
                        advance: g.advance.unwrap_or(0) as f64,
                        rect_w: rw,
                        rect_h: rh,
                        cell,
                    },
                );
            }
        }
        Ok(())
    }
}

// ── Text evaluation ──────────────────────────────────────────────────────────

fn prop_string(f: &Feature, key: &str) -> String {
    match f.properties.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                n.to_string()
            }
        }
        Some(Value::Bool(b)) => b.to_string(),
        None => String::new(),
    }
}

fn val_text(v: Val) -> Option<String> {
    match v {
        Val::Str(s) => Some(s),
        Val::Num(n) => Some(if n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) } else { n.to_string() }),
        Val::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The label text a symbol layer shows for a feature (text-field + tokens +
/// format + text-transform), or None.
pub fn feature_text(layer: &Json, f: &Feature, zoom: f64) -> Option<String> {
    let layout = &layer["layout"];
    let tf = layout.get("text-field")?;
    let ctx = Ctx { zoom, feature: Some(f) };
    let text = match tf {
        Json::String(s) => {
            // Legacy "{token}" interpolation (getValueAndResolveTokens).
            let mut out = String::new();
            let mut rest = s.as_str();
            while let Some(i) = rest.find('{') {
                out.push_str(&rest[..i]);
                let Some(j) = rest[i..].find('}') else {
                    out.push_str(&rest[i..]);
                    rest = "";
                    break;
                };
                out.push_str(&prop_string(f, &rest[i + 1..i + j]));
                rest = &rest[i + j + 1..];
            }
            out.push_str(rest);
            out
        }
        Json::Array(a) if a.first().and_then(|v| v.as_str()) == Some("format") => {
            let mut out = String::new();
            let mut i = 1;
            while i < a.len() {
                if !a[i].is_object() {
                    if let Some(t) = val_text(eval_property(&a[i], Kind::Enum, &ctx)) {
                        out.push_str(&t);
                    }
                }
                i += 1;
            }
            out
        }
        other => val_text(eval_property(other, Kind::Enum, &ctx))?,
    };
    let transform = layout
        .get("text-transform")
        .map(|j| eval_property(j, Kind::Enum, &ctx))
        .and_then(|v| v.as_str().map(str::to_owned));
    Some(match transform.as_deref() {
        Some("uppercase") => text.to_uppercase(),
        Some("lowercase") => text.to_lowercase(),
        _ => text,
    })
}

/// The comma-joined font stack of a symbol layer for a feature.
pub fn feature_fontstack(layer: &Json, f: Option<&Feature>, zoom: f64) -> String {
    let ctx = Ctx { zoom, feature: f };
    match layer["layout"].get("text-font") {
        // A plain list of font names (the common case) is a literal, not an
        // expression, even though its first element is a string.
        Some(Json::Array(a)) if !a.is_empty() && a.iter().all(|v| v.is_string()) => {
            a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(",")
        }
        Some(j) => match eval_property(j, Kind::Array, &ctx) {
            Val::Array(a) => a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect::<Vec<_>>().join(","),
            _ => "Open Sans Regular,Arial Unicode MS Regular".into(),
        },
        None => "Open Sans Regular,Arial Unicode MS Regular".into(),
    }
}

/// Glyph ranges (font stack, range start) the visible symbol layers need.
pub fn required_glyph_ranges<'a>(
    style: &Json,
    tiles: impl Iterator<Item = &'a super::mvt::Tile> + Clone,
    zoom: f64,
) -> HashSet<(String, u32)> {
    let mut out = HashSet::new();
    for layer in style["layers"].as_array().map(|v| v.as_slice()).unwrap_or(&[]) {
        if layer["type"] != "symbol" || layer["layout"].get("text-field").is_none() || !layer_visible(layer, zoom) {
            continue;
        }
        let Some(sl) = layer["source-layer"].as_str() else { continue };
        for t in tiles.clone() {
            let Some(l) = t.layers.get(sl) else { continue };
            for f in &l.features {
                let ctx = Ctx { zoom, feature: Some(f) };
                if layer.get("filter").map_or(false, |flt| !eval_filter(flt, &ctx)) {
                    continue;
                }
                if let Some(text) = feature_text(layer, f, zoom) {
                    let stack = feature_fontstack(layer, Some(f), zoom);
                    for ch in text.chars() {
                        out.insert((stack.clone(), (ch as u32 / 256) * 256));
                    }
                }
            }
        }
    }
    out
}

pub fn layer_visible(layer: &Json, zoom: f64) -> bool {
    layer["layout"]["visibility"] != "none"
        && !layer["minzoom"].as_f64().map_or(false, |mz| zoom < mz)
        && !layer["maxzoom"].as_f64().map_or(false, |mz| zoom >= mz)
}

// ── Shaping ──────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct PositionedGlyph {
    codepoint: u32,
    x: f64,
    y: f64,
}

#[derive(Clone, Debug)]
pub struct Shaping {
    lines: Vec<Vec<PositionedGlyph>>,
    top: f64,
    bottom: f64,
    left: f64,
    right: f64,
}

fn is_whitespace(c: char) -> bool {
    c.is_whitespace()
}

fn breakable(cp: u32) -> bool {
    matches!(cp, 0x0a | 0x20 | 0x26 | 0x29 | 0x2b | 0x2d | 0x2f | 0xad | 0xb7 | 0x200b | 0x2010 | 0x2013 | 0x2027)
}

/// Rough port of codePointAllowsIdeographicBreaking (CJK and related blocks).
fn allows_ideographic_breaking(cp: u32) -> bool {
    if cp < 0x2E80 {
        return false;
    }
    matches!(cp,
        0x2E80..=0x2FDF | 0x2FF0..=0x303F | 0x3040..=0x309F | 0x30A0..=0x30FF | 0x3100..=0x312F
        | 0x3130..=0x318F | 0x3190..=0x31BF | 0x31C0..=0x31EF | 0x31F0..=0x31FF | 0x3200..=0x32FF
        | 0x3300..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xA000..=0xA4CF | 0xF900..=0xFAFF
        | 0xFE10..=0xFE1F | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF)
}

struct Break {
    index: usize,
    x: f64,
    prior: Option<usize>,
    badness: f64,
}

fn badness(line_width: f64, target: f64, penalty: f64, is_last: bool) -> f64 {
    let raggedness = (line_width - target).powi(2);
    if is_last {
        if line_width < target { raggedness / 2.0 } else { raggedness * 2.0 }
    } else {
        raggedness + penalty.abs() * penalty
    }
}

fn penalty(cp: u32, next: u32, ideographic: bool) -> f64 {
    let mut p = 0.0;
    if cp == 0x0a {
        p -= 10000.0;
    }
    if ideographic {
        p += 150.0;
    }
    if cp == 0x28 || cp == 0xff08 {
        p += 50.0;
    }
    if next == 0x29 || next == 0xff09 {
        p += 50.0;
    }
    p
}

fn evaluate_break(index: usize, x: f64, target: f64, breaks: &[Break], pen: f64, is_last: bool) -> Break {
    let mut best: Option<usize> = None;
    let mut best_badness = badness(x, target, pen, is_last);
    for (i, b) in breaks.iter().enumerate() {
        let bb = badness(x - b.x, target, pen, is_last) + b.badness;
        if bb <= best_badness {
            best = Some(i);
            best_badness = bb;
        }
    }
    Break { index, x, prior: best, badness: best_badness }
}

fn determine_line_breaks(chars: &[char], spacing: f64, max_width: f64, glyphs: &HashMap<u32, Glyph>) -> Vec<usize> {
    let advance = |c: char| glyphs.get(&(c as u32)).map_or(0.0, |g| g.advance + spacing);
    let total: f64 = chars.iter().map(|&c| advance(c)).sum();
    let line_count = (total / max_width).ceil().max(1.0);
    let target = total / line_count;
    let has_zwsp = chars.contains(&'\u{200b}');

    let mut breaks: Vec<Break> = Vec::new();
    let mut x = 0.0;
    for i in 0..chars.len() {
        let cp = chars[i] as u32;
        if !is_whitespace(chars[i]) {
            x += advance(chars[i]);
        }
        if i + 1 < chars.len() {
            let ideographic = allows_ideographic_breaking(cp);
            let next = chars[i + 1] as u32;
            let next_next_exists = i + 2 < chars.len();
            if breakable(cp) || ideographic || (next_next_exists && next == 0x28) {
                let b = evaluate_break(i + 1, x, target, &breaks, penalty(cp, next, ideographic && has_zwsp), false);
                breaks.push(b);
            }
        }
    }
    let last = evaluate_break(chars.len(), x, target, &breaks, 0.0, true);
    let mut out = vec![last.index];
    let mut prior = last.prior;
    while let Some(p) = prior {
        out.push(breaks[p].index);
        prior = breaks[p].prior;
    }
    out.reverse();
    out
}

fn anchor_alignment(anchor: &str) -> (f64, f64) {
    let h = match anchor {
        "right" | "top-right" | "bottom-right" => 1.0,
        "left" | "top-left" | "bottom-left" => 0.0,
        _ => 0.5,
    };
    let v = match anchor {
        "bottom" | "bottom-right" | "bottom-left" => 1.0,
        "top" | "top-right" | "top-left" => 0.0,
        _ => 0.5,
    };
    (h, v)
}

fn trim_line(line: &[char]) -> &[char] {
    let start = line.iter().position(|c| !is_whitespace(*c)).unwrap_or(line.len());
    let end = line.iter().rposition(|c| !is_whitespace(*c)).map_or(start, |e| e + 1);
    &line[start..end.max(start)]
}

#[allow(clippy::too_many_arguments)]
fn shape_text(
    text: &str,
    glyphs: &HashMap<u32, Glyph>,
    max_width: f64,
    line_height: f64,
    anchor: &str,
    justify: &str,
    spacing: f64,
    translate: (f64, f64),
) -> Option<Shaping> {
    let chars: Vec<char> = text.chars().collect();
    let breaks = determine_line_breaks(&chars, spacing, max_width, glyphs);
    let mut lines: Vec<&[char]> = Vec::new();
    let mut start = 0;
    for &b in &breaks {
        lines.push(&chars[start..b]);
        start = b;
    }
    if start < chars.len() {
        lines.push(&chars[start..]);
    }

    let justify_f = match justify {
        "right" => 1.0,
        "left" => 0.0,
        _ => 0.5,
    };
    let mut positioned: Vec<Vec<PositionedGlyph>> = Vec::new();
    let (mut y, mut max_line_length, mut max_line_height) = (0.0, 0.0f64, 0.0f64);
    for line in &lines {
        let line = trim_line(line);
        let mut pg: Vec<PositionedGlyph> = Vec::new();
        if line.is_empty() {
            y += line_height;
            positioned.push(pg);
            continue;
        }
        let mut x = 0.0;
        for &c in line {
            let cp = c as u32;
            let Some(g) = glyphs.get(&cp) else { continue };
            pg.push(PositionedGlyph { codepoint: cp, x, y: y + SHAPING_DEFAULT_OFFSET });
            x += g.advance + spacing;
        }
        if !pg.is_empty() {
            let line_length = x - spacing;
            max_line_length = max_line_length.max(line_length);
            // justifyLine
            if justify_f != 0.0 {
                let last = pg.last().unwrap();
                let indent = (last.x + glyphs[&last.codepoint].advance) * justify_f;
                for p in &mut pg {
                    p.x -= indent;
                }
            }
        }
        let current_line_height = line_height;
        y += current_line_height;
        max_line_height = max_line_height.max(current_line_height);
        positioned.push(pg);
    }
    if positioned.iter().all(|l| l.is_empty()) {
        return None;
    }
    let (ha, va) = anchor_alignment(anchor);
    // align()
    let shift_x = (justify_f - ha) * max_line_length;
    let shift_y = if max_line_height != line_height {
        -y * va - SHAPING_DEFAULT_OFFSET
    } else {
        -va * lines.len() as f64 * line_height + 0.5 * line_height
    };
    for l in &mut positioned {
        for p in l {
            p.x += shift_x;
            p.y += shift_y;
        }
    }
    let top = translate.1 - va * y;
    let left = translate.0 - ha * max_line_length;
    Some(Shaping { lines: positioned, top, bottom: top + y, left, right: left + max_line_length })
}

// ── Quads ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct GlyphQuad {
    /// Corner offsets tl, tr, bl, br in glyph-em units, quantised to 1/32.
    corners: [(f64, f64); 4],
    codepoint: u32,
}

/// JavaScript Math.round.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn glyph_quads(shaping: &Shaping, glyphs: &HashMap<u32, Glyph>, offset: (f64, f64), rotate_deg: f64) -> Vec<GlyphQuad> {
    let rect_buffer = GLYPH_PBF_BORDER + ATLAS_PADDING as f64;
    let (s, c) = (rotate_deg.to_radians().sin(), rotate_deg.to_radians().cos());
    let mut quads = Vec::new();
    for line in &shaping.lines {
        for pg in line {
            let g = &glyphs[&pg.codepoint];
            let half_advance = g.advance / 2.0;
            let bx = pg.x + half_advance + offset.0;
            let by = pg.y + offset.1;
            let x1 = (g.left - rect_buffer) - half_advance + bx;
            let y1 = (-g.top - rect_buffer) + by;
            let x2 = x1 + g.rect_w as f64;
            let y2 = y1 + g.rect_h as f64;
            let mut corners = [(x1, y1), (x2, y1), (x1, y2), (x2, y2)];
            if rotate_deg != 0.0 {
                for p in &mut corners {
                    *p = (c * p.0 - s * p.1, s * p.0 + c * p.1);
                }
            }
            // a_offset = Math.round(o * 32), decoded as a_offset / 32.
            for p in &mut corners {
                *p = (js_round(p.0 * 32.0) / 32.0, js_round(p.1 * 32.0) / 32.0);
            }
            quads.push(GlyphQuad { corners, codepoint: pg.codepoint });
        }
    }
    quads
}

// ── Pole of inaccessibility (util/find_pole_of_inaccessibility.ts) ──────────

#[derive(Clone, Copy)]
struct Cell {
    x: f64,
    y: f64,
    h: f64,
    d: f64,
    max: f64,
}

fn dist_to_segment_sq(px: f64, py: f64, v: (f64, f64), w: (f64, f64)) -> f64 {
    let l2 = (v.0 - w.0).powi(2) + (v.1 - w.1).powi(2);
    if l2 == 0.0 {
        return (px - v.0).powi(2) + (py - v.1).powi(2);
    }
    let t = ((px - v.0) * (w.0 - v.0) + (py - v.1) * (w.1 - v.1)) / l2;
    if t < 0.0 {
        return (px - v.0).powi(2) + (py - v.1).powi(2);
    }
    if t > 1.0 {
        return (px - w.0).powi(2) + (py - w.1).powi(2);
    }
    let (qx, qy) = (v.0 + (w.0 - v.0) * t, v.1 + (w.1 - v.1) * t);
    (px - qx).powi(2) + (py - qy).powi(2)
}

fn point_to_polygon_dist(px: f64, py: f64, poly: &[Vec<(f64, f64)>]) -> f64 {
    let mut inside = false;
    let mut min_sq = f64::INFINITY;
    for ring in poly {
        let len = ring.len();
        let mut j = len.wrapping_sub(1);
        for i in 0..len {
            let a = ring[i];
            let b = ring[j];
            if (a.1 > py) != (b.1 > py) && px < (b.0 - a.0) * (py - a.1) / (b.1 - a.1) + a.0 {
                inside = !inside;
            }
            min_sq = min_sq.min(dist_to_segment_sq(px, py, a, b));
            j = i;
        }
    }
    (if inside { 1.0 } else { -1.0 }) * min_sq.sqrt()
}

fn cell(x: f64, y: f64, h: f64, poly: &[Vec<(f64, f64)>]) -> Cell {
    let d = point_to_polygon_dist(x, y, poly);
    Cell { x, y, h, d, max: d + h * std::f64::consts::SQRT_2 }
}

/// tinyqueue with compareMax (b.max - a.max): a max-heap on `max`.
struct Queue {
    data: Vec<Cell>,
}

impl Queue {
    fn cmp(a: &Cell, b: &Cell) -> f64 {
        b.max - a.max
    }
    fn push(&mut self, item: Cell) {
        self.data.push(item);
        let mut pos = self.data.len() - 1;
        while pos > 0 {
            let parent = (pos - 1) >> 1;
            if Self::cmp(&item, &self.data[parent]) >= 0.0 {
                break;
            }
            self.data[pos] = self.data[parent];
            pos = parent;
        }
        self.data[pos] = item;
    }
    fn pop(&mut self) -> Option<Cell> {
        if self.data.is_empty() {
            return None;
        }
        let top = self.data[0];
        let bottom = self.data.pop().unwrap();
        if !self.data.is_empty() {
            let len = self.data.len();
            let half = len >> 1;
            let item = bottom;
            let mut pos = 0;
            while pos < half {
                let mut left = (pos << 1) + 1;
                let mut best = self.data[left];
                let right = left + 1;
                if right < len && Self::cmp(&self.data[right], &best) < 0.0 {
                    left = right;
                    best = self.data[right];
                }
                if Self::cmp(&best, &item) >= 0.0 {
                    break;
                }
                self.data[pos] = best;
                pos = left;
            }
            self.data[pos] = item;
        }
        Some(top)
    }
}

fn pole_of_inaccessibility(poly: &[Vec<(f64, f64)>], precision: f64) -> (f64, f64) {
    let outer = &poly[0];
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in outer {
        minx = minx.min(p.0);
        miny = miny.min(p.1);
        maxx = maxx.max(p.0);
        maxy = maxy.max(p.1);
    }
    let cell_size = (maxx - minx).min(maxy - miny);
    let h = cell_size / 2.0;
    if cell_size == 0.0 {
        return (minx, miny);
    }
    let mut q = Queue { data: Vec::new() };
    let mut x = minx;
    while x < maxx {
        let mut y = miny;
        while y < maxy {
            q.push(cell(x + h, y + h, h, poly));
            y += cell_size;
        }
        x += cell_size;
    }
    // Centroid of the outer ring.
    let (mut area, mut cx, mut cy) = (0.0, 0.0, 0.0);
    let len = outer.len();
    let mut j = len - 1;
    for i in 0..len {
        let (a, b) = (outer[i], outer[j]);
        let f = a.0 * b.1 - b.0 * a.1;
        cx += (a.0 + b.0) * f;
        cy += (a.1 + b.1) * f;
        area += f * 3.0;
        j = i;
    }
    let centroid = cell(cx / area, cy / area, 0.0, poly);
    let mut best = centroid;
    while let Some(c) = q.pop() {
        if c.d > best.d || best.d == 0.0 {
            best = c;
        }
        if c.max - best.d <= precision {
            continue;
        }
        let h = c.h / 2.0;
        q.push(cell(c.x - h, c.y - h, h, poly));
        q.push(cell(c.x + h, c.y - h, h, poly));
        q.push(cell(c.x - h, c.y + h, h, poly));
        q.push(cell(c.x + h, c.y + h, h, poly));
    }
    if centroid.d > 0.0 && best.d - centroid.d <= precision {
        return (centroid.x, centroid.y);
    }
    (best.x, best.y)
}

// ── Placement + drawing ──────────────────────────────────────────────────────

/// A tile placed on the canvas, as the renderer sees it.
pub struct SymbolTile<'a> {
    pub coord: (u8, u32, u32),
    pub tile: &'a super::mvt::Tile,
    pub ox: f64,
    pub oy: f64,
    /// Canvas px per 8192-extent tile unit.
    pub unit_px: f64,
    /// 2^(floor(zoom) - tile z): MapLibre's overscaling.
    pub overscaling: f64,
}

/// A label that survived placement, ready to draw.
struct Label {
    anchor_px: (f64, f64),
    quads: Vec<GlyphQuad>,
    fontstack: String,
    size: f64,
    fill: Color,
    halo: Color,
    halo_width: f64,
    halo_blur: f64,
    opacity: f64,
    /// Tile index (draw order groups by tile).
    tile: usize,
}

/// Placed labels per style layer index.
pub struct Placement {
    labels: HashMap<usize, Vec<Label>>,
}

struct Instance {
    layer_idx: usize,
    tile_idx: usize,
    sort_key: f64,
    anchor_px: (f64, f64),
    box_px: [f64; 4],
    overlap_always: bool,
    ignore_placement: bool,
    label: Label,
}

fn num(layout: &Json, key: &str, ctx: &Ctx, default: f64) -> f64 {
    layout.get(key).map(|j| eval_property(j, Kind::Number, ctx)).and_then(|v| v.as_num()).unwrap_or(default)
}

fn string(layout: &Json, key: &str, ctx: &Ctx, default: &str) -> String {
    layout
        .get(key)
        .map(|j| eval_property(j, Kind::Enum, ctx))
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| default.to_owned())
}

fn num_pair(layout: &Json, key: &str, ctx: &Ctx) -> (f64, f64) {
    match layout.get(key).map(|j| eval_property(j, Kind::Array, ctx)).and_then(|v| v.as_num_array()) {
        Some(v) if v.len() == 2 => (v[0], v[1]),
        _ => (0.0, 0.0),
    }
}

fn color(paint: &Json, key: &str, ctx: &Ctx, default: Color) -> Color {
    let c = paint
        .get(key)
        .map(|j| eval_property(j, Kind::Color, ctx))
        .and_then(|v| v.as_color())
        .unwrap_or(default);
    // Data-driven colors are packed to 8 bits (packColor floors).
    if paint.get(key).map_or(false, is_data_driven) {
        let q = |v: f32| (v * 255.0).floor().clamp(0.0, 255.0) / 255.0;
        Color { r: q(c.r), g: q(c.g), b: q(c.b), a: q(c.a) }
    } else {
        c
    }
}

fn to_extent(p: (f32, f32), scale: f64) -> (f64, f64) {
    ((p.0 as f64 * scale + 0.5).floor(), (p.1 as f64 * scale + 0.5).floor())
}

/// Run MapLibre's placement for every visible point-text symbol layer.
pub fn place(style: &Json, tiles: &[SymbolTile], glyphs: &GlyphMap, zoom: f64, width: f64, height: f64) -> Placement {
    let layers = style["layers"].as_array().map(|v| v.as_slice()).unwrap_or(&[]);
    // Tile order for placement: by x, then y (OverscaledTileID.isLessThan).
    let mut tile_order: Vec<usize> = (0..tiles.len()).collect();
    tile_order.sort_by(|&a, &b| {
        let (za, xa, ya) = tiles[a].coord;
        let (zb, xb, yb) = tiles[b].coord;
        zb.cmp(&za).then(xa.cmp(&xb)).then(ya.cmp(&yb))
    });

    let mut boxes: Vec<[f64; 4]> = Vec::new();
    let mut placed: HashMap<usize, Vec<Label>> = HashMap::new();

    for (layer_idx, layer) in layers.iter().enumerate().rev() {
        if layer["type"] != "symbol" || !layer_visible(layer, zoom) || layer["layout"].get("text-field").is_none() {
            continue;
        }
        let layout = &layer["layout"];
        let placement = layout["symbol-placement"].as_str().unwrap_or("point");
        if placement != "point" {
            continue; // line placement: not yet
        }
        let Some(sl) = layer["source-layer"].as_str() else { continue };
        let paint = &layer["paint"];
        let sort_key_j = layout.get("symbol-sort-key");
        let sort_by_key = layout["symbol-z-order"] != "viewport-y" && sort_key_j.map_or(false, is_data_driven);
        let overlap_always = layout["text-overlap"] == "always"
            || (layout.get("text-overlap").is_none() && layout["text-allow-overlap"] == true);
        let ignore_placement = layout["text-ignore-placement"] == true;

        let mut instances: Vec<Instance> = Vec::new();
        for &ti in &tile_order {
            let st = &tiles[ti];
            let Some(src) = st.tile.layers.get(sl) else { continue };
            let scale = EXTENT / src.extent as f64;
            let tile_zoom = st.coord.0 as f64 + st.overscaling.log2();
            let tile_pixel_ratio = EXTENT / (512.0 * st.overscaling);
            let text_pixel_ratio = 512.0 * st.overscaling / EXTENT;

            // Features (filtered), sorted by sort key (stable) if data-driven.
            let mut feats: Vec<(f64, &Feature)> = Vec::new();
            for f in &src.features {
                let ctx = Ctx { zoom: tile_zoom, feature: Some(f) };
                if layer.get("filter").map_or(false, |flt| !eval_filter(flt, &ctx)) {
                    continue;
                }
                let key = if sort_by_key {
                    sort_key_j.map(|j| eval_property(j, Kind::Number, &ctx)).and_then(|v| v.as_num()).unwrap_or(0.0)
                } else {
                    0.0
                };
                feats.push((key, f));
            }
            if sort_by_key {
                feats.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            }

            for (key, f) in feats {
                let ctx = Ctx { zoom, feature: Some(f) };
                let lctx = Ctx { zoom: tile_zoom, feature: Some(f) };
                let Some(text) = feature_text(layer, f, tile_zoom) else { continue };
                let stack = feature_fontstack(layer, Some(f), tile_zoom);
                let Some(gmap) = glyphs.stacks.get(&stack) else { continue };
                let spacing = num(layout, "text-letter-spacing", &lctx, 0.0) * ONE_EM;
                let anchor = string(layout, "text-anchor", &lctx, "center");
                let mut justify = string(layout, "text-justify", &lctx, "center");
                if justify == "auto" {
                    justify = match anchor.as_str() {
                        "right" | "top-right" | "bottom-right" => "right",
                        "left" | "top-left" | "bottom-left" => "left",
                        _ => "center",
                    }
                    .to_owned();
                }
                let max_width = num(layout, "text-max-width", &lctx, 10.0) * ONE_EM;
                let line_height = num(layout, "text-line-height", &lctx, 1.2) * ONE_EM;
                let radial = num(layout, "text-radial-offset", &lctx, 0.0);
                let text_offset = if radial != 0.0 {
                    radial_offset(&anchor, radial * ONE_EM)
                } else {
                    let o = num_pair(layout, "text-offset", &lctx);
                    (o.0 * ONE_EM, o.1 * ONE_EM)
                };
                let Some(shaping) =
                    shape_text(&text, gmap, max_width, line_height, &anchor, &justify, spacing, text_offset)
                else {
                    continue;
                };
                let rotate = num(layout, "text-rotate", &lctx, 0.0);
                let quads = glyph_quads(&shaping, gmap, text_offset, rotate);
                if quads.is_empty() {
                    continue;
                }
                // Collision box: shaping bounds * textBoxScale ± padding, in
                // tile units; fontScale uses text-size at tile zoom + 1.
                let layout_size = num(layout, "text-size", &Ctx { zoom: tile_zoom + 1.0, feature: Some(f) }, 16.0);
                let text_box_scale = tile_pixel_ratio * layout_size / 24.0;
                let padding = num(layout, "text-padding", &lctx, 2.0) * tile_pixel_ratio;
                let (mut bx1, mut by1, mut bx2, mut by2) = (
                    shaping.left * text_box_scale - padding,
                    shaping.top * text_box_scale - padding,
                    shaping.right * text_box_scale + padding,
                    shaping.bottom * text_box_scale + padding,
                );
                if rotate != 0.0 {
                    let (s, c) = (rotate.to_radians().sin(), rotate.to_radians().cos());
                    let pts = [(bx1, by1), (bx2, by1), (bx1, by2), (bx2, by2)].map(|(x, y)| (c * x - s * y, s * x + c * y));
                    bx1 = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
                    bx2 = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
                    by1 = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
                    by2 = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
                }

                let size = num(layout, "text-size", &ctx, 16.0);
                let translate = num_pair(paint, "text-translate", &ctx);
                let mk_label = |ax: f64, ay: f64| Label {
                    anchor_px: (ax, ay),
                    quads: quads.clone(),
                    fontstack: stack.clone(),
                    size,
                    fill: color(paint, "text-color", &ctx, Color::BLACK),
                    halo: color(paint, "text-halo-color", &ctx, Color::TRANSPARENT),
                    halo_width: num(paint, "text-halo-width", &ctx, 0.0),
                    halo_blur: num(paint, "text-halo-blur", &ctx, 0.0),
                    opacity: num(paint, "text-opacity", &ctx, 1.0),
                    tile: ti,
                };

                // Anchors (addFeature).
                let mut anchors: Vec<(f64, f64)> = Vec::new();
                match &f.geometry {
                    Geometry::Points(pts) => {
                        anchors.extend(pts.iter().map(|&p| to_extent(p, scale)));
                    }
                    Geometry::Lines(ls) => {
                        anchors.extend(ls.iter().filter_map(|l| l.first().map(|&p| to_extent(p, scale))));
                    }
                    Geometry::Polygons(polys) => {
                        for poly in polys {
                            let rings: Vec<Vec<(f64, f64)>> =
                                poly.iter().map(|r| r.iter().map(|&p| to_extent(p, scale)).collect()).collect();
                            anchors.push(pole_of_inaccessibility(&rings, 16.0));
                        }
                    }
                }
                for (ax, ay) in anchors {
                    if ax < 0.0 || ax >= EXTENT || ay < 0.0 || ay >= EXTENT {
                        continue;
                    }
                    let px = st.ox + ax * st.unit_px + translate.0;
                    let py = st.oy + ay * st.unit_px + translate.1;
                    let r = text_pixel_ratio;
                    // Collision index coordinates are offset by the viewport padding.
                    let (cx, cy) = (px + VIEWPORT_PADDING, py + VIEWPORT_PADDING);
                    instances.push(Instance {
                        layer_idx,
                        tile_idx: ti,
                        sort_key: key,
                        anchor_px: (px, py),
                        box_px: [cx + bx1 * r, cy + by1 * r, cx + bx2 * r, cy + by2 * r],
                        overlap_always,
                        ignore_placement,
                        label: mk_label(px, py),
                    });
                }
            }
        }
        // sortAcrossTiles: placement in sort-key order across all tiles.
        if sort_by_key {
            instances.sort_by(|a, b| a.sort_key.partial_cmp(&b.sort_key).unwrap_or(std::cmp::Ordering::Equal));
        }
        let grid_w = width + 2.0 * VIEWPORT_PADDING;
        let grid_h = height + 2.0 * VIEWPORT_PADDING;
        let mut layer_labels: Vec<(f64, usize, Label)> = Vec::new();
        for inst in instances {
            let [x1, y1, x2, y2] = inst.box_px;
            let inside_grid = x2 >= 0.0 && x1 < grid_w && y2 >= 0.0 && y1 < grid_h;
            if !inside_grid {
                continue;
            }
            let hit = !inst.overlap_always
                && boxes.iter().any(|b| x1 <= b[2] && y1 <= b[3] && x2 >= b[0] && y2 >= b[1]);
            if hit {
                continue;
            }
            if !inst.ignore_placement {
                boxes.push(inst.box_px);
            }
            let _ = (inst.layer_idx, inst.anchor_px);
            layer_labels.push((inst.sort_key, inst.tile_idx, inst.label));
        }
        // Draw order: per tile (draw coords), symbols in bucket order (which
        // is sort-key order within the tile).
        layer_labels.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)));
        placed.insert(layer_idx, layer_labels.into_iter().map(|l| l.2).collect());
    }
    Placement { labels: placed }
}

fn radial_offset(anchor: &str, r: f64) -> (f64, f64) {
    // evaluateVariableOffset for a radial offset (no explicit y).
    let hypot = r / std::f64::consts::SQRT_2;
    match anchor {
        "top-right" | "top-left" => (if anchor.ends_with("left") { hypot } else { -hypot }, hypot),
        "bottom-right" | "bottom-left" => (if anchor.ends_with("left") { hypot } else { -hypot }, -hypot),
        "bottom" => (0.0, -r),
        "top" => (0.0, r),
        "right" => (-r, 0.0),
        "left" => (r, 0.0),
        _ => (0.0, 0.0),
    }
}

/// Draw the placed labels of one symbol layer.
pub fn draw_layer(fb: &mut Framebuffer, layer: &Json, layer_idx: usize, placement: &Placement, glyphs: &GlyphMap) {
    let Some(labels) = placement.labels.get(&layer_idx) else { return };
    let (w, h) = (fb.w, fb.h);
    let full = Clip { x0: 0.0, y0: 0.0, x1: w as f64, y1: h as f64 };
    let paint = &layer["paint"];
    let layout = &layer["layout"];
    // draw_symbol.ts: halo drawn in its own pass when the (constant) halo
    // width exceeds 1, it is data-driven, or letter spacing is negative.
    let halo_j = paint.get("text-halo-width");
    let has_halo = halo_j.map_or(true, |j| is_data_driven(j) || j.as_f64() != Some(0.0));
    let halo_const = halo_j.and_then(|j| if is_data_driven(j) { None } else { j.as_f64() });
    let spacing_neg = layout.get("text-letter-spacing").and_then(|j| j.as_f64()).map_or(false, |s| s * ONE_EM < 0.0);
    let glyph_overlap = spacing_neg || halo_const.map_or(halo_j.is_some(), |hw| hw > 1.0);

    // Group by tile, preserving order.
    let mut start = 0;
    while start < labels.len() {
        let tile = labels[start].tile;
        let end = labels[start..].iter().position(|l| l.tile != tile).map_or(labels.len(), |p| start + p);
        let group = &labels[start..end];
        let passes: &[(bool, bool)] = if has_halo && glyph_overlap {
            &[(true, false), (false, true)]
        } else if has_halo {
            &[(true, true)]
        } else {
            &[(false, true)]
        };
        for &(is_halo, is_plain) in passes {
            for l in group {
                draw_label(fb, l, glyphs, is_halo, is_plain, &full, w, h);
            }
        }
        start = end;
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_label(fb: &mut Framebuffer, l: &Label, glyphs: &GlyphMap, is_halo: bool, is_plain: bool, clip: &Clip, w: usize, h: usize) {
    if l.opacity < 0.1 {
        return; // symbol_sdf.vertex: total_opacity < 0.1 is culled
    }
    let Some(gmap) = glyphs.stacks.get(&l.fontstack) else { return };
    let font_scale = l.size / 24.0;
    let gamma = EDGE_GAMMA / font_scale;
    let inner_edge = (256.0 - 64.0) / 256.0;
    let gamma_halo = (l.halo_blur * 1.19 / SDF_PX + EDGE_GAMMA) / font_scale;
    let inner_edge_halo = inner_edge + gamma_halo;
    let halo_edge = (6.0 - l.halo_width / font_scale) / SDF_PX;
    let fill = [l.fill.r as f64, l.fill.g as f64, l.fill.b as f64, l.fill.a as f64];
    let halo = [l.halo.r as f64, l.halo.g as f64, l.halo.b as f64, l.halo.a as f64];
    let op = l.opacity;

    for q in &l.quads {
        let Some(g) = gmap.get(&q.codepoint) else { continue };
        let tex = [(0.0, 0.0), (g.rect_w as f64, 0.0), (0.0, g.rect_h as f64), (g.rect_w as f64, g.rect_h as f64)];
        let v: Vec<Vert<2>> = (0..4)
            .map(|i| Vert {
                x: l.anchor_px.0 + q.corners[i].0 * font_scale,
                y: l.anchor_px.1 + q.corners[i].1 * font_scale,
                v: [tex[i].0, tex[i].1],
            })
            .collect();
        for tri in [[0usize, 2, 1], [1, 2, 3]] {
            raster_triangle(w, h, [v[tri[0]], v[tri[1]], v[tri[2]]], clip, |x, y, t| {
                let dist = g.sample(t[0], t[1]);
                let mut out = [0f64; 4];
                let mut text = [0f64; 4];
                if is_plain {
                    let a = smoothstep(inner_edge - gamma, inner_edge + gamma, dist);
                    for i in 0..4 {
                        text[i] = op * a * fill[i];
                    }
                }
                if is_halo {
                    let ah = smoothstep(inner_edge_halo - gamma_halo, inner_edge_halo + gamma_halo, dist);
                    let ah = smoothstep(halo_edge - gamma_halo, halo_edge + gamma_halo, dist).min(1.0 - ah);
                    let mut hc = [0f64; 4];
                    for i in 0..4 {
                        hc[i] = op * ah * halo[i];
                    }
                    if is_plain {
                        for i in 0..4 {
                            out[i] = text[i] + (1.0 - text[3]) * hc[i];
                        }
                    } else {
                        out = hc;
                    }
                } else {
                    out = text;
                }
                if out[3] > 0.0 || out[0] > 0.0 {
                    fb.blend(x, y, [out[0] as f32, out[1] as f32, out[2] as f32, out[3] as f32]);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyphs_fixed(advance: f64) -> HashMap<u32, Glyph> {
        let mut m = HashMap::new();
        for c in [' ', 'a', 'b', 'c'] {
            m.insert(
                c as u32,
                Glyph { width: 10.0, height: 10.0, left: 0.0, top: -5.0, advance, rect_w: 18, rect_h: 18, cell: vec![0; 324] },
            );
        }
        m
    }

    #[test]
    fn single_line_centered() {
        let g = glyphs_fixed(10.0);
        let s = shape_text("abc", &g, 240.0, 28.8, "center", "center", 0.0, (0.0, 0.0)).unwrap();
        assert_eq!(s.lines.len(), 1);
        assert_eq!((s.left, s.right), (-15.0, 15.0));
        assert_eq!(s.lines[0][0].x, -15.0);
    }

    #[test]
    fn breaks_long_text() {
        let g = glyphs_fixed(10.0);
        // 11 words of 3 chars + spaces, max width 50 -> several lines.
        let s = shape_text("abc abc abc abc", &g, 50.0, 28.8, "center", "center", 0.0, (0.0, 0.0)).unwrap();
        assert!(s.lines.len() >= 2);
    }

    #[test]
    fn pole_of_square_is_centre() {
        let sq = vec![vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0), (0.0, 0.0)]];
        let (x, y) = pole_of_inaccessibility(&sq, 1.0);
        assert!((x - 50.0).abs() < 1.0 && (y - 50.0).abs() < 1.0);
    }
}
