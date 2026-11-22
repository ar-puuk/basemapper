// PyO3's #[pyfunction] macro generates From<PyErr>→PyErr conversions internally.
#![allow(clippy::useless_conversion)]
//! The compiled core of the Python package (`basemapper._basemapper`). The
//! public Python API (`basemap()`, `render_basemap_raw()`, the providers, ...)
//! wraps these functions; the same core backs the R package, so both return
//! the same results for the same call.
use basemapper_core::{
    providers, render_stack, render_svg_stack_with, BasemapError, FontEmbed, RenderRequest, StyleInput, SvgOptions,
    SvgText,
};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

fn validation_error(py: Python<'_>, msg: &str) -> PyErr {
    py.import_bound("basemapper.exceptions")
        .and_then(|m| m.getattr("ValidationError"))
        .and_then(|cls| cls.call1((msg,)))
        .map(|exc| PyErr::from_value_bound(exc.into()))
        .unwrap_or_else(|_| PyRuntimeError::new_err(msg.to_owned()))
}

/// Map a `BasemapError` to the typed Python exception from
/// `basemapper.exceptions`, by its stable `kind()`:
///   "validation" → ValidationError, "style" → StyleError,
///   "network" → NetworkError, otherwise BasemapError.
fn core_err_to_py(py: Python<'_>, e: BasemapError) -> PyErr {
    let msg = e.to_string();
    let exc_mod = match py.import_bound("basemapper.exceptions") {
        Ok(m) => m,
        Err(_) => return PyRuntimeError::new_err(msg),
    };
    let cls_name = match e.kind() {
        "validation" => "ValidationError",
        "style" => "StyleError",
        "network" => "NetworkError",
        _ => "BasemapError",
    };
    match exc_mod.getattr(cls_name) {
        Ok(cls) => match cls.call1((msg.clone(),)) {
            Ok(exc) => PyErr::from_value_bound(exc.into()),
            Err(_) => PyRuntimeError::new_err(msg),
        },
        Err(_) => PyRuntimeError::new_err(msg),
    }
}

/// One request per style (bottom to top) over the same view.
#[allow(clippy::too_many_arguments)]
fn stack_requests(
    py: Python<'_>,
    bbox_3857: &[f64],
    width: u32,
    height: u32,
    styles: &[String],
    pixel_ratio: f64,
    tile_zoom: Option<u8>,
    tile_timeout_ms: u32,
    max_tiles: u32,
    layers: &Option<Vec<String>>,
    auth_token: &Option<String>,
    fail_on_tile_error: bool,
    tile_concurrency: u32,
) -> PyResult<Vec<RenderRequest>> {
    if bbox_3857.len() != 4 {
        return Err(validation_error(py, "bounds must have exactly 4 elements"));
    }
    if styles.is_empty() {
        return Err(validation_error(py, "at least one style is required"));
    }
    Ok(styles
        .iter()
        .map(|s| RenderRequest {
            bbox: [bbox_3857[0], bbox_3857[1], bbox_3857[2], bbox_3857[3]],
            width,
            height,
            style_input: StyleInput::detect(s),
            tile_zoom,
            tile_timeout_ms,
            max_tiles,
            tile_concurrency: tile_concurrency.max(1),
            layers: layers.clone(),
            auth_token: auth_token.clone(),
            fail_on_tile_error,
            pixel_ratio,
        })
        .collect())
}

/// Render styles (stacked bottom to top) over an EPSG:3857 bbox.
///
/// `width`/`height` are CSS pixels; the image is `pixel_ratio` times larger.
///
/// Returns:
///     (bytes, width, height): straight-alpha RGBA pixels and their size.
#[pyfunction]
#[pyo3(signature = (
    bbox_3857, width, height, styles, pixel_ratio=1.0,
    tile_zoom=None, tile_timeout_ms=10_000, max_tiles=256, layers=None,
    auth_token=None, fail_on_tile_error=false, tile_concurrency=16
))]
#[allow(clippy::too_many_arguments)]
fn render_raw(
    py: Python<'_>,
    bbox_3857: Vec<f64>,
    width: u32,
    height: u32,
    styles: Vec<String>,
    pixel_ratio: f64,
    tile_zoom: Option<u8>,
    tile_timeout_ms: u32,
    max_tiles: u32,
    layers: Option<Vec<String>>,
    auth_token: Option<String>,
    fail_on_tile_error: bool,
    tile_concurrency: u32,
) -> PyResult<(Py<PyBytes>, u32, u32)> {
    let reqs = stack_requests(
        py, &bbox_3857, width, height, &styles, pixel_ratio, tile_zoom, tile_timeout_ms, max_tiles, &layers,
        &auth_token, fail_on_tile_error, tile_concurrency,
    )?;
    match py.allow_threads(|| render_stack(reqs)) {
        Ok(r) => Ok((PyBytes::new_bound(py, &r.pixels).unbind(), r.width, r.height)),
        Err(e) => Err(core_err_to_py(py, e)),
    }
}

