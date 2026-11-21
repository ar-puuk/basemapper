//! Render decoded vector tiles with a MapLibre style, modelling MapLibre GL
//! JS's WebGL pipeline in software (see `gl.rs`).
//!
//! The view is centred on the bbox centre at MapLibre's scale zoom (512 px
//! tiles), exactly as MapLibre's transform places it. Layers are drawn in style
//! order; fill and line fragments are clipped to their tile's square (MapLibre's
//! stencil test). Circles are not clipped; points outside the tile are dropped,
//! as MapLibre's CircleBucket does.

use super::expr::{eval_filter, eval_property, is_data_driven, Color, Ctx, Kind, Val};
use super::gl::{
    raster_gl_line, raster_polygon, raster_triangle, smoothstep, Cap, Clip, Dash, Framebuffer, Join,
    LineBuilder, Vert, ATLAS_W, EXTENT,
};
use super::mvt::{self, Feature, Geometry, Tile};
use super::symbol::{self, SymbolAssets, SymbolTile};
use crate::tile_fetcher::TileCoord;
use serde_json::Value as Json;

const EARTH_HALF: f64 = 20_037_508.342_789_244;
const EARTH_CIRC: f64 = 2.0 * EARTH_HALF;
/// MapLibre vector tiles are laid out on a 512 px grid.
pub const TILE_PX: f64 = 512.0;

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

struct PlacedTile {
    coord: TileCoord,
    tile: Tile,
    /// Canvas position of the tile's top-left corner, in px.
    ox: f64,
    oy: f64,
    /// Canvas px per 8192-extent tile unit.
    unit_px: f64,
    clip: Clip,
    overscaling: f64,
}

pub struct RenderOutput {
    pub pixels: Vec<u8>,
    /// Layer types present in the style that this renderer does not draw yet.
    pub skipped_layer_types: Vec<String>,
}

/// Render `tiles` (raw MVT bytes) with `style` over `bbox` at `width × height`.
/// `assets` holds glyphs and sprite images for symbol layers (may be empty).
pub fn render(
    style: &Json,
    tiles: &[(TileCoord, Vec<u8>)],
    assets: &SymbolAssets,
    bbox: [f64; 4],
    width: u32,
    height: u32,
) -> Result<RenderOutput, String> {
    let zoom = scale_zoom(bbox, width);
    let world_px = TILE_PX * 2f64.powf(zoom);
    // MapLibre's transform: centre point minus half the viewport.
    let cx = ((bbox[0] + bbox[2]) / 2.0 + EARTH_HALF) / EARTH_CIRC * world_px;
    let cy = (EARTH_HALF - (bbox[1] + bbox[3]) / 2.0) / EARTH_CIRC * world_px;
    let view_x0 = cx - width as f64 / 2.0;
    let view_y0 = cy - height as f64 / 2.0;

    let (w, h) = (width as usize, height as usize);
    let mut fb = Framebuffer::new(w, h);

    let mut placed = Vec::with_capacity(tiles.len());
    for (c, bytes) in tiles {
        let tile = match mvt::decode(bytes) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("skipping undecodable tile {}/{}/{}: {e}", c.z, c.x, c.y);
                continue;
            }
        };
        let tile_px = world_px / 2f64.powi(c.z as i32);
        let ox = c.x as f64 * tile_px - view_x0;
        let oy = c.y as f64 * tile_px - view_y0;
        placed.push(PlacedTile {
            coord: *c,
            tile,
            ox,
            oy,
            unit_px: tile_px / EXTENT,
            clip: Clip { x0: ox, y0: oy, x1: ox + tile_px, y1: oy + tile_px },
            overscaling: 2f64.powi((zoom.floor() as i32 - c.z as i32).max(0)),
        });
    }

    // Symbol placement runs once, top layer first, before drawing.
    let symbol_tiles: Vec<SymbolTile> = placed
        .iter()
        .map(|p| SymbolTile {
            coord: (p.coord.z, p.coord.x, p.coord.y),
            tile: &p.tile,
            ox: p.ox,
            oy: p.oy,
            unit_px: p.unit_px,
            overscaling: p.overscaling,
        })
        .collect();
    let placement = symbol::place(style, &symbol_tiles, assets, zoom, width as f64, height as f64);

    let mut skipped: Vec<String> = Vec::new();
    for (layer_idx, layer) in style["layers"].as_array().map(|v| v.as_slice()).unwrap_or(&[]).iter().enumerate() {
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
            "background" => draw_background(&mut fb, layer, zoom),
            "fill" => draw_fill_layer(&mut fb, layer, zoom, &placed),
            "line" => draw_line_layer(&mut fb, layer, zoom, &placed),
            "circle" => draw_circle_layer(&mut fb, layer, zoom, &placed),
            "symbol" => symbol::draw_layer(&mut fb, layer, layer_idx, &placement, assets),
            other => {
                if !skipped.iter().any(|s| s == other) {
                    skipped.push(other.to_owned());
                }
            }
        }
    }

    Ok(RenderOutput { pixels: fb.into_straight_rgba(), skipped_layer_types: skipped })
}

