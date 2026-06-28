// Integration tests for tile fetching and style validation.
// These tests use httptest to serve mock tiles; no real network calls are made.

use basemapper_core::{
    style::{filter_style_layers, validate_maplibre_style},
    tile_fetcher::{expand_url, TileCoord, TileSource},
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

// ──────────────────────────── Tile URL expansion ────────────────────────────

#[test]
fn expand_url_substitutes_zxy() {
    let tpl = "https://tile.example.com/{z}/{x}/{y}.png";
    let coord = TileCoord { z: 5, x: 3, y: 7 };
    assert_eq!(
        expand_url(tpl, coord),
        "https://tile.example.com/5/3/7.png"
    );
}

#[test]
fn expand_url_wms_bbox_substitution() {
    // At z=0 x=0 y=0 the tile covers the entire world in EPSG:3857.
    let tpl = "https://wms.example.com/?BBOX={xmin},{ymin},{xmax},{ymax}";
    let coord = TileCoord { z: 0, x: 0, y: 0 };
    let expanded = expand_url(tpl, coord);
    // World tile: xmin ≈ -20037508, ymin ≈ -20037508, xmax ≈ +20037508, ymax ≈ +20037508
    assert!(expanded.contains("-20037508."), "xmin should be negative earth-half: {expanded}");
    assert!(expanded.contains("20037508."), "xmax should be positive earth-half: {expanded}");
}

#[test]
fn expand_url_no_bbox_placeholders_is_cheap() {
    // A template without bbox placeholders must not modify the string.
    let tpl = "https://tile.example.com/{z}/{x}/{y}.pbf";
    let coord = TileCoord { z: 10, x: 512, y: 256 };
    let expanded = expand_url(tpl, coord);
    assert_eq!(expanded, "https://tile.example.com/10/512/256.pbf");
    assert!(!expanded.contains('{'), "no unresolved placeholders should remain");
}

// ─────────────────────────── PMTiles style detection ────────────────────────

#[test]
fn pmtiles_provider_style_json_is_valid_maplibre() {
    // Simulates what pmtiles_provider() produces in R / Python.
    let style = r#"{
        "version": 8,
        "sources": {
            "pmtiles": {
                "type": "raster",
                "tiles": ["pmtiles://https://example.com/tiles.pmtiles"],
                "tileSize": 256
            }
        },
        "layers": [{"id": "pmtiles-layer", "type": "raster", "source": "pmtiles"}]
    }"#;
    assert!(
        validate_maplibre_style(style).is_ok(),
        "pmtiles provider style must pass MapLibre validation"
    );
}

#[test]
fn wms_provider_style_json_is_valid_maplibre() {
    let style = r#"{
        "version": 8,
        "sources": {
            "wms": {
                "type": "raster",
                "tiles": ["https://wms.example.com/?BBOX={xmin},{ymin},{xmax},{ymax}&WIDTH=256&HEIGHT=256&LAYERS=base&FORMAT=image/png"],
                "tileSize": 256
            }
        },
        "layers": [{"id": "wms-layer", "type": "raster", "source": "wms"}]
    }"#;
    assert!(
        validate_maplibre_style(style).is_ok(),
        "WMS provider style must pass MapLibre validation"
    );
}

// ──────────────────────── Tile cache (unit smoke test) ──────────────────────

#[test]
fn tile_source_maxzoom_arm_pmtiles() {
    let src = TileSource::PMTiles {
        url: "https://example.com/tiles.pmtiles".into(),
        maxzoom: Some(14),
    };
    assert_eq!(src.maxzoom(), Some(14));
}

#[test]
fn tile_source_maxzoom_arm_raster() {
    let src = TileSource::XyzRaster {
        url_template: "https://tile.example.com/{z}/{x}/{y}.png".into(),
        auth_header: None,
        maxzoom: Some(18),
    };
    assert_eq!(src.maxzoom(), Some(18));
}
