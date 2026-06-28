use basemapper_core::{
    bbox::{compute_zoom, mercator_to_tile, tile_count, RenderRequest},
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
fn anti_meridian_bbox_accepted_by_validate() {
    // xmin > xmax in EPSG:3857 → anti-meridian crossing → now supported.
    let mut req = base_request();
    req.bbox = [15_000_000.0, 4_500_000.0, -15_000_000.0, 4_600_000.0];
    assert!(
        req.validate().is_ok(),
        "anti-meridian bbox (xmin > xmax) must be accepted, got: {:?}",
        req.validate()
    );
}

#[test]
fn zero_width_bbox_still_rejected() {
    let mut req = base_request();
    req.bbox[0] = req.bbox[2]; // xmin == xmax → zero width
    assert!(matches!(req.validate(), Err(BasemapError::InvalidBbox(_))));
}

#[test]
fn out_of_bounds_bbox_rejected() {
    let mut req = base_request();
    req.bbox[2] = 21_000_000.0; // > EARTH_HALF
    assert!(matches!(req.validate(), Err(BasemapError::InvalidBbox(_))));
}

#[test]
fn tile_count_antimeridian_is_nonzero() {
    // Pacific-centred bbox: tiles come from both sides of the dateline.
    let bbox = [15_000_000.0_f64, -5_000_000.0, -15_000_000.0, 5_000_000.0];
    let count = tile_count(bbox, 5);
    assert!(count > 0, "anti-meridian tile_count must be non-zero, got {count}");
    // At zoom 5 the tile grid is 32×32 = 1024 tiles.  The bbox spans most of
    // the east + west halves in x, and a moderate band in y.
    assert!(count <= 1024, "tile_count {count} suspiciously large at zoom 5");
}

#[test]
fn tile_count_antimeridian_equals_two_halves() {
    // tile_count(full anti-meridian bbox) == tile_count(west) + tile_count(east).
    const EARTH_HALF: f64 = 20_037_508.342_789_244;
    let bbox = [15_000_000.0_f64, -5_000_000.0, -15_000_000.0, 5_000_000.0];
    let west = [15_000_000.0_f64, -5_000_000.0, EARTH_HALF, 5_000_000.0];
    let east = [-EARTH_HALF, -5_000_000.0, -15_000_000.0, 5_000_000.0];
    let z = 5;
    assert_eq!(
        tile_count(bbox, z),
        tile_count(west, z) + tile_count(east, z),
        "anti-meridian tile_count must equal sum of two halves"
    );
}

#[test]
fn compute_zoom_antimeridian_uses_wrapped_width() {
    // A Pacific bbox (xmin > xmax) should give a zoom consistent with the
    // wrapped geographic width, not some degenerate negative value.
    let bbox = [15_000_000.0_f64, -5_000_000.0, -15_000_000.0, 5_000_000.0];
    let z = compute_zoom(bbox, 800);
    assert!(z >= 2 && z <= 10, "unexpected zoom {z} for Pacific bbox at 800 px");
}

#[test]
fn mercator_to_tile_smoke() {
    // Origin (0, 0) at zoom 1 should be in the centre tile (1, 1).
    let (tx, ty) = mercator_to_tile(0.0, 0.0, 1);
    assert_eq!((tx, ty), (1, 1));
}
