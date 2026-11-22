use basemapper_core::{render, render_stack, render_svg_stack_with, FontEmbed, RenderRequest, StyleInput, SvgOptions, SvgText};
use extendr_api::prelude::*;
use extendr_api::Result;

/// Render a basemap and return raw RGBA bytes.
///
/// @param bbox_3857 Numeric vector of length 4: c(xmin, ymin, xmax, ymax) in EPSG:3857.
/// @param width Integer output pixel width.
/// @param height Integer output pixel height.
/// @param style_input Character: style URL or inline MapLibre GL JSON.
/// @param zoom Integer zoom override (0–22), or NULL for auto.
/// @param tile_timeout_ms Integer per-tile HTTP timeout in milliseconds.
/// @param max_tiles Integer maximum tiles per render.
/// @param layers Character vector of layer IDs to include (plain) or exclude
///        (minus-prefixed), or NULL for all layers.
/// @param auth_token Character authentication token, or NULL. Sent as
///        `Authorization: Bearer <token>` for raster sources; appended as
///        `?access_token=<token>` for Mapbox vector sources.
/// @param fail_on_tile_error Logical. When FALSE, individual tile failures are
///        skipped rather than aborting the render.
/// @param tile_concurrency Integer maximum parallel tile downloads.
/// @return Raw vector of length width * height * 4 (RGBA bytes).
#[extendr]
#[allow(clippy::too_many_arguments)]
fn render_basemap_raw(
    bbox_3857: Vec<f64>,
    width: i32,
    height: i32,
    style_input: &str,
    zoom: Nullable<i32>,
    tile_timeout_ms: i32,
    max_tiles: i32,
    layers: Nullable<Vec<String>>,
    auth_token: Nullable<String>,
    fail_on_tile_error: bool,
    tile_concurrency: i32,
) -> Result<Raw> {
    if bbox_3857.len() != 4 {
        return Err(extendr_api::Error::Other(
            "bbox_3857 must have exactly 4 elements".into(),
        ));
    }

    // Do not clamp zoom here — let core validate() reject out-of-range values
    // with a clear InvalidBbox error rather than silently accepting bad input.
    let zoom_val: Option<u8> = match zoom {
        Nullable::NotNull(z) => Some(z as u8),
        Nullable::Null => None,
    };

    let layers_val: Option<Vec<String>> = match layers {
        Nullable::NotNull(v) => Some(v),
        Nullable::Null => None,
    };

    let auth_token_val: Option<String> = match auth_token {
        Nullable::NotNull(s) => Some(s),
        Nullable::Null => None,
    };

    let request = RenderRequest {
        bbox: [bbox_3857[0], bbox_3857[1], bbox_3857[2], bbox_3857[3]],
        width: width as u32,
        height: height as u32,
        style_input: StyleInput::detect(style_input),
        zoom: zoom_val,
        tile_timeout_ms: tile_timeout_ms as u32,
        max_tiles: max_tiles as u32,
        tile_concurrency: tile_concurrency.max(1) as u32,
        layers: layers_val,
        auth_token: auth_token_val,
        fail_on_tile_error,
        pixel_ratio: 1.0,
    };

    // render() drives async work on its own internal tokio Runtime; it is
    // synchronous from the caller's perspective.
    let result = render(request).map_err(|e| extendr_api::Error::Other(e.to_string()))?;

    Ok(Raw::from_bytes(&result.pixels))
}

