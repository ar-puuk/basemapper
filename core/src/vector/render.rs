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

/// Raster tiles of one source at the zoom MapLibre covers the view with.
pub struct RasterTiles {
    pub zoom: u8,
    pub tiles: Vec<(TileCoord, Vec<u8>)>,
}

/// Tiles for every tile source a style's layers use, keyed by source id.
/// Vector tiles carry their world-copy wrap (see [`covering_tiles`]).
#[derive(Default)]
pub struct SourceTiles {
    pub vector: std::collections::HashMap<String, Vec<(TileCoord, i32, Vec<u8>)>>,
    pub raster: std::collections::HashMap<String, RasterTiles>,
    /// `raster-dem` sources (hillshade layers).
    pub dem: std::collections::HashMap<String, super::hillshade::DemTiles>,
}

impl SourceTiles {
    /// One vector source (as styles with a single source have).
    pub fn single_vector(id: &str, tiles: Vec<(TileCoord, i32, Vec<u8>)>) -> Self {
        let mut s = SourceTiles::default();
        s.vector.insert(id.to_owned(), tiles);
        s
    }
}

/// Placed vector tiles per source id.
type Placed = std::collections::HashMap<String, Vec<PlacedTile>>;

/// The placed tiles of `layer`'s source (empty when it has none).
fn layer_tiles<'a>(placed: &'a Placed, layer: &Json) -> &'a [PlacedTile] {
    layer["source"].as_str().and_then(|s| placed.get(s)).map_or(&[], |v| v.as_slice())
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
fn prepare(sources: &SourceTiles, bbox: [f64; 4], width: u32, height: u32, ratio: f64) -> (f64, Placed) {
    let zoom = scale_zoom(bbox, width);
    let placed = sources
        .vector
        .iter()
        .map(|(id, tiles)| (id.clone(), place_tiles(tiles, bbox, width, height, ratio, zoom)))
        .collect();
    (zoom, placed)
}

fn place_tiles(
    tiles: &[(TileCoord, i32, Vec<u8>)],
    bbox: [f64; 4],
    width: u32,
    height: u32,
    ratio: f64,
    zoom: f64,
) -> Vec<PlacedTile> {
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
    placed
}

