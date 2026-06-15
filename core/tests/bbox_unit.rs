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

// ──────────────────────────── Anti-meridian tests ────────────────────────────

#[test]
fn anti_meridian_bbox_xmin_gt_xmax_returns_invalid_bbox() {
    // Simulate a bbox that wraps the anti-meridian in Web Mercator:
    // e.g. from 170°E → 190°E (= -170°W) would give xmin > xmax in metres.
    let mut req = base_request();
    req.bbox = [15_000_000.0, 4_500_000.0, -15_000_000.0, 4_600_000.0]; // xmin > xmax
    let err = req.validate().unwrap_err();
    assert!(
        matches!(err, BasemapError::InvalidBbox(_)),
        "expected InvalidBbox for anti-meridian bbox, got {err:?}"
    );
    // Error message must mention anti-meridian so the user knows what to do.
    assert!(
        err.to_string().to_lowercase().contains("anti-meridian")
            || err.to_string().to_lowercase().contains("anti"),
        "error message should mention anti-meridian: {err}"
    );
}

#[test]
fn tile_count_does_not_overflow_when_x0_gt_x1() {
    // Even if validate() is bypassed, tile_count must not panic in release or
    // wrap to a huge value in debug builds.
    let bbox = [15_000_000.0_f64, -5_000_000.0, -15_000_000.0, 5_000_000.0];
    let count = tile_count(bbox, 10);
    // Correct behaviour: returns 0 rather than panicking or overflowing.
    assert_eq!(count, 0, "tile_count should return 0 for anti-meridian bbox");
}