/// One request per style (bottom to top) over the same view.
#[allow(clippy::too_many_arguments)]
fn stack_requests(
    bbox_3857: &[f64],
    width: i32,
    height: i32,
    styles: Vec<String>,
    pixel_ratio: f64,
    zoom: Nullable<i32>,
    tile_timeout_ms: i32,
    max_tiles: i32,
    layers: Nullable<Vec<String>>,
    auth_token: Nullable<String>,
    fail_on_tile_error: bool,
    tile_concurrency: i32,
) -> Result<Vec<RenderRequest>> {
    if bbox_3857.len() != 4 {
        return Err(extendr_api::Error::Other("bbox_3857 must have exactly 4 elements".into()));
    }
    if styles.is_empty() {
        return Err(extendr_api::Error::Other("at least one style is required".into()));
    }
    let zoom: Option<u8> = match zoom {
        Nullable::NotNull(z) => Some(z as u8),
        Nullable::Null => None,
    };
    let layers: Option<Vec<String>> = match layers {
        Nullable::NotNull(v) => Some(v),
        Nullable::Null => None,
    };
    let auth_token: Option<String> = match auth_token {
        Nullable::NotNull(s) => Some(s),
        Nullable::Null => None,
    };
    Ok(styles
        .iter()
        .map(|s| RenderRequest {
            bbox: [bbox_3857[0], bbox_3857[1], bbox_3857[2], bbox_3857[3]],
            width: width as u32,
            height: height as u32,
            style_input: StyleInput::detect(s),
            zoom,
            tile_timeout_ms: tile_timeout_ms as u32,
            max_tiles: max_tiles as u32,
            tile_concurrency: tile_concurrency.max(1) as u32,
            layers: layers.clone(),
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
    zoom: Nullable<i32>,
    tile_timeout_ms: i32,
    max_tiles: i32,
    layers: Nullable<Vec<String>>,
    auth_token: Nullable<String>,
    fail_on_tile_error: bool,
    tile_concurrency: i32,
) -> Result<Robj> {
    let reqs = stack_requests(
        &bbox_3857, width, height, styles, pixel_ratio, zoom, tile_timeout_ms, max_tiles, layers,
        auth_token, fail_on_tile_error, tile_concurrency,
    )?;
    let r = render_stack(reqs).map_err(|e| extendr_api::Error::Other(e.to_string()))?;
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
        return Err(extendr_api::Error::Other("pixel buffer size does not match width * height * 4".into()));
    }
    let png = basemapper_core::encode_png(px, width as u32, height as u32)
        .map_err(|e| extendr_api::Error::Other(e.to_string()))?;
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
    zoom: Nullable<i32>,
    tile_timeout_ms: i32,
    max_tiles: i32,
    layers: Nullable<Vec<String>>,
    auth_token: Nullable<String>,
    fail_on_tile_error: bool,
    tile_concurrency: i32,
    text: &str,
    embed_fonts: &str,
    font_dirs: Nullable<Vec<String>>,
) -> Result<String> {
    let reqs = stack_requests(
        &bbox_3857, width, height, styles, 1.0, zoom, tile_timeout_ms, max_tiles, layers,
        auth_token, fail_on_tile_error, tile_concurrency,
    )?;
    let mut opts = SvgOptions::default();
    opts.text = match text {
        "editable" => SvgText::Editable,
        "bitmap" => SvgText::Bitmap,
        "paths" => SvgText::Paths,
        _ => return Err(extendr_api::Error::Other("text must be 'editable', 'bitmap' or 'paths'".into())),
    };
    opts.embed_fonts = match embed_fonts {
        "subset" => FontEmbed::Subset,
        "full" => FontEmbed::Full,
        "none" => FontEmbed::None,
        _ => return Err(extendr_api::Error::Other("embed_fonts must be 'subset', 'full' or 'none'".into())),
    };
    if let Nullable::NotNull(dirs) = font_dirs {
        let mut d: Vec<std::path::PathBuf> = dirs.into_iter().map(Into::into).collect();
        d.extend(opts.font_dirs);
        opts.font_dirs = d;
    }
    render_svg_stack_with(reqs, &opts).map_err(|e| extendr_api::Error::Other(e.to_string()))
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
        return Err(extendr_api::Error::Other("bounds_3857 must have exactly 4 elements".into()));
    }
    let b = [bounds_3857[0], bounds_3857[1], bounds_3857[2], bounds_3857[3]];
    let (lon, lat, zoom) = basemapper_core::fit_bounds(b, width as u32, height as u32);
    Ok(vec![lon, lat, zoom])
}

// Macro to generate R wrappers.
extendr_module! {
    mod basemapper;
    fn render_basemap_raw;
    fn render_raw;
    fn encode_png_raw;
    fn render_svg_raw;
    fn bbox_from_center_raw;
    fn fit_bounds_raw;
}
