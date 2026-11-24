//! The compiled core of the R package.
//!
//! The R package is an adapter (docs/design/api-review.md §4.1): its
//! objects are plain values, and every function here turns them into the
//! core's objects, so validation, view resolution, rendering and notes are
//! the core's, the same as in Python and Rust. R adds only what needs R:
//! CRSs and geometries (sf), graphics devices, ggplot2 and tmap.
//!
//! Values cross as JSON (specs: styles, view inputs, fetch options, frames,
//! panels; results: views, notes, layer lists) and as plain vectors (the
//! bulk data: pixels, mesh coordinates, clip rings). Named R lists are not
//! read or built here: extendr reaches their names through R symbols that
//! are not reliable in every build (a crash on Windows with the GNU
//! toolchain), and JSON is as fast for these small values.
use basemapper_core::view::{self, CrsFrame};
use basemapper_core::{
    providers, Basemap, BasemapError, Context, Device, FetchOptions, FontEmbed, Keep, Mesh, Note, OnError, Panel,
    PanelCrs, Rendering, Shape, Source, Style, SvgOptions, SvgText, TileKind, TileStyleOptions, Units, View,
    ViewInputs, Warp,
};
use extendr_api::prelude::*;
use extendr_api::Result;
use serde_json::{json, Value};

thread_local! {
    static QUIET_PANIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// ── Errors ───────────────────────────────────────────────────────────────────

/// An input error found here (a validation error).
fn err(e: impl std::fmt::Display) -> extendr_api::Error {
    kind_err("validation", e)
}

/// A core error, with its kind (validation, style, network, render).
fn core_err(e: BasemapError) -> extendr_api::Error {
    kind_err(e.kind(), e)
}

/// An error for R, its message prefixed with its kind (`[network] ...`):
/// the R side raises it as a condition of class `basemapper_<kind>_error`
/// (see `call_core()` in R/errors.R).
///
/// extendr raises a returned `Err` as a panic that it catches and turns
/// into an R error; the default panic hook would also print "thread
/// panicked at ..." to the console. Errors made here mark the next panic on
/// this thread as expected, which the hook then keeps quiet; real panics
/// still print.
fn kind_err(kind: &str, e: impl std::fmt::Display) -> extendr_api::Error {
    static HOOK: std::sync::Once = std::sync::Once::new();
    HOOK.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !QUIET_PANIC.with(|q| q.replace(false)) {
                default(info);
            }
        }));
    });
    QUIET_PANIC.with(|q| q.set(true));
    extendr_api::Error::Other(format!("[{kind}] {e}"))
}

// ── Reading values ───────────────────────────────────────────────────────────

fn parse(json: &str, what: &str) -> Result<Value> {
    serde_json::from_str(json).map_err(|e| err(format!("{what}: {e}")))
}

/// `v[key]`, or None when missing or null.
fn item<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.get(key).filter(|x| !x.is_null())
}

/// A JSON value R may have written as a length-one array.
fn scalar(v: &Value) -> Option<&Value> {
    match v {
        Value::Array(a) if a.len() == 1 => a.first(),
        other => Some(other),
    }
}

fn num(v: &Value, key: &str) -> Result<Option<f64>> {
    item(v, key).map(|x| scalar(x).and_then(Value::as_f64).ok_or_else(|| err(format!("{key} must be a number")))).transpose()
}

fn string(v: &Value, key: &str) -> Result<Option<String>> {
    item(v, key)
        .map(|x| scalar(x).and_then(Value::as_str).map(str::to_owned).ok_or_else(|| err(format!("{key} must be a string"))))
        .transpose()
}

fn flag(v: &Value, key: &str) -> Result<Option<bool>> {
    item(v, key).map(|x| scalar(x).and_then(Value::as_bool).ok_or_else(|| err(format!("{key} must be TRUE or FALSE")))).transpose()
}

/// Strings: one (R's auto-unboxed length-one vector) or an array.
fn strings(v: &Value, key: &str) -> Result<Option<Vec<String>>> {
    let bad = || err(format!("{key} must be strings"));
    match item(v, key) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(vec![s.clone()])),
        Some(Value::Array(a)) => a.iter().map(|x| x.as_str().map(str::to_owned).ok_or_else(bad)).collect::<Result<_>>().map(Some),
        Some(_) => Err(bad()),
    }
}

