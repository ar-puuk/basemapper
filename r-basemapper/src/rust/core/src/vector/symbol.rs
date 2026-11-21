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
//! - Icons (`render/image_atlas.ts`, `getIconQuads`, `symbol_icon.*.glsl`):
//!   premultiplied sprite images with 1 px padding, NEAREST/LINEAR per
//!   draw_symbol.ts, SDF icons through the text shader; text/icon placement
//!   combined through text-optional / icon-optional.
//!
//! - Line placement (`symbol_line.rs`): merged, clipped lines; anchors by
//!   spacing and max angle; collision circles; glyphs placed along the
//!   projected line with keep-upright flipping.
//!
//! Not yet: variable anchors, icon-text-fit, vertical text.

use super::expr::{eval_filter, eval_property, is_data_driven, Color, Ctx, Kind, Val};
use super::gl::{raster_triangle, smoothstep, Clip, Framebuffer, Vert, EXTENT};
use super::mvt::{Feature, Geometry, Value};
use super::symbol_line::{self as line, ProjectedLine};
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

// ── Sprites ──────────────────────────────────────────────────────────────────

/// A sprite image as MapLibre stores it in its icon atlas: premultiplied RGBA
/// (`premultiplyAlpha`, Math.round(c * a / 255)) with 1 px of zero padding.
#[derive(Clone, Debug)]
pub struct SpriteImage {
    pub w: usize,
    pub h: usize,
    pub pixel_ratio: f64,
    pub sdf: bool,
    /// (w + 2) * (h + 2) * 4 bytes.
    pub cell: Vec<u8>,
}

impl SpriteImage {
    fn texel(&self, ix: i64, iy: i64) -> [f64; 4] {
        let (cw, ch) = (self.w as i64 + 2, self.h as i64 + 2);
        if ix < 0 || iy < 0 || ix >= cw || iy >= ch {
            return [0.0; 4];
        }
        let o = ((iy * cw + ix) * 4) as usize;
        [self.cell[o] as f64, self.cell[o + 1] as f64, self.cell[o + 2] as f64, self.cell[o + 3] as f64]
    }

