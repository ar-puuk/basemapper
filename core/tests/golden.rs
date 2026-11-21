//! Golden-image render tests: basemapper vs MapLibre GL JS.
//!
//! Each directory under `tests/fixtures/golden/` holds an offline fixture
//! captured by `examples/capture_fixture.rs`:
//!
//! - `case.json`    bbox / width / height / zoom / optional layer filter
//!                  (`fixture_from` reuses another case's style + tiles), and
//!                  the case's fidelity limits `max_bad_pixel_frac` /
//!                  `max_mean_diff`
//! - `style.json`   style whose tile source points at `{{BASE}}/{z}/{x}/{y}`
//! - `tiles/z/x/y`  raw tile bytes
//! - `reference-maplibre-gl.png`  MapLibre GL JS render of the same view, from
//!   `tools/reference-render`. This is the target.
//!
//! The tiles are served from a local mock server, so the test never touches the
//! network. A case fails if basemapper's render differs from the MapLibre
//! reference by more than its limits. The limits are a ratchet: when a change
//! improves fidelity, lower them in `case.json` (the test prints a hint) so the
//! gain can't silently regress. The goal is 0.
//!
//! `actual.png` and `diff.png` (differing pixels in red) are always written to
//! `target/golden-output/<case>/` for inspection.

use basemapper_core::{render, RenderRequest, StyleInput};
use httptest::{matchers::request::method_path, responders::status_code, Expectation, Server};
use image::{Rgba, RgbaImage};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A channel difference above this counts the pixel as "bad".
const BAD_CHANNEL_DIFF: u8 = 24;
/// Limits when a case does not set its own: exact parity.
const DEFAULT_MAX_MEAN_DIFF: f64 = 0.0;
const DEFAULT_MAX_BAD_FRAC: f64 = 0.0;

fn golden_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden")
}

/// Serve every file under `tiles/` at `/{z}/{x}/{y}`.
fn serve_tiles(server: &Server, tiles_dir: &Path) {
    for z in std::fs::read_dir(tiles_dir).expect("tiles dir") {
        let z = z.unwrap().path();
        for x in std::fs::read_dir(&z).unwrap() {
            let x = x.unwrap().path();
            for y in std::fs::read_dir(&x).unwrap() {
                let y = y.unwrap().path();
                let url_path = format!(
                    "/{}/{}/{}",
                    z.file_name().unwrap().to_str().unwrap(),
                    x.file_name().unwrap().to_str().unwrap(),
                    y.file_name().unwrap().to_str().unwrap()
                );
                let bytes = std::fs::read(&y).unwrap();
                server.expect(
                    Expectation::matching(method_path("GET", url_path))
                        .times(..)
                        .respond_with(status_code(200).body(bytes)),
                );
            }
        }
    }
}

struct Diff {
    mean: f64,
    bad_frac: f64,
    image: RgbaImage,
}

fn compare(actual: &RgbaImage, expected: &RgbaImage) -> Diff {
    let mut total: u64 = 0;
    let mut bad: u64 = 0;
    let mut image = RgbaImage::new(actual.width(), actual.height());
    for (x, y, a) in actual.enumerate_pixels() {
        let e = expected.get_pixel(x, y);
        let max_d = (0..4).map(|i| a[i].abs_diff(e[i])).max().unwrap();
        total += (0..4).map(|i| a[i].abs_diff(e[i]) as u64).sum::<u64>();
        if max_d > BAD_CHANNEL_DIFF {
            bad += 1;
            image.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        } else {
            // Faded expected image as context.
            let g = ((e[0] as u16 + e[1] as u16 + e[2] as u16) / 3 / 4 + 191) as u8;
            image.put_pixel(x, y, Rgba([g, g, g, 255]));
        }
    }
    let n = (actual.width() * actual.height()) as f64;
    Diff { mean: total as f64 / (n * 4.0), bad_frac: bad as f64 / n, image }
}

