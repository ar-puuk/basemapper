//! Render benchmark on golden fixtures, served from a local mock server
//! (offline, deterministic).
//!
//! For each case: one cold render (empty in-process caches), then warm
//! renders of the same view, as a plot redrawn on resize. Reports the time
//! and the HTTP requests each render makes: requests per warm render are
//! what a real network makes slow, and the local server hides their
//! latency.
//!
//! ```sh
//! cargo run --release -p basemapper-core --example bench_render
//! cargo run --release -p basemapper-core --example bench_render -- carto_positron
//! BASEMAPPER_BENCH_RUNS=10 cargo run --release -p basemapper-core --example bench_render
//! ```
//!
//! Results also go to `target/bench-render.json`, for before/after
//! comparison.

#[path = "../tests/support/fixture.rs"]
mod fixture;

use std::time::{Duration, Instant};

/// A city view with labels, a dense mid zoom, a 2x render, a big stacked
/// view, a PMTiles archive, raster tiles and a hillshade.
const DEFAULT_CASES: &[&str] = &[
    "carto_positron_z13",
    "carto_voyager_z10",
    "carto_dark_matter_z7_x2",
    "ugrc_full_slc",
    "pmtiles_liberty_z5",
    "osm_raster_slc",
    "synthetic_hillshade_combined_x2",
];

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn main() {
    // Arguments pick cases by name (substring).
    let filter: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with("--")).collect();
    let cases: Vec<String> = match std::env::var("BASEMAPPER_BENCH_CASES") {
        Ok(v) => v.split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect(),
        Err(_) => DEFAULT_CASES.iter().map(|s| s.to_string()).collect(),
    };
    let cases: Vec<String> =
        cases.into_iter().filter(|c| filter.is_empty() || filter.iter().any(|f| c.contains(f.as_str()))).collect();
    let runs: usize = std::env::var("BASEMAPPER_BENCH_RUNS").ok().and_then(|v| v.parse().ok()).unwrap_or(5).max(1);

    println!(
        "{:<34} {:>9} {:>9} {:>11} {:>11} {:>10}",
        "case", "cold ms", "cold req", "warm ms", "warm min", "warm req"
    );
    let mut rows = Vec::new();
    for name in &cases {
        let dir = fixture::golden_root().join(name);
        let Ok(text) = std::fs::read_to_string(dir.join("case.json")) else {
            eprintln!("{name}: no such fixture, skipped");
            continue;
        };
        let case: serde_json::Value = serde_json::from_str(&text).unwrap();
        // A stack case renders its member cases' styles bottom to top.
        let fixtures: Vec<fixture::Fixture> = match case["stack"].as_array() {
            Some(members) => members
                .iter()
                .map(|m| {
                    let mdir = fixture::golden_root().join(m.as_str().unwrap());
                    let mcase = serde_json::from_str(&std::fs::read_to_string(mdir.join("case.json")).unwrap()).unwrap();
                    fixture::load(&mdir, &mcase)
                })
                .collect(),
            None => vec![fixture::load(&dir, &case)],
        };
        let hits = || fixtures.iter().map(|f| f.hits.get()).sum::<usize>();
        let render = || basemapper_core::render_stack(fixtures.iter().map(|f| f.request.clone()).collect());

        let t = Instant::now();
        let before = hits();
        if let Err(e) = render() {
            eprintln!("{name}: render failed: {e}");
            continue;
        }
        let (cold_ms, cold_req) = (ms(t.elapsed()), hits() - before);

        let mut times = Vec::with_capacity(runs);
        let before = hits();
        for _ in 0..runs {
            let t = Instant::now();
            render().expect("warm render");
            times.push(ms(t.elapsed()));
        }
        let warm_req = (hits() - before) as f64 / runs as f64;
        let (warm_med, warm_min) = (median(times.clone()), times.iter().cloned().fold(f64::INFINITY, f64::min));
        println!(
            "{name:<34} {cold_ms:>9.1} {cold_req:>9} {warm_med:>11.1} {warm_min:>11.1} {warm_req:>10.1}"
        );
        rows.push(serde_json::json!({
            "case": name, "cold_ms": cold_ms, "cold_requests": cold_req,
            "warm_ms_median": warm_med, "warm_ms_min": warm_min, "warm_requests": warm_req, "runs": runs,
        }));
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/bench-render.json");
    if std::fs::write(&out, serde_json::to_string_pretty(&rows).unwrap()).is_ok() {
        println!("\nwritten to {}", out.display());
    }
}
