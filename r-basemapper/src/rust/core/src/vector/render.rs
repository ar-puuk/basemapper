//! Rasterise decoded vector tiles with a MapLibre style using tiny-skia.
//!
//! The view is the EPSG:3857 bbox mapped onto a `width × height` canvas at
//! MapLibre's scale zoom (512 px tiles). Layers are drawn in style order; within
//! a layer, each tile's features are hard-clipped to that tile's square (as
//! MapLibre's stencil test does), so geometry repeated in neighbouring tiles'
//! buffers is never drawn twice.

use super::expr::{eval_filter, eval_property, is_data_driven, Color, Ctx, Kind, Val};
use super::mvt::{self, Feature, Geometry, Point, Tile};
use crate::tile_fetcher::TileCoord;
use serde_json::Value as Json;
use tiny_skia::{
    FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, StrokeDash,
    Transform,
};

const EARTH_HALF: f64 = 20_037_508.342_789_244;
const EARTH_CIRC: f64 = 2.0 * EARTH_HALF;
/// MapLibre vector tiles are laid out on a 512 px grid.
pub const TILE_PX: f64 = 512.0;
/// Supersampling factor. MapLibre computes exact per-pixel coverage in its
/// shaders; tiny-skia's 4x4 AA quantises edges to 1/4 px. Rendering at SS x
/// and box-filtering down approximates exact area coverage.
const SS: u32 = 2;

/// MapLibre's scale zoom for a bbox drawn `width` pixels wide.
pub fn scale_zoom(bbox: [f64; 4], width: u32) -> f64 {
    let mpp = (bbox[2] - bbox[0]) / width as f64;
    let z = (EARTH_CIRC / (TILE_PX * mpp)).log2();
    // MapLibre picks tiles by floor(zoom); don't let float noise from a
    // bbox computed for an integer zoom (12.99994…) drop a whole level.
    if (z - z.round()).abs() < 1e-3 {
        z.round()
    } else {
        z
    }
}

/// Tile zoom MapLibre would request for a vector source at this view.
pub fn tile_zoom(scale_zoom: f64, source_maxzoom: Option<u8>) -> u8 {
    let z = scale_zoom.floor().clamp(0.0, 22.0) as u8;
    source_maxzoom.map_or(z, |m| z.min(m))
}

/// A decoded tile placed on the canvas.
struct PlacedTile {
    tile: Tile,
    /// Canvas-pixel origin of the tile's top-left corner.
    ox: f64,
    oy: f64,
    /// Canvas pixels per tile unit, for a given layer extent (set per layer).
    tile_px: f64,
    clip: Option<Mask>,
}

pub struct RenderOutput {
    pub pixels: Vec<u8>,
    /// Layer types present in the style that this renderer does not draw yet.
    pub skipped_layer_types: Vec<String>,
}