fn symbol_tiles(placed: &Placed) -> Vec<SymbolTile<'_>> {
    let mut ids: Vec<&String> = placed.keys().collect();
    ids.sort();
    ids.into_iter()
        .flat_map(|id| placed[id].iter().map(move |p| (id, p)))
        .map(|(id, p)| SymbolTile {
            source: id.as_str(),
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
    sources: &SourceTiles,
    assets: &SymbolAssets,
    bbox: [f64; 4],
    width: u32,
    height: u32,
    ratio: f64,
) -> Result<RenderOutput, String> {
    let (zoom, placed) = prepare(sources, bbox, width, height, ratio);
    let (dw, dh) = (device_px(width, ratio), device_px(height, ratio));
    let mut fb = Framebuffer::new(dw as usize, dh as usize);
    let mut depth3d: Vec<f64> = Vec::new();

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
            "fill" => draw_fill_layer(&mut fb, layer, zoom, layer_tiles(&placed, layer), ratio, assets),
            "line" => draw_line_layer(&mut fb, layer, zoom, layer_tiles(&placed, layer), ratio, assets),
            "circle" => draw_circle_layer(&mut fb, layer, zoom, layer_tiles(&placed, layer), ratio),
            "heatmap" => draw_heatmap_layer(&mut fb, layer, zoom, layer_tiles(&placed, layer), ratio),
            "fill-extrusion" => {
                let view = ExtrusionView::new(style, bbox, width, height, ratio, zoom);
                draw_fill_extrusion_layer(&mut fb, layer, zoom, layer_tiles(&placed, layer), &view, assets, &mut depth3d);
            }
            "raster" => {
                if let Some(rt) = layer["source"].as_str().and_then(|s| sources.raster.get(s)) {
                    draw_raster_layer(&mut fb, layer, zoom, rt, bbox, ratio);
                }
            }
            "hillshade" => {
                if let Some(dem) = layer["source"].as_str().and_then(|s| sources.dem.get(s)) {
                    let nearest = super::hillshade::source_nearest(style, layer["source"].as_str().unwrap_or(""), zoom);
                    super::hillshade::draw_layer(&mut fb, layer, zoom, dem, bbox, width, height, ratio, nearest);
                }
            }
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
    let src = pt.tile.layers.get(mvt::source_layer(layer));
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
            Color::new(q(c.r), q(c.g), q(c.b), q(c.a))
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

/// fill_pattern shaders: where a fragment falls in the pattern. The pattern
/// repeats in tile space with its display size (image px / pixelRatio) in
/// CSS px at the tile's integer zoom, phased by the tile's world pixel
/// position (u_pixel_coord), so it lines up across tiles and stretches with
/// the fractional zoom. Only pattern B shows (no cross-fade).
struct PatternFrame {
    ox: f64,
    oy: f64,
    unit_px: f64,
    tile_ratio: f64,
    world_x: f64,
    world_y: f64,
}

impl PatternFrame {
    fn new(pt: &PlacedTile) -> Self {
        let n = 2f64.powi(pt.coord.z as i32);
        let tile_px = TILE_PX * pt.overscaling;
        PatternFrame {
            ox: pt.ox,
            oy: pt.oy,
            unit_px: pt.unit_px,
            tile_ratio: tile_px / EXTENT,
            world_x: tile_px * (pt.coord.x as f64 + pt.wrap as f64 * n),
            world_y: tile_px * pt.coord.y as f64,
        }
    }

    /// Pattern coordinates (in repeats) at device position (x, y), with the
    /// layer's translate `t` (device px) removed.
    fn coord(&self, img: &symbol::SpriteImage, x: f64, y: f64, t: (f64, f64)) -> (f64, f64) {
        let size = (img.w as f64 / img.pixel_ratio, img.h as f64 / img.pixel_ratio);
        let pos = ((x - t.0 - self.ox) / self.unit_px, (y - t.1 - self.oy) / self.unit_px);
        (
            (self.tile_ratio * pos.0 + self.world_x.rem_euclid(size.0)) / size.0,
            (self.tile_ratio * pos.1 + self.world_y.rem_euclid(size.1)) / size.1,
        )
    }

    /// Premultiplied colour (0..1) of `img` at device pixel (x, y).
    fn sample(&self, img: &symbol::SpriteImage, x: usize, y: usize, t: (f64, f64)) -> [f64; 4] {
        let v = self.coord(img, x as f64 + 0.5, y as f64 + 0.5, t);
        let s = img.sample_wrapped(v.0.rem_euclid(1.0) * img.w as f64, v.1.rem_euclid(1.0) * img.h as f64);
        [s[0] / 255.0, s[1] / 255.0, s[2] / 255.0, s[3] / 255.0]
    }
}

fn draw_fill_layer(fb: &mut Framebuffer, layer: &Json, zoom: f64, placed: &[PlacedTile], dpr: f64, assets: &SymbolAssets) {
    let p = &layer["paint"];
    let color = Prop::new(p.get("fill-color"), Kind::Color, zoom);
    let opacity = Prop::new(p.get("fill-opacity"), Kind::Number, zoom);
    let outline = Prop::new(p.get("fill-outline-color"), Kind::Color, zoom);
    let translate = Prop::new(p.get("fill-translate"), Kind::Array, zoom);
    let antialias = p["fill-antialias"].as_bool().unwrap_or(true);
    let pattern = Prop::new(p.get("fill-pattern"), Kind::Enum, zoom);
    if pattern.is_set() {
        return draw_fill_pattern_layer(fb, layer, zoom, placed, dpr, assets, &pattern, &opacity, &outline, &translate, antialias);
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

/// draw_fill.ts with a pattern: always the translucent pass; interiors with
/// fill_pattern, then (fill-antialias) outlines with fill_outline_pattern, or
/// in fill-outline-color when one is set. Features whose image is missing
/// are not drawn.
#[allow(clippy::too_many_arguments)]
fn draw_fill_pattern_layer(
    fb: &mut Framebuffer,
    layer: &Json,
    zoom: f64,
    placed: &[PlacedTile],
    dpr: f64,
    assets: &SymbolAssets,
    pattern: &Prop,
    opacity: &Prop,
    outline: &Prop,
    translate: &Prop,
    antialias: bool,
) {
    let (w, h) = (fb.w, fb.h);
    let image_of = |ctx: &Ctx| -> Option<&symbol::SpriteImage> {
        match pattern.get(ctx)? {
            Val::Str(name) => assets.sprite.images.get(&name),
            _ => None,
        }
    };
    for pass_outline in [false, true] {
        if pass_outline && !antialias {
            break;
        }
        for pt in placed {
            let frame = PatternFrame::new(pt);
            for (f, scale) in features(layer, pt, zoom) {
                let ctx = Ctx { zoom, feature: Some(f) };
                let Some(img) = image_of(&ctx) else { continue };
                let op = opacity.num(&ctx, 1.0);
                if op <= 0.0 {
                    continue;
                }
                let t = translate.translate_px(&ctx, dpr);
                let polys = polys_px(pt, f, scale, t);
                if !pass_outline {
                    for poly in &polys {
                        raster_polygon(w, h, poly, &pt.clip, |x, y| {
                            let c = frame.sample(img, x, y, t);
                            fb.blend(x, y, [(c[0] * op) as f32, (c[1] * op) as f32, (c[2] * op) as f32, (c[3] * op) as f32]);
                        });
                    }
                    continue;
                }
                let plain = outline.is_set().then(|| rgba(outline.color(&ctx, Color::BLACK), op));
                for poly in &polys {
                    for ring in poly {
                        for seg in ring.windows(2) {
                            raster_gl_line(w, h, seg[0], seg[1], &pt.clip, |x, y, d| {
                                let k = 1.0 - smoothstep(0.0, 1.0, d);
                                let c = match plain {
                                    Some(c) => scaled(c, k),
                                    None => {
                                        let s = frame.sample(img, x, y, t);
                                        let m = op * k;
                                        [(s[0] * m) as f32, (s[1] * m) as f32, (s[2] * m) as f32, (s[3] * m) as f32]
                                    }
                                };
                                fb.blend(x, y, c);
                            });
                        }
                    }
                }
            }
        }
    }
}

// ── Line ─────────────────────────────────────────────────────────────────────

fn draw_line_layer(fb: &mut Framebuffer, layer: &Json, zoom: f64, placed: &[PlacedTile], dpr: f64, assets: &SymbolAssets) {
    let p = &layer["paint"];
    // line-pattern replaces color and dashes (the linePattern program).
    let pattern = Prop::new(p.get("line-pattern"), Kind::Enum, zoom);
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
    let tz = zoom.floor();
    let cap = match super::expr::layout_enum(l, "line-cap", tz).as_deref() {
        Some("round") => Cap::Round,
        Some("square") => Cap::Square,
        _ => Cap::Butt,
    };
    let join = match super::expr::layout_enum(l, "line-join", tz).as_deref() {
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
            let image = if pattern.is_set() {
                match pattern.get(&ctx) {
                    Some(Val::Str(name)) => match assets.sprite.images.get(&name) {
                        Some(img) => Some(img),
                        None => continue,
                    },
                    _ => continue,
                }
            } else {
                None
            };
            let op = opacity.num(&ctx, 1.0);
            let c = if image.is_some() { [op as f32; 4] } else { rgba(color.color(&ctx, Color::BLACK), op) };
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

            // line_pattern.fragment.glsl (pattern B, toScale 1): one repeat
            // spans display_w / tileZoomRatio * floorwidth / display_h tile
            // units along the line and the full outset across it, mapped onto
            // the image plus its 1 px atlas padding.
            let pattern_params = image.map(|img| {
                let tile_ratio = TILE_PX * pt.overscaling / EXTENT;
                let (dw, dh) = (img.w as f64 / img.pixel_ratio, img.h as f64 / img.pixel_ratio);
                (img, tile_ratio * dh / (dw * floorwidth))
            });
            let dash_params = dash.as_ref().filter(|_| image.is_none()).map(|d| {
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
                        if let Some((img, k)) = pattern_params {
                            let xb = (var[2] * k).rem_euclid(1.0);
                            let yb = 0.5 * var[1] + 0.5;
                            let s = img.sample_atlas_pattern((img.w as f64 + 2.0) * xb, (img.h as f64 + 2.0) * yb);
                            let m = alpha * op / 255.0;
                            fb.blend(x, y, [(s[0] * m) as f32, (s[1] * m) as f32, (s[2] * m) as f32, (s[3] * m) as f32]);
                        } else {
                            fb.blend(x, y, scaled(c, alpha));
                        }
                    }
                });
            }
        }
    }
}

// ── Fill extrusion ───────────────────────────────────────────────────────────

/// MapLibre's camera for 3D layers at pitch 0: a perspective camera
/// cameraToCenterDistance = 0.5 / tan(fov / 2) * height (fov 36.87°, so 1.5
/// heights) above the view centre. A point at height Z (px) projects to
/// centre + offset * d / (d - Z). Heights convert from metres with
/// mercatorZfromAltitude at the centre latitude (Earth radius 6371008.8 m).
struct ExtrusionView {
    /// Projection centre in device px.
    cx: f64,
    cy: f64,
    /// Camera distance in CSS px.
    d: f64,
    /// CSS px per metre of height.
    px_per_m: f64,
    /// Light (style "light", viewport anchor, pitch/bearing 0).
    light_pos: [f64; 3],
    light_color: [f64; 3],
    light_intensity: f64,
}

impl ExtrusionView {
    fn new(style: &Json, bbox: [f64; 4], width: u32, height: u32, ratio: f64, zoom: f64) -> Self {
        let world = TILE_PX * 2f64.powf(zoom);
        let lat = (2.0 * (((bbox[1] + bbox[3]) / 2.0) / (EARTH_CIRC / (2.0 * std::f64::consts::PI))).exp().atan()
            - std::f64::consts::FRAC_PI_2)
            .to_degrees();
        let maplibre_circ = 2.0 * std::f64::consts::PI * 6_371_008.8;
        let fov: f64 = 0.643_501_108_793_284_4;
        let light = &style["light"];
        let ctx = Ctx { zoom, feature: None };
        let pos = light.get("position").map(|j| eval_property(j, Kind::Array, &ctx)).and_then(|v| v.as_num_array());
        let pos = pos.filter(|p| p.len() == 3).unwrap_or_else(|| vec![1.15, 210.0, 30.0]);
        // sphericalToCartesian
        let az = (pos[1] + 90.0).to_radians();
        let polar = pos[2].to_radians();
        let light_pos = [pos[0] * az.cos() * polar.sin(), pos[0] * az.sin() * polar.sin(), pos[0] * polar.cos()];
        let color = light
            .get("color")
            .map(|j| eval_property(j, Kind::Color, &ctx))
            .and_then(|v| v.as_color())
            .unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0));
        let intensity = light.get("intensity").map(|j| eval_property(j, Kind::Number, &ctx)).and_then(|v| v.as_num()).unwrap_or(0.5);
        ExtrusionView {
            cx: width as f64 * ratio / 2.0,
            cy: height as f64 * ratio / 2.0,
            d: 0.5 / (fov / 2.0).tan() * height as f64,
            px_per_m: world / (maplibre_circ * lat.to_radians().cos()),
            light_pos,
            light_color: [color.r as f64, color.g as f64, color.b as f64],
            light_intensity: intensity,
        }
    }

    /// Screen position (device px) and depth key 1 / (d - Z) of a map point
    /// (device px) at `h` metres.
    fn project(&self, p: (f64, f64), h: f64) -> (f64, f64, f64) {
        let z = h * self.px_per_m;
        let s = self.d / (self.d - z);
        (self.cx + (p.0 - self.cx) * s, self.cy + (p.1 - self.cy) * s, 1.0 / (self.d - z))
    }

    /// fill_extrusion.vertex.glsl lighting for one vertex: premultiplied
    /// colour (before opacity).
    #[allow(clippy::too_many_arguments)]
    fn shade(&self, color: [f64; 4], normal: (f64, f64, f64), t: f64, base: f64, height: f64, vertical_gradient: bool) -> [f64; 3] {
        let i = self.light_intensity;
        let colorvalue = color[0] * 0.2126 + color[1] * 0.7152 + color[2] * 0.0722;
        let c = [color[0] + 0.03, color[1] + 0.03, color[2] + 0.03];
        let lp = self.light_pos;
        let mut directional = (normal.0 * lp[0] + normal.1 * lp[1] + normal.2 * lp[2]).clamp(0.0, 1.0);
        let hi = (1.0 - colorvalue + i).max(1.0);
        directional = (1.0 - i) * (1.0 - directional) + hi * directional;
        if normal.1 != 0.0 {
            let vg = if vertical_gradient { 1.0 } else { 0.0 };
            let lo = 0.7 * i + 0.98 * (1.0 - i);
            directional *= (1.0 - vg) + vg * ((t + base) * (height / 150.0).sqrt()).clamp(lo, 1.0);
        }
        let mut out = [0.0; 3];
        for k in 0..3 {
            let lc = self.light_color[k];
            out[k] = (c[k] * directional * lc).clamp(0.3 * (1.0 - lc), 1.0);
        }
        out
    }
}