fn nums(v: &Value, key: &str) -> Result<Option<Vec<f64>>> {
    let bad = || err(format!("{key} must be numbers"));
    match item(v, key) {
        None => Ok(None),
        Some(Value::Array(a)) => a.iter().map(|x| x.as_f64().ok_or_else(bad)).collect::<Result<_>>().map(Some),
        Some(x) => x.as_f64().map(|n| Some(vec![n])).ok_or_else(bad),
    }
}

fn required<T>(v: Option<T>, what: &str) -> Result<T> {
    v.ok_or_else(|| err(format!("{what} is required")))
}

fn pair(v: Vec<f64>, what: &str) -> Result<(f64, f64)> {
    match v.as_slice() {
        [a, b] => Ok((*a, *b)),
        _ => Err(err(format!("{what} must be two numbers"))),
    }
}

fn four(v: Vec<f64>, what: &str) -> Result<[f64; 4]> {
    <[f64; 4]>::try_from(v.as_slice()).map_err(|_| err(format!("{what} must be four numbers")))
}

fn small(v: Option<f64>, what: &str) -> Result<Option<u8>> {
    v.map(|x| if (0.0..=255.0).contains(&x) { Ok(x as u8) } else { Err(err(format!("{what} out of range"))) }).transpose()
}

fn kind_of(s: Option<String>) -> Result<Option<TileKind>> {
    s.map(|k| TileKind::parse(&k)).transpose().map_err(core_err)
}

/// A style spec (`{source: {kind, ...}, layers, features, key}`) as the
/// core's `Style`, validated.
fn style_of(spec: &Value) -> Result<Style> {
    let src = required(item(spec, "source"), "a style's source")?;
    let kind = string(src, "kind")?.unwrap_or_else(|| "auto".into());
    let source = match kind.as_str() {
        "auto" => Source::from(required(string(src, "value")?, "the style")?),
        "provider" => {
            Source::provider(required(string(src, "name")?, "provider(name)")?, kind_of(string(src, "form")?)?, string(src, "tiles")?)
        }
        "tiles" => {
            let paint: serde_json::Map<String, Value> = match string(src, "paint")? {
                Some(json) => serde_json::from_str(&json).map_err(|e| err(format!("paint must be a named list: {e}")))?,
                None => serde_json::Map::new(),
            };
            Source::tiles(
                required(string(src, "input")?, "tiles(input)")?,
                TileStyleOptions {
                    kind: kind_of(string(src, "type")?)?,
                    tile_size: num(src, "tile_size")?.map(|v| v as u32),
                    scheme: string(src, "scheme")?,
                    minzoom: small(num(src, "minzoom")?, "minzoom")?,
                    maxzoom: small(num(src, "maxzoom")?, "maxzoom")?,
                    attribution: string(src, "attribution")?,
                    source_layers: strings(src, "source_layers")?,
                    paint,
                },
            )
        }
        "wms" => {
            let mut s = Source::wms(required(string(src, "url")?, "wms(url)")?, required(string(src, "layers")?, "wms(layers)")?);
            if let Source::Wms { styles, version, format, transparent, extra, .. } = &mut s {
                if let Some(v) = string(src, "styles")? {
                    *styles = v;
                }
                if let Some(v) = string(src, "version")? {
                    *version = v;
                }
                if let Some(v) = string(src, "format")? {
                    *format = v;
                }
                if let Some(v) = flag(src, "transparent")? {
                    *transparent = v;
                }
                *extra = string(src, "extra_params")?;
            }
            s
        }
        "wmts" => {
            let mut s = Source::wmts(required(string(src, "url")?, "wmts(url)")?, required(string(src, "layer")?, "wmts(layer)")?);
            if let Source::Wmts { tilematrixset, format, style, version, extra, .. } = &mut s {
                if let Some(v) = string(src, "tilematrixset")? {
                    *tilematrixset = v;
                }
                if let Some(v) = string(src, "format")? {
                    *format = v;
                }
                if let Some(v) = string(src, "style")? {
                    *style = v;
                }
                if let Some(v) = string(src, "version")? {
                    *version = v;
                }
                *extra = string(src, "extra_params")?;
            }
            s
        }
        other => return Err(err(format!("unknown style source kind {other:?}"))),
    };
    let mut style = Style::new(source);
    if let Some(l) = strings(spec, "layers")? {
        style = style.layers(l);
    }
    if let Some(f) = strings(spec, "features")? {
        style = style.features(f);
    }
    if let Some(k) = string(spec, "key")? {
        style = style.key(k);
    }
    style.validate().map_err(core_err)?;
    Ok(style)
}