/// Render `tiles` (raw MVT bytes) with `style` over `bbox` at `width × height`.
pub fn render(
    style: &Json,
    tiles: &[(TileCoord, Vec<u8>)],
    bbox: [f64; 4],
    width: u32,
    height: u32,
) -> Result<RenderOutput, String> {
    let zoom = scale_zoom(bbox, width);
    let world_px = TILE_PX * 2f64.powf(zoom);
    let view_x0 = (bbox[0] + EARTH_HALF) / EARTH_CIRC * world_px;
    let view_y0 = (EARTH_HALF - bbox[3]) / EARTH_CIRC * world_px;

    let ss = SS as f64;
    let (cw, ch) = (width * SS, height * SS);
    let mut pixmap = Pixmap::new(cw, ch).ok_or("invalid canvas size")?;

    let mut placed = Vec::with_capacity(tiles.len());
    for (c, bytes) in tiles {
        let tile = match mvt::decode(bytes) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("skipping undecodable tile {}/{}/{}: {e}", c.z, c.x, c.y);
                continue;
            }
        };
        let tile_px = world_px / 2f64.powi(c.z as i32) * ss;
        let ox = (c.x as f64 * world_px / 2f64.powi(c.z as i32) - view_x0) * ss;
        let oy = (c.y as f64 * world_px / 2f64.powi(c.z as i32) - view_y0) * ss;
        // Hard (non-AA) clip like a stencil test.
        let clip = Rect::from_xywh(ox as f32, oy as f32, tile_px as f32, tile_px as f32)
            .map(PathBuilder::from_rect)
            .and_then(|p| {
                let mut m = Mask::new(cw, ch)?;
                m.fill_path(&p, FillRule::Winding, false, Transform::identity());
                Some(m)
            });
        placed.push(PlacedTile { tile, ox, oy, tile_px, clip });
    }

    let mut skipped: Vec<String> = Vec::new();
    for layer in style["layers"].as_array().map(|v| v.as_slice()).unwrap_or(&[]) {
        let ltype = layer["type"].as_str().unwrap_or("");
        if layer["layout"]["visibility"] == "none" {
            continue;
        }
        if layer["minzoom"].as_f64().map_or(false, |mz| zoom < mz)
            || layer["maxzoom"].as_f64().map_or(false, |mz| zoom >= mz)
        {
            continue;
        }
        match ltype {
            "background" => draw_background(&mut pixmap, layer, zoom),
            "fill" | "line" | "circle" => {
                let Some(source_layer) = layer["source-layer"].as_str() else { continue };
                for pt in &placed {
                    let Some(l) = pt.tile.layers.get(source_layer) else { continue };
                    let scale = pt.tile_px / l.extent as f64;
                    let ctx_tile = TileXform { ox: pt.ox, oy: pt.oy, scale };
                    let lp = LayerPaint::new(layer, ltype, zoom, SS as f32);
                    for f in &l.features {
                        let ctx = Ctx { zoom, feature: Some(f) };
                        if let Some(filter) = layer.get("filter") {
                            if !eval_filter(filter, &ctx) {
                                continue;
                            }
                        }
                        match ltype {
                            "fill" => draw_fill(&mut pixmap, &lp, f, &ctx, &ctx_tile, pt.clip.as_ref()),
                            "line" => draw_line(&mut pixmap, &lp, f, &ctx, &ctx_tile, pt.clip.as_ref()),
                            _ => draw_circle(&mut pixmap, &lp, f, &ctx, &ctx_tile, pt.clip.as_ref()),
                        }
                    }
                }
            }
            other => {
                if !skipped.iter().any(|s| s == other) {
                    skipped.push(other.to_owned());
                }
            }
        }
    }

    // Box-filter the supersampled canvas (premultiplied), then convert to
    // straight alpha, which is what MapLibre's canvas readback produces.
    let src = pixmap.data();
    let n = (SS * SS) as u32;
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let mut acc = [0u32; 4];
            for sy in 0..SS {
                let row = ((y * SS + sy) * cw + x * SS) as usize * 4;
                for sx in 0..SS as usize {
                    for (i, a) in acc.iter_mut().enumerate() {
                        *a += src[row + sx * 4 + i] as u32;
                    }
                }
            }
            let a = (acc[3] + n / 2) / n;
            let mut px = [0u8; 4];
            px[3] = a as u8;
            if a > 0 {
                for i in 0..3 {
                    // straight = premultiplied_sum / alpha_sum
                    px[i] = ((acc[i] * 255 + acc[3] / 2) / acc[3]).min(255) as u8;
                }
            }
            out.extend_from_slice(&px);
        }
    }
    Ok(RenderOutput { pixels: out, skipped_layer_types: skipped })
}

struct TileXform {
    ox: f64,
    oy: f64,
    scale: f64,
}

impl TileXform {
    fn apply(&self, p: Point) -> (f32, f32) {
        ((self.ox + p.0 as f64 * self.scale) as f32, (self.oy + p.1 as f64 * self.scale) as f32)
    }
}

// ── Paint properties ─────────────────────────────────────────────────────────

/// A paint/layout property, hoisted to a constant when it doesn't depend on
/// feature data.
enum Prop<'a> {
    Const(Val),
    Data(&'a Json, Kind),
    Absent,
}

impl<'a> Prop<'a> {
    fn new(j: Option<&'a Json>, kind: Kind, zoom: f64) -> Self {
        match j {
            None | Some(Json::Null) => Prop::Absent,
            Some(j) if is_data_driven(j) => Prop::Data(j, kind),
            Some(j) => Prop::Const(eval_property(j, kind, &Ctx { zoom, feature: None })),
        }
    }

    fn get(&self, ctx: &Ctx) -> Option<Val> {
        match self {
            Prop::Const(v) => Some(v.clone()),
            Prop::Data(j, k) => Some(eval_property(j, *k, ctx)),
            Prop::Absent => None,
        }
    }