/// Render one case; returns an error message on mismatch.
fn run_case(dir: &Path) -> Result<String, String> {
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let case: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("case.json")).unwrap()).unwrap();

    let fixture_dir = match case["fixture_from"].as_str() {
        Some(other) => golden_root().join(other),
        None => dir.to_path_buf(),
    };
    let style_template = std::fs::read_to_string(fixture_dir.join("style.json")).unwrap();
    let server = Server::run();
    serve_tiles(&server, &fixture_dir.join("tiles"));
    let base = server.url_str("");
    let style = style_template.replace("{{BASE}}", base.trim_end_matches('/'));

    let bbox: Vec<f64> =
        case["bbox"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let width = case["width"].as_u64().unwrap() as u32;
    let height = case["height"].as_u64().unwrap() as u32;
    let layers = case["layers"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_str().unwrap().to_owned()).collect());

    let request = RenderRequest {
        bbox: [bbox[0], bbox[1], bbox[2], bbox[3]],
        width,
        height,
        style_input: StyleInput::InlineJson(style),
        zoom: case["zoom"].as_u64().map(|z| z as u8),
        tile_timeout_ms: 10_000,
        max_tiles: 256,
        tile_concurrency: 8,
        layers,
        auth_token: None,
        fail_on_tile_error: true,
    };
    let result = render(request).map_err(|e| format!("{name}: render failed: {e}"))?;
    let actual = RgbaImage::from_raw(result.width, result.height, result.pixels)
        .ok_or_else(|| format!("{name}: pixel buffer size mismatch"))?;

    let reference_path = dir.join("reference-maplibre-gl.png");
    let reference = image::open(&reference_path)
        .map_err(|e| {
            format!(
                "{name}: cannot read {} ({e}); render it with tools/reference-render",
                reference_path.display()
            )
        })?
        .to_rgba8();
    if reference.dimensions() != actual.dimensions() {
        return Err(format!(
            "{name}: size {:?} != reference {:?}",
            actual.dimensions(),
            reference.dimensions()
        ));
    }

    let diff = compare(&actual, &reference);
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/golden-output").join(&name);
    std::fs::create_dir_all(&out).unwrap();
    actual.save(out.join("actual.png")).unwrap();
    diff.image.save(out.join("diff.png")).unwrap();

    let max_mean = case["max_mean_diff"].as_f64().unwrap_or(DEFAULT_MAX_MEAN_DIFF);
    let max_bad = case["max_bad_pixel_frac"].as_f64().unwrap_or(DEFAULT_MAX_BAD_FRAC);
    let summary = format!(
        "{name}: vs MapLibre GL: bad pixels {:.3}% (limit {:.3}%), mean diff {:.3} (limit {max_mean})",
        diff.bad_frac * 100.0,
        max_bad * 100.0,
        diff.mean,
    );
    if diff.mean > max_mean + 1e-9 || diff.bad_frac > max_bad + 1e-9 {
        return Err(format!("{summary} — see {}", out.display()));
    }
    let hint = if diff.bad_frac < max_bad * 0.9 || diff.mean < max_mean * 0.9 {
        format!(
            "  [ratchet: tighten to max_bad_pixel_frac {:.5}, max_mean_diff {:.3}]",
            diff.bad_frac + 0.00005,
            diff.mean + 0.005
        )
    } else {
        String::new()
    };
    Ok(summary + &hint)
}

#[test]
fn golden_images() {
    let mut cases: Vec<PathBuf> = std::fs::read_dir(golden_root())
        .expect("golden fixture dir")
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("case.json").exists())
        .collect();
    cases.sort();
    assert!(!cases.is_empty(), "no golden cases found");

    let mut failures = Vec::new();
    for dir in &cases {
        match run_case(dir) {
            Ok(msg) => println!("ok   {msg}"),
            Err(msg) => failures.push(msg),
        }
    }
    assert!(failures.is_empty(), "golden mismatches:\n{}", failures.join("\n"));
}
