//! Line-placed symbols: a port of the line parts of MapLibre GL JS's symbol
//! pipeline.
//!
//! - `merge_lines`       symbol/merge_lines.ts
//! - `clip_line`         symbol/clip_line.ts (clipLine)
//! - `get_anchors`       symbol/get_anchors.ts (+ check_max_angle.ts)
//! - `place_glyph_along_line`, `place_glyphs_along_line`
//!                       symbol/projection.ts (label plane = screen pixels;
//!                       no pitch, so projection is affine)
//! - `collision_path`    the projected path used by placeCollisionCircles
//!
//! All geometry here is in MapLibre's 8192-unit tile space unless noted.

use std::collections::HashMap;

pub type P = (f64, f64);

fn dist(a: P, b: P) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// point-geometry `a.angleTo(b)` = atan2(a.y - b.y, a.x - b.x).
fn angle_to(a: P, b: P) -> f64 {
    (a.1 - b.1).atan2(a.0 - b.0)
}

fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

// ── mergeLines ───────────────────────────────────────────────────────────────

/// A line-placed feature during layout: its (possibly merged) geometry.
pub struct LineFeature<'a> {
    pub feature: &'a super::mvt::Feature,
    pub text: Option<String>,
    pub geometry: Vec<Vec<P>>,
}

/// mergeLines: join features whose first lines share an endpoint and text.
pub fn merge_lines(features: Vec<LineFeature>) -> Vec<LineFeature> {
    let mut left: HashMap<String, usize> = HashMap::new();
    let mut right: HashMap<String, usize> = HashMap::new();
    let mut merged: Vec<Option<LineFeature>> = Vec::new();
    let key = |text: &str, geom: &[Vec<P>], on_right: bool| {
        let p = if on_right { *geom[0].last().unwrap() } else { geom[0][0] };
        format!("{text}:{}:{}", p.0, p.1)
    };
    for f in features {
        let text = match &f.text {
            Some(t) if !t.is_empty() && !f.geometry.is_empty() && !f.geometry[0].is_empty() => t.clone(),
            _ => {
                merged.push(Some(f));
                continue;
            }
        };
        let lk = key(&text, &f.geometry, false);
        let rk = key(&text, &f.geometry, true);
        let in_right = right.get(&lk).copied();
        let in_left = left.get(&rk).copied();
        match (in_right, in_left) {
            (Some(ri), Some(li)) if ri != li => {
                // mergeFromLeft(lk, rk, geom)
                left.remove(&rk);
                left.insert(lk.clone(), li);
                {
                    let m = merged[li].as_mut().unwrap();
                    m.geometry[0].remove(0);
                    let mut g = f.geometry[0].clone();
                    g.extend(m.geometry[0].iter().copied());
                    m.geometry[0] = g;
                }
                // mergeFromRight(lk, rk, merged[li].geometry)
                let geom_j = merged[li].as_ref().unwrap().geometry[0].clone();
                right.remove(&lk);
                right.insert(rk.clone(), ri);
                {
                    let m = merged[ri].as_mut().unwrap();
                    m.geometry[0].pop();
                    m.geometry[0].extend(geom_j);
                }
                left.remove(&lk);
                right.remove(&rk);
                let new_rk = key(&text, &merged[ri].as_ref().unwrap().geometry, true);
                right.insert(new_rk, ri);
                merged[li] = None;
            }
            (Some(ri), _) => {
                right.remove(&lk);
                right.insert(rk, ri);
                let m = merged[ri].as_mut().unwrap();
                m.geometry[0].pop();
                m.geometry[0].extend(f.geometry[0].iter().copied());
            }
            (None, Some(li)) => {
                left.remove(&rk);
                left.insert(lk, li);
                let m = merged[li].as_mut().unwrap();
                m.geometry[0].remove(0);
                let mut g = f.geometry[0].clone();
                g.extend(m.geometry[0].iter().copied());
                m.geometry[0] = g;
            }
            (None, None) => {
                merged.push(Some(f));
                let i = merged.len() - 1;
                left.insert(lk, i);
                right.insert(rk, i);
            }
        }
    }
    merged.into_iter().flatten().collect()
}