    fn num(&self, ctx: &Ctx, default: f64) -> f64 {
        self.get(ctx).and_then(|v| v.as_num()).unwrap_or(default)
    }

    fn color(&self, ctx: &Ctx, default: Color) -> Color {
        self.get(ctx).and_then(|v| v.as_color()).unwrap_or(default)
    }

    fn is_set(&self) -> bool {
        !matches!(self, Prop::Absent)
    }
}

struct LayerPaint<'a> {
    color: Prop<'a>,
    opacity: Prop<'a>,
    outline_color: Prop<'a>,
    translate: Prop<'a>,
    width: Prop<'a>,
    gap_width: Prop<'a>,
    offset: Prop<'a>,
    dasharray: Prop<'a>,
    cap: LineCap,
    join: LineJoin,
    miter_limit: f32,
    radius: Prop<'a>,
    stroke_width: Prop<'a>,
    stroke_color: Prop<'a>,
    stroke_opacity: Prop<'a>,
    antialias: bool,
    /// Canvas pixels per CSS pixel (supersampling factor).
    scale: f32,
}

impl<'a> LayerPaint<'a> {
    fn new(layer: &'a Json, ltype: &str, zoom: f64, scale: f32) -> Self {
        let p = &layer["paint"];
        let l = &layer["layout"];
        let g = |k: &str| p.get(format!("{ltype}-{k}"));
        let cap = match l["line-cap"].as_str() {
            Some("round") => LineCap::Round,
            Some("square") => LineCap::Square,
            _ => LineCap::Butt,
        };
        let join = match l["line-join"].as_str() {
            Some("round") => LineJoin::Round,
            Some("bevel") => LineJoin::Bevel,
            _ => LineJoin::Miter,
        };
        LayerPaint {
            color: Prop::new(g("color"), Kind::Color, zoom),
            opacity: Prop::new(g("opacity"), Kind::Number, zoom),
            outline_color: Prop::new(p.get("fill-outline-color"), Kind::Color, zoom),
            translate: Prop::new(g("translate"), Kind::Array, zoom),
            width: Prop::new(g("width"), Kind::Number, zoom),
            gap_width: Prop::new(p.get("line-gap-width"), Kind::Number, zoom),
            offset: Prop::new(p.get("line-offset"), Kind::Number, zoom),
            dasharray: Prop::new(p.get("line-dasharray"), Kind::Array, zoom),
            cap,
            join,
            miter_limit: l["line-miter-limit"].as_f64().unwrap_or(2.0) as f32,
            radius: Prop::new(p.get("circle-radius"), Kind::Number, zoom),
            stroke_width: Prop::new(p.get("circle-stroke-width"), Kind::Number, zoom),
            stroke_color: Prop::new(p.get("circle-stroke-color"), Kind::Color, zoom),
            stroke_opacity: Prop::new(p.get("circle-stroke-opacity"), Kind::Number, zoom),
            antialias: p["fill-antialias"].as_bool().unwrap_or(true),
            scale,
        }
    }
}

fn paint_for(c: Color, aa: bool) -> Option<Paint<'static>> {
    if c.a <= 0.0 {
        return None;
    }
    let a = c.a.clamp(0.0, 1.0);
    let un = |v: f32| (v / a).clamp(0.0, 1.0);
    let mut paint = Paint::default();
    paint.set_color(tiny_skia::Color::from_rgba(un(c.r), un(c.g), un(c.b), a)?);
    paint.anti_alias = aa;
    Some(paint)
}

fn translate_of(lp: &LayerPaint, ctx: &Ctx) -> Transform {
    match lp.translate.get(ctx).and_then(|v| v.as_num_array()) {
        Some(t) if t.len() == 2 => {
            Transform::from_translate(t[0] as f32 * lp.scale, t[1] as f32 * lp.scale)
        }
        _ => Transform::identity(),
    }
}

// ── Layer drawing ────────────────────────────────────────────────────────────

