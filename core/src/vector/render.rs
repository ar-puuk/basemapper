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
use super::svg;
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

/// Tiles covering `bbox` at tile zoom `z`, as (canonical tile, wrap). Columns
/// past ±180° belong to a wrapped world copy: wrap -1 is the copy to the
/// west, +1 to the east (MapLibre's renderWorldCopies).
pub fn covering_tiles(bbox: [f64; 4], z: u8) -> Vec<(TileCoord, i32)> {
    let n = 1i64 << z;
    let col = |x: f64| ((x + EARTH_HALF) / EARTH_CIRC * n as f64).floor() as i64;
    let row = |y: f64| (((EARTH_HALF - y) / EARTH_CIRC * n as f64).floor() as i64).clamp(0, n - 1);
    let (x0, x1) = (col(bbox[0]), col(bbox[2]));
    let (y0, y1) = (row(bbox[3]), row(bbox[1]));
    let mut out = Vec::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            let c = TileCoord { z, x: x.rem_euclid(n) as u32, y: y as u32 };
            out.push((c, x.div_euclid(n) as i32));
        }
    }
    out
}

/// Tile zoom MapLibre would request for a vector source at this view.
pub fn tile_zoom(scale_zoom: f64, source_maxzoom: Option<u8>) -> u8 {
    let z = scale_zoom.floor().clamp(0.0, 22.0) as u8;
    source_maxzoom.map_or(z, |m| z.min(m))
}

struct PlacedTile {
    coord: TileCoord,
    /// World copy: -1 west, 0 the primary world, +1 east.
    wrap: i32,
    tile: Tile,
    /// Canvas position of the tile's top-left corner, in px.
    ox: f64,
    oy: f64,
    /// Canvas px per 8192-extent tile unit.
    unit_px: f64,
    clip: Clip,
    overscaling: f64,
    /// Device pixels per CSS pixel; `ox`, `oy`, `unit_px` and `clip` are in
    /// device pixels (MapLibre's drawing buffer), placement uses CSS pixels.
    ratio: f64,
}

/// Device pixels for `css` CSS pixels at `ratio` (MapLibre's drawing buffer
/// is the canvas size times devicePixelRatio).
pub fn device_px(css: u32, ratio: f64) -> u32 {
    ((css as f64 * ratio).round() as u32).max(1)
}

pub struct RenderOutput {
    /// Device pixel size of `pixels`.
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Layer types present in the style that this renderer does not draw yet.
    pub skipped_layer_types: Vec<String>,
}

/// Decode `tiles` and place them on a `width x height` view of `bbox` at
/// MapLibre's scale zoom (centre point minus half the viewport). `width` and
/// `height` are CSS pixels; tiles are placed in device pixels (`ratio`).
fn prepare(
    tiles: &[(TileCoord, i32, Vec<u8>)],
    bbox: [f64; 4],
    width: u32,
    height: u32,
    ratio: f64,
) -> (f64, Vec<PlacedTile>) {
    let zoom = scale_zoom(bbox, width);
    let world_px = TILE_PX * 2f64.powf(zoom);
    let cx = ((bbox[0] + bbox[2]) / 2.0 + EARTH_HALF) / EARTH_CIRC * world_px;
    let cy = (EARTH_HALF - (bbox[1] + bbox[3]) / 2.0) / EARTH_CIRC * world_px;
    let view_x0 = cx - width as f64 / 2.0;
    let view_y0 = cy - height as f64 / 2.0;
    let mut placed = Vec::with_capacity(tiles.len());
    for (c, wrap, bytes) in tiles {
        let tile = match mvt::decode(bytes) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("skipping undecodable tile {}/{}/{}: {e}", c.z, c.x, c.y);
                continue;
            }
        };
        let tile_px = world_px / 2f64.powi(c.z as i32);
        let ox = (c.x as f64 + (*wrap as f64) * 2f64.powi(c.z as i32)) * tile_px - view_x0;
        let oy = c.y as f64 * tile_px - view_y0;
        placed.push(PlacedTile {
            coord: *c,
            wrap: *wrap,
            tile,
            ox: ox * ratio,
            oy: oy * ratio,
            unit_px: tile_px / EXTENT * ratio,
            clip: Clip { x0: ox * ratio, y0: oy * ratio, x1: (ox + tile_px) * ratio, y1: (oy + tile_px) * ratio },
            overscaling: 2f64.powi((zoom.floor() as i32 - c.z as i32).max(0)),
            ratio,
        });
    }
    (zoom, placed)
}

