use basemapper_core::{render, BasemapError, RenderRequest, StyleInput};
use extendr_api::prelude::*;
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

// Package-level tokio Runtime, initialized once at load time.
// R's event loop is not blocked: block_on returns synchronously from R's
// perspective while the OS-level I/O runs non-blocking underneath.
static RT: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("failed to create basemapper tokio runtime")
});

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
/// @return Raw vector of length width * height * 4 (RGBA bytes).
#[extendr]
fn render_basemap_raw(
    bbox_3857: Vec<f64>,
    width: i32,
    height: i32,
    style_input: &str,
    zoom: Nullable<i32>,
    tile_timeout_ms: i32,
    max_tiles: i32,
    layers: Nullable<Vec<String>>,
) -> Result<Raw> {
    if bbox_3857.len() != 4 {
        return Err(extendr_api::Error::Other("bbox_3857 must have exactly 4 elements".into()));
    }

    let zoom_val: Option<u8> = match zoom {
        Nullable::NotNull(z) => Some(z.clamp(0, 22) as u8),
        Nullable::Null => None,
    };

    let layers_val: Option<Vec<String>> = match layers {
        Nullable::NotNull(v) => Some(v),
        Nullable::Null => None,
    };

    let request = RenderRequest {
        bbox: [bbox_3857[0], bbox_3857[1], bbox_3857[2], bbox_3857[3]],
        width: width as u32,
        height: height as u32,
        style_input: StyleInput::from_str(style_input),
        zoom: zoom_val,
        tile_timeout_ms: tile_timeout_ms as u32,
        max_tiles: max_tiles as u32,
        tile_concurrency: 16,
        layers: layers_val,
    };

    // block_on is safe here: RT is a dedicated runtime that never calls back
    // into R, so there is no re-entrancy risk.
    let result = RT.block_on(async { render(request) })
        .map_err(|e| extendr_api::Error::Other(e.to_string()))?;

    Ok(Raw::from_bytes(&result.pixels))
}

// Macro to generate R wrappers.
extendr_module! {
    mod basemapper;
    fn render_basemap_raw;
}
