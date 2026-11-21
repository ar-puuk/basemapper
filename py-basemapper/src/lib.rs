// PyO3's #[pyfunction] macro generates From<PyErr>→PyErr conversions internally.
#![allow(clippy::useless_conversion)]
use basemapper_core::{render, render_stack, render_svg_stack, BasemapError, RenderRequest, StyleInput};
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

/// Map a `BasemapError` to the appropriate typed Python exception from
/// `basemapper.exceptions`, using the stable `kind()` discriminant.
///
/// Hierarchy (all subclass `RuntimeError` via `BasemapError`):
///   "validation" → ValidationError
///   "style"      → StyleError
///   "network"    → NetworkError
///   _            → BasemapError
fn core_err_to_py(py: Python<'_>, e: BasemapError) -> PyErr {
    let msg = e.to_string();
    let kind = e.kind();

    // Import the exceptions module from the installed Python package.
    // If the import fails for any reason, fall back to PyRuntimeError.
    let exc_mod = match py.import_bound("basemapper.exceptions") {
        Ok(m) => m,
        Err(_) => return PyRuntimeError::new_err(msg),
    };

    let cls_name = match kind {
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

/// Render a basemap and return raw RGBA bytes.
///
/// Args:
///     bbox_3857: [xmin, ymin, xmax, ymax] in EPSG:3857 metres.
///     width: Output pixel width.
///     height: Output pixel height.
///     style_input: Style URL or inline MapLibre GL JSON string.
///     zoom: Tile zoom level (0–22). Auto-computed when None.
///     tile_timeout_ms: Per-tile HTTP timeout in milliseconds.
///     max_tiles: Maximum tiles fetched per render call.
///     layers: Optional layer filter list. Plain IDs → keep only those;
///             minus-prefixed IDs → exclude those. Mixed lists raise BasemapError.
///     auth_token: Optional authentication token. Sent as
///                 `Authorization: Bearer <token>` for raster sources;
///                 appended as `?access_token=<token>` for Mapbox vector sources.
///     fail_on_tile_error: When False, individual tile failures are skipped
///                         rather than aborting the render. Default True.
///     tile_concurrency: Maximum parallel tile downloads. Default 16.
///     pixel_ratio: Device pixels per CSS pixel (MapLibre devicePixelRatio).
///                  The output is width * pixel_ratio by height * pixel_ratio.
///
/// Returns:
///     bytes: RGBA pixel data of length width * height * 4.
///
/// Raises:
///     BasemapError: On any rendering or network failure.
#[pyfunction]
#[pyo3(signature = (
    bbox_3857, width, height, style_input,
    zoom=None, tile_timeout_ms=10_000, max_tiles=256, layers=None,
    auth_token=None, fail_on_tile_error=true, tile_concurrency=16, pixel_ratio=1.0
))]
#[allow(clippy::too_many_arguments, clippy::useless_conversion)]
fn render_basemap_raw(
    py: Python<'_>,
    bbox_3857: Vec<f64>,
    width: u32,
    height: u32,
    style_input: &str,
    zoom: Option<u8>,
    tile_timeout_ms: u32,
    max_tiles: u32,
    layers: Option<Vec<String>>,
    auth_token: Option<String>,
    fail_on_tile_error: bool,
    tile_concurrency: u32,
    pixel_ratio: f64,
) -> PyResult<Py<PyBytes>> {
    if bbox_3857.len() != 4 {
        // Raise ValidationError for this input error too.
        let exc_msg = "bbox_3857 must have exactly 4 elements";
        let exc = py
            .import_bound("basemapper.exceptions")
            .and_then(|m| m.getattr("ValidationError"))
            .and_then(|cls| cls.call1((exc_msg,)))
            .map(|exc| PyErr::from_value_bound(exc.into()))
            .unwrap_or_else(|_| PyRuntimeError::new_err(exc_msg));
        return Err(exc);
    }
    let request = RenderRequest {
        bbox: [bbox_3857[0], bbox_3857[1], bbox_3857[2], bbox_3857[3]],
        width,
        height,
        style_input: StyleInput::detect(style_input),
        zoom,
        tile_timeout_ms,
        max_tiles,
        tile_concurrency: tile_concurrency.max(1),
        layers,
        auth_token,
        fail_on_tile_error,
        pixel_ratio,
    };

    // map_err then map avoids a ?-desugared From<PyErr>→PyErr that clippy flags.
    let result = py.allow_threads(|| render(request));
    match result {
        Ok(r) => Ok(PyBytes::new_bound(py, &r.pixels).unbind()),
        Err(e) => Err(core_err_to_py(py, e)),
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
    zoom: Option<u8>,
    tile_timeout_ms: u32,
    max_tiles: u32,
    layers: &Option<Vec<String>>,
    auth_token: &Option<String>,
    fail_on_tile_error: bool,
    tile_concurrency: u32,
) -> PyResult<Vec<RenderRequest>> {
    if bbox_3857.len() != 4 {
        return Err(validation_error(py, "bbox_3857 must have exactly 4 elements"));
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
            zoom,
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
    zoom=None, tile_timeout_ms=10_000, max_tiles=256, layers=None,
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
    zoom: Option<u8>,
    tile_timeout_ms: u32,
    max_tiles: u32,
    layers: Option<Vec<String>>,
    auth_token: Option<String>,
    fail_on_tile_error: bool,
    tile_concurrency: u32,
) -> PyResult<(Py<PyBytes>, u32, u32)> {
    let reqs = stack_requests(
        py, &bbox_3857, width, height, &styles, pixel_ratio, zoom, tile_timeout_ms, max_tiles,
        &layers, &auth_token, fail_on_tile_error, tile_concurrency,
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
/// document (true vector: paths and traced glyph outlines).
#[pyfunction]
#[pyo3(signature = (
    bbox_3857, width, height, styles,
    zoom=None, tile_timeout_ms=10_000, max_tiles=256, layers=None,
    auth_token=None, fail_on_tile_error=false, tile_concurrency=16
))]
#[allow(clippy::too_many_arguments)]
fn render_svg_raw(
    py: Python<'_>,
    bbox_3857: Vec<f64>,
    width: u32,
    height: u32,
    styles: Vec<String>,
    zoom: Option<u8>,
    tile_timeout_ms: u32,
    max_tiles: u32,
    layers: Option<Vec<String>>,
    auth_token: Option<String>,
    fail_on_tile_error: bool,
    tile_concurrency: u32,
) -> PyResult<String> {
    let reqs = stack_requests(
        py, &bbox_3857, width, height, &styles, 1.0, zoom, tile_timeout_ms, max_tiles,
        &layers, &auth_token, fail_on_tile_error, tile_concurrency,
    )?;
    match py.allow_threads(|| render_svg_stack(reqs)) {
        Ok(svg) => Ok(svg),
        Err(e) => Err(core_err_to_py(py, e)),
    }
}

/// EPSG:3857 bbox of a width x height CSS-pixel view at MapLibre zoom.
#[pyfunction]
fn bbox_from_center(lon: f64, lat: f64, zoom: f64, width: u32, height: u32) -> Vec<f64> {
    basemapper_core::bbox_from_center(lon, lat, zoom, width, height).to_vec()
}

/// (lon, lat, zoom) of the view fitting EPSG:3857 bounds in width x height.
#[pyfunction]
fn fit_bounds(bounds_3857: Vec<f64>, width: u32, height: u32) -> PyResult<(f64, f64, f64)> {
    if bounds_3857.len() != 4 {
        return Err(PyRuntimeError::new_err("bounds_3857 must have exactly 4 elements"));
    }
    Ok(basemapper_core::fit_bounds([bounds_3857[0], bounds_3857[1], bounds_3857[2], bounds_3857[3]], width, height))
}

#[pymodule]
fn _basemapper(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(render_basemap_raw, m)?)?;
    m.add_function(wrap_pyfunction!(render_raw, m)?)?;
    m.add_function(wrap_pyfunction!(render_svg_raw, m)?)?;
    m.add_function(wrap_pyfunction!(encode_png, m)?)?;
    m.add_function(wrap_pyfunction!(bbox_from_center, m)?)?;
    m.add_function(wrap_pyfunction!(fit_bounds, m)?)?;
    Ok(())
}