impl ExtrusionView {
    /// fill_extrusion_pattern.vertex.glsl lighting for one vertex (the
    /// texture's multiplier, before opacity).
    fn shade_pattern(&self, normal: (f64, f64, f64), t: f64, base: f64, height: f64, vertical_gradient: bool) -> [f64; 3] {
        let i = self.light_intensity;
        let lp = self.light_pos;
        let mut directional = (normal.0 * lp[0] + normal.1 * lp[1] + normal.2 * lp[2]).clamp(0.0, 1.0);
        let hi = (0.5 + i).max(1.0);
        directional = (1.0 - i) * (1.0 - directional) + hi * directional;
        if normal.1 != 0.0 {
            let vg = if vertical_gradient { 1.0 } else { 0.0 };
            let lo = 0.7 * i + 0.98 * (1.0 - i);
            directional *= (1.0 - vg) + vg * ((t + base) * (height / 150.0).sqrt()).clamp(lo, 1.0);
        }
        let mut out = [0.0; 3];
        for k in 0..3 {
            let lc = self.light_color[k];
            out[k] = (directional * lc).clamp(0.3 * (1.0 - lc), 1.0);
        }
        out
    }

    /// Map position (device px) under screen point (x, y) on a plane `h`
    /// metres up.
    fn unproject(&self, x: f64, y: f64, h: f64) -> (f64, f64) {
        let s = self.d / (self.d - h * self.px_per_m);
        (self.cx + (x - self.cx) / s, self.cy + (y - self.cy) / s)
    }
}

/// FillExtrusionBucket's isEntirelyOutside: every point beyond one tile edge.
fn ring_entirely_outside(ring: &[(f64, f64)]) -> bool {
    let e = EXTENT;
    ring.iter().all(|p| p.0 < 0.0) || ring.iter().all(|p| p.0 > e) || ring.iter().all(|p| p.1 < 0.0) || ring.iter().all(|p| p.1 > e)
}