    /// Sample at padded-cell coordinates; GL_LINEAR or GL_NEAREST. 0..=255.
    fn sample(&self, tx: f64, ty: f64, linear: bool) -> [f64; 4] {
        if !linear {
            return self.texel(tx.floor() as i64, ty.floor() as i64);
        }
        let (x, y) = (tx - 0.5, ty - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (ix, iy) = (x0 as i64, y0 as i64);
        let (a, b, c, d) = (self.texel(ix, iy), self.texel(ix + 1, iy), self.texel(ix, iy + 1), self.texel(ix + 1, iy + 1));
        let mut out = [0.0; 4];
        for i in 0..4 {
            out[i] = (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy;
        }
        out
    }
}

#[derive(Default)]
pub struct Sprite {
    pub images: HashMap<String, SpriteImage>,
    /// The device pixel ratio the sprite is drawn at (0 means 1). Icons whose
    /// image pixelRatio differs are sampled linearly (iconsNeedLinear).
    pub device_pixel_ratio: f64,
}

impl Sprite {
    pub fn dpr(&self) -> f64 {
        if self.device_pixel_ratio > 0.0 { self.device_pixel_ratio } else { 1.0 }
    }

    /// Build from a sprite's `.json` index and `.png` sheet.
    pub fn from_json_png(json: &[u8], png: &[u8]) -> Result<Sprite, String> {
        let index: Json = serde_json::from_slice(json).map_err(|e| format!("sprite json: {e}"))?;
        let sheet = image::load_from_memory(png).map_err(|e| format!("sprite png: {e}"))?.to_rgba8();
        let mut images = HashMap::new();
        for (name, e) in index.as_object().ok_or("sprite json is not an object")? {
            let g = |k: &str| e[k].as_u64().unwrap_or(0) as usize;
            let (x, y, w, h) = (g("x"), g("y"), g("width"), g("height"));
            if w == 0 || h == 0 || x + w > sheet.width() as usize || y + h > sheet.height() as usize {
                continue;
            }
            let cw = w + 2;
            let mut cell = vec![0u8; cw * (h + 2) * 4];
            for row in 0..h {
                for col in 0..w {
                    let p = sheet.get_pixel((x + col) as u32, (y + row) as u32).0;
                    let a = p[3] as f64;
                    let o = ((row + 1) * cw + col + 1) * 4;
                    for i in 0..3 {
                        cell[o + i] = (p[i] as f64 * a / 255.0 + 0.5).floor() as u8;
                    }
                    cell[o + 3] = p[3];
                }
            }
            images.insert(
                name.clone(),
                SpriteImage {
                    w,
                    h,
                    pixel_ratio: e["pixelRatio"].as_f64().unwrap_or(1.0),
                    sdf: e["sdf"].as_bool().unwrap_or(false),
                    cell,
                },
            );
        }
        Ok(Sprite { images, device_pixel_ratio: 1.0 })
    }
}

/// Everything symbol layers need besides the tiles.
#[derive(Default)]
pub struct SymbolAssets {
    pub glyphs: GlyphMap,
    pub sprite: Sprite,
}

/// The icon image name a symbol layer shows for a feature, or None.
pub fn feature_icon(layer: &Json, f: &Feature, zoom: f64) -> Option<String> {
    let ii = layer["layout"].get("icon-image")?;
    let ctx = Ctx { zoom, feature: Some(f) };
    let name = match ii {
        Json::String(s) if s.contains('{') => {
            let mut out = String::new();
            let mut rest = s.as_str();
            while let Some(i) = rest.find('{') {
                out.push_str(&rest[..i]);
                let Some(j) = rest[i..].find('}') else { break };
                out.push_str(&prop_string(f, &rest[i + 1..i + j]));
                rest = &rest[i + j + 1..];
            }
            out.push_str(rest);
            out
        }
        other => val_text(eval_property(other, Kind::Enum, &ctx))?,
    };
    (!name.is_empty()).then_some(name)
}

// ── Placement + drawing ──────────────────────────────────────────────────────

/// A tile placed on the canvas, as the renderer sees it.
pub struct SymbolTile<'a> {
    /// Style source id the tile belongs to; layers place features only from
    /// their own source's tiles.
    pub source: &'a str,
    pub coord: (u8, u32, u32),
    /// World copy (OverscaledTileID.wrap); placement orders by it first.
    pub wrap: i32,
    pub tile: &'a super::mvt::Tile,
    pub ox: f64,
    pub oy: f64,
    /// Canvas px per 8192-extent tile unit.
    pub unit_px: f64,
    /// 2^(floor(zoom) - tile z): MapLibre's overscaling.
    pub overscaling: f64,
}

#[derive(Clone)]
struct SdfStyle {
    fill: Color,
    halo: Color,
    halo_width: f64,
    halo_blur: f64,
    opacity: f64,
}

#[derive(Clone)]
struct TextPart {
    quads: Vec<GlyphQuad>,
    fontstack: String,
    size: f64,
    style: SdfStyle,
    along: Option<AlongLine>,
}

/// Geometry for a symbol drawn along a line (updateLineLabels).
#[derive(Clone)]
struct AlongLine {
    /// The symbol's line in screen px (label plane).
    line_px: Vec<(f64, f64)>,
    segment: usize,
    /// Per-quad glyph offsets along the line, in glyph units.
    glyph_offsets: Vec<f64>,
    line_offset: (f64, f64),
    keep_upright: bool,
    rotate_to_line: bool,
    /// Font scale used for placement along the line (size / 24).
    placement_scale: f64,
}

#[derive(Clone)]
struct IconPart {
    image: String,
    /// tl, tr, bl, br in icon px (before icon-size), quantised to 1/32.
    corners: [(f64, f64); 4],
    size: f64,
    sdf: bool,
    linear: bool,
    style: SdfStyle,
    along: Option<AlongLine>,
}

/// A symbol that survived placement, ready to draw.
struct Label {
    anchor_px: (f64, f64),
    text: Option<TextPart>,
    icon: Option<IconPart>,
    tile: usize,
}

/// Placed symbols per style layer index.
pub struct Placement {
    labels: HashMap<usize, Vec<Label>>,
}

struct Instance {
    tile_idx: usize,
    sort_key: f64,
    text_box: Option<[f64; 4]>,
    /// Line labels collide with runtime circles along their projected path.
    text_line: Option<TextCircles>,
    icon_box: Option<[f64; 4]>,
    label: Label,
}

struct TextCircles {
    radius: f64,
    /// Projected path from the first to the last glyph; None if they don't fit.
    path: Option<Vec<(f64, f64)>>,
}

/// An entry in the collision index.
enum Item {
    Box([f64; 4]),
    Circle(f64, f64, f64),
}

fn circle_rect(cx: f64, cy: f64, r: f64, b: &[f64; 4]) -> bool {
    let hw = (b[2] - b[0]) / 2.0;
    let dx = (cx - (b[0] + hw)).abs();
    if dx > hw + r {
        return false;
    }
    let hh = (b[3] - b[1]) / 2.0;
    let dy = (cy - (b[1] + hh)).abs();
    if dy > hh + r {
        return false;
    }
    if dx <= hw || dy <= hh {
        return true;
    }
    let (ex, ey) = (dx - hw, dy - hh);
    ex * ex + ey * ey <= r * r
}

/// GridIndex.hitTest for a box (with its whole-grid shortcut).
fn hit_box(items: &[Item], b: &[f64; 4], gw: f64, gh: f64) -> bool {
    if b[2] < 0.0 || b[0] > gw || b[3] < 0.0 || b[1] > gh {
        return false;
    }
    if b[0] <= 0.0 && b[1] <= 0.0 && gw <= b[2] && gh <= b[3] {
        return !items.is_empty();
    }
    items.iter().any(|it| match it {
        Item::Box(o) => b[0] <= o[2] && b[1] <= o[3] && b[2] >= o[0] && b[3] >= o[1],
        Item::Circle(x, y, r) => circle_rect(*x, *y, *r, b),
    })
}

/// GridIndex.hitTestCircle.
fn hit_circle(items: &[Item], x: f64, y: f64, r: f64, gw: f64, gh: f64) -> bool {
    if x + r < 0.0 || x - r > gw || y + r < 0.0 || y - r > gh {
        return false;
    }
    items.iter().any(|it| match it {
        Item::Box(o) => circle_rect(x, y, r, o),
        Item::Circle(x2, y2, r2) => {
            let (dx, dy) = (x2 - x, y2 - y);
            (r + r2) * (r + r2) > dx * dx + dy * dy
        }
    })
}

/// placeCollisionCircles: returns (circles, collision detected).
fn place_circles(
    items: &[Item],
    path: &[(f64, f64)],
    radius: f64,
    width: f64,
    height: f64,
    always: bool,
) -> (Vec<(f64, f64, f64)>, bool) {
    let (gw, gh) = (width + 2.0 * VIEWPORT_PADDING, height + 2.0 * VIEWPORT_PADDING);
    let (minx, miny, maxx, maxy) = (-VIEWPORT_PADDING, -VIEWPORT_PADDING, width + VIEWPORT_PADDING, height + VIEWPORT_PADDING);
    let mut circles = Vec::new();
    let mut in_grid = false;
    if path.is_empty() {
        return (circles, false);
    }
    let (mut lo_x, mut lo_y, mut hi_x, mut hi_y) = (path[0].0, path[0].1, path[0].0, path[0].1);
    for p in path {
        lo_x = lo_x.min(p.0);
        lo_y = lo_y.min(p.1);
        hi_x = hi_x.max(p.0);
        hi_y = hi_y.max(p.1);
    }
    let segments: Vec<Vec<(f64, f64)>> = if lo_x >= minx && hi_x <= maxx && lo_y >= miny && hi_y <= maxy {
        vec![path.to_vec()]
    } else if hi_x < minx || lo_x > maxx || hi_y < miny || lo_y > maxy {
        Vec::new()
    } else {
        line::clip_line(&[path.to_vec()], minx, miny, maxx, maxy)
    };
    let circle_dist = radius * 2.5;
    for seg in segments {
        let interp = line::PathInterpolator::new(seg, radius * 0.25);
        let n = if interp.length <= 0.5 * radius { 1 } else { (interp.padded_length / circle_dist).ceil() as usize + 1 };
        for i in 0..n {
            let t = i as f64 / (n.max(2) - 1) as f64;
            let p = interp.lerp(t);
            let (cx, cy) = (p.0 + VIEWPORT_PADDING, p.1 + VIEWPORT_PADDING);
            circles.push((cx, cy, radius));
            let (x1, y1, x2, y2) = (cx - radius, cy - radius, cx + radius, cy + radius);
            in_grid |= x2 >= 0.0 && x1 < gw && y2 >= 0.0 && y1 < gh;
            if !always && hit_circle(items, cx, cy, radius, gw, gh) {
                return (Vec::new(), true);
            }
        }
    }
    if !in_grid {
        circles.clear();
    }
    (circles, false)
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

fn sdf_style(paint: &Json, prefix: &str, ctx: &Ctx) -> SdfStyle {
    SdfStyle {
        fill: color(paint, &format!("{prefix}-color"), ctx, Color::BLACK),
        halo: color(paint, &format!("{prefix}-halo-color"), ctx, Color::TRANSPARENT),
        halo_width: num(paint, &format!("{prefix}-halo-width"), ctx, 0.0),
        halo_blur: num(paint, &format!("{prefix}-halo-blur"), ctx, 0.0),
        opacity: num(paint, &format!("{prefix}-opacity"), ctx, 1.0),
    }
}

fn to_extent(p: (f32, f32), scale: f64) -> (f64, f64) {
    ((p.0 as f64 * scale + 0.5).floor(), (p.1 as f64 * scale + 0.5).floor())
}

fn rotated_envelope(b: [f64; 4], deg: f64) -> [f64; 4] {
    if deg == 0.0 {
        return b;
    }
    let (s, c) = (deg.to_radians().sin(), deg.to_radians().cos());
    let pts = [(b[0], b[1]), (b[2], b[1]), (b[0], b[3]), (b[2], b[3])].map(|(x, y)| (c * x - s * y, s * x + c * y));
    [
        pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min),
        pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min),
        pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max),
        pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max),
    ]
}

fn padding4(layout: &Json, key: &str, ctx: &Ctx, default: f64) -> [f64; 4] {
    // [top, right, bottom, left] (CSS order); a single number applies to all.
    match layout.get(key).map(|j| eval_property(j, Kind::Array, ctx)) {
        Some(Val::Num(n)) => [n; 4],
        Some(Val::Array(a)) => {
            let v: Vec<f64> = a.iter().filter_map(|x| x.as_num()).collect();
            match v.len() {
                1 => [v[0]; 4],
                2 => [v[0], v[1], v[0], v[1]],
                3 => [v[0], v[1], v[2], v[1]],
                4 => [v[0], v[1], v[2], v[3]],
                _ => [default; 4],
            }
        }
        _ => [default; 4],
    }
}