fn draw_background(pixmap: &mut Pixmap, layer: &Json, zoom: f64) {
    let ctx = Ctx { zoom, feature: None };
    let p = &layer["paint"];
    let color = p
        .get("background-color")
        .map(|j| eval_property(j, Kind::Color, &ctx))
        .and_then(|v| v.as_color())
        .unwrap_or(Color::BLACK);
    let opacity = p
        .get("background-opacity")
        .map(|j| eval_property(j, Kind::Number, &ctx))
        .and_then(|v| v.as_num())
        .unwrap_or(1.0);
    if let Some(paint) = paint_for(color.with_opacity(opacity as f32), false) {
        let r = Rect::from_xywh(0.0, 0.0, pixmap.width() as f32, pixmap.height() as f32).unwrap();
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
    }
}

fn ring_path(pb: &mut PathBuilder, ring: &[Point], xf: &TileXform, close: bool) {
    let mut it = ring.iter();
    if let Some(&p0) = it.next() {
        let (x, y) = xf.apply(p0);
        pb.move_to(x, y);
        for &p in it {
            let (x, y) = xf.apply(p);
            pb.line_to(x, y);
        }
        if close {
            pb.close();
        }
    }
}

fn draw_fill(
    pixmap: &mut Pixmap,
    lp: &LayerPaint,
    f: &Feature,
    ctx: &Ctx,
    xf: &TileXform,
    clip: Option<&Mask>,
) {
    let Geometry::Polygons(polys) = &f.geometry else { return };
    let opacity = lp.opacity.num(ctx, 1.0) as f32;
    let color = lp.color.color(ctx, Color::BLACK).with_opacity(opacity);
    let mut pb = PathBuilder::new();
    for poly in polys {
        for ring in poly {
            ring_path(&mut pb, ring, xf, true);
        }
    }
    let Some(path) = pb.finish() else { return };
    let t = translate_of(lp, ctx);
    if let Some(paint) = paint_for(color, lp.antialias) {
        pixmap.fill_path(&path, &paint, FillRule::Winding, t, clip);
    }
    // MapLibre draws a 1 px outline when fill-outline-color is set.
    if lp.antialias && lp.outline_color.is_set() {
        let oc = lp.outline_color.color(ctx, Color::BLACK).with_opacity(opacity);
        if let Some(paint) = paint_for(oc, true) {
            let stroke = Stroke { width: lp.scale, ..Stroke::default() };
            pixmap.stroke_path(&path, &paint, &stroke, t, clip);
        }
    }
}

/// Offset a polyline perpendicular to its direction; positive is to the right
/// in screen space (y down), using mitred vertex normals like MapLibre's
/// line extrusion.
fn offset_line(pts: &[(f32, f32)], d: f32) -> Vec<(f32, f32)> {
    if pts.len() < 2 || d == 0.0 {
        return pts.to_vec();
    }
    let normal = |a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            (0.0, 0.0)
        } else {
            (-dy / len, dx / len)
        }
    };
    let n = pts.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let prev = if i > 0 { normal(pts[i - 1], pts[i]) } else { normal(pts[0], pts[1]) };
        let next = if i + 1 < n { normal(pts[i], pts[i + 1]) } else { prev };
        let (mut mx, mut my) = (prev.0 + next.0, prev.1 + next.1);
        let mlen = (mx * mx + my * my).sqrt();
        if mlen < 1e-6 {
            mx = next.0;
            my = next.1;
        } else {
            mx /= mlen;
            my /= mlen;
        }
        // Scale so the offset is `d` along both segment normals.
        let cos = mx * next.0 + my * next.1;
        let k = if cos.abs() > 0.1 { d / cos } else { d };
        out.push((pts[i].0 + mx * k, pts[i].1 + my * k));
    }
    out
}

fn line_path(lines: &[Vec<Point>], xf: &TileXform, offset: f32, closed_rings: bool) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for line in lines {
        let pts: Vec<(f32, f32)> = line.iter().map(|&p| xf.apply(p)).collect();
        let pts = offset_line(&pts, offset);
        let mut it = pts.iter();
        if let Some(&(x, y)) = it.next() {
            pb.move_to(x, y);
            for &(x, y) in it {
                pb.line_to(x, y);
            }
            if closed_rings && line.len() > 2 && line.first() == line.last() {
                pb.close();
            }
        }
    }
    pb.finish()
}