fn symbol_tiles(placed: &[PlacedTile]) -> Vec<SymbolTile<'_>> {
    placed
        .iter()
        .map(|p| SymbolTile {
            coord: (p.coord.z, p.coord.x, p.coord.y),
            wrap: p.wrap,
            tile: &p.tile,
            ox: p.ox / p.ratio,
            oy: p.oy / p.ratio,
            unit_px: p.unit_px / p.ratio,
            overscaling: p.overscaling,
        })
        .collect()
}

/// Render `tiles` (raw MVT bytes) with `style` over `bbox` at `width × height`.
/// `assets` holds glyphs and sprite images for symbol layers (may be empty).
/// Each tile carries its world-copy `wrap` (see [`covering_tiles`]).
/// `width` and `height` are CSS pixels; the image is `ratio` times larger
/// (MapLibre's devicePixelRatio: same view and layout, drawn denser).
#[allow(clippy::too_many_arguments)]
pub fn render(
    style: &Json,
    tiles: &[(TileCoord, i32, Vec<u8>)],
    assets: &SymbolAssets,
    bbox: [f64; 4],
    width: u32,
    height: u32,
    ratio: f64,
) -> Result<RenderOutput, String> {
    let (zoom, placed) = prepare(tiles, bbox, width, height, ratio);
    let (dw, dh) = (device_px(width, ratio), device_px(height, ratio));
    let mut fb = Framebuffer::new(dw as usize, dh as usize);

    // Symbol placement runs once, top layer first, before drawing.
    let placement = symbol::place(style, &symbol_tiles(&placed), assets, zoom, width as f64, height as f64);

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
            "fill" => draw_fill_layer(&mut fb, layer, zoom, &placed, ratio),
            "line" => draw_line_layer(&mut fb, layer, zoom, &placed, ratio),
            "circle" => draw_circle_layer(&mut fb, layer, zoom, &placed, ratio),
            "symbol" => symbol::draw_layer(
                &mut fb,
                layer,
                layer_idx,
                &placement,
                assets,
                ratio,
                (width as usize, height as usize),
            ),
            other => {
                if !skipped.iter().any(|s| s == other) {
                    skipped.push(other.to_owned());
                }
            }
        }
    }

    Ok(RenderOutput { width: dw, height: dh, pixels: fb.into_straight_rgba(), skipped_layer_types: skipped })
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

    /// `translate` in device pixels.
    fn translate_px(&self, ctx: &Ctx, ratio: f64) -> (f64, f64) {
        let (x, y) = self.translate(ctx);
        (x * ratio, y * ratio)
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

fn draw_fill_layer(fb: &mut Framebuffer, layer: &Json, zoom: f64, placed: &[PlacedTile], dpr: f64) {
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
            for poly in polys_px(pt, f, scale, translate.translate_px(&ctx, dpr)) {
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
            for poly in polys_px(pt, f, scale, translate.translate_px(&ctx, dpr)) {
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

fn draw_line_layer(fb: &mut Framebuffer, layer: &Json, zoom: f64, placed: &[PlacedTile], dpr: f64) {
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
    // line-dasharray is not interpolatable: legacy functions default to
    // "interval" (Kind::Enum), unlike interpolatable arrays such as offsets.
    let dasharray = Prop::new(p.get("line-dasharray"), Kind::Enum, zoom);
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
            // Widths in device pixels (CSS width * devicePixelRatio); the
            // antialiasing ramp stays one device pixel wide.
            let lw_css = width.num(&ctx, 1.0);
            let lw = lw_css * dpr;
            // line.vertex.glsl: gapwidth / 2, halfwidth = width / 2, offset negated.
            let gapw = gap.num(&ctx, 0.0) * dpr / 2.0;
            let halfwidth = lw / 2.0;
            let line_offset = -offset.num(&ctx, 0.0) * dpr;
            let aa = 0.5; // ANTIALIASING = 1 / devicePixelRatio / 2 CSS px
            let inset = gapw + if gapw > 0.0 { aa } else { 0.0 };
            let outset =
                gapw + halfwidth * if gapw > 0.0 { 2.0 } else { 1.0 } + if halfwidth == 0.0 { 0.0 } else { aa };
            if outset <= 0.0 {
                continue;
            }
            let blur2 = blur.num(&ctx, 0.0) * dpr + 1.0;
            // Dash pattern scale is in CSS pixels (ratio-independent).
            let floorwidth = floor_width.num(&fctx, lw_css);
            let (tx, ty) = translate.translate_px(&ctx, dpr);

            let dash_params = dash.as_ref().map(|d| {
                let width_b = d.width;
                let width_a = d.width * from_scale;
                let sdfgamma = ATLAS_W as f64 / (width_a.min(width_b) * 256.0 * dpr) / 2.0;
                // lineSDFUniformValues: tileRatio = 1 / pixelsToTileUnits(tile, 1,
                // tileZoom); an overscaled tile is 512 * overscaling px wide.
                let tile_ratio = TILE_PX * pt.overscaling / EXTENT;
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

fn draw_circle_layer(fb: &mut Framebuffer, layer: &Json, zoom: f64, placed: &[PlacedTile], dpr: f64) {
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
            let r = radius.num(&ctx, 5.0) * dpr;
            let sw = stroke_width.num(&ctx, 0.0) * dpr;
            let fill = rgba(color.color(&ctx, Color::BLACK), opacity.num(&ctx, 1.0));
            let stroke = rgba(stroke_color.color(&ctx, Color::BLACK), stroke_opacity.num(&ctx, 1.0));
            // circle.vertex.glsl: antialiasblur = -max(1 / dpr / (r + sw), blur),
            // with r and sw in CSS px (here device px, so no 1 / dpr).
            let aablur = -(1.0 / (r + sw)).max(blur.num(&ctx, 0.0));
            let (tx, ty) = translate.translate_px(&ctx, dpr);
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


// ── SVG output ───────────────────────────────────────────────────────────────

/// One style rendered as SVG fragments (ids prefixed so several styles can
/// share one document).
pub struct SvgParts {
    pub defs: String,
    pub body: String,
    pub skipped_layer_types: Vec<String>,
}

/// Render `tiles` with `style` as vector SVG: the same view, tile clipping,
/// evaluated style values and label placement as [`render`], written as
/// paths instead of pixels. Labels are glyph outlines traced from the
/// style's SDF glyphs at the shader's edge threshold.
pub fn render_svg(
    style: &Json,
    tiles: &[(TileCoord, i32, Vec<u8>)],
    assets: &SymbolAssets,
    bbox: [f64; 4],
    width: u32,
    height: u32,
    id_prefix: &str,
) -> Result<SvgParts, String> {
    use std::fmt::Write as _;
    let (zoom, placed) = prepare(tiles, bbox, width, height, 1.0);
    let placement = symbol::place(style, &symbol_tiles(&placed), assets, zoom, width as f64, height as f64);
    let mut defs = String::new();
    for (i, pt) in placed.iter().enumerate() {
        let _ = write!(
            defs,
            r#"<clipPath id="{id_prefix}t{i}"><rect x="{}" y="{}" width="{}" height="{}" shape-rendering="crispEdges"/></clipPath>"#,
            svg::num(pt.clip.x0),
            svg::num(pt.clip.y0),
            svg::num(pt.clip.x1 - pt.clip.x0),
            svg::num(pt.clip.y1 - pt.clip.y0)
        );
    }
    let mut body = String::new();
    let mut skipped: Vec<String> = Vec::new();
    for (layer_idx, layer) in style["layers"].as_array().map(|v| v.as_slice()).unwrap_or(&[]).iter().enumerate() {
        let ltype = layer["type"].as_str().unwrap_or("");
        if !symbol::layer_visible(layer, zoom) {
            continue;
        }
        let id = svg::xml_escape(layer["id"].as_str().unwrap_or(""));
        let mut out = String::new();
        match ltype {
            "background" => {
                let ctx = Ctx { zoom, feature: None };
                let p = &layer["paint"];
                let c = Prop::new(p.get("background-color"), Kind::Color, zoom).color(&ctx, Color::BLACK);
                let o = Prop::new(p.get("background-opacity"), Kind::Number, zoom).num(&ctx, 1.0);
                if let Some((hex, a)) = svg::paint(c, o) {
                    let _ = write!(out, r#"<rect width="{width}" height="{height}" fill="{hex}" fill-opacity="{}"/>"#, svg::alpha(a));
                }
            }
            "fill" => svg_fill(&mut out, layer, zoom, &placed, id_prefix),
            "line" => svg_line(&mut out, layer, zoom, &placed, id_prefix),
            "circle" => svg_circle(&mut out, layer, zoom, &placed),
            "symbol" => symbol::svg_layer(&mut out, &mut defs, layer, layer_idx, &placement, assets, width as usize, height as usize, id_prefix),
            other => {
                if !skipped.iter().any(|x| x == other) {
                    skipped.push(other.to_owned());
                }
            }
        }
        if !out.is_empty() {
            let _ = write!(body, r#"<g id="{id_prefix}{id}">{out}</g>"#);
        }
    }
    Ok(SvgParts { defs, body, skipped_layer_types: skipped })
}

/// Fills as draw_fill_layer draws them: interiors are sampled at pixel
/// centres without antialiasing (`crispEdges`), one path per polygon (each
/// polygon is triangulated and blended on its own), then the 1 px
/// antialiasing outline in the fill colour (or `fill-outline-color`). For
/// opaque layers MapLibre draws that outline only outside the fills, which is
/// the same as drawing it beneath them. The outline is an aliased gl.LINES
/// line (one pixel per column, alpha 1 - smoothstep(0, 1, d)), which covers
/// about 0.81 px on average: hence a 0.8 px stroke.
fn svg_fill(out: &mut String, layer: &Json, zoom: f64, placed: &[PlacedTile], prefix: &str) {
    use std::fmt::Write as _;
    let p = &layer["paint"];
    let color = Prop::new(p.get("fill-color"), Kind::Color, zoom);
    let opacity = Prop::new(p.get("fill-opacity"), Kind::Number, zoom);
    let outline = Prop::new(p.get("fill-outline-color"), Kind::Color, zoom);
    let translate = Prop::new(p.get("fill-translate"), Kind::Array, zoom);
    let antialias = p["fill-antialias"].as_bool().unwrap_or(true);
    let opaque = matches!(&color, Prop::Const(v) if v.as_color().map_or(false, |c| c.a == 1.0))
        && match &opacity {
            Prop::Absent => true,
            Prop::Const(v) => v.as_num() == Some(1.0),
            Prop::Data(..) => false,
        };
    let outline_beneath = opaque && !outline.is_set();

    let mut fills = String::new();
    let mut outlines = String::new();
    for (i, pt) in placed.iter().enumerate() {
        let (mut gf, mut go) = (String::new(), String::new());
        for (f, scale) in features(layer, pt, zoom) {
            let ctx = Ctx { zoom, feature: Some(f) };
            let op = opacity.num(&ctx, 1.0);
            let polys = polys_px(pt, f, scale, translate.translate(&ctx));
            if polys.is_empty() {
                continue;
            }
            if let Some((hex, a)) = svg::paint(color.color(&ctx, Color::BLACK), op) {
                for poly in &polys {
                    let _ = write!(gf, r#"<path d="{}" fill="{hex}" fill-opacity="{}"/>"#, svg::path_data(poly, true), svg::alpha(a));
                }
            }
            if antialias {
                let oc = if outline.is_set() { outline.color(&ctx, Color::BLACK) } else { color.color(&ctx, Color::BLACK) };
                if let Some((hex, a)) = svg::paint(oc, op) {
                    let rings: Vec<Vec<(f64, f64)>> = polys.into_iter().flatten().collect();
                    let _ = write!(go, r#"<path d="{}" stroke="{hex}" stroke-opacity="{}"/>"#, svg::path_data(&rings, true), svg::alpha(a));
                }
            }
        }
        if !gf.is_empty() {
            let _ = write!(fills, r#"<g clip-path="url(#{prefix}t{i})">{gf}</g>"#);
        }
        if !go.is_empty() {
            let _ = write!(outlines, r#"<g clip-path="url(#{prefix}t{i})">{go}</g>"#);
        }
    }
    let fills = format!(r#"<g fill-rule="evenodd" shape-rendering="crispEdges">{fills}</g>"#);
    let outlines = if outlines.is_empty() { outlines } else { format!(r#"<g fill="none" stroke-width="0.8">{outlines}</g>"#) };
    if outline_beneath {
        out.push_str(&outlines);
        out.push_str(&fills);
    } else {
        out.push_str(&fills);
        out.push_str(&outlines);
    }
}

/// A MapLibre dasharray as an SVG one: (even-length pattern, phase).
/// MapLibre's line atlas joins an odd array's last dash onto its first
/// (getDashRanges starts the first dash at -last), so [1, 2, 3] is a dash
/// of 4 and a gap of 2 entered 3 units into the dash; SVG would instead
/// repeat it as 1 2 3 1 2 3. A single value is a solid line. None when no
/// dashes show.
fn even_dash_pattern(d: &[f64]) -> Option<(Vec<f64>, f64)> {
    match d.len() {
        0 | 1 => None,
        n if n % 2 == 0 => Some((d.to_vec(), 0.0)),
        n => {
            let last = d[n - 1];
            let mut p = vec![d[0] + last];
            p.extend_from_slice(&d[1..n - 1]);
            Some((p, last))
        }
    }
}

/// Offset a polyline perpendicular to its direction (positive = right of
/// the direction in screen space), with mitred vertex normals.
/// With `closed`, `pts` is a ring (no repeated end point) and the offset
/// wraps around it.
fn offset_polyline(pts: &[(f64, f64)], d: f64, closed: bool, join: &str, miter_limit: f64, round_limit: f64) -> Vec<(f64, f64)> {
    if pts.len() < 2 || d == 0.0 {
        return pts.to_vec();
    }
    let normal = |a: (f64, f64), b: (f64, f64)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 { (0.0, 0.0) } else { (-dy / len, dx / len) }
    };
    let n = pts.len();
    let mut out = Vec::with_capacity(n + 8);
    for i in 0..n {
        let p = pts[i];
        let n1 = if i > 0 {
            normal(pts[i - 1], p)
        } else if closed {
            normal(pts[n - 1], p)
        } else {
            normal(p, pts[1])
        };
        let n2 = if i + 1 < n {
            normal(p, pts[i + 1])
        } else if closed {
            normal(p, pts[0])
        } else {
            n1
        };
        let (mut mx, mut my) = (n1.0 + n2.0, n1.1 + n2.1);
        let m = (mx * mx + my * my).sqrt();
        if m < 1e-9 {
            mx = n2.0;
            my = n2.1;
        } else {
            mx /= m;
            my /= m;
        }
        let cos = (mx * n2.0 + my * n2.1).max(1e-6);
        let miter_len = 1.0 / cos;
        // The offset side is the outside of the turn when the next segment
        // heads away from it (its direction is n2 rotated back by 90°).
        let dir2 = (n2.1, -n2.0);
        let outer = (dir2.0 * n1.0 + dir2.1 * n1.1) * d < -1e-9;
        // MapLibre offsets each join vertex along its extrusion, so the offset
        // path takes the join's shape on the outside of a turn (LineBucket:
        // round joins below round_limit and miters within the limit stay
        // mitred; otherwise round arcs or bevels).
        let shape = if !outer {
            "miter"
        } else {
            match join {
                "round" if miter_len >= round_limit => "round",
                "round" => "miter",
                "bevel" => "bevel",
                _ if miter_len > miter_limit => "bevel",
                _ => "miter",
            }
        };
        match shape {
            "round" => {
                let a0 = n1.1.atan2(n1.0);
                let mut sweep = n2.1.atan2(n2.0) - a0;
                while sweep > std::f64::consts::PI {
                    sweep -= 2.0 * std::f64::consts::PI;
                }
                while sweep < -std::f64::consts::PI {
                    sweep += 2.0 * std::f64::consts::PI;
                }
                let steps = ((sweep.abs() / (std::f64::consts::PI / 12.0)).ceil() as usize).max(1);
                for k in 0..=steps {
                    let a = a0 + sweep * k as f64 / steps as f64;
                    out.push((p.0 + a.cos() * d, p.1 + a.sin() * d));
                }
            }
            "bevel" => {
                out.push((p.0 + n1.0 * d, p.1 + n1.1 * d));
                out.push((p.0 + n2.0 * d, p.1 + n2.1 * d));
            }
            _ => {
                let k = if cos > 0.1 { d / cos } else { d };
                out.push((p.0 + mx * k, p.1 + my * k));
            }
        }
    }
    out
}

fn svg_line(out: &mut String, layer: &Json, zoom: f64, placed: &[PlacedTile], prefix: &str) {
    use std::fmt::Write as _;
    let p = &layer["paint"];
    let l = &layer["layout"];
    let color = Prop::new(p.get("line-color"), Kind::Color, zoom);
    let opacity = Prop::new(p.get("line-opacity"), Kind::Number, zoom);
    let width = Prop::new(p.get("line-width"), Kind::Number, zoom);
    let floor_width = Prop::new(p.get("line-width"), Kind::Number, zoom.floor());
    let gap = Prop::new(p.get("line-gap-width"), Kind::Number, zoom);
    let offset = Prop::new(p.get("line-offset"), Kind::Number, zoom);
    let translate = Prop::new(p.get("line-translate"), Kind::Array, zoom);
    let dasharray = Prop::new(p.get("line-dasharray"), Kind::Enum, zoom);
    let cap = l["line-cap"].as_str().unwrap_or("butt");
    let join = l["line-join"].as_str().unwrap_or("miter");
    let miter = l["line-miter-limit"].as_f64().unwrap_or(2.0);
    let round_limit = l["line-round-limit"].as_f64().unwrap_or(1.05);
    let dash = match &dasharray {
        Prop::Const(v) => v.as_num_array().filter(|d| d.len() >= 2 && d.iter().sum::<f64>() > 0.0),
        _ => None,
    };
    for (i, pt) in placed.iter().enumerate() {
        let mut g = String::new();
        for (f, scale) in features(layer, pt, zoom) {
            let ctx = Ctx { zoom, feature: Some(f) };
            let fctx = Ctx { zoom: zoom.floor(), feature: Some(f) };
            // Polygon rings are closed lines (LineBucket's isPolygon): their
            // first vertex gets a join, not caps.
            let (lines, closed): (Vec<&Vec<(f32, f32)>>, bool) = match &f.geometry {
                Geometry::Lines(ls) => (ls.iter().collect(), false),
                Geometry::Polygons(ps) => (ps.iter().flatten().collect(), true),
                Geometry::Points(_) => continue,
            };
            let lw = width.num(&ctx, 1.0);
            if lw <= 0.0 {
                continue;
            }
            let Some((hex, a)) = svg::paint(color.color(&ctx, Color::BLACK), opacity.num(&ctx, 1.0)) else { continue };
            let (tx, ty) = translate.translate(&ctx);
            let px: Vec<Vec<(f64, f64)>> = lines
                .iter()
                .map(|ln| {
                    let mut v: Vec<(f64, f64)> = ln
                        .iter()
                        .map(|&q| {
                            let (x, y) = to_extent(q, scale);
                            (pt.ox + x * pt.unit_px + tx, pt.oy + y * pt.unit_px + ty)
                        })
                        .collect();
                    if closed && v.len() > 2 && v.first() == v.last() {
                        v.pop();
                    }
                    v
                })
                .collect();
            let off = offset.num(&ctx, 0.0);
            let gw = gap.num(&ctx, 0.0);
            let offsets: Vec<f64> = if gw > 0.0 { vec![off - (gw / 2.0 + lw / 2.0), off + (gw / 2.0 + lw / 2.0)] } else { vec![off] };
            let mut dash_attr = String::new();
            if let Some((pattern, phase)) = dash.as_deref().and_then(even_dash_pattern) {
                // MapLibre dash lengths are multiples of the line width at
                // floor(zoom); round caps are drawn inside the dash in MapLibre
                // but extend it in SVG, so shorten dashes by one width.
                // The pattern is laid out in pixels at the tile's integer zoom
                // (tileRatio), so it stretches with the fractional zoom.
                let fw = floor_width.num(&fctx, lw) * pt.unit_px * EXTENT / (TILE_PX * pt.overscaling);
                let round = cap == "round";
                let vals: Vec<String> = pattern
                    .iter()
                    .enumerate()
                    .map(|(k, &v)| {
                        let base = v * fw;
                        let adj = if round { if k % 2 == 0 { (base - lw).max(0.01) } else { base + lw } } else { base };
                        svg::num(adj)
                    })
                    .collect();
                let offset = phase * fw - if round { lw / 2.0 } else { 0.0 };
                dash_attr = format!(r#" stroke-dasharray="{}""#, vals.join(" "));
                if offset != 0.0 {
                    dash_attr += &format!(r#" stroke-dashoffset="{}""#, svg::num(offset));
                }
            }
            // line.fragment.glsl ramps alpha over one pixel centred on each
            // edge, so a line of width w < 1 covers (w + 1)^2 / 4 px, not w:
            // stroke at the width with that coverage.
            let stroke_w = if lw < 1.0 { (lw + 1.0).powi(2) / 4.0 } else { lw };
            // One path per line: MapLibre blends each line on its own, so
            // translucent lines darken where they overlap.
            let _ = write!(
                g,
                r#"<g fill="none" stroke="{hex}" stroke-opacity="{}" stroke-width="{}" stroke-linecap="{cap}" stroke-linejoin="{join}" stroke-miterlimit="{}"{dash_attr}>"#,
                svg::alpha(a),
                svg::num(stroke_w),
                svg::num(miter)
            );
            for o in &offsets {
                for r in &px {
                    let line = offset_polyline(r, *o, closed, join, miter, round_limit);
                    let _ = write!(g, r#"<path d="{}"/>"#, svg::path_data(std::slice::from_ref(&line), closed));
                }
            }
            g.push_str("</g>");
        }
        if !g.is_empty() {
            let _ = write!(out, r#"<g clip-path="url(#{prefix}t{i})">{g}</g>"#);
        }
    }
}

fn svg_circle(out: &mut String, layer: &Json, zoom: f64, placed: &[PlacedTile]) {
    use std::fmt::Write as _;
    let p = &layer["paint"];
    let radius = Prop::new(p.get("circle-radius"), Kind::Number, zoom);
    let color = Prop::new(p.get("circle-color"), Kind::Color, zoom);
    let opacity = Prop::new(p.get("circle-opacity"), Kind::Number, zoom);
    let stroke_width = Prop::new(p.get("circle-stroke-width"), Kind::Number, zoom);
    let stroke_color = Prop::new(p.get("circle-stroke-color"), Kind::Color, zoom);
    let stroke_opacity = Prop::new(p.get("circle-stroke-opacity"), Kind::Number, zoom);
    let translate = Prop::new(p.get("circle-translate"), Kind::Array, zoom);
    for pt in placed {
        for (f, scale) in features(layer, pt, zoom) {
            let Geometry::Points(points) = &f.geometry else { continue };
            let ctx = Ctx { zoom, feature: Some(f) };
            let r = radius.num(&ctx, 5.0);
            let sw = stroke_width.num(&ctx, 0.0);
            let (tx, ty) = translate.translate(&ctx);
            for &q in points {
                let (x, y) = to_extent(q, scale);
                if x < 0.0 || x >= EXTENT || y < 0.0 || y >= EXTENT {
                    continue;
                }
                let (cx, cy) = (pt.ox + x * pt.unit_px + tx, pt.oy + y * pt.unit_px + ty);
                if let Some((hex, a)) = svg::paint(color.color(&ctx, Color::BLACK), opacity.num(&ctx, 1.0)) {
                    let _ = write!(out, r#"<circle cx="{}" cy="{}" r="{}" fill="{hex}" fill-opacity="{}"/>"#, svg::num(cx), svg::num(cy), svg::num(r), svg::alpha(a));
                }
                if sw > 0.0 {
                    if let Some((hex, a)) = svg::paint(stroke_color.color(&ctx, Color::BLACK), stroke_opacity.num(&ctx, 1.0)) {
                        let _ = write!(
                            out,
                            r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{hex}" stroke-opacity="{}" stroke-width="{}"/>"#,
                            svg::num(cx),
                            svg::num(cy),
                            svg::num(r + sw / 2.0),
                            svg::alpha(a),
                            svg::num(sw)
                        );
                    }
                }
            }
        }
    }
}