/// Encode straight-alpha RGBA pixels as PNG bytes.
#[pyfunction]
fn encode_png(py: Python<'_>, pixels: &[u8], width: u32, height: u32) -> PyResult<Py<PyBytes>> {
    if pixels.len() != width as usize * height as usize * 4 {
        return Err(validation_error(py, "pixel buffer size does not match width * height * 4"));
    }
    match basemapper_core::encode_png(pixels, width, height) {
        Ok(b) => Ok(PyBytes::new_bound(py, &b).unbind()),
        Err(e) => Err(core_err_to_py(py, e)),
    }
}

/// Render styles (stacked bottom to top) over an EPSG:3857 bbox as an SVG
/// document (true vector: paths, text and embedded images).
#[pyfunction]
#[pyo3(signature = (
    bbox_3857, width, height, styles,
    tile_zoom=None, tile_timeout_ms=10_000, max_tiles=256, layers=None,
    auth_token=None, fail_on_tile_error=false, tile_concurrency=16,
    text="editable", embed_fonts="subset", font_dirs=None
))]
#[allow(clippy::too_many_arguments)]
fn render_svg_raw(
    py: Python<'_>,
    bbox_3857: Vec<f64>,
    width: u32,
    height: u32,
    styles: Vec<String>,
    tile_zoom: Option<u8>,
    tile_timeout_ms: u32,
    max_tiles: u32,
    layers: Option<Vec<String>>,
    auth_token: Option<String>,
    fail_on_tile_error: bool,
    tile_concurrency: u32,
    text: &str,
    embed_fonts: &str,
    font_dirs: Option<Vec<String>>,
) -> PyResult<String> {
    let reqs = stack_requests(
        py, &bbox_3857, width, height, &styles, 1.0, tile_zoom, tile_timeout_ms, max_tiles, &layers, &auth_token,
        fail_on_tile_error, tile_concurrency,
    )?;
    let opts = svg_options(py, text, embed_fonts, font_dirs)?;
    match py.allow_threads(|| render_svg_stack_with(reqs, &opts)) {
        Ok(svg) => Ok(svg),
        Err(e) => Err(core_err_to_py(py, e)),
    }
}

/// SVG options from Python arguments.
fn svg_options(py: Python<'_>, text: &str, embed_fonts: &str, font_dirs: Option<Vec<String>>) -> PyResult<SvgOptions> {
    let mut opts = SvgOptions::default();
    opts.text = match text {
        "editable" => SvgText::Editable,
        "bitmap" => SvgText::Bitmap,
        "paths" => SvgText::Paths,
        _ => return Err(validation_error(py, "text must be 'editable', 'bitmap' or 'paths'")),
    };
    opts.embed_fonts = match embed_fonts {
        "subset" => FontEmbed::Subset,
        "full" => FontEmbed::Full,
        "none" => FontEmbed::None,
        _ => return Err(validation_error(py, "embed_fonts must be 'subset', 'full' or 'none'")),
    };
    if let Some(dirs) = font_dirs {
        let mut d: Vec<std::path::PathBuf> = dirs.into_iter().map(Into::into).collect();
        d.extend(opts.font_dirs);
        opts.font_dirs = d;
    }
    Ok(opts)
}

/// EPSG:3857 bbox of a width x height CSS-pixel view at MapLibre zoom.
#[pyfunction]
fn bbox_from_center(lon: f64, lat: f64, zoom: f64, width: u32, height: u32) -> Vec<f64> {
    basemapper_core::bbox_from_center(lon, lat, zoom, width, height).to_vec()
}

/// (lon, lat, zoom) of the view fitting EPSG:3857 bounds in width x height.
#[pyfunction]
fn fit_bounds(py: Python<'_>, bounds_3857: Vec<f64>, width: u32, height: u32) -> PyResult<(f64, f64, f64)> {
    if bounds_3857.len() != 4 {
        return Err(validation_error(py, "bounds must have exactly 4 elements"));
    }
    Ok(basemapper_core::fit_bounds([bounds_3857[0], bounds_3857[1], bounds_3857[2], bounds_3857[3]], width, height))
}