fn styles_of(json: &str) -> Result<Vec<Style>> {
    let specs = parse(json, "styles")?;
    let specs = specs.as_array().ok_or_else(|| err("styles must be a list"))?;
    if specs.is_empty() {
        return Err(err("a basemap needs at least one style"));
    }
    specs.iter().map(style_of).collect()
}

/// View inputs (`bounds` in Web Mercator or the frame's CRS, `center`
/// lon/lat, `zoom`, `width`, `height`, `units`, `dpi`).
fn inputs_of(json: &str) -> Result<ViewInputs> {
    let l = parse(json, "view inputs")?;
    let mut v = ViewInputs::new();
    if let Some(b) = nums(&l, "bounds")? {
        v = v.bounds(four(b, "bounds")?);
    }
    if let Some(c) = nums(&l, "center")? {
        let (lon, lat) = pair(c, "center")?;
        v = v.center(lon, lat);
    }
    if let Some(z) = num(&l, "zoom")? {
        v = v.zoom(z);
    }
    match (num(&l, "width")?, num(&l, "height")?) {
        (Some(w), Some(h)) => {
            let units = Units::parse(&string(&l, "units")?.unwrap_or_else(|| "px".into())).map_err(core_err)?;
            v = v.size(w, h, units);
        }
        (None, None) => {}
        _ => return Err(err("give both width and height, or neither")),
    }
    if let Some(d) = num(&l, "dpi")? {
        v = v.dpi(d);
    }
    Ok(v)
}

fn fetch_of(json: &str) -> Result<FetchOptions> {
    let l = parse(json, "fetch options")?;
    let mut f = FetchOptions::new();
    if let Some(t) = num(&l, "timeout")? {
        if !(t > 0.0 && t.is_finite()) {
            return Err(err("timeout must be a number of seconds > 0"));
        }
        f = f.timeout(std::time::Duration::from_secs_f64(t));
    }
    if let Some(n) = num(&l, "max_tiles")? {
        f = f.max_tiles(n as u32);
    }
    if let Some(n) = num(&l, "concurrency")? {
        f = f.concurrency(n as u32);
    }
    if let Some(e) = string(&l, "on_error")? {
        f = f.on_error(OnError::parse(&e).map_err(core_err)?);
    }
    f.validate().map_err(core_err)?;
    Ok(f)
}

/// A clip, flat: each polygon's ring count, each ring's point count, the
/// points' x and y (in rings order), and the side kept. No clip when
/// `keep` is empty.
fn clip_of(keep: &str, rings_per_polygon: &[i32], points_per_ring: &[i32], x: &[f64], y: &[f64]) -> Result<Option<Shape>> {
    let keep = match keep {
        "" => return Ok(None),
        "inside" => Keep::Inside,
        "outside" => Keep::Outside,
        other => return Err(err(format!("clip keep must be \"inside\" or \"outside\", not {other:?}"))),
    };
    if x.len() != y.len() || points_per_ring.iter().map(|&n| n.max(0) as usize).sum::<usize>() != x.len() {
        return Err(err("the clip's ring sizes do not match its points"));
    }
    let mut rings = points_per_ring.iter();
    let mut at = 0usize;
    let mut polygons = Vec::with_capacity(rings_per_polygon.len());
    for &n_rings in rings_per_polygon {
        let mut polygon = Vec::with_capacity(n_rings.max(0) as usize);
        for _ in 0..n_rings {
            let n = *rings.next().ok_or_else(|| err("the clip has fewer rings than its polygons say"))? as usize;
            polygon.push((at..at + n).map(|i| (x[i], y[i])).collect());
            at += n;
        }
        polygons.push(polygon);
    }
    Shape::new(polygons, keep).map(Some).map_err(core_err)
}