// ── clipLine ─────────────────────────────────────────────────────────────────

pub fn clip_line(lines: &[Vec<P>], x1: f64, y1: f64, x2: f64, y2: f64) -> Vec<Vec<P>> {
    let mut out: Vec<Vec<P>> = Vec::new();
    for line in lines {
        let mut current: Option<usize> = None;
        for i in 0..line.len().saturating_sub(1) {
            let (mut p0, mut p1) = (line[i], line[i + 1]);
            let r = |p: P| (js_round(p.0), js_round(p.1));
            if p0.0 < x1 && p1.0 < x1 {
                continue;
            } else if p0.0 < x1 {
                p0 = r((x1, p0.1 + (p1.1 - p0.1) * ((x1 - p0.0) / (p1.0 - p0.0))));
            } else if p1.0 < x1 {
                p1 = r((x1, p0.1 + (p1.1 - p0.1) * ((x1 - p0.0) / (p1.0 - p0.0))));
            }
            if p0.1 < y1 && p1.1 < y1 {
                continue;
            } else if p0.1 < y1 {
                p0 = r((p0.0 + (p1.0 - p0.0) * ((y1 - p0.1) / (p1.1 - p0.1)), y1));
            } else if p1.1 < y1 {
                p1 = r((p0.0 + (p1.0 - p0.0) * ((y1 - p0.1) / (p1.1 - p0.1)), y1));
            }
            if p0.0 >= x2 && p1.0 >= x2 {
                continue;
            } else if p0.0 >= x2 {
                p0 = r((x2, p0.1 + (p1.1 - p0.1) * ((x2 - p0.0) / (p1.0 - p0.0))));
            } else if p1.0 >= x2 {
                p1 = r((x2, p0.1 + (p1.1 - p0.1) * ((x2 - p0.0) / (p1.0 - p0.0))));
            }
            if p0.1 >= y2 && p1.1 >= y2 {
                continue;
            } else if p0.1 >= y2 {
                p0 = r((p0.0 + (p1.0 - p0.0) * ((y2 - p0.1) / (p1.1 - p0.1)), y2));
            } else if p1.1 >= y2 {
                p1 = r((p0.0 + (p1.0 - p0.0) * ((y2 - p0.1) / (p1.1 - p0.1)), y2));
            }
            let start_new = match current {
                None => true,
                Some(c) => *out[c].last().unwrap() != p0,
            };
            if start_new {
                out.push(vec![p0]);
                current = Some(out.len() - 1);
            }
            out[current.unwrap()].push(p1);
        }
    }
    out
}

// ── getAnchors ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub x: f64,
    pub y: f64,
    pub segment: usize,
}

fn line_length(line: &[P]) -> f64 {
    line.windows(2).map(|w| dist(w[0], w[1])).sum()
}

