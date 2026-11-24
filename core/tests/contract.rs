//! The view-resolution contract (docs/design/api-review.md §4.2): cases in
//! `fixtures/contract/cases.json`, written by `tools/contract_cases.py`
//! from the design's formulas. The R and Python suites run the same file,
//! so every language resolves a view the same way.
//!
//! Phase 0 checks the cases are well formed. Resolving them needs
//! `view::resolve` (implementation plan, phase 1); that test is ignored
//! until then.

use serde_json::Value;

fn cases() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/contract/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("contract cases")).expect("valid JSON")
}

const INPUTS: &[&str] = &["bounds_lonlat", "center", "zoom", "width", "height", "dpi"];
const PROVENANCE: &[&str] = &["given", "derived", "default", "context"];
const LEVELS: &[&str] = &["decided", "adjusted", "problem"];

#[test]
fn contract_cases_are_well_formed() {
    let all = cases();
    assert_eq!(all["version"], 1);
    let views = all["views"].as_array().expect("views");
    assert!(views.len() >= 17, "{} view cases", views.len());
    let mut names = std::collections::HashSet::new();
    for c in views {
        let name = c["name"].as_str().expect("name");
        assert!(names.insert(name), "{name}: duplicate name");
        assert!(c["doc"].as_str().is_some_and(|d| d.contains('§') || d.starts_with('F')), "{name}: doc reference");
        for key in c["inputs"].as_object().expect("inputs").keys() {
            assert!(INPUTS.contains(&key.as_str()), "{name}: unknown input {key}");
        }
        match (c.get("expect"), c.get("error")) {
            (Some(e), None) => {
                for k in ["width", "height", "dpi", "zoom"] {
                    assert!(e[k].as_f64().is_some(), "{name}: expect.{k}");
                }
                let b = e["bounds_3857"].as_array().expect("bounds_3857");
                assert_eq!(b.len(), 4, "{name}: bounds_3857");
                assert!(b[3].as_f64() > b[1].as_f64(), "{name}: ymax > ymin");
                for (k, v) in e["provenance"].as_object().expect("provenance") {
                    assert!(["extent", "size", "dpi", "zoom"].contains(&k.as_str()), "{name}: provenance.{k}");
                    assert!(PROVENANCE.contains(&v.as_str().unwrap_or("")), "{name}: provenance.{k} = {v}");
                }
                for n in e["notes"].as_array().expect("notes") {
                    assert!(LEVELS.contains(&n["level"].as_str().unwrap_or("")), "{name}: note level");
                    assert!(n["kind"].as_str().is_some(), "{name}: note kind");
                }
            }
            (None, Some(err)) => {
                assert_eq!(err["kind"], "validation", "{name}: error kind");
                assert!(err["contains"].as_array().is_some_and(|a| !a.is_empty()), "{name}: error.contains");
            }
            _ => panic!("{name}: needs exactly one of expect / error"),
        }
        if let Some(ctx) = c.get("context") {
            let p = &ctx["plot"];
            assert!(p["extent_3857"].as_array().is_some_and(|a| a.len() == 4), "{name}: plot extent");
            assert!(["screen", "raster_export", "vector_export"].contains(&p["device"].as_str().unwrap_or("")), "{name}: device");
        }
    }
    let arounds = all["around"].as_array().expect("around cases");
    assert!(!arounds.is_empty());
    for a in arounds {
        let e = a["expect_lonlat"].as_array().expect("expect_lonlat");
        assert!(e[0].as_f64() < e[2].as_f64() && e[1].as_f64() < e[3].as_f64(), "{}: a valid box", a["name"]);
    }
}

#[test]
#[ignore = "needs view::resolve: implementation plan, phase 1"]
fn contract_cases_resolve() {
    panic!("view::resolve is not implemented yet (implementation plan, phase 1)");
}