/// A mesh: `nx` x `ny` nodes, row-major from the top-left, NA where a node
/// has no Web Mercator position. None when `nx` is 0.
fn mesh_of(nx: i32, ny: i32, x: Vec<f64>, y: Vec<f64>) -> Result<Option<Mesh>> {
    if nx <= 0 {
        return Ok(None);
    }
    if x.len() != y.len() {
        return Err(err("the mesh's x and y differ in length"));
    }
    Mesh::new(nx as usize, ny.max(0) as usize, x.into_iter().zip(y).collect()).map(Some).map_err(core_err)
}

fn frame_of(json: &str) -> Result<Option<CrsFrame>> {
    if json.is_empty() {
        return Ok(None);
    }
    let f = parse(json, "a CRS frame")?;
    CrsFrame::new(
        required(string(&f, "name")?, "the frame's CRS name")?,
        pair(required(nums(&f, "origin")?, "the frame's origin")?, "origin")?,
        pair(required(nums(&f, "origin_3857")?, "the frame's Web Mercator origin")?, "origin_3857")?,
        required(num(&f, "scale")?, "the frame's scale")?,
    )
    .map(Some)
    .map_err(core_err)
}

fn panel_of(json: &str, mesh: Option<Mesh>) -> Result<Option<Panel>> {
    if json.is_empty() {
        return Ok(None);
    }
    let p = parse(json, "a panel")?;
    let name = required(string(&p, "crs")?, "the panel's CRS")?;
    let crs = match name.as_str() {
        "EPSG:3857" => PanelCrs::WebMercator,
        "EPSG:4326" => PanelCrs::LonLat,
        _ => PanelCrs::Other { name: name.clone(), mesh: mesh.ok_or_else(|| err(format!("a panel in {name} needs a mesh")))? },
    };
    let d = required(item(&p, "device"), "the panel's device")?;
    let device = match string(d, "kind")?.as_deref() {
        Some("screen") => Device::Screen { pixel_ratio: num(d, "pixel_ratio")?.unwrap_or(1.0) },
        Some("raster_export") => Device::RasterExport { dpi: required(num(d, "dpi")?, "a raster export's dpi")? },
        Some("vector_export") => Device::VectorExport,
        other => return Err(err(format!("unknown device kind {other:?}"))),
    };
    let (w, h) = pair(required(nums(&p, "px")?, "the panel's size")?, "the panel's size")?;
    Panel::new(
        four(required(nums(&p, "limits")?, "the panel's limits")?, "limits")?,
        crs,
        (w.round().max(1.0) as u32, h.round().max(1.0) as u32),
        device,
    )
    .map(Some)
    .map_err(core_err)
}

// ── Writing results ──────────────────────────────────────────────────────────

/// Notes as parallel arrays (R makes the data frame).
fn notes_json(notes: &[Note]) -> Value {
    json!({
        "level": notes.iter().map(|n| n.level.name()).collect::<Vec<_>>(),
        "kind": notes.iter().map(|n| n.kind).collect::<Vec<_>>(),
        "message": notes.iter().map(|n| n.message.as_str()).collect::<Vec<_>>(),
        "count": notes.iter().map(|n| n.count).collect::<Vec<_>>(),
        "details": notes.iter().map(|n| n.details.clone()).collect::<Vec<_>>(),
    })
}

fn view_json(v: &View, crs_bounds: Option<[f64; 4]>, warp: Option<(String, [f64; 4])>) -> String {
    let (lon, lat) = v.center();
    json!({
        "bounds": v.bounds, "width": v.width, "height": v.height, "dpi": v.dpi, "zoom": v.zoom,
        "scale": v.scale(), "center": [lon, lat],
        "provenance": {
            "extent": v.provenance.extent.name(), "size": v.provenance.size.name(),
            "dpi": v.provenance.dpi.name(), "zoom": v.provenance.zoom.name(),
        },
        "notes": notes_json(&v.notes),
        "decided": notes_json(&v.decided()),
        "crs_bounds": crs_bounds,
        "warp_crs": warp.as_ref().map(|w| w.0.clone()),
        "warp_bounds": warp.map(|w| w.1),
    })
    .to_string()
}