/// fill-extrusion at pitch 0 (draw_fill_extrusion.ts): per polygon, side
/// walls (skipping edges along the tile boundary) then the top face, with
/// per-vertex lighting interpolated perspective-correctly and a depth test.
/// Translucent layers draw only the closest surface per pixel (a depth
/// pre-pass, then colour where the depth matches, once per pixel).
#[allow(clippy::too_many_arguments)]
fn draw_fill_extrusion_layer(
    fb: &mut Framebuffer,
    layer: &Json,
    zoom: f64,
    placed: &[PlacedTile],
    view: &ExtrusionView,
    assets: &SymbolAssets,
    depth: &mut Vec<f64>,
) {
    let p = &layer["paint"];
    let color = Prop::new(p.get("fill-extrusion-color"), Kind::Color, zoom);
    let height = Prop::new(p.get("fill-extrusion-height"), Kind::Number, zoom);
    let base = Prop::new(p.get("fill-extrusion-base"), Kind::Number, zoom);
    let opacity = Prop::new(p.get("fill-extrusion-opacity"), Kind::Number, zoom).num(&Ctx { zoom, feature: None }, 1.0);
    let vertical_gradient = p["fill-extrusion-vertical-gradient"].as_bool().unwrap_or(true);
    // fill-extrusion-pattern (the fillExtrusionPattern program): the image
    // replaces the colour, lit without it; always drawn in two passes.
    let pattern = Prop::new(p.get("fill-extrusion-pattern"), Kind::Enum, zoom);
    let patterned = pattern.is_set();
    if opacity <= 0.0 {
        return;
    }
    let (w, h) = (fb.w, fb.h);
    let full = Clip { x0: 0.0, y0: 0.0, x1: w as f64, y1: h as f64 };

    // Triangles in screen space: vertices (x, y, [r/w, g/w, b/w, u/w, v/w,
    // 1/w]), (u, v) the pattern position in repeats.
    let mut tris: Vec<[Vert<6>; 3]> = Vec::new();
    // Top faces: one planar polygon each (constant depth, colour), with the
    // height for the pattern's inverse projection.
    let mut tops: Vec<(Rings, f64, [f64; 3], f64)> = Vec::new();
    // Patterned pieces: the image and the tile's pattern frame.
    type Pat<'a> = Option<(Option<&'a symbol::SpriteImage>, usize)>;
    let mut frames: Vec<PatternFrame> = Vec::new();
    // Draw order: MapLibre's bucket order (per polygon: walls, then top).
    enum Piece<'a> {
        Tris(usize, usize, Pat<'a>),
        Top(usize, Pat<'a>),
    }
    let mut order: Vec<Piece> = Vec::new();
    for pt in placed {
        let frame = PatternFrame::new(pt);
        // get_pattern_pos for a wall: (edgedistance, elevation *
        // u_height_factor) in tile units, u_height_factor = -2^z / tileSize / 8
        // (z overscaled, tileSize 512 times the overscaling).
        let height_factor = -(2f64.powi(pt.coord.z as i32)) / TILE_PX / 8.0;
        let world = (frame.world_x, frame.world_y);
        let tile_ratio = frame.tile_ratio;
        frames.push(frame);
        let fi = frames.len() - 1;
        for (f, scale) in features(layer, pt, zoom) {
            let Geometry::Polygons(polys) = &f.geometry else { continue };
            let ctx = Ctx { zoom, feature: Some(f) };
            // A feature whose pattern image is missing gets no pattern
            // attributes: it draws nothing but still takes depth.
            let img = match pattern.get(&ctx) {
                Some(Val::Str(name)) if patterned => assets.sprite.images.get(&name),
                _ => None,
            };
            let pat: Pat = patterned.then_some((img, fi));
            let size = img.map_or((1.0, 1.0), |i| (i.w as f64 / i.pixel_ratio, i.h as f64 / i.pixel_ratio));
            let pattern_pos = |pos: (f64, f64)| {
                (
                    (tile_ratio * pos.0 + world.0.rem_euclid(size.0)) / size.0,
                    (tile_ratio * pos.1 + world.1.rem_euclid(size.1)) / size.1,
                )
            };
            let c = color.color(&ctx, Color::BLACK);
            let col = [c.r as f64, c.g as f64, c.b as f64, c.a as f64];
            let hgt = height.num(&ctx, 0.0).max(0.0);
            let bse = base.num(&ctx, 0.0).max(0.0);
            for poly in polys {
                let rings: Vec<Vec<(f64, f64)>> = poly.iter().map(|r| r.iter().map(|&q| to_extent(q, scale)).collect()).collect();
                if rings.first().map_or(true, |r| ring_entirely_outside(r)) {
                    continue;
                }
                let to_px = |q: (f64, f64)| (pt.ox + q.0 * pt.unit_px, pt.oy + q.1 * pt.unit_px);
                let first = tris.len();
                for ring in &rings {
                    if ring_entirely_outside(ring) {
                        continue;
                    }
                    let mut edge_distance = 0.0f64;
                    for e in ring.windows(2) {
                        // FillExtrusionBucket: p1 is the current point, p2 the previous.
                        let (p1, p2) = (e[1], e[0]);
                        if (p1.0 == p2.0 && (p1.0 < 0.0 || p1.0 > EXTENT)) || (p1.1 == p2.1 && (p1.1 < 0.0 || p1.1 > EXTENT)) {
                            continue; // isBoundaryEdge
                        }
                        let (dx, dy) = (p1.0 - p2.0, p1.1 - p2.1);
                        let len = (dx * dx + dy * dy).sqrt();
                        if edge_distance + len > 32768.0 {
                            edge_distance = 0.0;
                        }
                        // a_normal_ed is Int16: the distance is truncated (and
                        // 32768 wraps).
                        let ed1 = edge_distance.trunc();
                        edge_distance += len;
                        let ed2 = (edge_distance.trunc() as i64 as i16) as f64;
                        if len == 0.0 {
                            continue;
                        }
                        // perp of (p1 - p2), unit; encoded y is exact, x carries the top flag.
                        let n = (-dy / len, dx / len, 0.0);
                        let ny = (n.1 * 16384.0).trunc() / 16384.0;
                        let vert = |q: (f64, f64), t: f64, ed: f64| -> Vert<6> {
                            let elevation = if t > 0.0 { hgt } else { bse };
                            let (x, y, iw) = view.project(to_px(q), elevation);
                            let s = if patterned {
                                view.shade_pattern((n.0, ny, n.2), t, bse, hgt, vertical_gradient)
                            } else {
                                view.shade(col, (n.0, ny, n.2), t, bse, hgt, vertical_gradient)
                            };
                            let (u, v) = pattern_pos((ed, elevation * height_factor));
                            Vert { x, y, v: [s[0] * iw, s[1] * iw, s[2] * iw, u * iw, v * iw, iw] }
                        };
                        let v = [vert(p1, 0.0, ed1), vert(p1, 1.0, ed1), vert(p2, 0.0, ed2), vert(p2, 1.0, ed2)];
                        tris.push([v[0], v[2], v[1]]);
                        tris.push([v[1], v[2], v[3]]);
                    }
                }
                order.push(Piece::Tris(first, tris.len(), pat));
                let (_, _, iw) = view.project((0.0, 0.0), hgt);
                let top: Rings = rings
                    .iter()
                    .map(|r| r.iter().map(|&q| { let (x, y, _) = view.project(to_px(q), hgt); (x, y) }).collect())
                    .collect();
                let light = if patterned {
                    view.shade_pattern((0.0, 0.0, 1.0), 1.0, bse, hgt, vertical_gradient)
                } else {
                    view.shade(col, (0.0, 0.0, 1.0), 1.0, bse, hgt, vertical_gradient)
                };
                tops.push((top, iw, light, hgt));
                order.push(Piece::Top(tops.len() - 1, pat));
            }
        }
    }

    // One depth buffer for all 3D layers (cleared once per frame).
    if depth.len() != w * h {
        *depth = vec![0.0f64; w * h];
    }
    let mut drawn = vec![false; w * h];
    let translucent = opacity < 1.0 || patterned;
    let passes: &[bool] = if translucent { &[true, false] } else { &[false] };
    for &depth_only in passes {
        for piece in &order {
            let mut frag = |x: usize, y: usize, iw: f64, rgba: [f64; 4], fb: &mut Framebuffer| {
                let k = y * w + x;
                if depth_only {
                    if iw > depth[k] {
                        depth[k] = iw;
                    }
                    return;
                }
                if translucent {
                    if iw < depth[k] || drawn[k] {
                        return;
                    }
                    drawn[k] = true;
                } else {
                    if iw < depth[k] {
                        return;
                    }
                    depth[k] = iw;
                }
                let a = opacity;
                fb.blend(x, y, [(rgba[0] * a) as f32, (rgba[1] * a) as f32, (rgba[2] * a) as f32, (rgba[3] * a) as f32]);
            };
            match piece {
                Piece::Tris(a, b, pat) => {
                    for t in &tris[*a..*b] {
                        // CullFaceMode.backCCW: GL's clip space is y-up, so a
                        // front (CCW) face winds the other way on screen.
                        let cross = (t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[1].y - t[0].y) * (t[2].x - t[0].x);
                        if cross > 0.0 {
                            continue;
                        }
                        raster_triangle(w, h, *t, &full, |x, y, v| {
                            let iw = v[5];
                            if iw > 0.0 {
                                let c = if depth_only {
                                    [0.0; 4]
                                } else {
                                    extrusion_texel(*pat, v[3] / iw, v[4] / iw, [v[0] / iw, v[1] / iw, v[2] / iw])
                                };
                                frag(x, y, iw, c, fb);
                            }
                        });
                    }
                }
                Piece::Top(i, pat) => {
                    let (rings, iw, rgb, hgt) = &tops[*i];
                    raster_polygon(w, h, rings, &full, |x, y| {
                        let c = match pat {
                            Some((Some(img), fi)) if !depth_only => {
                                // get_pattern_pos for a top: a_pos (no translate).
                                let m = view.unproject(x as f64 + 0.5, y as f64 + 0.5, *hgt);
                                let (u, v) = frames[*fi].coord(img, m.0, m.1, (0.0, 0.0));
                                extrusion_texel(*pat, u, v, *rgb)
                            }
                            _ if depth_only => [0.0; 4],
                            _ => extrusion_texel(*pat, 0.0, 0.0, *rgb),
                        };
                        frag(x, y, *iw, c, fb)
                    });
                }
            }
        }
    }
}