fn draw_line(
    pixmap: &mut Pixmap,
    lp: &LayerPaint,
    f: &Feature,
    ctx: &Ctx,
    xf: &TileXform,
    clip: Option<&Mask>,
) {
    // Polygon outlines can be styled as lines too.
    let (lines, closed): (Vec<Vec<Point>>, bool) = match &f.geometry {
        Geometry::Lines(l) => (l.clone(), false),
        Geometry::Polygons(p) => (p.iter().flatten().cloned().collect(), true),
        Geometry::Points(_) => return,
    };
    let opacity = lp.opacity.num(ctx, 1.0) as f32;
    let color = lp.color.color(ctx, Color::BLACK).with_opacity(opacity);
    let Some(paint) = paint_for(color, true) else { return };
    let width = lp.width.num(ctx, 1.0) as f32 * lp.scale;
    if width <= 0.0 {
        return;
    }
    let gap = lp.gap_width.num(ctx, 0.0) as f32 * lp.scale;
    let offset = lp.offset.num(ctx, 0.0) as f32 * lp.scale;
    let dash = lp
        .dasharray
        .get(ctx)
        .and_then(|v| v.as_num_array())
        .filter(|d| d.len() >= 2 && d.iter().any(|&x| x > 0.0));
    let mut stroke = Stroke {
        width,
        line_cap: lp.cap,
        line_join: lp.join,
        miter_limit: lp.miter_limit,
        dash: None,
    };
    if let Some(d) = dash {
        // MapLibre dash lengths are multiples of the line width.
        let mut v: Vec<f32> = d.iter().map(|&x| (x as f32 * width).max(0.0)).collect();
        if v.len() % 2 == 1 {
            let copy = v.clone();
            v.extend(copy);
        }
        stroke.dash = StrokeDash::new(v, 0.0);
    }
    let t = translate_of(lp, ctx);
    // line-gap-width draws two lines (a casing) either side of the gap.
    let offsets: Vec<f32> = if gap > 0.0 {
        let side = gap / 2.0 + width / 2.0;
        vec![offset - side, offset + side]
    } else {
        vec![offset]
    };
    for off in offsets {
        if let Some(path) = line_path(&lines, xf, off, closed) {
            pixmap.stroke_path(&path, &paint, &stroke, t, clip);
        }
    }
}

fn draw_circle(
    pixmap: &mut Pixmap,
    lp: &LayerPaint,
    f: &Feature,
    ctx: &Ctx,
    xf: &TileXform,
    clip: Option<&Mask>,
) {
    let Geometry::Points(pts) = &f.geometry else { return };
    let radius = lp.radius.num(ctx, 5.0) as f32 * lp.scale;
    let opacity = lp.opacity.num(ctx, 1.0) as f32;
    let color = lp.color.color(ctx, Color::BLACK).with_opacity(opacity);
    let sw = lp.stroke_width.num(ctx, 0.0) as f32 * lp.scale;
    let sop = lp.stroke_opacity.num(ctx, 1.0) as f32;
    let sc = lp.stroke_color.color(ctx, Color::BLACK).with_opacity(sop);
    let t = translate_of(lp, ctx);
    for &p in pts {
        let (x, y) = xf.apply(p);
        let Some(path) = PathBuilder::from_circle(x, y, radius.max(0.01)) else { continue };
        if let Some(paint) = paint_for(color, true) {
            pixmap.fill_path(&path, &paint, FillRule::Winding, t, clip);
        }
        if sw > 0.0 {
            // MapLibre's circle stroke sits outside the radius.
            if let (Some(paint), Some(ring)) =
                (paint_for(sc, true), PathBuilder::from_circle(x, y, radius + sw / 2.0))
            {
                let stroke = Stroke { width: sw, ..Stroke::default() };
                pixmap.stroke_path(&ring, &paint, &stroke, t, clip);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_zoom_matches_maplibre_convention() {
        // 800 px over the fixture bbox is MapLibre zoom 13 (GL JS reports 12.99994).
        let z = scale_zoom([-12459252.0, 4963454.0, -12451608.0, 4969187.0], 800);
        assert_eq!(z, 13.0);
        assert_eq!(tile_zoom(13.7, None), 13);
        assert_eq!(tile_zoom(13.7, Some(12)), 12);
    }

    #[test]
    fn offset_is_to_the_right_in_screen_space() {
        // Heading east (+x) in y-down screen space, "right" is +y.
        let out = offset_line(&[(0.0, 0.0), (10.0, 0.0)], 2.0);
        assert!((out[0].1 - 2.0).abs() < 1e-6 && (out[1].1 - 2.0).abs() < 1e-6);
    }
}