/// A rendering: its pixels as a raw vector, the rest as JSON in its
/// `meta` attribute.
fn rendering_robj(r: Rendering) -> Result<Robj> {
    let meta = json!({
        "width": r.width, "height": r.height, "bounds": r.bounds, "crs": r.crs, "zoom": r.zoom,
        "tile_zoom": r.tile_zoom, "dpi": r.dpi, "scale": r.scale(), "notes": notes_json(&r.notes),
    })
    .to_string();
    let mut out: Robj = Raw::from_bytes(&r.pixels).into();
    out.set_attrib("meta", meta)?;
    Ok(out)
}

fn svg_options(text: &str, embed_fonts: &str, font_dirs: Vec<String>) -> Result<SvgOptions> {
    let mut opts = SvgOptions::default();
    opts.text = match text {
        "editable" => SvgText::Editable,
        "bitmap" => SvgText::Bitmap,
        "paths" => SvgText::Paths,
        _ => return Err(err("text must be \"editable\", \"bitmap\" or \"paths\"")),
    };
    opts.embed_fonts = match embed_fonts {
        "subset" => FontEmbed::Subset,
        "full" => FontEmbed::Full,
        "none" => FontEmbed::None,
        _ => return Err(err("embed_fonts must be \"subset\", \"full\" or \"none\"")),
    };
    let mut d: Vec<std::path::PathBuf> = font_dirs.into_iter().map(Into::into).collect();
    d.extend(opts.font_dirs);
    opts.font_dirs = d;
    Ok(opts)
}

// ── Functions ────────────────────────────────────────────────────────────────

/// Check a style spec (its source and filters) without fetching anything.
#[extendr]
fn check_style_raw(spec: &str) -> Result<bool> {
    style_of(&parse(spec, "a style")?).map(|_| true)
}

/// Resolve a standalone view: Web Mercator, or with a `frame` (JSON, ""
/// for none) a map in that frame's CRS (then also `crs_bounds`).
#[extendr]
fn resolve_raw(inputs: &str, frame: &str) -> Result<String> {
    let inputs = inputs_of(inputs)?;
    match frame_of(frame)? {
        None => Ok(view_json(&view::resolve(&inputs, &Context::Standalone).map_err(core_err)?, None, None)),
        Some(f) => {
            let (v, crs_bounds) = view::resolve_in_crs(&inputs, &f).map_err(core_err)?;
            Ok(view_json(&v, Some(crs_bounds), None))
        }
    }
}

/// The view for a plot `panel` (and its mesh, for a CRS other than Web
/// Mercator and lon/lat; `nx` 0 for none).
#[extendr]
fn plan_raw(inputs: &str, panel: &str, nx: i32, ny: i32, mx: Vec<f64>, my: Vec<f64>) -> Result<String> {
    let p = panel_of(panel, mesh_of(nx, ny, mx, my)?)?.ok_or_else(|| err("a panel is required"))?;
    let (v, warp) = basemapper_core::plot::plan(&p, &inputs_of(inputs)?).map_err(core_err)?;
    Ok(view_json(&v, None, warp.map(|w| (w.crs.clone(), w.bounds))))
}