/// fill_extrusion(_pattern).fragment.glsl before opacity: the pattern texel
/// at (u, v) repeats times the lighting, or the lit colour.
fn extrusion_texel(pat: Option<(Option<&symbol::SpriteImage>, usize)>, u: f64, v: f64, rgb: [f64; 3]) -> [f64; 4] {
    match pat {
        Some((None, _)) => [0.0; 4],
        Some((Some(img), _)) => {
            let s = img.sample_wrapped(u.rem_euclid(1.0) * img.w as f64, v.rem_euclid(1.0) * img.h as f64);
            [s[0] / 255.0 * rgb[0], s[1] / 255.0 * rgb[1], s[2] / 255.0 * rgb[2], s[3] / 255.0]
        }
        None => [rgb[0], rgb[1], rgb[2], 1.0],
    }
}

/// fill-extrusion as SVG: every visible face (walls facing the camera, tops)
/// as a polygon, sorted far to near (painter's algorithm), walls shaded with
/// their mean vertex colour. The layer opacity goes on the group, so only
/// the nearest surface shows through it, as MapLibre draws translucent
/// extrusions.
fn svg_fill_extrusion(out: &mut String, layer: &Json, zoom: f64, placed: &[PlacedTile], view: &ExtrusionView) {
    use std::fmt::Write as _;
    let p = &layer["paint"];
    let color = Prop::new(p.get("fill-extrusion-color"), Kind::Color, zoom);
    let height = Prop::new(p.get("fill-extrusion-height"), Kind::Number, zoom);
    let base = Prop::new(p.get("fill-extrusion-base"), Kind::Number, zoom);
    let opacity = Prop::new(p.get("fill-extrusion-opacity"), Kind::Number, zoom).num(&Ctx { zoom, feature: None }, 1.0);
    let vertical_gradient = p["fill-extrusion-vertical-gradient"].as_bool().unwrap_or(true);
    if opacity <= 0.0 {
        return;
    }
    // (depth key, path data, colour)
    let mut faces: Vec<(f64, String, [f64; 3])> = Vec::new();
    for pt in placed {
        for (f, scale) in features(layer, pt, zoom) {
            let Geometry::Polygons(polys) = &f.geometry else { continue };
            let ctx = Ctx { zoom, feature: Some(f) };
            let c = color.color(&ctx, Color::BLACK);
            let col = [c.r as f64, c.g as f64, c.b as f64, c.a as f64];
            let hgt = height.num(&ctx, 0.0).max(0.0);
            let bse = base.num(&ctx, 0.0).max(0.0);
            for poly in polys {
                let rings: Vec<Vec<(f64, f64)>> = poly.iter().map(|r| r.iter().map(|&q| to_extent(q, scale)).collect()).collect();
                if rings.first().map_or(true, |r| ring_entirely_outside(r)) {
                    continue;
                }
                let to_px = |q: (f64, f64)| (pt.ox + q.0 * pt.unit_px, pt.oy + q.1 * pt.unit_px);
                for ring in &rings {
                    if ring_entirely_outside(ring) {
                        continue;
                    }
                    for e in ring.windows(2) {
                        let (p1, p2) = (e[1], e[0]);
                        if (p1.0 == p2.0 && (p1.0 < 0.0 || p1.0 > EXTENT)) || (p1.1 == p2.1 && (p1.1 < 0.0 || p1.1 > EXTENT)) {
                            continue;
                        }
                        let (dx, dy) = (p1.0 - p2.0, p1.1 - p2.1);
                        let len = (dx * dx + dy * dy).sqrt();
                        if len == 0.0 {
                            continue;
                        }
                        let n = (-dy / len, ((dx / len) * 16384.0).trunc() / 16384.0, 0.0);
                        let a = view.project(to_px(p1), bse);
                        let b = view.project(to_px(p1), hgt);
                        let c2 = view.project(to_px(p2), bse);
                        let d2 = view.project(to_px(p2), hgt);
                        // Back-face culling, as for the PNG (triangle a, c2, b).
                        let cross = (c2.0 - a.0) * (b.1 - a.1) - (c2.1 - a.1) * (b.0 - a.0);
                        if cross > 0.0 {
                            continue;
                        }
                        let s0 = view.shade(col, n, 0.0, bse, hgt, vertical_gradient);
                        let s1 = view.shade(col, n, 1.0, bse, hgt, vertical_gradient);
                        let rgb = [(s0[0] + s1[0]) / 2.0, (s0[1] + s1[1]) / 2.0, (s0[2] + s1[2]) / 2.0];
                        let quad = vec![(a.0, a.1), (c2.0, c2.1), (d2.0, d2.1), (b.0, b.1)];
                        faces.push(((a.2 + b.2 + c2.2 + d2.2) / 4.0, svg::path_data(&[quad], true), rgb));
                    }
                }
                let top: Vec<Vec<(f64, f64)>> = rings
                    .iter()
                    .map(|r| r.iter().map(|&q| { let (x, y, _) = view.project(to_px(q), hgt); (x, y) }).collect())
                    .collect();
                let (_, _, iw) = view.project((0.0, 0.0), hgt);
                faces.push((iw + 1e-12, svg::path_data(&top, true), view.shade(col, (0.0, 0.0, 1.0), 1.0, bse, hgt, vertical_gradient)));
            }
        }
    }
    faces.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let _ = write!(out, r#"<g opacity="{}" fill-rule="evenodd">"#, svg::alpha(opacity.min(1.0)));
    for (_, d, rgb) in faces {
        let hex = format!(
            "#{:02x}{:02x}{:02x}",
            (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
            (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
            (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8
        );
        let _ = write!(out, r#"<path d="{d}" fill="{hex}"/>"#);
    }
    out.push_str("</g>");
}

// ── Raster ───────────────────────────────────────────────────────────────────

/// raster.fragment.glsl uniforms: saturation, contrast and hue-rotate factors.
fn raster_factors(saturation: f64, contrast: f64, hue_deg: f64) -> (f64, f64, [f64; 3]) {
    let sat = if saturation > 0.0 { 1.0 - 1.0 / (1.001 - saturation) } else { -saturation };
    let con = if contrast > 0.0 { 1.0 / (1.0 - contrast) } else { 1.0 + contrast };
    let a = hue_deg.to_radians();
    let (s, c) = (a.sin(), a.cos());
    let spin = [(2.0 * c + 1.0) / 3.0, (-(3f64.sqrt()) * s - c + 1.0) / 3.0, (3f64.sqrt() * s - c + 1.0) / 3.0];
    (sat, con, spin)
}

/// The view's raster tiles composited at device size (tiles snapped to the
/// CSS pixel grid, linearly resampled, as for raster-only styles), as
/// straight-alpha RGBA. None when nothing could be decoded.
fn raster_image(rt: &RasterTiles, bbox: [f64; 4], w: usize, h: usize, ratio: f64) -> Option<image::RgbaImage> {
    crate::renderer::composite_raster_tiles(&rt.tiles, bbox, rt.zoom, w as u32, h as u32, true, ratio)
        .map_err(|e| log::warn!("raster layer: {e}"))
        .ok()
}

/// A raster layer inside a vector style: raster.fragment.glsl per pixel
/// (opacity, hue rotation, saturation, contrast, brightness range), blended
/// premultiplied like every MapLibre layer.
fn draw_raster_layer(fb: &mut Framebuffer, layer: &Json, zoom: f64, rt: &RasterTiles, bbox: [f64; 4], ratio: f64) {
    let ctx = Ctx { zoom, feature: None };
    let p = &layer["paint"];
    let num = |k: &str, d: f64| Prop::new(p.get(k), Kind::Number, zoom).num(&ctx, d);
    let opacity = num("raster-opacity", 1.0);
    if opacity <= 0.0 {
        return;
    }
    let (sat, con, spin) = raster_factors(num("raster-saturation", 0.0), num("raster-contrast", 0.0), num("raster-hue-rotate", 0.0));
    let (lo, hi) = (num("raster-brightness-min", 0.0), num("raster-brightness-max", 1.0));
    let identity = sat == 0.0 && con == 1.0 && spin == [1.0, 0.0, 0.0] && lo == 0.0 && hi == 1.0;
    let Some(img) = raster_image(rt, bbox, fb.w, fb.h, ratio) else { return };
    for (x, y, px) in img.enumerate_pixels() {
        let a = px[3] as f64 / 255.0;
        if a <= 0.0 {
            continue;
        }
        let mut rgb = [px[0] as f64 / 255.0, px[1] as f64 / 255.0, px[2] as f64 / 255.0];
        if !identity {
            let r = rgb;
            rgb = [
                r[0] * spin[0] + r[1] * spin[1] + r[2] * spin[2],
                r[0] * spin[2] + r[1] * spin[0] + r[2] * spin[1],
                r[0] * spin[1] + r[1] * spin[2] + r[2] * spin[0],
            ];
            let avg = (r[0] + r[1] + r[2]) / 3.0;
            for c in rgb.iter_mut() {
                *c += (avg - *c) * sat;
                *c = (*c - 0.5) * con + 0.5;
                *c = lo + (hi - lo) * *c;
            }
        }
        let a = a * opacity;
        fb.blend(x as usize, y as usize, [(rgb[0] * a) as f32, (rgb[1] * a) as f32, (rgb[2] * a) as f32, a as f32]);
    }
}

// ── Heatmap ──────────────────────────────────────────────────────────────────

/// A heatmap layer (draw_heatmap.ts, flat map):
/// 1. heatmap.vertex/fragment.glsl: every point (every vertex of the
///    feature's geometry, inside its tile) adds a Gaussian kernel
///    `weight · intensity · e^(-4.5 d² / r²) / √(2π)` over a square of half
///    size `S · r` (where the kernel falls to 1/4080) to a half-float texture
///    a quarter of the canvas size, blended additively;
/// 2. heatmap_texture.fragment.glsl: the density texture sampled LINEAR at
///    each canvas pixel, mapped through the 256-texel `heatmap-color` ramp
///    and multiplied by `heatmap-opacity`.
fn draw_heatmap_layer(fb: &mut Framebuffer, layer: &Json, zoom: f64, placed: &[PlacedTile], dpr: f64) {
    use super::heatmap::{color_ramp, default_heatmap_color, sample_ramp, to_f16};
    const ZERO: f32 = 1.0 / 255.0 / 16.0;
    const GAUSS_COEF: f32 = 0.398_942_3;
    let p = &layer["paint"];
    let ctx0 = Ctx { zoom, feature: None };
    let opacity = Prop::new(p.get("heatmap-opacity"), Kind::Number, zoom).num(&ctx0, 1.0);
    if opacity == 0.0 {
        return;
    }
    let intensity = Prop::new(p.get("heatmap-intensity"), Kind::Number, zoom).num(&ctx0, 1.0) as f32;
    let weight = Prop::new(p.get("heatmap-weight"), Kind::Number, zoom);
    let radius = Prop::new(p.get("heatmap-radius"), Kind::Number, zoom);
    let color_expr = p.get("heatmap-color").filter(|j| !j.is_null()).cloned().unwrap_or_else(default_heatmap_color);
    let ramp = color_ramp(&color_expr, "heatmap-density", zoom);

    // The kernel texture: painter.width / 4 × painter.height / 4 (truncated).
    let (w4, h4) = (fb.w / 4, fb.h / 4);
    if w4 == 0 || h4 == 0 {
        return;
    }
    let (sx, sy) = (fb.w as f64 / w4 as f64, fb.h as f64 / h4 as f64);
    let mut density = vec![0f32; w4 * h4];
    for pt in placed.iter().rev() {
        for (f, scale) in features(layer, pt, zoom) {
            let ctx = Ctx { zoom, feature: Some(f) };
            let w = weight.num(&ctx, 1.0) as f32;
            let r = radius.num(&ctx, 30.0) * dpr;
            let s = (-2.0 * (ZERO / w / intensity / GAUSS_COEF).ln()).sqrt() / 3.0;
            if !s.is_finite() || r <= 0.0 {
                continue;
            }
            let k = w * intensity * GAUSS_COEF;
            let half = s as f64 * r;
            let mut points: Vec<(f32, f32)> = Vec::new();
            match &f.geometry {
                Geometry::Points(ps) => points.extend(ps.iter().copied()),
                Geometry::Lines(ls) => ls.iter().for_each(|l| points.extend(l.iter().copied())),
                Geometry::Polygons(polys) => polys.iter().flatten().for_each(|r| points.extend(r.iter().copied())),
            }
            for q in points {
                let (x, y) = to_extent(q, scale);
                if x < 0.0 || x >= EXTENT || y < 0.0 || y >= EXTENT {
                    continue;
                }
                let (cx, cy) = (pt.ox + x * pt.unit_px, pt.oy + y * pt.unit_px);
                // Texels whose centres fall inside the quad.
                let i0 = ((cx - half) / sx - 0.5).ceil().max(0.0) as usize;
                let i1 = (((cx + half) / sx - 0.5).ceil().max(0.0) as usize).min(w4);
                let j0 = ((cy - half) / sy - 0.5).ceil().max(0.0) as usize;
                let j1 = (((cy + half) / sy - 0.5).ceil().max(0.0) as usize).min(h4);
                for j in j0..j1 {
                    let vy = (((j as f64 + 0.5) * sy - cy) / r) as f32;
                    for i in i0..i1 {
                        let vx = (((i as f64 + 0.5) * sx - cx) / r) as f32;
                        let val = k * (-0.5 * 9.0 * (vx * vx + vy * vy)).exp();
                        let d = &mut density[j * w4 + i];
                        *d = to_f16(*d + val);
                    }
                }
            }
        }
    }

    // Colorize: LINEAR sampling of the density texture across the canvas.
    let op = opacity as f32;
    for py in 0..fb.h {
        let t = ((py as f64 + 0.5) / fb.h as f64 * h4 as f64 - 0.5).clamp(0.0, (h4 - 1) as f64);
        let (j0, fy) = (t.floor() as usize, (t - t.floor()) as f32);
        let j1 = (j0 + 1).min(h4 - 1);
        for px in 0..fb.w {
            let s = ((px as f64 + 0.5) / fb.w as f64 * w4 as f64 - 0.5).clamp(0.0, (w4 - 1) as f64);
            let (i0, fx) = (s.floor() as usize, (s - s.floor()) as f32);
            let i1 = (i0 + 1).min(w4 - 1);
            let d = |i: usize, j: usize| density[j * w4 + i];
            let v = (d(i0, j0) * (1.0 - fx) + d(i1, j0) * fx) * (1.0 - fy) + (d(i0, j1) * (1.0 - fx) + d(i1, j1) * fx) * fy;
            let c = sample_ramp(&ramp, v);
            if c[3] > 0.0 || c[0] > 0.0 || c[1] > 0.0 || c[2] > 0.0 {
                fb.blend(px, py, [c[0] * op, c[1] * op, c[2] * op, c[3] * op]);
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
    sources: &SourceTiles,
    assets: &SymbolAssets,
    bbox: [f64; 4],
    width: u32,
    height: u32,
    id_prefix: &str,
    text_mode: super::fonts::SvgText,
    fonts: Option<&super::fonts::SvgFonts>,
) -> Result<SvgParts, String> {
    use std::fmt::Write as _;
    let (zoom, placed) = prepare(sources, bbox, width, height, 1.0);
    let placement = symbol::place(style, &symbol_tiles(&placed), assets, zoom, width as f64, height as f64);
    let mut defs = String::new();
    // Tile clip paths per source: "{prefix}{source number}t{tile index}".
    let mut source_ids: Vec<&String> = placed.keys().collect();
    source_ids.sort();
    let clip_prefix: std::collections::HashMap<&str, String> =
        source_ids.iter().enumerate().map(|(k, id)| (id.as_str(), format!("{id_prefix}{k}"))).collect();
    for id in &source_ids {
        let cp = &clip_prefix[id.as_str()];
        for (i, pt) in placed[*id].iter().enumerate() {
            let _ = write!(
                defs,
                r#"<clipPath id="{cp}t{i}"><rect x="{}" y="{}" width="{}" height="{}" shape-rendering="crispEdges"/></clipPath>"#,
                svg::num(pt.clip.x0),
                svg::num(pt.clip.y0),
                svg::num(pt.clip.x1 - pt.clip.x0),
                svg::num(pt.clip.y1 - pt.clip.y0)
            );
        }
    }
    let mut pattern_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut depth3d: Vec<f64> = Vec::new();
    let layer_clip = |layer: &Json| -> String {
        layer["source"].as_str().and_then(|s| clip_prefix.get(s)).cloned().unwrap_or_default()
    };
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
            "fill" => svg_fill(&mut out, &mut defs, layer, zoom, layer_tiles(&placed, layer), &layer_clip(layer), assets, &mut pattern_ids),
            "line" if layer["paint"].get("line-pattern").map_or(false, |v| !v.is_null()) => {
                // Patterned lines stay raster: the layer drawn alone, embedded.
                let mut lfb = Framebuffer::new(width as usize, height as usize);
                draw_line_layer(&mut lfb, layer, zoom, layer_tiles(&placed, layer), 1.0, assets);
                if lfb.px.iter().any(|p| p[3] > 0 || p[0] > 0 || p[1] > 0 || p[2] > 0) {
                    let px = lfb.into_straight_rgba();
                    let _ = write!(out, r#"<image width="{width}" height="{height}" href="{}"/>"#, svg::png_data_uri(width, height, &px));
                }
            }
            "line" => svg_line(&mut out, layer, zoom, layer_tiles(&placed, layer), &layer_clip(layer)),
            "circle" => svg_circle(&mut out, layer, zoom, layer_tiles(&placed, layer)),
            "fill-extrusion" => {
                let view = ExtrusionView::new(style, bbox, width, height, 1.0, zoom);
                if layer["paint"].get("fill-extrusion-pattern").map_or(false, |v| !v.is_null()) {
                    // Patterned extrusions stay raster: the layer drawn alone, embedded.
                    let mut lfb = Framebuffer::new(width as usize, height as usize);
                    draw_fill_extrusion_layer(&mut lfb, layer, zoom, layer_tiles(&placed, layer), &view, assets, &mut depth3d);
                    if lfb.px.iter().any(|p| p[3] > 0 || p[0] > 0 || p[1] > 0 || p[2] > 0) {
                        let px = lfb.into_straight_rgba();
                        let _ = write!(out, r#"<image width="{width}" height="{height}" href="{}"/>"#, svg::png_data_uri(width, height, &px));
                    }
                } else {
                    svg_fill_extrusion(&mut out, layer, zoom, layer_tiles(&placed, layer), &view);
                }
            }
            "raster" => {
                // Raster tiles stay raster: the composited view, embedded.
                if let Some(rt) = layer["source"].as_str().and_then(|s| sources.raster.get(s)) {
                    let ctx = Ctx { zoom, feature: None };
                    let op = Prop::new(layer["paint"].get("raster-opacity"), Kind::Number, zoom).num(&ctx, 1.0);
                    if let Some(img) = raster_image(rt, bbox, width as usize, height as usize, 1.0) {
                        let _ = write!(
                            out,
                            r#"<image width="{width}" height="{height}" opacity="{}" href="{}"/>"#,
                            svg::alpha(op.clamp(0.0, 1.0)),
                            svg::png_data_uri(width, height, img.as_raw())
                        );
                    }
                }
            }
            "heatmap" => {
                // The colored density stays raster: the layer drawn alone, embedded.
                let mut lfb = Framebuffer::new(width as usize, height as usize);
                draw_heatmap_layer(&mut lfb, layer, zoom, layer_tiles(&placed, layer), 1.0);
                if lfb.px.iter().any(|p| p[3] > 0 || p[0] > 0 || p[1] > 0 || p[2] > 0) {
                    let px = lfb.into_straight_rgba();
                    let _ = write!(out, r#"<image width="{width}" height="{height}" href="{}"/>"#, svg::png_data_uri(width, height, &px));
                }
            }
            "hillshade" => {
                // Shading stays raster: the layer drawn alone, embedded.
                if let Some(dem) = layer["source"].as_str().and_then(|s| sources.dem.get(s)) {
                    let mut lfb = Framebuffer::new(width as usize, height as usize);
                    let nearest = super::hillshade::source_nearest(style, layer["source"].as_str().unwrap_or(""), zoom);
                    super::hillshade::draw_layer(&mut lfb, layer, zoom, dem, bbox, width, height, 1.0, nearest);
                    if lfb.px.iter().any(|p| p[3] > 0 || p[0] > 0 || p[1] > 0 || p[2] > 0) {
                        let px = lfb.into_straight_rgba();
                        let _ = write!(out, r#"<image width="{width}" height="{height}" href="{}"/>"#, svg::png_data_uri(width, height, &px));
                    }
                }
            }
            "symbol" => symbol::svg_layer(&mut out, &mut defs, layer, layer_idx, &placement, assets, width as usize, height as usize, id_prefix, text_mode, fonts),
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
#[allow(clippy::too_many_arguments)]
fn svg_fill(
    out: &mut String,
    defs: &mut String,
    layer: &Json,
    zoom: f64,
    placed: &[PlacedTile],
    prefix: &str,
    assets: &SymbolAssets,
    pattern_ids: &mut std::collections::HashMap<String, String>,
) {
    use std::fmt::Write as _;
    let p = &layer["paint"];
    let color = Prop::new(p.get("fill-color"), Kind::Color, zoom);
    let opacity = Prop::new(p.get("fill-opacity"), Kind::Number, zoom);
    let outline = Prop::new(p.get("fill-outline-color"), Kind::Color, zoom);
    let translate = Prop::new(p.get("fill-translate"), Kind::Array, zoom);
    let antialias = p["fill-antialias"].as_bool().unwrap_or(true);
    let pattern = Prop::new(p.get("fill-pattern"), Kind::Enum, zoom);
    // fill-pattern: an SVG <pattern> per image, aligned to the world grid as
    // the fill_pattern shader tiles it (see PatternFrame), for interiors and
    // the antialiasing outline. Always the translucent pass (outline above).
    let mut pattern_paint = |ctx: &Ctx, pt: &PlacedTile| -> Option<String> {
        let Some(Val::Str(name)) = pattern.get(ctx) else { return None };
        let img = assets.sprite.images.get(&name)?;
        let frame = PatternFrame::new(pt);
        let k = pt.unit_px / frame.tile_ratio; // screen px per CSS px at tile zoom
        let (sw, sh) = (img.w as f64 / img.pixel_ratio * k, img.h as f64 / img.pixel_ratio * k);
        let (x0, y0) = (pt.ox - frame.world_x * k, pt.oy - frame.world_y * k);
        let key = format!("{name}@{}", svg::num(k * 1000.0));
        let n = pattern_ids.len();
        let id = pattern_ids
            .entry(key)
            .or_insert_with(|| {
                let id = format!("{prefix}pat{n}");
                let _ = write!(
                    defs,
                    r#"<pattern id="{id}" patternUnits="userSpaceOnUse" x="{}" y="{}" width="{}" height="{}"><image width="{}" height="{}" preserveAspectRatio="none" href="{}"/></pattern>"#,
                    svg::num(x0.rem_euclid(sw)),
                    svg::num(y0.rem_euclid(sh)),
                    sw,
                    sh,
                    sw,
                    sh,
                    img.png_data_uri()
                );
                id
            })
            .clone();
        Some(format!("url(#{id})"))
    };
    let opaque = matches!(&color, Prop::Const(v) if v.as_color().map_or(false, |c| c.a == 1.0))
        && match &opacity {
            Prop::Absent => true,
            Prop::Const(v) => v.as_num() == Some(1.0),
            Prop::Data(..) => false,
        };
    let outline_beneath = opaque && !outline.is_set() && !pattern.is_set();

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
            if pattern.is_set() {
                // Features whose pattern image is missing are not drawn.
                let Some(paint) = pattern_paint(&ctx, pt) else { continue };
                if op <= 0.0 {
                    continue;
                }
                for poly in &polys {
                    let _ = write!(gf, r#"<path d="{}" fill="{paint}" fill-opacity="{}"/>"#, svg::path_data(poly, true), svg::alpha(op.min(1.0)));
                }
                if antialias {
                    let rings: Vec<Vec<(f64, f64)>> = polys.into_iter().flatten().collect();
                    let stroke = match outline.is_set().then(|| svg::paint(outline.color(&ctx, Color::BLACK), op)).flatten() {
                        Some((hex, a)) => format!(r#"stroke="{hex}" stroke-opacity="{}""#, svg::alpha(a)),
                        None => format!(r#"stroke="{paint}" stroke-opacity="{}""#, svg::alpha(op.min(1.0))),
                    };
                    let _ = write!(go, r#"<path d="{}" {stroke}/>"#, svg::path_data(&rings, true));
                }
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
    let blur = Prop::new(p.get("line-blur"), Kind::Number, zoom);
    let cap_s = super::expr::layout_enum(l, "line-cap", zoom.floor()).unwrap_or_else(|| "butt".into());
    let join_s = super::expr::layout_enum(l, "line-join", zoom.floor()).unwrap_or_else(|| "miter".into());
    let (cap, join) = (cap_s.as_str(), join_s.as_str());
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
                // floor(zoom). Round caps are used as-is: compared against
                // MapLibre, untrimmed SVG round dashes match best.
                // The pattern is laid out in pixels at the tile's integer zoom
                // (tileRatio), so it stretches with the fractional zoom.
                let fw = floor_width.num(&fctx, lw) * pt.unit_px * EXTENT / (TILE_PX * pt.overscaling);
                let vals: Vec<String> = pattern.iter().map(|&v| svg::num(v * fw)).collect();
                let offset = phase * fw;
                dash_attr = format!(r#" stroke-dasharray="{}""#, vals.join(" "));
                if offset != 0.0 {
                    dash_attr += &format!(r#" stroke-dashoffset="{}""#, svg::num(offset));
                }
            }
            // line.fragment.glsl ramps alpha over one pixel centred on each
            // edge, so a line of width w < 1 covers (w + 1)^2 / 4 px, not w:
            // stroke at the width with that coverage.
            let stroke_w = if lw < 1.0 { (lw + 1.0).powi(2) / 4.0 } else { lw };
            // line-blur widens the edge ramp to blur + 1 px (blur2), centred
            // half the blur inside the edge: nested strokes whose edges spread
            // over the blur (the rasteriser's 1 px antialiasing adds the
            // rest), with opacities stacking to the line's alpha.
            let b = blur.num(&ctx, 0.0).max(0.0);
            let bands: Vec<(f64, f64)> = if b > 0.0 {
                let n = 3;
                let mut below = 0.0;
                (1..=n)
                    .map(|k| {
                        let y = (k as f64 - 0.5) / n as f64;
                        let half = (lw / 2.0 - b / 2.0 + b * (0.5 - y)).max(0.05);
                        let target = a * k as f64 / n as f64;
                        let layer = if below >= 1.0 { 0.0 } else { ((target - below) / (1.0 - below)).clamp(0.0, 1.0) };
                        below = target;
                        (2.0 * half, layer)
                    })
                    .collect()
            } else {
                vec![(stroke_w, a)]
            };
            // One path per line: MapLibre blends each line on its own, so
            // translucent lines darken where they overlap.
            for (sw, op) in bands {
                if op <= 0.0 {
                    continue;
                }
                let _ = write!(
                    g,
                    r#"<g fill="none" stroke="{hex}" stroke-opacity="{}" stroke-width="{}" stroke-linecap="{cap}" stroke-linejoin="{join}" stroke-miterlimit="{}"{dash_attr}>"#,
                    svg::alpha(op),
                    svg::num(sw),
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