/// Features of `layer`'s source-layer in `pt` that pass the layer filter,
/// with the scale from the layer's extent to MapLibre's 8192 extent.
fn features<'a>(layer: &'a Json, pt: &'a PlacedTile, zoom: f64) -> impl Iterator<Item = (&'a Feature, f64)> + 'a {
    let src = layer["source-layer"].as_str().and_then(|sl| pt.tile.layers.get(sl));
    let scale = src.map_or(1.0, |l| EXTENT / l.extent as f64);
    let feats = src.map(|l| l.features.as_slice()).unwrap_or(&[]);
    feats.iter().filter_map(move |f| {
        let ctx = Ctx { zoom, feature: Some(f) };
        match layer.get("filter") {
            Some(filter) if !eval_filter(filter, &ctx) => None,
            _ => Some((f, scale)),
        }
    })
}

/// MapLibre's `loadGeometry`: scale to the 8192 extent and round.
#[inline]
fn to_extent(p: (f32, f32), scale: f64) -> (f64, f64) {
    ((p.0 as f64 * scale + 0.5).floor(), (p.1 as f64 * scale + 0.5).floor())
}

// ── Paint properties ─────────────────────────────────────────────────────────

/// A paint property, hoisted to a constant when it doesn't depend on feature
/// data (MapLibre passes those as uniforms; data-driven ones as attributes).
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

    /// Data-driven colors are packed into 8-bit attributes (`packColor`
    /// floors each premultiplied channel); uniforms keep full precision.
    fn color(&self, ctx: &Ctx, default: Color) -> Color {
        let c = self.get(ctx).and_then(|v| v.as_color()).unwrap_or(default);
        if matches!(self, Prop::Data(..)) {
            let q = |v: f32| (v * 255.0).floor().clamp(0.0, 255.0) / 255.0;
            Color { r: q(c.r), g: q(c.g), b: q(c.b), a: q(c.a) }
        } else {
            c
        }
    }

    fn is_set(&self) -> bool {
        !matches!(self, Prop::Absent)
    }

    fn translate(&self, ctx: &Ctx) -> (f64, f64) {
        match self.get(ctx).and_then(|v| v.as_num_array()) {
            Some(t) if t.len() == 2 => (t[0], t[1]),
            _ => (0.0, 0.0),
        }
    }
}

#[inline]
fn rgba(c: Color, k: f64) -> [f32; 4] {
    let k = k as f32;
    [c.r * k, c.g * k, c.b * k, c.a * k]
}

#[inline]
fn scaled(c: [f32; 4], a: f64) -> [f32; 4] {
    let a = a as f32;
    [c[0] * a, c[1] * a, c[2] * a, c[3] * a]
}

// ── Background ───────────────────────────────────────────────────────────────

fn draw_background(fb: &mut Framebuffer, layer: &Json, zoom: f64) {
    let ctx = Ctx { zoom, feature: None };
    let p = &layer["paint"];
    let color = Prop::new(p.get("background-color"), Kind::Color, zoom).color(&ctx, Color::BLACK);
    let opacity = Prop::new(p.get("background-opacity"), Kind::Number, zoom).num(&ctx, 1.0);
    let c = rgba(color, opacity);
    if c[3] <= 0.0 {
        return;
    }
    for y in 0..fb.h {
        for x in 0..fb.w {
            fb.blend(x, y, c);
        }
    }
}

// ── Fill ─────────────────────────────────────────────────────────────────────

type Rings = Vec<Vec<(f64, f64)>>;