/// Render: standalone in Web Mercator; with a `frame` and its mesh, in the
/// frame's CRS (the mesh built over the `crs_bounds` that `resolve_raw`
/// gave, for its pixel size); with a `panel` (and its mesh, for a CRS other
/// than Web Mercator and lon/lat), for a plot panel.
#[extendr]
#[allow(clippy::too_many_arguments)]
fn render_raw(
    styles: &str,
    inputs: &str,
    fetch: &str,
    clip_keep: &str,
    clip_rings: Vec<i32>,
    clip_points: Vec<i32>,
    clip_x: Vec<f64>,
    clip_y: Vec<f64>,
    frame: &str,
    nx: i32,
    ny: i32,
    mx: Vec<f64>,
    my: Vec<f64>,
    panel: &str,
) -> Result<Robj> {
    let mut m = Basemap::stack(styles_of(styles)?).with_inputs(inputs_of(inputs)?).fetch(fetch_of(fetch)?);
    if let Some(c) = clip_of(clip_keep, &clip_rings, &clip_points, &clip_x, &clip_y)? {
        m = m.clip(c);
    }
    let mesh = mesh_of(nx, ny, mx, my)?;
    let r = if let Some(p) = panel_of(panel, mesh.clone())? {
        m.render_panel(&p)
    } else if let Some(f) = frame_of(frame)? {
        let (view, crs_bounds) = view::resolve_in_crs(m.inputs(), &f).map_err(core_err)?;
        let mesh = required(mesh, "a warp mesh for a map not in Web Mercator")?;
        let warp = Warp::new(f.name.clone(), crs_bounds, mesh).map_err(core_err)?;
        m.render_view(&view, Some(&warp))
    } else {
        m.render(&Context::Standalone)
    };
    rendering_robj(r.map_err(core_err)?)
}

/// An SVG document (Web Mercator only: with a `frame`, the core's error for
/// a map in another CRS).
#[extendr]
#[allow(clippy::too_many_arguments)]
fn render_svg_raw(
    styles: &str,
    inputs: &str,
    fetch: &str,
    clip_keep: &str,
    clip_rings: Vec<i32>,
    clip_points: Vec<i32>,
    clip_x: Vec<f64>,
    clip_y: Vec<f64>,
    frame: &str,
    text: &str,
    embed_fonts: &str,
    font_dirs: Vec<String>,
) -> Result<String> {
    let mut m = Basemap::stack(styles_of(styles)?).with_inputs(inputs_of(inputs)?).fetch(fetch_of(fetch)?);
    if let Some(c) = clip_of(clip_keep, &clip_rings, &clip_points, &clip_x, &clip_y)? {
        m = m.clip(c);
    }
    let opts = svg_options(text, embed_fonts, font_dirs)?;
    match frame_of(frame)? {
        None => m.render_svg(&Context::Standalone, &opts),
        Some(f) => {
            let (view, crs_bounds) = view::resolve_in_crs(m.inputs(), &f).map_err(core_err)?;
            let b = view.bounds;
            let mesh = Mesh::new(2, 2, vec![(b[0], b[3]), (b[2], b[3]), (b[0], b[1]), (b[2], b[1])]).map_err(core_err)?;
            let warp = Warp::new(f.name.clone(), crs_bounds, mesh).map_err(core_err)?;
            m.render_svg_view(&view, Some(&warp), &opts)
        }
    }
    .map_err(core_err)
}

/// The layers a style spec draws, with its filters (JSON columns).
#[extendr]
fn list_layers_raw(spec: &str) -> Result<String> {
    let rows = basemapper_core::list_style_layers(&style_of(&parse(spec, "a style")?)?).map_err(core_err)?;
    Ok(json!({
        "id": rows.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
        "type": rows.iter().map(|l| l.kind.as_str()).collect::<Vec<_>>(),
        "source_layer": rows.iter().map(|l| l.source_layer.clone()).collect::<Vec<_>>(),
        "feature": rows.iter().map(|l| l.feature.as_str()).collect::<Vec<_>>(),
    })
    .to_string())
}

/// The attribution text of style specs (stacked), joined with " | ".
#[extendr]
fn attribution_raw(styles: &str) -> Result<String> {
    Basemap::stack(styles_of(styles)?).attribution().map_err(core_err)
}

/// The credit line's layout in a plot `width_pt` points wide (JSON), or "".
#[extendr]
fn attribution_layout_raw(text: &str, width_pt: f64) -> String {
    basemapper_core::plot::attribution_layout(text, width_pt)
        .map(|a| {
            json!({
                "lines": a.lines, "font_size_pt": a.font_size_pt, "color": a.color,
                "background": a.background, "background_alpha": a.background_alpha, "padding_pt": a.padding_pt,
            })
            .to_string()
        })
        .unwrap_or_default()
}

