// Integration tests for tile fetching and style validation.
// These tests use httptest to serve mock tiles; no real network calls are made.

use basemapper_core::{
    style::{filter_style_layers, validate_maplibre_style},
    BasemapError,
};

const STYLE_3_LAYERS: &str = r#"{
  "version": 8,
  "sources": { "s": { "type": "raster", "tiles": ["http://localhost/{z}/{x}/{y}.png"] } },
  "layers": [
    {"id": "water",  "type": "raster", "source": "s"},
    {"id": "roads",  "type": "raster", "source": "s"},
    {"id": "labels", "type": "raster", "source": "s"}
  ]
}"#;

#[test]
fn validate_version_8_passes() {
    assert!(validate_maplibre_style(STYLE_3_LAYERS).is_ok());
}

#[test]
fn validate_version_7_fails() {
    let bad = r#"{"version":7,"sources":{},"layers":[]}"#;
    assert!(matches!(
        validate_maplibre_style(bad),
        Err(BasemapError::StyleParseError(_))
    ));
}

#[test]
fn filter_positive_keeps_only_named_layers() {
    let filtered = filter_style_layers(STYLE_3_LAYERS, &["water".into()]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&filtered).unwrap();
    let layers = v["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0]["id"], "water");
}

#[test]
fn filter_negative_removes_named_layers() {
    let filtered =
        filter_style_layers(STYLE_3_LAYERS, &["-roads".into(), "-labels".into()]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&filtered).unwrap();
    let layers = v["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0]["id"], "water");
}

#[test]
fn filter_preserves_sources_and_sprite() {
    // H3: sources must survive the filter regardless of which layers remain.
    let style_with_sprite = format!(
        r#"{{"version":8,"sources":{{"s":{{}}}},"sprite":"https://example.com/sprite","glyphs":"https://example.com/glyphs/{{fontstack}}/{{range}}.pbf","layers":[{{"id":"water","type":"raster","source":"s"}}]}}"#
    );
    let filtered = filter_style_layers(&style_with_sprite, &["-water".into()]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&filtered).unwrap();
    assert!(v.get("sources").is_some(), "sources must be preserved");
    assert!(v.get("sprite").is_some(), "sprite must be preserved");
    assert!(v.get("glyphs").is_some(), "glyphs must be preserved");
}
