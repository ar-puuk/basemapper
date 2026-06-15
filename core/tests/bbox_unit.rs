use basemapper_core::{
    bbox::{compute_zoom, tile_count, RenderRequest},
    error::BasemapError,
    style::StyleInput,
};

fn base_request() -> RenderRequest {
    RenderRequest {
        bbox: [-13_700_000.0, 4_500_000.0, -13_600_000.0, 4_600_000.0],
        width: 800,
        height: 600,
        style_input: StyleInput::InlineJson(r#"{"version":8,"sources":{},"layers":[]}"#.into()),
        zoom: None,
        tile_timeout_ms: 5_000,
        max_tiles: 256,
        tile_concurrency: 4,
        layers: None,
        auth_token: None,
        fail_on_tile_error: false,
    }
}

#[test]
fn valid_request_validates() {
    assert!(base_request().validate().is_ok());
}

#[test]
fn xmin_ge_xmax_returns_invalid_bbox() {
    let mut req = base_request();
    req.bbox[0] = req.bbox[2]; // xmin == xmax
    assert!(matches!(req.validate(), Err(BasemapError::InvalidBbox(_))));
}

#[test]
fn zero_width_returns_invalid_dimensions() {
    let mut req = base_request();
    req.width = 0;
    assert!(matches!(
        req.validate(),
        Err(BasemapError::InvalidDimensions(0, _))
    ));
}

#[test]
fn zoom_23_returns_invalid_bbox() {
    let mut req = base_request();
    req.zoom = Some(23);
    assert!(matches!(req.validate(), Err(BasemapError::InvalidBbox(_))));
}

#[test]
fn zoom_formula_produces_reasonable_result() {
    // SF bbox at ~100 km wide → zoom 12–14 at 800 px
    let bbox = [-13_700_000.0_f64, 4_500_000.0, -13_600_000.0, 4_600_000.0];
    let z = compute_zoom(bbox, 800);
    assert!(
        z >= 10 && z <= 16,
        "unexpected zoom {z} for 100 km bbox at 800 px"
    );
}

#[test]
fn tile_count_guard_reduces_zoom() {
    // Very large bbox at high zoom would explode tile count.
    let bbox = [-20_037_508.0, -20_037_508.0, 20_037_508.0, 20_037_508.0];
    let z = compute_zoom(bbox, 256);
    assert!(
        tile_count(bbox, z) <= 256,
        "tile count should fit in 256 at zoom {z}"
    );
}

#[test]
fn mixed_layer_filter_returns_error() {
    let mut req = base_request();
    req.layers = Some(vec!["water".into(), "-roads".into()]);
    assert!(matches!(
        req.validate(),
        Err(BasemapError::InvalidLayerFilter(_))
    ));
}