fn polys_px(pt: &PlacedTile, f: &Feature, scale: f64, t: (f64, f64)) -> Vec<Rings> {
    let Geometry::Polygons(polys) = &f.geometry else { return Vec::new() };
    polys
        .iter()
        .map(|poly| {
            poly.iter()
                .map(|ring| {
                    ring.iter()
                        .map(|&q| {
                            let (x, y) = to_extent(q, scale);
                            (pt.ox + x * pt.unit_px + t.0, pt.oy + y * pt.unit_px + t.1)
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}

fn draw_fill_layer(fb: &mut Framebuffer, layer: &Json, zoom: f64, placed: &[PlacedTile]) {
    let p = &layer["paint"];
    let color = Prop::new(p.get("fill-color"), Kind::Color, zoom);
    let opacity = Prop::new(p.get("fill-opacity"), Kind::Number, zoom);
    let outline = Prop::new(p.get("fill-outline-color"), Kind::Color, zoom);
    let translate = Prop::new(p.get("fill-translate"), Kind::Array, zoom);
    let antialias = p["fill-antialias"].as_bool().unwrap_or(true);
    if p.get("fill-pattern").is_some() {
        log::debug!("fill-pattern is not supported yet (layer {})", layer["id"]);
    }

    // draw_fill.ts: opaque layers go through the opaque pass and write depth,
    // so their default outline (sublayer 0) only lands outside the fill.
    let opaque = matches!(&color, Prop::Const(v) if v.as_color().map_or(false, |c| c.a == 1.0))
        && match &opacity {
            Prop::Absent => true,
            Prop::Const(v) => v.as_num() == Some(1.0),
            Prop::Data(..) => false,
        };
    let cull_outline = opaque && !outline.is_set();
    let (w, h) = (fb.w, fb.h);
    let mut covered = if cull_outline { vec![false; w * h] } else { Vec::new() };

    // Pass 1: interiors for all tiles (drawFillTiles, isOutline = false).
    for pt in placed {
        for (f, scale) in features(layer, pt, zoom) {
            let ctx = Ctx { zoom, feature: Some(f) };
            let c = rgba(color.color(&ctx, Color::BLACK), opacity.num(&ctx, 1.0));
            if c[3] <= 0.0 {
                continue;
            }
            for poly in polys_px(pt, f, scale, translate.translate(&ctx)) {
                raster_polygon(w, h, &poly, &pt.clip, |x, y| {
                    fb.blend(x, y, c);
                    if cull_outline {
                        covered[y * w + x] = true;
                    }
                });
            }
        }
    }

    // Pass 2: 1 px antialiasing outlines (gl.LINES + fill_outline shader).
    if !antialias {
        return;
    }
    for pt in placed {
        for (f, scale) in features(layer, pt, zoom) {
            let ctx = Ctx { zoom, feature: Some(f) };
            let oc = if outline.is_set() { outline.color(&ctx, Color::BLACK) } else { color.color(&ctx, Color::BLACK) };
            let base = rgba(oc, opacity.num(&ctx, 1.0));
            if base[3] <= 0.0 {
                continue;
            }
            for poly in polys_px(pt, f, scale, translate.translate(&ctx)) {
                for ring in &poly {
                    for seg in ring.windows(2) {
                        raster_gl_line(w, h, seg[0], seg[1], &pt.clip, |x, y, d| {
                            if cull_outline && covered[y * w + x] {
                                return;
                            }
                            fb.blend(x, y, scaled(base, 1.0 - smoothstep(0.0, 1.0, d)));
                        });
                    }
                }
            }
        }
    }
}

// ── Line ─────────────────────────────────────────────────────────────────────

fn draw_line_layer(fb: &mut Framebuffer, layer: &Json, zoom: f64, placed: &[PlacedTile]) {
    let p = &layer["paint"];
    let l = &layer["layout"];
    let color = Prop::new(p.get("line-color"), Kind::Color, zoom);
    let opacity = Prop::new(p.get("line-opacity"), Kind::Number, zoom);
    let width = Prop::new(p.get("line-width"), Kind::Number, zoom);
    let floor_width = Prop::new(p.get("line-width"), Kind::Number, zoom.floor());
    let gap = Prop::new(p.get("line-gap-width"), Kind::Number, zoom);
    let offset = Prop::new(p.get("line-offset"), Kind::Number, zoom);
    let blur = Prop::new(p.get("line-blur"), Kind::Number, zoom);
    let translate = Prop::new(p.get("line-translate"), Kind::Array, zoom);
    let dasharray = Prop::new(p.get("line-dasharray"), Kind::Array, zoom);
    let cap = match l["line-cap"].as_str() {
        Some("round") => Cap::Round,
        Some("square") => Cap::Square,
        _ => Cap::Butt,
    };
    let join = match l["line-join"].as_str() {
        Some("round") => Join::Round,
        Some("bevel") => Join::Bevel,
        _ => Join::Miter,
    };
    let miter_limit = l["line-miter-limit"].as_f64().unwrap_or(2.0);
    let round_limit = l["line-round-limit"].as_f64().unwrap_or(1.05);

    // line_sdf uniforms (lineSDFUniformValues): only pattern B shows
    // (u_mix = 1 with no fade); gamma uses the smaller cross-fade width.
    let dash = match &dasharray {
        Prop::Const(v) => v
            .as_num_array()
            .filter(|d| d.len() >= 2 && d.iter().sum::<f64>() > 0.0)
            .map(|d| Dash::new(&d, cap == Cap::Round)),
        _ => None,
    };
    let from_scale = if zoom > zoom.floor() { 2.0 } else { 0.5 };
    let tile_ratio = TILE_PX / EXTENT;

    let (w, h) = (fb.w, fb.h);
    for pt in placed {
        for (f, scale) in features(layer, pt, zoom) {
            let ctx = Ctx { zoom, feature: Some(f) };
            let fctx = Ctx { zoom: zoom.floor(), feature: Some(f) };
            let (lines, is_polygon): (Vec<&Vec<(f32, f32)>>, bool) = match &f.geometry {
                Geometry::Lines(ls) => (ls.iter().collect(), false),
                Geometry::Polygons(ps) => (ps.iter().flatten().collect(), true),
                Geometry::Points(_) => continue,
            };
            let c = rgba(color.color(&ctx, Color::BLACK), opacity.num(&ctx, 1.0));
            if c[3] <= 0.0 {
                continue;
            }
            let lw = width.num(&ctx, 1.0);
            // line.vertex.glsl: gapwidth / 2, halfwidth = width / 2, offset negated.
            let gapw = gap.num(&ctx, 0.0) / 2.0;
            let halfwidth = lw / 2.0;
            let line_offset = -offset.num(&ctx, 0.0);
            let aa = 0.5; // ANTIALIASING = 1 / devicePixelRatio / 2
            let inset = gapw + if gapw > 0.0 { aa } else { 0.0 };
            let outset =
                gapw + halfwidth * if gapw > 0.0 { 2.0 } else { 1.0 } + if halfwidth == 0.0 { 0.0 } else { aa };
            if outset <= 0.0 {
                continue;
            }
            let blur2 = blur.num(&ctx, 0.0) + 1.0;
            let floorwidth = floor_width.num(&fctx, lw);
            let (tx, ty) = translate.translate(&ctx);

            let dash_params = dash.as_ref().map(|d| {
                let width_b = d.width;
                let width_a = d.width * from_scale;
                let sdfgamma = ATLAS_W as f64 / (width_a.min(width_b) * 256.0) / 2.0;
                (d, tile_ratio / width_b / floorwidth, sdfgamma / floorwidth)
            });

            let mut b = LineBuilder::new(pt.overscaling);
            for line in &lines {
                let pts: Vec<(f64, f64)> = line.iter().map(|&q| to_extent(q, scale)).collect();
                b.add_line(&pts, is_polygon, join, cap, miter_limit, round_limit);
            }

            // line.vertex.glsl, per vertex.
            let verts: Vec<Vert<3>> = b
                .verts
                .iter()
                .map(|v| {
                    let ny = if v.up { 1.0 } else { -1.0 };
                    let u = 0.5 * v.dir;
                    let t = 1.0 - u.abs();
                    let (ox, oy) = (line_offset * v.ex * ny, line_offset * v.ey * ny);
                    let offset2 = (ox * t - oy * u, ox * u + oy * t);
                    let dist = (outset * v.ex, outset * v.ey);
                    Vert {
                        x: pt.ox + v.x * pt.unit_px + offset2.0 + dist.0 + tx,
                        y: pt.oy + v.y * pt.unit_px + offset2.1 + dist.1 + ty,
                        v: [if v.round { 1.0 } else { 0.0 }, ny, v.linesofar],
                    }
                })
                .collect();

            for tri in &b.tris {
                let t = [verts[tri[0] as usize], verts[tri[1] as usize], verts[tri[2] as usize]];
                raster_triangle(w, h, t, &pt.clip, |x, y, var| {
                    // line.fragment.glsl / line_sdf.fragment.glsl
                    let d = (var[0] * var[0] + var[1] * var[1]).sqrt() * outset;
                    let mut alpha = ((d - (inset - blur2)).min(outset - d) / blur2).clamp(0.0, 1.0);
                    if let Some((dash, sx, g)) = dash_params {
                        let sdf = dash.sample(var[2] * sx, dash.n as f64 * (1.0 - var[1]));
                        alpha *= smoothstep(0.5 - g, 0.5 + g, sdf);
                    }
                    if alpha > 0.0 {
                        fb.blend(x, y, scaled(c, alpha));
                    }
                });
            }
        }
    }
}

// ── Circle ───────────────────────────────────────────────────────────────────

fn draw_circle_layer(fb: &mut Framebuffer, layer: &Json, zoom: f64, placed: &[PlacedTile]) {
    let p = &layer["paint"];
    let radius = Prop::new(p.get("circle-radius"), Kind::Number, zoom);
    let color = Prop::new(p.get("circle-color"), Kind::Color, zoom);
    let opacity = Prop::new(p.get("circle-opacity"), Kind::Number, zoom);
    let blur = Prop::new(p.get("circle-blur"), Kind::Number, zoom);
    let stroke_width = Prop::new(p.get("circle-stroke-width"), Kind::Number, zoom);
    let stroke_color = Prop::new(p.get("circle-stroke-color"), Kind::Color, zoom);
    let stroke_opacity = Prop::new(p.get("circle-stroke-opacity"), Kind::Number, zoom);
    let translate = Prop::new(p.get("circle-translate"), Kind::Array, zoom);
    let (w, h) = (fb.w, fb.h);
    let full = Clip { x0: 0.0, y0: 0.0, x1: w as f64, y1: h as f64 };

    for pt in placed {
        for (f, scale) in features(layer, pt, zoom) {
            let Geometry::Points(points) = &f.geometry else { continue };
            let ctx = Ctx { zoom, feature: Some(f) };
            let r = radius.num(&ctx, 5.0);
            let sw = stroke_width.num(&ctx, 0.0);
            let fill = rgba(color.color(&ctx, Color::BLACK), opacity.num(&ctx, 1.0));
            let stroke = rgba(stroke_color.color(&ctx, Color::BLACK), stroke_opacity.num(&ctx, 1.0));
            // circle.vertex.glsl: antialiasblur = -max(1 / dpr / (r + sw), blur)
            let aablur = -(1.0 / (r + sw)).max(blur.num(&ctx, 0.0));
            let (tx, ty) = translate.translate(&ctx);
            let ext = r + sw;
            for &q in points {
                let (x, y) = to_extent(q, scale);
                // CircleBucket: points outside the tile are not drawn.
                if x < 0.0 || x >= EXTENT || y < 0.0 || y >= EXTENT {
                    continue;
                }
                let (cx, cy) = (pt.ox + x * pt.unit_px + tx, pt.oy + y * pt.unit_px + ty);
                let corner = |ex: f64, ey: f64| Vert { x: cx + ex * ext, y: cy + ey * ext, v: [ex, ey] };
                let quad = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
                for tri in [[0, 1, 2], [0, 2, 3]] {
                    raster_triangle(w, h, [quad[tri[0]], quad[tri[1]], quad[tri[2]]], &full, |px, py, v| {
                        // circle.fragment.glsl
                        let el = (v[0] * v[0] + v[1] * v[1]).sqrt();
                        let opacity_t = smoothstep(0.0, aablur, el - 1.0);
                        let color_t = if sw < 0.01 { 0.0 } else { smoothstep(aablur, 0.0, el - r / (r + sw)) };
                        if opacity_t > 0.0 {
                            let mut out = [0f32; 4];
                            for i in 0..4 {
                                out[i] = ((fill[i] as f64 * (1.0 - color_t) + stroke[i] as f64 * color_t)
                                    * opacity_t) as f32;
                            }
                            fb.blend(px, py, out);
                        }
                    });
                }
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
}