/// `c(west, south, east, north)` (lon/lat) of `span` (width, height in
/// `units`) on the ground around `center`.
#[extendr]
fn around_raw(center: Vec<f64>, span: Vec<f64>, units: &str) -> Result<Vec<f64>> {
    let per = view::distance_unit(units).map_err(core_err)?;
    let (w, h) = pair(span, "span")?;
    view::around(pair(center, "center")?, (w * per, h * per)).map(|b| b.to_vec()).map_err(core_err)
}

/// Longitude / latitude points as Web Mercator: `c(x, y)`, x then y.
#[extendr]
fn lonlat_to_3857_raw(lon: Vec<f64>, lat: Vec<f64>) -> Vec<f64> {
    let (x, y): (Vec<f64>, Vec<f64>) = lon.iter().zip(&lat).map(|(&a, &b)| view::lonlat_to_mercator(a, b)).unzip();
    x.into_iter().chain(y).collect()
}

/// Web Mercator points as longitude / latitude: `c(lon, lat)`, lon then lat.
#[extendr]
fn mercator_to_lonlat_raw(x: Vec<f64>, y: Vec<f64>) -> Vec<f64> {
    let (lon, lat): (Vec<f64>, Vec<f64>) = x.iter().zip(&y).map(|(&a, &b)| view::mercator_to_lonlat(a, b)).unzip();
    lon.into_iter().chain(lat).collect()
}

/// Encode straight-alpha RGBA pixels as PNG bytes.
#[extendr]
fn encode_png_raw(pixels: Raw, width: i32, height: i32) -> Result<Raw> {
    let px = pixels.as_slice();
    let (w, h) = (u32::try_from(width).map_err(err)?, u32::try_from(height).map_err(err)?);
    if px.len() != w as usize * h as usize * 4 {
        return Err(err("pixel buffer size does not match width * height * 4"));
    }
    basemapper_core::internal::encode_png(px, w, h).map(|b| Raw::from_bytes(&b)).map_err(core_err)
}

/// The catalogue as JSON columns: name, vector, raster, API key variable,
/// attribution. `kind`: "" for all.
#[extendr]
fn list_providers_raw(kind: &str) -> Result<String> {
    let kind = if kind.is_empty() { None } else { Some(kind.to_owned()) };
    let rows = providers::list_providers(kind_of(kind)?);
    Ok(json!({
        "name": rows.iter().map(|p| p.name).collect::<Vec<_>>(),
        "vector": rows.iter().map(|p| p.kinds().contains(&TileKind::Vector)).collect::<Vec<_>>(),
        "raster": rows.iter().map(|p| p.raster.is_some()).collect::<Vec<_>>(),
        "api_key_env": rows.iter().map(|p| p.key.map(|k| k.env)).collect::<Vec<_>>(),
        "attribution": rows.iter().map(|p| p.attribution).collect::<Vec<_>>(),
    })
    .to_string())
}

/// The tile cache directory ("" when off); `dir` moves it ("off" turns it
/// off, "" leaves it), `clear` deletes the cached tiles, `max_size_mb` sets
/// the limit (0 leaves it).
#[extendr]
fn tile_cache_raw(dir: &str, clear: bool, max_size_mb: f64) -> Result<String> {
    match dir {
        "" => {}
        "off" => basemapper_core::cache::set_dir(None),
        d => basemapper_core::cache::set_dir(Some(d.into())),
    }
    if max_size_mb != 0.0 {
        if !(max_size_mb > 0.0) {
            return Err(err("max_size must be a positive number of megabytes"));
        }
        basemapper_core::cache::set_max_size((max_size_mb * 1024.0 * 1024.0) as u64);
    }
    if clear {
        basemapper_core::cache::clear().map_err(|e| kind_err("render", format!("could not clear the tile cache: {e}")))?;
    }
    Ok(basemapper_core::cache::dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default())
}

extendr_module! {
    mod basemapper;
    fn check_style_raw;
    fn resolve_raw;
    fn plan_raw;
    fn render_raw;
    fn render_svg_raw;
    fn list_layers_raw;
    fn attribution_raw;
    fn attribution_layout_raw;
    fn around_raw;
    fn lonlat_to_3857_raw;
    fn mercator_to_lonlat_raw;
    fn encode_png_raw;
    fn list_providers_raw;
    fn tile_cache_raw;
}