pub fn check_max_angle(line: &[P], anchor: &Anchor, label_length: f64, window_size: f64, max_angle: f64) -> bool {
    if label_length == 0.0 {
        return true;
    }
    let mut p = (anchor.x, anchor.y);
    let mut index = anchor.segment as i64 + 1;
    let mut anchor_distance = 0.0;
    while anchor_distance > -label_length / 2.0 {
        index -= 1;
        if index < 0 {
            return false;
        }
        anchor_distance -= dist(line[index as usize], p);
        p = line[index as usize];
    }
    anchor_distance += dist(line[index as usize], line[index as usize + 1]);
    index += 1;
    let mut recent: std::collections::VecDeque<(f64, f64)> = std::collections::VecDeque::new();
    let mut recent_delta = 0.0;
    while anchor_distance < label_length / 2.0 {
        let i = index as usize;
        let prev = line[i - 1];
        let current = line[i];
        let Some(&next) = line.get(i + 1) else { return false };
        let mut delta = angle_to(prev, current) - angle_to(current, next);
        let tau = std::f64::consts::PI * 2.0;
        // JS % keeps the sign of the dividend.
        let m = (delta + 3.0 * std::f64::consts::PI) % tau;
        delta = (m - std::f64::consts::PI).abs();
        recent.push_back((anchor_distance, delta));
        recent_delta += delta;
        while anchor_distance - recent.front().unwrap().0 > window_size {
            recent_delta -= recent.pop_front().unwrap().1;
        }
        if recent_delta > max_angle {
            return false;
        }
        index += 1;
        anchor_distance += dist(current, next);
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn resample(
    line: &[P],
    offset: f64,
    spacing: f64,
    angle_window: f64,
    max_angle: f64,
    label_length: f64,
    continued: bool,
    place_at_middle: bool,
    extent: f64,
) -> Vec<Anchor> {
    let half = label_length / 2.0;
    let total = line_length(line);
    let mut distance = 0.0;
    let mut marked = offset - spacing;
    let mut anchors = Vec::new();
    for i in 0..line.len().saturating_sub(1) {
        let (a, b) = (line[i], line[i + 1]);
        let seg = dist(a, b);
        while marked + spacing < distance + seg {
            marked += spacing;
            let t = (marked - distance) / seg;
            let x = a.0 + (b.0 - a.0) * t;
            let y = a.1 + (b.1 - a.1) * t;
            if x >= 0.0 && x < extent && y >= 0.0 && y < extent && marked - half >= 0.0 && marked + half <= total {
                let anchor = Anchor { x: js_round(x), y: js_round(y), segment: i };
                if angle_window == 0.0 || check_max_angle(line, &anchor, label_length, angle_window, max_angle) {
                    anchors.push(anchor);
                }
            }
        }
        distance += seg;
    }
    if !place_at_middle && anchors.is_empty() && !continued {
        anchors = resample(line, distance / 2.0, spacing, angle_window, max_angle, label_length, continued, true, extent);
    }
    anchors
}

/// getAnchors. `label_len` is the shaped label length (max of text and icon)
/// in glyph units; `has_text` enables the angle window.
#[allow(clippy::too_many_arguments)]
pub fn get_anchors(
    line: &[P],
    spacing: f64,
    max_angle: f64,
    has_text: bool,
    label_len: f64,
    glyph_size: f64,
    box_scale: f64,
    overscaling: f64,
    extent: f64,
) -> Vec<Anchor> {
    let angle_window = if has_text { 3.0 / 5.0 * glyph_size * box_scale } else { 0.0 };
    let label_length = label_len * box_scale;
    let continued = line[0].0 == 0.0 || line[0].0 == extent || line[0].1 == 0.0 || line[0].1 == extent;
    let mut spacing = spacing;
    if spacing - label_length < spacing / 4.0 {
        spacing = label_length + spacing / 4.0;
    }
    let fixed_extra = glyph_size * 2.0;
    let offset = if !continued {
        ((label_len / 2.0 + fixed_extra) * box_scale * overscaling) % spacing
    } else {
        (spacing / 2.0 * overscaling) % spacing
    };
    resample(line, offset, spacing, angle_window, max_angle, label_length, continued, false, extent)
}

/// getCenterAnchor (symbol-placement: line-center).
pub fn get_center_anchor(line: &[P], max_angle: f64, has_text: bool, label_len: f64, glyph_size: f64, box_scale: f64) -> Option<Anchor> {
    let angle_window = if has_text { 3.0 / 5.0 * glyph_size * box_scale } else { 0.0 };
    let label_length = label_len * box_scale;
    let center = line_length(line) / 2.0;
    let mut prev = 0.0;
    for i in 0..line.len().saturating_sub(1) {
        let (a, b) = (line[i], line[i + 1]);
        let seg = dist(a, b);
        if prev + seg > center {
            let t = (center - prev) / seg;
            let anchor = Anchor { x: js_round(a.0 + (b.0 - a.0) * t), y: js_round(a.1 + (b.1 - a.1) * t), segment: i };
            return if angle_window == 0.0 || check_max_angle(line, &anchor, label_length, angle_window, max_angle) {
                Some(anchor)
            } else {
                None
            };
        }
        prev += seg;
    }
    None
}

// ── Glyph placement along the projected line (projection.ts) ────────────────

/// A line symbol projected to screen pixels.
pub struct ProjectedLine<'a> {
    /// Line vertices in screen px (label plane).
    pub line: &'a [P],
    pub anchor: P,
    pub segment: usize,
}

#[derive(Default)]
pub struct ProjectionCache {
    offsets: HashMap<usize, P>,
}

#[derive(Clone, Debug)]
pub struct PlacedGlyph {
    pub point: P,
    pub angle: f64,
    pub path: Vec<P>,
}

fn unit(p: P) -> P {
    let m = (p.0 * p.0 + p.1 * p.1).sqrt();
    (p.0 / m, p.1 / m)
}

fn perp(p: P) -> P {
    (-p.1, p.0)
}

fn offset_normal(seg: P, offset: f64, direction: f64) -> P {
    let n = perp(unit(seg));
    (n.0 * offset * direction, n.1 * offset * direction)
}

fn line_intersection(a1: P, a2: P, b1: P, b2: P) -> Option<P> {
    let (ady, adx) = (a2.1 - a1.1, a2.0 - a1.0);
    let (bdy, bdx) = (b2.1 - b1.1, b2.0 - b1.0);
    let den = bdy * adx - bdx * ady;
    if den == 0.0 {
        return None;
    }
    let (ody, odx) = (a1.1 - b1.1, a1.0 - b1.0);
    let t = (bdx * ody - bdy * odx) / den;
    Some((a1.0 + t * adx, a1.1 + t * ady))
}

#[allow(clippy::too_many_arguments)]
fn find_offset_intersection(
    pl: &ProjectedLine,
    cache: &mut ProjectionCache,
    index: usize,
    prev_to_current_normal: P,
    current: P,
    offset_prev: P,
    line_offset_y: f64,
    direction: i64,
) -> P {
    if let Some(&p) = cache.offsets.get(&index) {
        return p;
    }
    let offset_current = (current.0 + prev_to_current_normal.0, current.1 + prev_to_current_normal.1);
    let next_index = index as i64 + direction;
    if next_index < 0 || next_index >= pl.line.len() as i64 {
        cache.offsets.insert(index, offset_current);
        return offset_current;
    }
    let next = pl.line[next_index as usize];
    let n = offset_normal((next.0 - current.0, next.1 - current.1), line_offset_y, direction as f64);
    let b1 = (current.0 + n.0, current.1 + n.1);
    let b2 = (next.0 + n.0, next.1 + n.1);
    let p = line_intersection(offset_prev, offset_current, b1, b2).unwrap_or(offset_current);
    cache.offsets.insert(index, p);
    p
}

/// placeGlyphAlongLine.
#[allow(clippy::too_many_arguments)]
pub fn place_glyph_along_line(
    pl: &ProjectedLine,
    cache: &mut ProjectionCache,
    offset_x: f64,
    line_offset_x: f64,
    line_offset_y: f64,
    flip: bool,
    rotate_to_line: bool,
) -> Option<PlacedGlyph> {
    let combined = if flip { offset_x - line_offset_x } else { offset_x + line_offset_x };
    let mut direction: i64 = if combined > 0.0 { 1 } else { -1 };
    let mut angle = 0.0;
    if flip {
        direction *= -1;
        angle = std::f64::consts::PI;
    }
    if direction < 0 {
        angle += std::f64::consts::PI;
    }
    let mut current_index: i64 = if direction > 0 { pl.segment as i64 } else { pl.segment as i64 + 1 };
    let anchor = pl.anchor;
    let mut current = anchor;
    let mut previous;
    let mut offset_intersection: Option<P> = None;
    let mut offset_prev: Option<P> = None;
    let mut distance_from_anchor = 0.0;
    let mut current_segment_distance = 0.0;
    let abs_offset = combined.abs();
    let mut path: Vec<P> = Vec::new();
    let mut current_segment: P = (0.0, 0.0);
    previous = anchor;

    while distance_from_anchor + current_segment_distance <= abs_offset {
        current_index += direction;
        if current_index < 0 || current_index >= pl.line.len() as i64 {
            return None;
        }
        distance_from_anchor += current_segment_distance;
        previous = current;
        offset_prev = offset_intersection;
        current = pl.line[current_index as usize];
        if line_offset_y == 0.0 {
            path.push(previous);
            current_segment = (current.0 - previous.0, current.1 - previous.1);
        } else {
            let prev_to_current = (current.0 - previous.0, current.1 - previous.1);
            let normal = if prev_to_current.0 == 0.0 && prev_to_current.1 == 0.0 {
                let ni = current_index + direction;
                if ni < 0 || ni >= pl.line.len() as i64 {
                    return None;
                }
                let next = pl.line[ni as usize];
                offset_normal((next.0 - current.0, next.1 - current.1), line_offset_y, direction as f64)
            } else {
                offset_normal(prev_to_current, line_offset_y, direction as f64)
            };
            let op = offset_prev.unwrap_or((previous.0 + normal.0, previous.1 + normal.1));
            offset_prev = Some(op);
            let oi = find_offset_intersection(pl, cache, current_index as usize, normal, current, op, line_offset_y, direction);
            offset_intersection = Some(oi);
            path.push(op);
            current_segment = (oi.0 - op.0, oi.1 - op.1);
        }
        current_segment_distance = (current_segment.0.powi(2) + current_segment.1.powi(2)).sqrt();
    }
    let t = (abs_offset - distance_from_anchor) / current_segment_distance;
    let base = offset_prev.unwrap_or(previous);
    let p = (current_segment.0 * t + base.0, current_segment.1 * t + base.1);
    let segment_angle = angle + (current.1 - previous.1).atan2(current.0 - previous.0);
    path.push(p);
    Some(PlacedGlyph { point: p, angle: if rotate_to_line { segment_angle } else { 0.0 }, path })
}

/// Result of placeGlyphsAlongLine for drawing.
pub enum GlyphsPlacement {
    Hidden,
    Placed(Vec<PlacedGlyph>),
}

/// updateLineLabels for one symbol: place all glyphs (glyph offsets are in
/// glyph units; `font_scale` = size / 24), flipping for keep-upright.
pub fn place_glyphs_for_render(
    pl: &ProjectedLine,
    glyph_offsets: &[f64],
    font_scale: f64,
    line_offset: (f64, f64),
    keep_upright: bool,
    rotate_to_line: bool,
) -> GlyphsPlacement {
    let mut cache = ProjectionCache::default();
    match place_glyphs(pl, &mut cache, glyph_offsets, font_scale, line_offset, false, keep_upright, rotate_to_line) {
        Inner::NotEnoughRoom => GlyphsPlacement::Hidden,
        Inner::NeedsFlipping => {
            match place_glyphs(pl, &mut cache, glyph_offsets, font_scale, line_offset, true, keep_upright, rotate_to_line) {
                Inner::Placed(g) => GlyphsPlacement::Placed(g),
                _ => GlyphsPlacement::Hidden,
            }
        }
        Inner::Placed(g) => GlyphsPlacement::Placed(g),
    }
}

enum Inner {
    NotEnoughRoom,
    NeedsFlipping,
    Placed(Vec<PlacedGlyph>),
}

#[allow(clippy::too_many_arguments)]
fn place_glyphs(
    pl: &ProjectedLine,
    cache: &mut ProjectionCache,
    offsets: &[f64],
    font_scale: f64,
    line_offset: (f64, f64),
    flip: bool,
    keep_upright: bool,
    rotate_to_line: bool,
) -> Inner {
    let (lox, loy) = (line_offset.0 * font_scale, line_offset.1 * font_scale);
    if offsets.len() > 1 {
        let Some(first) = place_glyph_along_line(pl, cache, font_scale * offsets[0], lox, loy, flip, rotate_to_line) else {
            return Inner::NotEnoughRoom;
        };
        let Some(last) =
            place_glyph_along_line(pl, cache, font_scale * offsets[offsets.len() - 1], lox, loy, flip, rotate_to_line)
        else {
            return Inner::NotEnoughRoom;
        };
        // requiresOrientationChange for horizontal-only writing mode.
        if keep_upright && !flip && first.point.0 > last.point.0 {
            return Inner::NeedsFlipping;
        }
        let mut placed = vec![first];
        for &o in &offsets[1..offsets.len() - 1] {
            match place_glyph_along_line(pl, cache, font_scale * o, lox, loy, flip, rotate_to_line) {
                Some(g) => placed.push(g),
                None => return Inner::NotEnoughRoom,
            }
        }
        placed.push(last);
        Inner::Placed(placed)
    } else {
        if keep_upright && !flip {
            let a = pl.anchor;
            let b = pl.line[(pl.segment + 1).min(pl.line.len() - 1)];
            if a.0 > b.0 {
                return Inner::NeedsFlipping;
            }
        }
        match place_glyph_along_line(pl, cache, font_scale * offsets.first().copied().unwrap_or(0.0), lox, loy, flip, rotate_to_line)
        {
            Some(g) => Inner::Placed(vec![g]),
            None => Inner::NotEnoughRoom,
        }
    }
}

/// The projected path placeCollisionCircles builds from the first and last
/// glyph (flip = false, rotateToLine = false), or None if they don't fit.
pub fn collision_path(pl: &ProjectedLine, glyph_offsets: &[f64], font_scale: f64, line_offset: (f64, f64)) -> Option<Vec<P>> {
    let mut cache = ProjectionCache::default();
    let (lox, loy) = (line_offset.0 * font_scale, line_offset.1 * font_scale);
    let first = place_glyph_along_line(pl, &mut cache, font_scale * glyph_offsets[0], lox, loy, false, false)?;
    let last = place_glyph_along_line(pl, &mut cache, font_scale * glyph_offsets[glyph_offsets.len() - 1], lox, loy, false, false)?;
    let mut path = Vec::new();
    for i in (1..first.path.len()).rev() {
        path.push(first.path[i]);
    }
    for p in last.path.iter().skip(1) {
        path.push(*p);
    }
    Some(path)
}

/// PathInterpolator.
pub struct PathInterpolator {
    points: Vec<P>,
    distances: Vec<f64>,
    pub length: f64,
    pub padded_length: f64,
    padding: f64,
}

impl PathInterpolator {
    pub fn new(points: Vec<P>, padding: f64) -> Self {
        let mut distances = vec![0.0];
        for i in 1..points.len() {
            distances.push(distances[i - 1] + dist(points[i], points[i - 1]));
        }
        let length = *distances.last().unwrap();
        let padding = padding.min(length * 0.5);
        PathInterpolator { points, distances, length, padded_length: length - padding * 2.0, padding }
    }

    pub fn lerp(&self, t: f64) -> P {
        if self.points.len() == 1 {
            return self.points[0];
        }
        let t = t.clamp(0.0, 1.0);
        let mut idx = 1;
        let mut d = self.distances[idx];
        let target = t * self.padded_length + self.padding;
        while d < target && idx < self.distances.len() {
            idx += 1;
            d = self.distances.get(idx).copied().unwrap_or(f64::NAN);
        }
        let prev = idx - 1;
        let dp = self.distances[prev];
        let seg = d - dp;
        let st = if seg > 0.0 { (target - dp) / seg } else { 0.0 };
        let cur = self.points.get(idx).copied().unwrap_or(self.points[prev]);
        (self.points[prev].0 * (1.0 - st) + cur.0 * st, self.points[prev].1 * (1.0 - st) + cur.1 * st)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_line_splits_at_bounds() {
        let l = vec![vec![(-10.0, 5.0), (20.0, 5.0)]];
        let c = clip_line(&l, 0.0, 0.0, 10.0, 10.0);
        assert_eq!(c, vec![vec![(0.0, 5.0), (10.0, 5.0)]]);
    }

    #[test]
    fn anchors_on_long_straight_line() {
        let line = vec![(100.0, 4000.0), (8000.0, 4000.0)];
        let a = get_anchors(&line, 4000.0, 45f64.to_radians(), true, 50.0, 24.0, 16.0, 1.0, 8192.0);
        assert!(!a.is_empty());
        assert!(a.iter().all(|x| x.y == 4000.0));
    }

    #[test]
    fn glyph_along_straight_line() {
        let line = vec![(0.0, 0.0), (100.0, 0.0)];
        let pl = ProjectedLine { line: &line, anchor: (50.0, 0.0), segment: 0 };
        let mut c = ProjectionCache::default();
        let g = place_glyph_along_line(&pl, &mut c, 10.0, 0.0, 0.0, false, true).unwrap();
        assert!((g.point.0 - 60.0).abs() < 1e-9 && g.angle.abs() < 1e-12);
    }
}
