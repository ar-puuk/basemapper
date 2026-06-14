// PyO3's #[pyfunction] macro generates From<PyErr>→PyErr conversions internally.
#![allow(clippy::useless_conversion)]
use basemapper_core::{render, BasemapError, RenderRequest, StyleInput};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

fn core_err_to_py(e: BasemapError) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
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
    auth_token=None, fail_on_tile_error=true, tile_concurrency=16
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
) -> PyResult<Py<PyBytes>> {
    if bbox_3857.len() != 4 {
        return Err(PyRuntimeError::new_err(
            "bbox_3857 must have exactly 4 elements",
        ));
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
    };

    // map_err then map avoids a ?-desugared From<PyErr>→PyErr that clippy flags.
    py.allow_threads(|| render(request))
        .map_err(core_err_to_py)
        .map(|result| PyBytes::new_bound(py, &result.pixels).unbind())
}

#[pymodule]
fn _basemapper(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(render_basemap_raw, m)?)?;
    Ok(())
}