/// Text shaping parameters and output for one feature.
fn build_text(
    layer: &Json,
    f: &Feature,
    assets: &SymbolAssets,
    zoom: f64,
    tile_zoom: f64,
    tile_pixel_ratio: f64,
) -> Option<(TextPart, [f64; 4])> {
    let layout = &layer["layout"];
    let paint = &layer["paint"];
    let ctx = Ctx { zoom, feature: Some(f) };
    let lctx = Ctx { zoom: tile_zoom, feature: Some(f) };
    let next_ctx = Ctx { zoom: tile_zoom + 1.0, feature: Some(f) };
    let t = feature_text(layer, f, tile_zoom)?;
    let stack = feature_fontstack(layer, Some(f), tile_zoom);
    let gmap = assets.glyphs.stacks.get(&stack)?;
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
    let shaping = shape_text(&t, gmap, max_width, line_height, &anchor, &justify, spacing, text_offset)?;
    let rotate = num(layout, "text-rotate", &lctx, 0.0);
    let quads = glyph_quads(&shaping, gmap, text_offset, rotate);
    if quads.is_empty() {
        return None;
    }
    let layout_size = num(layout, "text-size", &next_ctx, 16.0);
    let box_scale = tile_pixel_ratio * layout_size / 24.0;
    let pad = num(layout, "text-padding", &lctx, 2.0) * tile_pixel_ratio;
    let b = rotated_envelope(
        [shaping.left * box_scale - pad, shaping.top * box_scale - pad, shaping.right * box_scale + pad, shaping.bottom * box_scale + pad],
        rotate,
    );
    Some((
        TextPart {
            quads,
            fontstack: stack,
            size: num(layout, "text-size", &ctx, 16.0),
            style: sdf_style(paint, "text", &ctx),
            along: None,
        },
        b,
    ))
}

/// Icon shaping (shapeIcon + getIconQuads without icon-text-fit). Returns
/// the part, its collision box (tile units) and whether it needs LINEAR.
fn build_icon(
    layer: &Json,
    f: &Feature,
    assets: &SymbolAssets,
    zoom: f64,
    tile_zoom: f64,
    tile_pixel_ratio: f64,
) -> Option<(IconPart, [f64; 4], bool)> {
    let layout = &layer["layout"];
    let paint = &layer["paint"];
    let ctx = Ctx { zoom, feature: Some(f) };
    let lctx = Ctx { zoom: tile_zoom, feature: Some(f) };
    let next_ctx = Ctx { zoom: tile_zoom + 1.0, feature: Some(f) };
    let name = feature_icon(layer, f, tile_zoom)?;
    let img = assets.sprite.images.get(&name)?;
    let rotate = num(layout, "icon-rotate", &lctx, 0.0);
    let needs_linear = img.pixel_ratio != assets.sprite.dpr() || rotate != 0.0 || layout.get("icon-rotate").map_or(false, is_data_driven);
    let (dw, dh) = (img.w as f64 / img.pixel_ratio, img.h as f64 / img.pixel_ratio);
    let (ha, va) = anchor_alignment(&string(layout, "icon-anchor", &lctx, "center"));
    let off = num_pair(layout, "icon-offset", &lctx);
    let (left, top) = (off.0 - dw * ha, off.1 - dh * va);
    let (right, bottom) = (left + dw, top + dh);
    // Padded by one image pixel on each side, like the atlas cell.
    let (px, py) = (dw / img.w as f64, dh / img.h as f64);
    let mut corners = [(left - px, top - py), (right + px, top - py), (left - px, bottom + py), (right + px, bottom + py)];
    if rotate != 0.0 {
        let (s, c) = (rotate.to_radians().sin(), rotate.to_radians().cos());
        for p in &mut corners {
            *p = (c * p.0 - s * p.1, s * p.0 + c * p.1);
        }
    }
    for p in &mut corners {
        *p = (js_round(p.0 * 32.0) / 32.0, js_round(p.1 * 32.0) / 32.0);
    }
    let box_scale = tile_pixel_ratio * num(layout, "icon-size", &next_ctx, 1.0);
    let pad = padding4(layout, "icon-padding", &lctx, 2.0).map(|v| v * tile_pixel_ratio);
    let b = rotated_envelope(
        [left * box_scale - pad[3], top * box_scale - pad[0], right * box_scale + pad[1], bottom * box_scale + pad[2]],
        rotate,
    );
    Some((
        IconPart {
            image: name,
            corners,
            size: num(layout, "icon-size", &ctx, 1.0),
            sdf: img.sdf,
            linear: false,
            style: sdf_style(paint, "icon", &ctx),
            along: None,
        },
        b,
        needs_linear,
    ))
}

/// Shaping for a line label: centred, no line breaks; quads with along-line
/// glyph offsets (getGlyphQuads with alongLine = true).
fn build_line_text(
    layer: &Json,
    f: &Feature,
    text: &str,
    assets: &SymbolAssets,
    zoom: f64,
    tile_zoom: f64,
) -> Option<(TextPart, Vec<f64>, f64, f64, (f64, f64))> {
    let layout = &layer["layout"];
    let paint = &layer["paint"];
    let ctx = Ctx { zoom, feature: Some(f) };
    let lctx = Ctx { zoom: tile_zoom, feature: Some(f) };
    let next_ctx = Ctx { zoom: tile_zoom + 1.0, feature: Some(f) };
    let stack = feature_fontstack(layer, Some(f), tile_zoom);
    let gmap = assets.glyphs.stacks.get(&stack)?;
    let spacing = num(layout, "text-letter-spacing", &lctx, 0.0) * ONE_EM;
    let anchor = string(layout, "text-anchor", &lctx, "center");
    let line_height = num(layout, "text-line-height", &lctx, 1.2) * ONE_EM;
    let o = num_pair(layout, "text-offset", &lctx);
    let text_offset = (o.0 * ONE_EM, o.1 * ONE_EM);
    let shaping = shape_text(text, gmap, f64::INFINITY, line_height, &anchor, "center", spacing, text_offset)?;
    let rect_buffer = GLYPH_PBF_BORDER + ATLAS_PADDING as f64;
    let mut quads = Vec::new();
    let mut offsets = Vec::new();
    for l in &shaping.lines {
        for pg in l {
            let g = &gmap[&pg.codepoint];
            let half = g.advance / 2.0;
            let x1 = (g.left - rect_buffer) - half;
            let y1 = (-g.top - rect_buffer) + pg.y;
            let (x2, y2) = (x1 + g.rect_w as f64, y1 + g.rect_h as f64);
            let mut corners = [(x1, y1), (x2, y1), (x1, y2), (x2, y2)];
            for c in &mut corners {
                *c = (js_round(c.0 * 32.0) / 32.0, js_round(c.1 * 32.0) / 32.0);
            }
            quads.push(GlyphQuad { corners, codepoint: pg.codepoint });
            offsets.push(pg.x + half);
        }
    }
    if quads.is_empty() {
        return None;
    }
    let height = shaping.bottom - shaping.top;
    let layout_size = num(layout, "text-size", &next_ctx, 16.0);
    let diameter = if height > 0.0 { height.max(10.0) * layout_size / ONE_EM } else { -1.0 };
    Some((
        TextPart {
            quads,
            fontstack: stack,
            size: num(layout, "text-size", &ctx, 16.0),
            style: sdf_style(paint, "text", &ctx),
            along: None,
        },
        offsets,
        shaping.right - shaping.left,
        diameter,
        text_offset,
    ))
}

