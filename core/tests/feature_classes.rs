//! How the catalogue's vector styles sort into features (`features =`).
//!
//! `tests/fixtures/styles/` holds each style's layers (id, type, source,
//! source-layer: all the classifier reads). The pinned test checks the
//! feature counts, so a change to the classifier shows up here. The ignored
//! live test catches providers changing their styles (as CARTO moved to the
//! Protomaps schema in 2026):
//!
//! ```sh
//! cargo test -p basemapper-core --test feature_classes -- --ignored
//! # after reviewing a change, refresh the fixtures:
//! BASEMAPPER_UPDATE_FIXTURES=1 cargo test -p basemapper-core --test feature_classes -- --ignored
//! ```

use basemapper_core::features::layer_feature;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/styles");

/// (fixture, live style URL)
const STYLES: &[(&str, &str)] = &[
    ("openfreemap_liberty", "https://tiles.openfreemap.org/styles/liberty"),
    ("openfreemap_positron", "https://tiles.openfreemap.org/styles/positron"),
    ("stadia_alidade_smooth", "https://tiles.stadiamaps.com/styles/alidade_smooth.json"),
    ("carto_positron", "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json"),
    ("versatiles_colorful", "https://tiles.versatiles.org/assets/styles/colorful/style.json"),
    (
        "esri_street",
        "https://www.arcgis.com/sharing/rest/content/items/de26a3cf4cc9451298ea173c4b324736/resources/styles/root.json",
    ),
    (
        "esri_topo",
        "https://www.arcgis.com/sharing/rest/content/items/7dc6cea0b1764a1f9af2e679f642f0f5/resources/styles/root.json",
    ),
    (
        "esri_gray",
        "https://www.arcgis.com/sharing/rest/content/items/291da5eab3a0412593b66d384379f89f/resources/styles/root.json",
    ),
];

/// Feature counts per fixture, as reviewed.
const EXPECTED: &[(&str, &[(&str, usize)])] = &[
    ("openfreemap_liberty", &[
        ("background", 1), ("boundaries", 3), ("buildings", 2), ("landcover", 11), ("parks", 2), ("places", 9),
        ("poi", 5), ("rail", 12), ("roads", 58), ("terrain", 1), ("water", 7),
    ]),
    ("openfreemap_positron", &[
        ("background", 1), ("boundaries", 3), ("buildings", 1), ("landcover", 4), ("parks", 1), ("places", 9),
        ("poi", 1), ("rail", 6), ("roads", 24), ("water", 5),
    ]),
    ("stadia_alidade_smooth", &[
        ("background", 1), ("boundaries", 2), ("buildings", 1), ("landcover", 4), ("parks", 2), ("places", 12),
        ("poi", 4), ("rail", 4), ("roads", 18), ("water", 5),
    ]),
    ("carto_positron", &[
        ("background", 1), ("boundaries", 4), ("buildings", 3), ("landcover", 4), ("parks", 2), ("places", 15),
        ("poi", 3), ("rail", 4), ("roads", 51), ("water", 8),
    ]),
    ("versatiles_colorful", &[
        ("background", 5), ("boundaries", 11), ("buildings", 3), ("landcover", 30), ("parks", 1), ("places", 9),
        ("poi", 9), ("rail", 29), ("roads", 152), ("water", 17),
    ]),
    ("esri_street", &[
        ("background", 2), ("boundaries", 15), ("buildings", 4), ("landcover", 44), ("parks", 14), ("places", 35),
        ("poi", 600), ("rail", 10), ("roads", 148), ("water", 44),
    ]),
    ("esri_topo", &[
        ("background", 2), ("boundaries", 14), ("buildings", 18), ("landcover", 66), ("parks", 14), ("places", 35),
        ("poi", 23), ("rail", 9), ("roads", 149), ("terrain", 13), ("water", 55),
    ]),
    ("esri_gray", &[
        ("background", 2), ("boundaries", 15), ("buildings", 2), ("landcover", 23), ("parks", 4), ("rail", 6),
        ("roads", 47), ("water", 22),
    ]),
];

/// A style's layers with only what the classifier reads.
fn reduce(style: &Value) -> Value {
    let layers: Vec<Value> = style["layers"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .map(|l| {
            let mut o = serde_json::Map::new();
            for k in ["id", "type", "source", "source-layer"] {
                if let Some(v) = l.get(k) {
                    o.insert(k.into(), v.clone());
                }
            }
            Value::Object(o)
        })
        .collect();
    json!({ "layers": layers })
}

fn counts(style: &Value) -> BTreeMap<&'static str, usize> {
    let mut c = BTreeMap::new();
    for l in style["layers"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        *c.entry(layer_feature(l)).or_insert(0) += 1;
    }
    c
}

fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(format!("{FIXTURES}/{name}.json")).unwrap_or_else(|e| panic!("{name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn catalogue_styles_classify_as_pinned() {
    assert_eq!(STYLES.len(), EXPECTED.len());
    for (name, expected) in EXPECTED {
        let got = counts(&fixture(name));
        let expected: BTreeMap<&str, usize> = expected.iter().copied().collect();
        assert!(!got.contains_key("other"), "{name}: layers classified as other: {got:?}");
        assert_eq!(got, expected, "{name}: feature counts changed");
    }
}

#[test]
#[ignore = "fetches the live styles; run to catch provider changes"]
fn live_styles_still_classify() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = reqwest::Client::builder().user_agent("basemapper-tests").build().unwrap();
    let update = std::env::var_os("BASEMAPPER_UPDATE_FIXTURES").is_some();
    let mut problems = Vec::new();
    for (name, url) in STYLES {
        let style: Value = rt.block_on(async { client.get(*url).send().await?.json().await }).unwrap();
        let live = reduce(&style);
        if update {
            std::fs::create_dir_all(FIXTURES).unwrap();
            std::fs::write(format!("{FIXTURES}/{name}.json"), serde_json::to_string(&live).unwrap()).unwrap();
            continue;
        }
        let (now, before) = (counts(&live), counts(&fixture(name)));
        let other: Vec<&str> = live["layers"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|l| layer_feature(l) == "other")
            .filter_map(|l| l["id"].as_str())
            .collect();
        if !other.is_empty() {
            problems.push(format!("{name}: classified as other: {other:?}"));
        }
        let (k_now, k_before): (Vec<_>, Vec<_>) = (now.keys().collect(), before.keys().collect());
        if k_now != k_before {
            problems.push(format!("{name}: features were {k_before:?}, now {k_now:?}"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
