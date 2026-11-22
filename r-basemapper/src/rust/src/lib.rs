//! The compiled core of the R package. The R functions (`basemap()`,
//! `render_basemap_raw()`, the providers, ...) wrap these through `.Call()`;
//! the same core backs the Python package, so both return the same results
//! for the same call.
use basemapper_core::{providers, render_stack, render_svg_stack_with, FontEmbed, RenderRequest, StyleInput, SvgOptions, SvgText};
use extendr_api::prelude::*;
use extendr_api::Result;

thread_local! {
    static QUIET_PANIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// An error for R. extendr raises a returned `Err` as a panic that it
/// catches and turns into an R error; the default panic hook would also
/// print "thread panicked at ..." to the console. Errors made here mark
/// the next panic on this thread as expected, which the hook then keeps
/// quiet; real panics still print.
fn err(e: impl std::fmt::Display) -> extendr_api::Error {
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
    extendr_api::Error::Other(e.to_string())
}

fn opt<T>(v: Nullable<T>) -> Option<T> {
    match v {
        Nullable::NotNull(x) => Some(x),
        Nullable::Null => None,
    }
}

/// One request per style (bottom to top) over the same view.
#[allow(clippy::too_many_arguments)]
fn stack_requests(
    bbox_3857: &[f64],
    width: i32,
    height: i32,
    styles: Vec<String>,
    pixel_ratio: f64,
    tile_zoom: Nullable<i32>,
    tile_timeout_ms: i32,
    max_tiles: i32,
    layers: Nullable<Vec<String>>,
    labels: &str,
    features: Nullable<Vec<String>>,
    auth_token: Nullable<String>,
    fail_on_tile_error: bool,
    tile_concurrency: i32,
) -> Result<Vec<RenderRequest>> {
    let labels = basemapper_core::Labels::parse(labels).map_err(err)?;
    let features = opt(features);
    if bbox_3857.len() != 4 {
        return Err(err("bounds must have exactly 4 elements"));
    }
    if styles.is_empty() {
        return Err(err("at least one style is required"));
    }
    // Not clamped: the core rejects out-of-range tile zooms with a clear error.
    let tile_zoom = opt(tile_zoom).map(|z| z as u8);
    let layers = opt(layers);
    let auth_token = opt(auth_token);
    Ok(styles
        .iter()
        .map(|s| RenderRequest {
            bbox: [bbox_3857[0], bbox_3857[1], bbox_3857[2], bbox_3857[3]],
            width: width as u32,
            height: height as u32,
            style_input: StyleInput::detect(s),
            tile_zoom,
            tile_timeout_ms: tile_timeout_ms as u32,
            max_tiles: max_tiles as u32,
            tile_concurrency: tile_concurrency.max(1) as u32,
            layers: layers.clone(),
            labels,
            features: features.clone(),
            auth_token: auth_token.clone(),
            fail_on_tile_error,
            pixel_ratio,
        })
        .collect())
}

/// Render styles stacked bottom to top; width/height are CSS pixels and the
/// image is pixel_ratio times larger.
/// @return Raw RGBA pixels with integer attributes `width` and `height`
///   (device pixels). (A list holding the raw vector is not used: building it
///   with `list!` crashed R, the raw vector being unprotected meanwhile.)
#[extendr]
#[allow(clippy::too_many_arguments)]
fn render_raw(
    bbox_3857: Vec<f64>,
    width: i32,
    height: i32,
    styles: Vec<String>,
    pixel_ratio: f64,
    tile_zoom: Nullable<i32>,
    tile_timeout_ms: i32,
    max_tiles: i32,
    layers: Nullable<Vec<String>>,
    labels: &str,
    features: Nullable<Vec<String>>,
    auth_token: Nullable<String>,
    fail_on_tile_error: bool,
    tile_concurrency: i32,
) -> Result<Robj> {
    let reqs = stack_requests(
        &bbox_3857, width, height, styles, pixel_ratio, tile_zoom, tile_timeout_ms, max_tiles, layers, labels, features,
        auth_token, fail_on_tile_error, tile_concurrency,
    )?;
    let r = render_stack(reqs).map_err(err)?;
    let mut out: Robj = Raw::from_bytes(&r.pixels).into();
    out.set_attrib("width", r.width as i32)?;
    out.set_attrib("height", r.height as i32)?;
    Ok(out)
}

/// Encode straight-alpha RGBA pixels as PNG bytes.
#[extendr]
fn encode_png_raw(pixels: Raw, width: i32, height: i32) -> Result<Raw> {
    let px = pixels.as_slice();
    if px.len() != width as usize * height as usize * 4 {
        return Err(err("pixel buffer size does not match width * height * 4"));
    }
    let png = basemapper_core::encode_png(px, width as u32, height as u32).map_err(err)?;
    Ok(Raw::from_bytes(&png))
}

/// Render styles stacked bottom to top as an SVG document.
#[extendr]
#[allow(clippy::too_many_arguments)]
fn render_svg_raw(
    bbox_3857: Vec<f64>,
    width: i32,
    height: i32,
    styles: Vec<String>,
    tile_zoom: Nullable<i32>,
    tile_timeout_ms: i32,
    max_tiles: i32,
    layers: Nullable<Vec<String>>,
    labels: &str,
    features: Nullable<Vec<String>>,
    auth_token: Nullable<String>,
    fail_on_tile_error: bool,
    tile_concurrency: i32,
    text: &str,
    embed_fonts: &str,
    font_dirs: Nullable<Vec<String>>,
) -> Result<String> {
    let reqs = stack_requests(
        &bbox_3857, width, height, styles, 1.0, tile_zoom, tile_timeout_ms, max_tiles, layers, labels, features,
        auth_token, fail_on_tile_error, tile_concurrency,
    )?;
    let mut opts = SvgOptions::default();
    opts.text = match text {
        "editable" => SvgText::Editable,
        "bitmap" => SvgText::Bitmap,
        "paths" => SvgText::Paths,
        _ => return Err(err("text must be 'editable', 'bitmap' or 'paths'")),
    };
    opts.embed_fonts = match embed_fonts {
        "subset" => FontEmbed::Subset,
        "full" => FontEmbed::Full,
        "none" => FontEmbed::None,
        _ => return Err(err("embed_fonts must be 'subset', 'full' or 'none'")),
    };
    if let Nullable::NotNull(dirs) = font_dirs {
        let mut d: Vec<std::path::PathBuf> = dirs.into_iter().map(Into::into).collect();
        d.extend(opts.font_dirs);
        opts.font_dirs = d;
    }
    render_svg_stack_with(reqs, &opts).map_err(err)
}

/// EPSG:3857 bbox of a width x height CSS-pixel view at a MapLibre zoom.
#[extendr]
fn bbox_from_center_raw(lon: f64, lat: f64, zoom: f64, width: i32, height: i32) -> Vec<f64> {
    basemapper_core::bbox_from_center(lon, lat, zoom, width as u32, height as u32).to_vec()
}

/// c(lon, lat, zoom) of the view fitting EPSG:3857 bounds in width x height.
#[extendr]
fn fit_bounds_raw(bounds_3857: Vec<f64>, width: i32, height: i32) -> Result<Vec<f64>> {
    if bounds_3857.len() != 4 {
        return Err(err("bounds must have exactly 4 elements"));
    }
    let b = [bounds_3857[0], bounds_3857[1], bounds_3857[2], bounds_3857[3]];
    let (lon, lat, zoom) = basemapper_core::fit_bounds(b, width as u32, height as u32);
    Ok(vec![lon, lat, zoom])
}

/// The attribution text of stacked styles (plain text joined with " | ").
#[extendr]
fn attribution_raw(styles: Vec<String>, auth_token: Nullable<String>) -> Result<String> {
    basemapper_core::attribution(&styles, opt(auth_token).as_deref()).map_err(err)
}

/// A style's layer ids in drawing order, with their types and features as
/// attributes `type` and `feature`.
#[extendr]
fn list_layers_raw(style: &str, auth_token: Nullable<String>) -> Result<Robj> {
    let layers = basemapper_core::list_layers(style, opt(auth_token).as_deref()).map_err(err)?;
    let ids: Vec<String> = layers.iter().map(|l| l.0.clone()).collect();
    let types: Vec<String> = layers.iter().map(|l| l.1.clone()).collect();
    let features: Vec<String> = layers.into_iter().map(|l| l.2).collect();
    let mut out: Robj = ids.into();
    out.set_attrib("type", types)?;
    out.set_attrib("feature", features)?;
    Ok(out)
}

// ── Styles (basemapper_core::providers, basemapper_core::tile_style) ────────

fn parse_kind(kind: Nullable<String>) -> Result<Option<basemapper_core::TileKind>> {
    opt(kind).map(|k| basemapper_core::TileKind::parse(&k)).transpose().map_err(err)
}

#[extendr]
fn basemap_provider_raw(
    name: &str,
    kind: Nullable<String>,
    api_key: Nullable<String>,
    tiles: Nullable<String>,
    labels: &str,
    features: Nullable<Vec<String>>,
) -> Result<String> {
    let opts = providers::ProviderOptions {
        kind: parse_kind(kind)?,
        api_key: opt(api_key),
        tiles: opt(tiles),
        labels: basemapper_core::Labels::parse(labels).map_err(err)?,
        features: opt(features),
    };
    providers::basemap_provider(name, &opts).map_err(err)
}

/// The catalogue's names, with the other columns as attributes: `vector`,
/// `raster`, `api_key_env` ("" when none) and `attribution`. (Not a
/// `list!`: see `render_raw`.)
#[extendr]
fn list_providers_raw(kind: Nullable<String>) -> Result<Robj> {
    let ps = providers::list_providers(parse_kind(kind)?);
    let names: Vec<String> = ps.iter().map(|p| p.name.to_owned()).collect();
    let mut out: Robj = names.into();
    out.set_attrib("vector", ps.iter().map(|p| p.vector.is_some()).collect::<Vec<bool>>())?;
    out.set_attrib("raster", ps.iter().map(|p| p.raster.is_some()).collect::<Vec<bool>>())?;
    out.set_attrib("api_key_env", ps.iter().map(|p| p.key.map_or("", |k| k.env).to_owned()).collect::<Vec<String>>())?;
    out.set_attrib("attribution", ps.iter().map(|p| p.attribution.to_owned()).collect::<Vec<String>>())?;
    Ok(out)
}

/// The tile cache directory (NULL when off); `dir` moves it ("" turns it
/// off), `clear` deletes the cached tiles.
#[extendr]
fn tile_cache_raw(dir: Nullable<String>, clear: bool) -> Result<Nullable<String>> {
    if let Some(d) = opt(dir) {
        basemapper_core::cache::set_dir((!d.is_empty()).then(|| d.into()));
    }
    if clear {
        basemapper_core::cache::clear().map_err(|e| err(format!("could not clear the tile cache: {e}")))?;
    }
    Ok(match basemapper_core::cache::dir() {
        Some(d) => Nullable::NotNull(d.to_string_lossy().into_owned()),
        None => Nullable::Null,
    })
}

/// A style for tile data with overrides; the paint keys that matched no
/// generated layer as attribute `dropped`. `paint_json` is a JSON object.
#[extendr]
#[allow(clippy::too_many_arguments)]
fn tile_style_raw(
    input: &str,
    kind: Nullable<String>,
    tile_size: Nullable<i32>,
    scheme: Nullable<String>,
    minzoom: Nullable<i32>,
    maxzoom: Nullable<i32>,
    attribution: Nullable<String>,
    source_layers: Nullable<Vec<String>>,
    paint_json: &str,
    auth_token: Nullable<String>,
) -> Result<Robj> {
    let paint: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(paint_json).map_err(|e| err(format!("paint must be a named list: {e}")))?;
    let kind = parse_kind(kind)?;
    let zoom = |z: Nullable<i32>| opt(z).map(|z| z.clamp(0, 30) as u8);
    let opts = basemapper_core::TileStyleOptions {
        kind,
        tile_size: opt(tile_size).map(|t| t.max(1) as u32),
        scheme: opt(scheme),
        minzoom: zoom(minzoom),
        maxzoom: zoom(maxzoom),
        attribution: opt(attribution),
        source_layers: opt(source_layers),
        paint,
    };
    let (style, dropped) = basemapper_core::tile_style(input, &opts, opt(auth_token).as_deref()).map_err(err)?;
    let mut out: Robj = style.into();
    out.set_attrib("dropped", dropped)?;
    Ok(out)
}

#[extendr]
#[allow(clippy::too_many_arguments)]
fn wms_style_raw(
    url: &str,
    layers: &str,
    styles: &str,
    version: &str,
    format: &str,
    transparent: bool,
    extra_params: Nullable<String>,
) -> String {
    providers::wms_style(url, layers, styles, version, format, transparent, opt(extra_params).as_deref())
}

#[extendr]
#[allow(clippy::too_many_arguments)]
fn wmts_style_raw(
    url: &str,
    layer: &str,
    tilematrixset: &str,
    format: &str,
    style: &str,
    version: &str,
    extra_params: Nullable<String>,
) -> String {
    providers::wmts_style(url, layer, tilematrixset, format, style, version, opt(extra_params).as_deref())
}

// Macro to generate R wrappers.
extendr_module! {
    mod basemapper;
    fn render_raw;
    fn encode_png_raw;
    fn render_svg_raw;
    fn bbox_from_center_raw;
    fn fit_bounds_raw;
    fn list_layers_raw;
    fn attribution_raw;
    fn basemap_provider_raw;
    fn list_providers_raw;
    fn tile_style_raw;
    fn tile_cache_raw;
    fn wms_style_raw;
    fn wmts_style_raw;
}