/// A style's layers in drawing order as (id, type) pairs.
#[pyfunction]
#[pyo3(signature = (style, auth_token=None))]
fn list_layers(py: Python<'_>, style: &str, auth_token: Option<String>) -> PyResult<Vec<(String, String)>> {
    let style = style.to_owned();
    match py.allow_threads(|| basemapper_core::list_layers(&style, auth_token.as_deref())) {
        Ok(l) => Ok(l),
        Err(e) => Err(core_err_to_py(py, e)),
    }
}

// ── Styles (basemapper_core::providers, basemapper_core::tile_style) ────────

#[pyfunction]
#[pyo3(signature = (name, api_key=None))]
fn basemap_provider(py: Python<'_>, name: &str, api_key: Option<&str>) -> PyResult<String> {
    providers::basemap_provider(name, api_key).map_err(|e| core_err_to_py(py, e))
}

#[pyfunction]
fn list_providers() -> Vec<&'static str> {
    providers::PROVIDER_NAMES.to_vec()
}

/// A style for tile data with overrides; returns (style, dropped paint keys).
/// `paint_json` is a JSON object.
#[pyfunction]
#[pyo3(signature = (
    input, kind=None, tile_size=None, scheme=None, minzoom=None, maxzoom=None,
    attribution=None, source_layers=None, paint_json="{}", auth_token=None
))]
#[allow(clippy::too_many_arguments)]
fn tile_style(
    py: Python<'_>,
    input: &str,
    kind: Option<&str>,
    tile_size: Option<u32>,
    scheme: Option<String>,
    minzoom: Option<u8>,
    maxzoom: Option<u8>,
    attribution: Option<String>,
    source_layers: Option<Vec<String>>,
    paint_json: &str,
    auth_token: Option<String>,
) -> PyResult<(String, Vec<String>)> {
    let paint: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(paint_json).map_err(|e| validation_error(py, &format!("paint must be a mapping: {e}")))?;
    let kind = match kind {
        Some(k) => Some(basemapper_core::TileKind::parse(k).map_err(|e| core_err_to_py(py, e))?),
        None => None,
    };
    let opts = basemapper_core::TileStyleOptions {
        kind, tile_size, scheme, minzoom, maxzoom, attribution, source_layers, paint,
    };
    let input = input.to_owned();
    match py.allow_threads(|| basemapper_core::tile_style(&input, &opts, auth_token.as_deref())) {
        Ok(r) => Ok(r),
        Err(e) => Err(core_err_to_py(py, e)),
    }
}

#[pyfunction]
#[pyo3(signature = (url, layers, styles, version, format, transparent, extra_params=None))]
#[allow(clippy::too_many_arguments)]
fn wms_style(
    url: &str,
    layers: &str,
    styles: &str,
    version: &str,
    format: &str,
    transparent: bool,
    extra_params: Option<&str>,
) -> String {
    providers::wms_style(url, layers, styles, version, format, transparent, extra_params)
}

#[pyfunction]
#[pyo3(signature = (url, layer, tilematrixset, format, style, version, extra_params=None))]
#[allow(clippy::too_many_arguments)]
fn wmts_style(
    url: &str,
    layer: &str,
    tilematrixset: &str,
    format: &str,
    style: &str,
    version: &str,
    extra_params: Option<&str>,
) -> String {
    providers::wmts_style(url, layer, tilematrixset, format, style, version, extra_params)
}

#[pymodule]
fn _basemapper(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(render_raw, m)?)?;
    m.add_function(wrap_pyfunction!(render_svg_raw, m)?)?;
    m.add_function(wrap_pyfunction!(encode_png, m)?)?;
    m.add_function(wrap_pyfunction!(bbox_from_center, m)?)?;
    m.add_function(wrap_pyfunction!(fit_bounds, m)?)?;
    m.add_function(wrap_pyfunction!(list_layers, m)?)?;
    m.add_function(wrap_pyfunction!(basemap_provider, m)?)?;
    m.add_function(wrap_pyfunction!(list_providers, m)?)?;
    m.add_function(wrap_pyfunction!(tile_style, m)?)?;
    m.add_function(wrap_pyfunction!(wms_style, m)?)?;
    m.add_function(wrap_pyfunction!(wmts_style, m)?)?;
    Ok(())
}
