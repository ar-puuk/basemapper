//! The view-resolution contract (docs/design/api-review.md §4.2): cases in
//! `fixtures/contract/cases.json`, written by `tools/contract_cases.py`
//! from the design's formulas. The R and Python suites run the same file,
//! so every language resolves a view the same way.
//!
//! `contract_cases_are_well_formed` checks the file; `contract_cases_resolve`
//! runs every case through `view::resolve` and `view::around`.

use basemapper_core::view::{self, Context, Device, Units, ViewInputs};
use serde_json::Value;

fn cases() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/contract/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("contract cases")).expect("valid JSON")
}

const INPUTS: &[&str] = &["bounds_lonlat", "center", "zoom", "width", "height", "units", "dpi"];
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

fn num(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn pair(v: &Value) -> Option<(f64, f64)> {
    let a = v.as_array()?;
    Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
}

fn inputs_of(c: &Value) -> ViewInputs {
    let i = &c["inputs"];
    let bounds = i["bounds_lonlat"].as_array().map(|b| {
        let v: Vec<f64> = b.iter().map(|x| x.as_f64().unwrap()).collect();
        let (x0, y0) = view::lonlat_to_mercator(v[0], v[1]);
        let (x1, y1) = view::lonlat_to_mercator(v[2], v[3]);
        [x0, y0, x1, y1]
    });
    let size = match (num(&i["width"]), num(&i["height"])) {
        (Some(w), Some(h)) => Some((w, h)),
        _ => None,
    };
    ViewInputs {
        bounds,
        center: pair(&i["center"]),
        zoom: num(&i["zoom"]),
        size,
        units: i["units"].as_str().map(|u| Units::parse(u).unwrap()).unwrap_or_default(),
        dpi: num(&i["dpi"]),
    }
}

fn context_of(c: &Value) -> Context {
    let Some(p) = c.get("context").map(|ctx| &ctx["plot"]) else { return Context::Standalone };
    let e: Vec<f64> = p["extent_3857"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    let px = pair(&p["panel_px"]).unwrap();
    let device = match p["device"].as_str().unwrap() {
        "screen" => Device::Screen { pixel_ratio: p["pixel_ratio"].as_f64().unwrap_or(1.0) },
        "raster_export" => Device::RasterExport { dpi: p["dpi"].as_f64().unwrap() },
        _ => Device::VectorExport,
    };
    Context::Plot { extent: [e[0], e[1], e[2], e[3]], panel_px: (px.0 as u32, px.1 as u32), device }
}

#[test]
fn contract_cases_resolve() {
    let all = cases();
    let mut failures = Vec::new();
    for c in all["views"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let tol = &c["tolerance"];
        let got = view::resolve(&inputs_of(c), &context_of(c));
        if let Some(err) = c.get("error") {
            match got {
                Ok(v) => failures.push(format!("{name}: expected an error, got {v:?}")),
                Err(e) => {
                    let msg = e.to_string();
                    for word in err["contains"].as_array().unwrap() {
                        let w = word.as_str().unwrap();
                        if !msg.contains(w) {
                            failures.push(format!("{name}: error {msg:?} lacks {w:?}"));
                        }
                    }
                }
            }
            continue;
        }
        let e = &c["expect"];
        let v = match got {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("{name}: unexpected error: {err}"));
                continue;
            }
        };
        let mut check = |what: &str, got: f64, want: f64, t: f64| {
            if (got - want).abs() > t {
                failures.push(format!("{name}: {what} = {got}, expected {want}"));
            }
        };
        check("width", v.width as f64, e["width"].as_f64().unwrap(), tol["px"].as_f64().unwrap());
        check("height", v.height as f64, e["height"].as_f64().unwrap(), tol["px"].as_f64().unwrap());
        check("dpi", v.dpi, e["dpi"].as_f64().unwrap(), tol["dpi"].as_f64().unwrap());
        check("zoom", v.zoom, e["zoom"].as_f64().unwrap(), tol["zoom"].as_f64().unwrap());
        for (k, want) in e["bounds_3857"].as_array().unwrap().iter().enumerate() {
            check(&format!("bounds[{k}]"), v.bounds[k], want.as_f64().unwrap(), tol["metres"].as_f64().unwrap());
        }
        let p = &e["provenance"];
        for (what, got) in [("extent", v.provenance.extent), ("size", v.provenance.size), ("dpi", v.provenance.dpi), ("zoom", v.provenance.zoom)] {
            if p[what].as_str() != Some(got.name()) {
                failures.push(format!("{name}: provenance.{what} = {}, expected {}", got.name(), p[what]));
            }
        }
        let want_notes: Vec<(String, String)> = e["notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| (n["level"].as_str().unwrap().to_owned(), n["kind"].as_str().unwrap().to_owned()))
            .collect();
        let got_notes: Vec<(String, String)> =
            v.notes.iter().map(|n| (n.level.name().to_owned(), n.kind.to_owned())).collect();
        if want_notes != got_notes {
            failures.push(format!("{name}: notes {got_notes:?}, expected {want_notes:?}"));
        }
    }
    for a in all["around"].as_array().unwrap() {
        let name = a["name"].as_str().unwrap();
        let center = pair(&a["center"]).unwrap();
        let span: Vec<f64> = a["span"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        let per = view::distance_unit(a["units"].as_str().unwrap_or("m")).unwrap();
        let span = if span.len() == 1 { (span[0] * per, span[0] * per) } else { (span[0] * per, span[1] * per) };
        let got = view::around(center, span).unwrap();
        for (k, want) in a["expect_lonlat"].as_array().unwrap().iter().enumerate() {
            if (got[k] - want.as_f64().unwrap()).abs() > 1e-9 {
                failures.push(format!("around {name}: [{k}] = {}, expected {want}", got[k]));
            }
        }
    }
    assert!(failures.is_empty(), "{} contract failures:\n{}", failures.len(), failures.join("\n"));
}