/// Symbol instances of a line- or line-center-placed layer in one tile.
#[allow(clippy::too_many_arguments)]
fn line_instances(
    layer: &Json,
    st: &SymbolTile,
    ti: usize,
    assets: &SymbolAssets,
    zoom: f64,
    sort_key_j: Option<&Json>,
    sort_by_key: bool,
    icons_need_linear: &mut bool,
) -> Vec<Instance> {
    let layout = &layer["layout"];
    let paint = &layer["paint"];
    let placement = layout["symbol-placement"].as_str().unwrap_or("point");
    let mut out = Vec::new();
    let Some(src) = layer["source-layer"].as_str().and_then(|sl| st.tile.layers.get(sl)) else { return out };
    let scale = EXTENT / src.extent as f64;
    let tile_zoom = st.coord.0 as f64 + st.overscaling.log2();
    let tile_pixel_ratio = EXTENT / (512.0 * st.overscaling);
    let text_pixel_ratio = 512.0 * st.overscaling / EXTENT;
    let has_text_field = layout.get("text-field").is_some();
    let has_icon_image = layout.get("icon-image").is_some();

    let mut feats: Vec<line::LineFeature> = Vec::new();
    for f in &src.features {
        let ctx = Ctx { zoom: tile_zoom, feature: Some(f) };
        if layer.get("filter").map_or(false, |flt| !eval_filter(flt, &ctx)) {
            continue;
        }
        let geometry: Vec<Vec<(f64, f64)>> = match &f.geometry {
            Geometry::Lines(ls) => ls.iter().map(|l| l.iter().map(|&p| to_extent(p, scale)).collect()).collect(),
            Geometry::Polygons(ps) => ps.iter().flatten().map(|r| r.iter().map(|&p| to_extent(p, scale)).collect()).collect(),
            Geometry::Points(_) => continue,
        };
        let text = if has_text_field { feature_text(layer, f, tile_zoom) } else { None };
        feats.push(line::LineFeature { feature: f, text, geometry });
    }
    if placement == "line" {
        feats = line::merge_lines(feats);
    }
    let mut keyed: Vec<(f64, line::LineFeature)> = feats
        .into_iter()
        .map(|lf| {
            let key = if sort_by_key {
                let ctx = Ctx { zoom: tile_zoom, feature: Some(lf.feature) };
                sort_key_j.map(|j| eval_property(j, Kind::Number, &ctx)).and_then(|v| v.as_num()).unwrap_or(0.0)
            } else {
                0.0
            };
            (key, lf)
        })
        .collect();
    if sort_by_key {
        keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    // symbol-placement line/line-center resolve rotation-alignment 'auto' to 'map'.
    let text_rotate_to_line = layout["text-rotation-alignment"] != "viewport";
    let icon_along = layout["icon-rotation-alignment"] != "viewport";
    let mut compare_text: HashMap<String, Vec<(f64, f64)>> = HashMap::new();

    for (key, lf) in keyed {
        let f = lf.feature;
        let ctx = Ctx { zoom, feature: Some(f) };
        let lctx = Ctx { zoom: tile_zoom, feature: Some(f) };
        let text = match (&lf.text, has_text_field) {
            (Some(t), true) if !t.is_empty() => build_line_text(layer, f, t, assets, zoom, tile_zoom),
            _ => None,
        };
        let icon = if has_icon_image { build_icon(layer, f, assets, zoom, tile_zoom, tile_pixel_ratio) } else { None };
        if icon.as_ref().map_or(false, |i| i.2) {
            *icons_need_linear = true;
        }
        if text.is_none() && icon.is_none() {
            continue;
        }
        // getShapedLabelLength: max of the text width and the shaped icon
        // width (display size, before icon-size), in glyph units.
        let text_w = text.as_ref().map_or(0.0, |t| t.2);
        let icon_w = icon
            .as_ref()
            .and_then(|i| assets.sprite.images.get(&i.0.image))
            .map_or(0.0, |img| img.w as f64 / img.pixel_ratio);
        let label_len = text_w.max(icon_w);
        let text_max_size = num(layout, "text-size", &Ctx { zoom: 18.0, feature: Some(f) }, 16.0);
        let text_max_box_scale = tile_pixel_ratio * text_max_size / 24.0;
        let min_distance = tile_pixel_ratio * num(layout, "symbol-spacing", &lctx, 250.0);
        let max_angle = num(layout, "text-max-angle", &lctx, 45.0).to_radians();
        let repeat_distance = min_distance / 2.0;
        let ttr = num_pair(paint, "text-translate", &ctx);
        let itr = num_pair(paint, "icon-translate", &ctx);

        let lines: Vec<Vec<(f64, f64)>> = if placement == "line" {
            line::clip_line(&lf.geometry, 0.0, 0.0, EXTENT, EXTENT)
        } else {
            lf.geometry.iter().filter(|l| l.len() > 1).cloned().collect()
        };
        for ln in &lines {
            let anchors: Vec<line::Anchor> = if placement == "line" {
                line::get_anchors(ln, min_distance, max_angle, text.is_some(), label_len, 24.0, text_max_box_scale, st.overscaling, EXTENT)
            } else {
                line::get_center_anchor(ln, max_angle, text.is_some(), label_len, 24.0, text_max_box_scale).into_iter().collect()
            };
            for a in anchors {
                if placement == "line" {
                    if let (Some(_), Some(t)) = (&text, &lf.text) {
                        // anchorIsTooClose
                        let list = compare_text.entry(t.clone()).or_default();
                        if list.iter().rev().any(|o| ((a.x - o.0).powi(2) + (a.y - o.1).powi(2)).sqrt() < repeat_distance) {
                            continue;
                        }
                        list.push((a.x, a.y));
                    }
                }
                if a.x < 0.0 || a.x >= EXTENT || a.y < 0.0 || a.y >= EXTENT {
                    continue;
                }
                let to_px = |p: (f64, f64), t: (f64, f64)| (st.ox + p.0 * st.unit_px + t.0, st.oy + p.1 * st.unit_px + t.1);
                let anchor_px = to_px((a.x, a.y), (0.0, 0.0));
                let mut text_line = None;
                let text_part = text.as_ref().map(|(part, offsets, _, diameter, text_offset)| {
                    let line_px: Vec<(f64, f64)> = ln.iter().map(|&p| to_px(p, ttr)).collect();
                    let anchor_t = to_px((a.x, a.y), ttr);
                    let font_scale = part.size / ONE_EM;
                    let pl = ProjectedLine { line: &line_px, anchor: anchor_t, segment: a.segment };
                    let path = line::collision_path(&pl, offsets, font_scale, *text_offset);
                    if *diameter > 0.0 {
                        let pad = num(layout, "text-padding", &lctx, 2.0);
                        text_line = Some(TextCircles { radius: diameter * 0.5 + pad, path });
                    }
                    let mut p = part.clone();
                    p.along = Some(AlongLine {
                        line_px,
                        segment: a.segment,
                        glyph_offsets: offsets.clone(),
                        line_offset: *text_offset,
                        keep_upright: layout["text-keep-upright"] != false,
                        rotate_to_line: text_rotate_to_line,
                        placement_scale: font_scale,
                    });
                    p
                });
                let r = text_pixel_ratio;
                let icon_box = icon.as_ref().map(|i| {
                    let (cx, cy) = (anchor_px.0 + itr.0 + VIEWPORT_PADDING, anchor_px.1 + itr.1 + VIEWPORT_PADDING);
                    [cx + i.1[0] * r, cy + i.1[1] * r, cx + i.1[2] * r, cy + i.1[3] * r]
                });
                let icon_part = icon.as_ref().map(|(part, _, _)| {
                    let mut p = part.clone();
                    if icon_along {
                        let line_px: Vec<(f64, f64)> = ln.iter().map(|&q| to_px(q, itr)).collect();
                        p.along = Some(AlongLine {
                            line_px,
                            segment: a.segment,
                            glyph_offsets: vec![0.0],
                            line_offset: num_pair(layout, "icon-offset", &lctx),
                            keep_upright: layout["icon-keep-upright"] == true,
                            rotate_to_line: text_rotate_to_line,
                            placement_scale: p.size / ONE_EM,
                        });
                    }
                    p
                });
                out.push(Instance {
                    tile_idx: ti,
                    sort_key: key,
                    text_box: None,
                    text_line,
                    icon_box,
                    label: Label { anchor_px, text: text_part, icon: icon_part, tile: ti },
                });
            }
        }
    }
    out
}

/// Run MapLibre's placement for every visible symbol layer.
pub fn place(style: &Json, tiles: &[SymbolTile], assets: &SymbolAssets, zoom: f64, width: f64, height: f64) -> Placement {
    let layers = style["layers"].as_array().map(|v| v.as_slice()).unwrap_or(&[]);
    // Tile order for placement: by x, then y (OverscaledTileID.isLessThan).
    let mut tile_order: Vec<usize> = (0..tiles.len()).collect();
    tile_order.sort_by(|&a, &b| {
        let (za, xa, ya) = tiles[a].coord;
        let (zb, xb, yb) = tiles[b].coord;
        zb.cmp(&za).then(tiles[a].wrap.cmp(&tiles[b].wrap)).then(xa.cmp(&xb)).then(ya.cmp(&yb))
    });

    // Collision grid (brute force; same results as GridIndex).
    let mut items: Vec<Item> = Vec::new();
    let mut placed: HashMap<usize, Vec<Label>> = HashMap::new();
    let grid_w = width + 2.0 * VIEWPORT_PADDING;
    let grid_h = height + 2.0 * VIEWPORT_PADDING;

    for (layer_idx, layer) in layers.iter().enumerate().rev() {
        if layer["type"] != "symbol" || !layer_visible(layer, zoom) {
            continue;
        }
        let layout = &layer["layout"];
        let has_text_field = layout.get("text-field").is_some();
        let has_icon_image = layout.get("icon-image").is_some();
        if !has_text_field && !has_icon_image {
            continue;
        }
        let placement = layout["symbol-placement"].as_str().unwrap_or("point").to_owned();
        let Some(sl) = layer["source-layer"].as_str() else { continue };
        let paint = &layer["paint"];
        let sort_key_j = layout.get("symbol-sort-key");
        let sort_by_key = layout["symbol-z-order"] != "viewport-y" && sort_key_j.map_or(false, is_data_driven);
        let overlap = |kind: &str| {
            layout[format!("{kind}-overlap")] == "always"
                || (layout.get(format!("{kind}-overlap")).is_none() && layout[format!("{kind}-allow-overlap")] == true)
        };
        let (text_always, icon_always) = (overlap("text"), overlap("icon"));
        let text_ignore = layout["text-ignore-placement"] == true;
        let icon_ignore = layout["icon-ignore-placement"] == true;
        let text_optional = layout["text-optional"] == true;
        let icon_optional = layout["icon-optional"] == true;
        // Bucket-level icon filtering (draw_symbol.ts).
        let icon_size_scaled = match layout.get("icon-size") {
            None => false,
            Some(j) if is_data_driven(j) => true,
            Some(j) => eval_property(j, Kind::Number, &Ctx { zoom, feature: None }).as_num() != Some(1.0),
        };
        let mut icons_need_linear = false;

        let mut instances: Vec<Instance> = Vec::new();
        let layer_source = layer["source"].as_str().unwrap_or("");
        for &ti in &tile_order {
            let st = &tiles[ti];
            if st.source != layer_source {
                continue;
            }
            if placement != "point" {
                instances.extend(line_instances(
                    layer, st, ti, assets, zoom, sort_key_j, sort_by_key, &mut icons_need_linear,
                ));
                continue;
            }
            let Some(src) = st.tile.layers.get(sl) else { continue };
            let scale = EXTENT / src.extent as f64;
            let tile_zoom = st.coord.0 as f64 + st.overscaling.log2();
            let tile_pixel_ratio = EXTENT / (512.0 * st.overscaling);
            let text_pixel_ratio = 512.0 * st.overscaling / EXTENT;

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
                let text = if has_text_field { build_text(layer, f, assets, zoom, tile_zoom, tile_pixel_ratio) } else { None };
                let icon = if has_icon_image { build_icon(layer, f, assets, zoom, tile_zoom, tile_pixel_ratio) } else { None };
                if icon.as_ref().map_or(false, |i| i.2) {
                    icons_need_linear = true;
                }
                if text.is_none() && icon.is_none() {
                    continue;
                }

                // Anchors (addFeature).
                let mut anchors: Vec<(f64, f64)> = Vec::new();
                match &f.geometry {
                    Geometry::Points(pts) => anchors.extend(pts.iter().map(|&p| to_extent(p, scale))),
                    Geometry::Lines(ls) => anchors.extend(ls.iter().filter_map(|l| l.first().map(|&p| to_extent(p, scale)))),
                    Geometry::Polygons(polys) => {
                        for poly in polys {
                            let rings: Vec<Vec<(f64, f64)>> =
                                poly.iter().map(|r| r.iter().map(|&p| to_extent(p, scale)).collect()).collect();
                            anchors.push(pole_of_inaccessibility(&rings, 16.0));
                        }
                    }
                }
                let ttr = num_pair(paint, "text-translate", &ctx);
                let itr = num_pair(paint, "icon-translate", &ctx);
                for (ax, ay) in anchors {
                    if ax < 0.0 || ax >= EXTENT || ay < 0.0 || ay >= EXTENT {
                        continue;
                    }
                    let px = st.ox + ax * st.unit_px;
                    let py = st.oy + ay * st.unit_px;
                    let r = text_pixel_ratio;
                    let proj = |b: [f64; 4], t: (f64, f64)| {
                        let (cx, cy) = (px + t.0 + VIEWPORT_PADDING, py + t.1 + VIEWPORT_PADDING);
                        [cx + b[0] * r, cy + b[1] * r, cx + b[2] * r, cy + b[3] * r]
                    };
                    instances.push(Instance {
                        tile_idx: ti,
                        sort_key: key,
                        text_line: None,
                        text_box: text.as_ref().map(|t| proj(t.1, ttr)),
                        icon_box: icon.as_ref().map(|i| proj(i.1, itr)),
                        label: Label {
                            anchor_px: (px, py),
                            text: text.as_ref().map(|t| t.0.clone()),
                            icon: icon.as_ref().map(|i| i.0.clone()),
                            tile: ti,
                        },
                    });
                }
            }
        }
        if sort_by_key {
            instances.sort_by(|a, b| a.sort_key.partial_cmp(&b.sort_key).unwrap_or(std::cmp::Ordering::Equal));
        }
        let has_icon_data = instances.iter().any(|i| i.icon_box.is_some());
        let has_text_data = instances.iter().any(|i| i.text_box.is_some() || i.text_line.is_some());
        let always_show_text = text_always && (icon_always || !has_icon_data || icon_optional);
        let always_show_icon = icon_always && (text_always || !has_text_data || text_optional);

        let mut layer_labels: Vec<(f64, usize, Label)> = Vec::new();
        for mut inst in instances {
            let placeable = |b: &[f64; 4], always: bool, items: &Vec<Item>| {
                let inside = b[2] >= 0.0 && b[0] < grid_w && b[3] >= 0.0 && b[1] < grid_h;
                inside && (always || !hit_box(items, b, grid_w, grid_h))
            };
            let mut circles: Vec<(f64, f64, f64)> = Vec::new();
            let mut place_text = match (&inst.text_box, &inst.text_line) {
                (Some(b), _) => placeable(b, text_always, &items),
                (None, Some(tc)) => {
                    let (c, collided) = match &tc.path {
                        Some(path) => place_circles(&items, path, tc.radius, width, height, text_always),
                        None => (Vec::new(), false),
                    };
                    circles = c;
                    text_always || (!circles.is_empty() && !collided)
                }
                (None, None) => false,
            };
            let mut place_icon = inst.icon_box.as_ref().map_or(false, |b| placeable(b, icon_always, &items));
            let has_text = inst.text_box.is_some() || inst.text_line.is_some();
            let icon_without_text = text_optional || !has_text;
            let text_without_icon = icon_optional || inst.icon_box.is_none();
            if !icon_without_text && !text_without_icon {
                let both = place_icon && place_text;
                place_icon = both;
                place_text = both;
            } else if !text_without_icon {
                place_text = place_icon && place_text;
            } else if !icon_without_text {
                place_icon = place_icon && place_text;
            }
            if place_text && !text_ignore {
                if let Some(b) = inst.text_box {
                    items.push(Item::Box(b));
                }
                for &(x, y, r) in &circles {
                    items.push(Item::Circle(x, y, r));
                }
            }
            if place_icon && !icon_ignore {
                items.push(Item::Box(inst.icon_box.unwrap()));
            }
            let text_visible = place_text || (always_show_text && has_text);
            let icon_visible = place_icon || (always_show_icon && inst.icon_box.is_some());
            if !text_visible {
                inst.label.text = None;
            }
            if !icon_visible {
                inst.label.icon = None;
            }
            if let Some(ic) = inst.label.icon.as_mut() {
                ic.linear = ic.sdf || icon_size_scaled || icons_need_linear;
            }
            if inst.label.text.is_some() || inst.label.icon.is_some() {
                layer_labels.push((inst.sort_key, inst.tile_idx, inst.label));
            }
        }
        // Draw order: per tile, symbols in bucket order (sort-key order).
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

/// Halo passes for a layer (draw_symbol.ts): a separate halo pass when the
/// (constant) halo width exceeds 1, is data-driven, or letter spacing is
/// negative; otherwise halo and fill in one pass.
fn halo_passes(paint: &Json, layout: &Json, prefix: &str) -> &'static [(bool, bool)] {
    let halo_j = paint.get(format!("{prefix}-halo-width"));
    let has_halo = halo_j.map_or(true, |j| is_data_driven(j) || j.as_f64() != Some(0.0));
    let halo_const = halo_j.and_then(|j| if is_data_driven(j) { None } else { j.as_f64() });
    let spacing_neg = layout.get("text-letter-spacing").and_then(|j| j.as_f64()).map_or(false, |s| s * ONE_EM < 0.0);
    let glyph_overlap = spacing_neg || halo_const.map_or(halo_j.is_some(), |hw| hw > 1.0);
    if has_halo && glyph_overlap {
        &[(true, false), (false, true)]
    } else if has_halo {
        &[(true, true)]
    } else {
        &[(false, true)]
    }
}

/// Draw the placed symbols of one layer: icons for all tiles, then text.
/// Draw one symbol layer. Placement is in CSS pixels (`css` is the CSS
/// viewport size); quads are drawn `dpr` times larger.
pub fn draw_layer(
    fb: &mut Framebuffer,
    layer: &Json,
    layer_idx: usize,
    placement: &Placement,
    assets: &SymbolAssets,
    dpr: f64,
    css: (usize, usize),
) {
    let Some(labels) = placement.labels.get(&layer_idx) else { return };
    let (w, h) = (fb.w, fb.h);
    let full = Clip { x0: 0.0, y0: 0.0, x1: w as f64, y1: h as f64 };
    let paint = &layer["paint"];
    let layout = &layer["layout"];

    for is_text in [false, true] {
        let passes = if is_text { halo_passes(paint, layout, "text") } else { halo_passes(paint, layout, "icon") };
        let mut start = 0;
        while start < labels.len() {
            let tile = labels[start].tile;
            let end = labels[start..].iter().position(|l| l.tile != tile).map_or(labels.len(), |p| start + p);
            let group = &labels[start..end];
            if is_text {
                for &(is_halo, is_plain) in passes {
                    for l in group {
                        if let Some(t) = &l.text {
                            draw_text(fb, l.anchor_px, t, &assets.glyphs, is_halo, is_plain, &full, dpr, css);
                        }
                    }
                }
            } else {
                let sdf = group.iter().any(|l| l.icon.as_ref().map_or(false, |i| i.sdf));
                let icon_passes: &[(bool, bool)] = if sdf { passes } else { &[(false, true)] };
                for &(is_halo, is_plain) in icon_passes {
                    for l in group {
                        if let Some(ic) = &l.icon {
                            draw_icon(fb, l.anchor_px, ic, &assets.sprite, is_halo, is_plain, &full, dpr, css);
                        }
                    }
                }
            }
            start = end;
        }
    }
}

/// symbol_sdf.fragment.glsl for one fragment.
fn sdf_fragment(dist: f64, font_scale: f64, s: &SdfStyle, is_halo: bool, is_plain: bool, dpr: f64) -> [f64; 4] {
    // EDGE_GAMMA = 0.105 / DEVICE_PIXEL_RATIO
    let edge_gamma = EDGE_GAMMA / dpr;
    let gamma = edge_gamma / font_scale;
    let inner_edge = (256.0 - 64.0) / 256.0;
    let fill = [s.fill.r as f64, s.fill.g as f64, s.fill.b as f64, s.fill.a as f64];
    let halo = [s.halo.r as f64, s.halo.g as f64, s.halo.b as f64, s.halo.a as f64];
    let op = s.opacity;
    let mut text = [0f64; 4];
    if is_plain {
        let a = smoothstep(inner_edge - gamma, inner_edge + gamma, dist);
        for i in 0..4 {
            text[i] = op * a * fill[i];
        }
    }
    if !is_halo {
        return text;
    }
    let gamma_halo = (s.halo_blur * 1.19 / SDF_PX + edge_gamma) / font_scale;
    let inner_edge_halo = inner_edge + gamma_halo;
    let halo_edge = (6.0 - s.halo_width / font_scale) / SDF_PX;
    let ah = smoothstep(inner_edge_halo - gamma_halo, inner_edge_halo + gamma_halo, dist);
    let ah = smoothstep(halo_edge - gamma_halo, halo_edge + gamma_halo, dist).min(1.0 - ah);
    let mut out = [0f64; 4];
    for i in 0..4 {
        let hc = op * ah * halo[i];
        out[i] = if is_plain { text[i] + (1.0 - text[3]) * hc } else { hc };
    }
    out
}

fn blend_out(fb: &mut Framebuffer, x: usize, y: usize, out: [f64; 4]) {
    if out[3] > 0.0 || out[0] > 0.0 || out[1] > 0.0 || out[2] > 0.0 {
        fb.blend(x, y, [out[0] as f32, out[1] as f32, out[2] as f32, out[3] as f32]);
    }
}

/// Per-quad screen origin and rotation: the anchor (angle 0) for point
/// symbols, or glyphs placed along the projected line (updateLineLabels).
/// None when the symbol is hidden.
fn along_positions(along: Option<&AlongLine>, anchor: (f64, f64), n: usize, w: usize, h: usize) -> Option<Vec<((f64, f64), f64)>> {
    let Some(al) = along else { return Some(vec![(anchor, 0.0); n]) };
    // isVisible: anchor within the clip-space padding.
    let (bx, by) = (256.0 / w as f64 * 2.0 + 1.0, 256.0 / h as f64 * 2.0 + 1.0);
    let (cx, cy) = (anchor.0 / w as f64 * 2.0 - 1.0, 1.0 - anchor.1 / h as f64 * 2.0);
    if cx < -bx || cx > bx || cy < -by || cy > by {
        return None;
    }
    let pl = ProjectedLine { line: &al.line_px, anchor, segment: al.segment };
    match line::place_glyphs_for_render(&pl, &al.glyph_offsets, al.placement_scale, al.line_offset, al.keep_upright, al.rotate_to_line) {
        line::GlyphsPlacement::Hidden => None,
        line::GlyphsPlacement::Placed(g) if g.len() == n => Some(g.into_iter().map(|p| (p.point, p.angle)).collect()),
        line::GlyphsPlacement::Placed(_) => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    fb: &mut Framebuffer,
    anchor: (f64, f64),
    t: &TextPart,
    glyphs: &GlyphMap,
    is_halo: bool,
    is_plain: bool,
    clip: &Clip,
    dpr: f64,
    css: (usize, usize),
) {
    let (w, h) = (clip.x1 as usize, clip.y1 as usize);
    if t.style.opacity < 0.1 {
        return; // symbol_sdf.vertex: total_opacity < 0.1 is culled
    }
    let Some(gmap) = glyphs.stacks.get(&t.fontstack) else { return };
    let font_scale = t.size / 24.0;
    let positions = match along_positions(t.along.as_ref(), anchor, t.quads.len(), css.0, css.1) {
        Some(p) => p,
        None => return,
    };
    for (qi, q) in t.quads.iter().enumerate() {
        let Some(g) = gmap.get(&q.codepoint) else { continue };
        let tex = [(0.0, 0.0), (g.rect_w as f64, 0.0), (0.0, g.rect_h as f64), (g.rect_w as f64, g.rect_h as f64)];
        let (origin, angle) = positions[qi];
        let (sa, ca) = (angle.sin(), angle.cos());
        let v: Vec<Vert<2>> = (0..4)
            .map(|i| {
                let (ox, oy) = (q.corners[i].0 * font_scale, q.corners[i].1 * font_scale);
                Vert {
                    x: (origin.0 + ca * ox - sa * oy) * dpr,
                    y: (origin.1 + sa * ox + ca * oy) * dpr,
                    v: [tex[i].0, tex[i].1],
                }
            })
            .collect();
        for tri in [[0usize, 2, 1], [1, 2, 3]] {
            raster_triangle(w, h, [v[tri[0]], v[tri[1]], v[tri[2]]], clip, |x, y, tc| {
                let out = sdf_fragment(g.sample(tc[0], tc[1]), font_scale, &t.style, is_halo, is_plain, dpr);
                blend_out(fb, x, y, out);
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_icon(
    fb: &mut Framebuffer,
    anchor: (f64, f64),
    ic: &IconPart,
    sprite: &Sprite,
    is_halo: bool,
    is_plain: bool,
    clip: &Clip,
    dpr: f64,
    css: (usize, usize),
) {
    let (w, h) = (clip.x1 as usize, clip.y1 as usize);
    if ic.style.opacity < 0.1 {
        return;
    }
    let Some(img) = sprite.images.get(&ic.image) else { return };
    let (cw, ch) = ((img.w + 2) as f64, (img.h + 2) as f64);
    let tex = [(0.0, 0.0), (cw, 0.0), (0.0, ch), (cw, ch)];
    let Some(positions) = along_positions(ic.along.as_ref(), anchor, 1, css.0, css.1) else { return };
    let (origin, angle) = positions[0];
    let (sa, ca) = (angle.sin(), angle.cos());
    let v: Vec<Vert<2>> = (0..4)
        .map(|i| {
            let (ox, oy) = (ic.corners[i].0 * ic.size, ic.corners[i].1 * ic.size);
            Vert {
                x: (origin.0 + ca * ox - sa * oy) * dpr,
                y: (origin.1 + sa * ox + ca * oy) * dpr,
                v: [tex[i].0, tex[i].1],
            }
        })
        .collect();
    for tri in [[0usize, 2, 1], [1, 2, 3]] {
        raster_triangle(w, h, [v[tri[0]], v[tri[1]], v[tri[2]]], clip, |x, y, tc| {
            let s = img.sample(tc[0], tc[1], ic.linear);
            let out = if ic.sdf {
                sdf_fragment(s[3] / 255.0, ic.size, &ic.style, is_halo, is_plain, dpr)
            } else {
                let op = ic.style.opacity;
                [s[0] / 255.0 * op, s[1] / 255.0 * op, s[2] / 255.0 * op, s[3] / 255.0 * op]
            };
            blend_out(fb, x, y, out);
        });
    }
}

// ── SVG output ───────────────────────────────────────────────────────────────

/// Affine map from a quad's texture cell (u, v in [0, cw] x [0, ch]) to its
/// screen corners tl, tr, bl.
fn cell_to_screen(c: [(f64, f64); 4], cw: f64, ch: f64) -> impl Fn((f64, f64)) -> (f64, f64) {
    let (tl, tr, bl) = (c[0], c[1], c[2]);
    move |(u, v)| {
        (
            tl.0 + (tr.0 - tl.0) * u / cw + (bl.0 - tl.0) * v / ch,
            tl.1 + (tr.1 - tl.1) * u / cw + (bl.1 - tl.1) * v / ch,
        )
    }
}

/// Screen corners (tl, tr, bl, br) of a quad placed at `origin`, rotated by
/// `angle`, with corner offsets scaled by `scale`.
fn quad_screen(corners: &[(f64, f64); 4], origin: (f64, f64), angle: f64, scale: f64) -> [(f64, f64); 4] {
    let (sa, ca) = (angle.sin(), angle.cos());
    corners.map(|(x, y)| {
        let (ox, oy) = (x * scale, y * scale);
        (origin.0 + ca * ox - sa * oy, origin.1 + sa * ox + ca * oy)
    })
}

/// SDF outline paths for one quad, as the SDF shader would draw it: for the
/// halo pass a ring (the halo contour minus the contour where the halo fades
/// out inside the glyph), for the fill pass the glyph contour at the edge
/// threshold. Rings wind consistently, so `nonzero` subtracts the inner ring.
#[allow(clippy::too_many_arguments)]
fn svg_sdf_quad(
    out: &mut String,
    cache: &mut HashMap<(String, u32, i64), Vec<Vec<(f64, f64)>>>,
    key: (&str, u32),
    w: usize,
    h: usize,
    value: &dyn Fn(usize, usize) -> f64,
    map: &dyn Fn((f64, f64)) -> (f64, f64),
    font_scale: f64,
    s: &SdfStyle,
    is_halo: bool,
    is_plain: bool,
) {
    use super::svg;
    use std::fmt::Write as _;
    let mut rings_at = |thr: f64, reverse: bool| -> Vec<Vec<(f64, f64)>> {
        let k = (key.0.to_owned(), key.1, (thr * 10_000.0).round() as i64);
        let rings = cache.entry(k).or_insert_with(|| svg::trace_contours(w, h, value, thr));
        rings
            .iter()
            .map(|r| {
                let mut pts: Vec<(f64, f64)> = r.iter().map(|&p| map(p)).collect();
                if reverse {
                    pts.reverse();
                }
                pts
            })
            .collect()
    };
    // symbol_sdf.fragment.glsl: alpha = smoothstep(edge - gamma, edge + gamma,
    // dist), a ramp about 0.84 px wide at any size (EDGE_GAMMA at ratio 1).
    // SVG has no such ramp, so it is approximated by SDF_BANDS nested
    // outlines traced where the smoothstep crosses (k - 1/2) / N, with
    // opacities that stack to k/N of the colour's alpha.
    let inner_edge = (256.0 - 64.0) / 256.0;
    let mut banded = |out: &mut String, hex: &str, a: f64, edge: f64, gamma: f64, hole: Option<f64>| {
        let n = SDF_BANDS;
        let mut below = 0.0; // stacked alpha so far
        for k in 1..=n {
            let y = (k as f64 - 0.5) / n as f64;
            let t = 0.5 - ((1.0 - 2.0 * y).asin() / 3.0).sin(); // smoothstep^-1(y)
            let thr = edge + gamma * BAND_SPREAD * (2.0 * t - 1.0);
            let target = a * k as f64 / n as f64;
            let layer = if below >= 1.0 { 0.0 } else { ((target - below) / (1.0 - below)).clamp(0.0, 1.0) };
            below = target;
            let mut rings = rings_at(thr, false);
            if let Some(h) = hole {
                rings.extend(rings_at(h, true));
            }
            if !rings.is_empty() && layer > 0.0 {
                let _ = write!(out, r#"<path d="{}" fill="{hex}" fill-opacity="{}"/>"#, svg::path_data(&rings, true), svg::alpha(layer));
            }
        }
    };
    if is_halo && s.halo_width > 0.0 && s.halo.a > 0.0 {
        if let Some((hex, a)) = svg::paint(s.halo, s.opacity) {
            let gamma_halo = (s.halo_blur * 1.19 / SDF_PX + EDGE_GAMMA) / font_scale;
            let halo_edge = (6.0 - s.halo_width / font_scale) / SDF_PX;
            banded(out, &hex, a, halo_edge, gamma_halo, Some(inner_edge + gamma_halo));
        }
    }
    if is_plain {
        if let Some((hex, a)) = svg::paint(s.fill, s.opacity) {
            banded(out, &hex, a, inner_edge, EDGE_GAMMA / font_scale, None);
        }
    }
}

/// Outlines per SDF edge in SVG output (see svg_sdf_quad), and how far apart
/// they sit relative to the smoothstep's quantiles. Measured against MapLibre
/// (resvg rasterisation): 2 bands at 0.85 fit best; more bands over-soften,
/// because the rasteriser's own edge antialiasing adds to the ramp.
const SDF_BANDS: usize = 2;
const BAND_SPREAD: f64 = 0.85;

/// Write one symbol layer as SVG in draw_layer's order: icons, then text,
/// each per tile with the shader's halo passes. Glyphs and SDF icons are
/// outlines traced from their SDFs at the shader's thresholds, one path per
/// quad (MapLibre blends each quad separately); other icons are embedded
/// sprite images.
#[allow(clippy::too_many_arguments)]
pub fn svg_layer(
    out: &mut String,
    defs: &mut String,
    layer: &Json,
    layer_idx: usize,
    placement: &Placement,
    assets: &SymbolAssets,
    w: usize,
    h: usize,
    prefix: &str,
) {
    use super::svg;
    use std::fmt::Write as _;
    let Some(labels) = placement.labels.get(&layer_idx) else { return };
    let paint = &layer["paint"];
    let layout = &layer["layout"];
    let mut cache: HashMap<(String, u32, i64), Vec<Vec<(f64, f64)>>> = HashMap::new();
    let mut image_ids: HashMap<String, String> = HashMap::new();
    let lid = layer["id"].as_str().unwrap_or("").replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    out.push_str(r#"<g fill-rule="nonzero">"#);

    let tile_groups = || {
        let mut groups = Vec::new();
        let mut start = 0;
        while start < labels.len() {
            let tile = labels[start].tile;
            let end = labels[start..].iter().position(|l| l.tile != tile).map_or(labels.len(), |p| start + p);
            groups.push(&labels[start..end]);
            start = end;
        }
        groups
    };

    // Icons.
    let icon_halo_passes = halo_passes(paint, layout, "icon");
    for group in tile_groups() {
        let sdf = group.iter().any(|l| l.icon.as_ref().map_or(false, |i| i.sdf));
        let passes: &[(bool, bool)] = if sdf { icon_halo_passes } else { &[(false, true)] };
        for &(is_halo, is_plain) in passes {
            for l in group {
                let Some(ic) = &l.icon else { continue };
                if ic.style.opacity < 0.1 {
                    continue;
                }
                let Some(img) = assets.sprite.images.get(&ic.image) else { continue };
                let Some(pos) = along_positions(ic.along.as_ref(), l.anchor_px, 1, w, h) else { continue };
                let q = quad_screen(&ic.corners, pos[0].0, pos[0].1, ic.size);
                let (cw, ch) = (img.w + 2, img.h + 2);
                if img.sdf {
                    let map = cell_to_screen(q, cw as f64, ch as f64);
                    let value = |i: usize, j: usize| img.cell[(j * cw + i) * 4 + 3] as f64 / 255.0;
                    let key = format!("icon:{}", ic.image);
                    svg_sdf_quad(out, &mut cache, (&key, 0), cw, ch, &value, &map, ic.size, &ic.style, is_halo, is_plain);
                    continue;
                }
                let n = image_ids.len();
                let id = image_ids
                    .entry(ic.image.clone())
                    .or_insert_with(|| {
                        let id = format!("{prefix}{lid}_img{n}");
                        // Straight-alpha, unpadded copy of the sprite image.
                        let mut rgba = Vec::with_capacity(img.w * img.h * 4);
                        for row in 0..img.h {
                            for col in 0..img.w {
                                let o = ((row + 1) * cw + col + 1) * 4;
                                let a = img.cell[o + 3] as u32;
                                for k in 0..3 {
                                    let c = img.cell[o + k] as u32;
                                    rgba.push(if a == 0 { 0 } else { ((c * 255 + a / 2) / a).min(255) as u8 });
                                }
                                rgba.push(a as u8);
                            }
                        }
                        let _ = write!(
                            defs,
                            r#"<image id="{id}" width="{}" height="{}" href="{}"/>"#,
                            img.w,
                            img.h,
                            svg::png_data_uri(img.w as u32, img.h as u32, &rgba)
                        );
                        id
                    })
                    .clone();
                // matrix maps cell (u, v) to screen; the image sits at cell (1, 1).
                let (tl, tr, bl) = (q[0], q[1], q[2]);
                let (a_, b_) = ((tr.0 - tl.0) / cw as f64, (tr.1 - tl.1) / cw as f64);
                let (c_, d_) = ((bl.0 - tl.0) / ch as f64, (bl.1 - tl.1) / ch as f64);
                let render = if ic.linear { "" } else { r#" style="image-rendering:pixelated""# };
                let _ = write!(
                    out,
                    r##"<use href="#{id}" x="1" y="1" opacity="{}" transform="matrix({} {} {} {} {} {})"{render}/>"##,
                    svg::alpha(ic.style.opacity.min(1.0)),
                    a_,
                    b_,
                    c_,
                    d_,
                    svg::num(tl.0),
                    svg::num(tl.1)
                );
            }
        }
    }

    // Text.
    let text_passes = halo_passes(paint, layout, "text");
    for group in tile_groups() {
        for &(is_halo, is_plain) in text_passes {
            for l in group {
                let Some(t) = &l.text else { continue };
                if t.style.opacity < 0.1 {
                    continue;
                }
                let Some(gmap) = assets.glyphs.stacks.get(&t.fontstack) else { continue };
                let fs = t.size / 24.0;
                let Some(pos) = along_positions(t.along.as_ref(), l.anchor_px, t.quads.len(), w, h) else { continue };
                for (qi, q) in t.quads.iter().enumerate() {
                    let Some(g) = gmap.get(&q.codepoint) else { continue };
                    let sq = quad_screen(&q.corners, pos[qi].0, pos[qi].1, fs);
                    let map = cell_to_screen(sq, g.rect_w as f64, g.rect_h as f64);
                    let value = |i: usize, j: usize| g.cell[j * g.rect_w + i] as f64 / 255.0;
                    svg_sdf_quad(
                        out,
                        &mut cache,
                        (&t.fontstack, q.codepoint),
                        g.rect_w,
                        g.rect_h,
                        &value,
                        &map,
                        fs,
                        &t.style,
                        is_halo,
                        is_plain,
                    );
                }
            }
        }
    }
    out.push_str("</g>");
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
