//! Golden-image render tests: basemapper vs MapLibre GL JS.
//!
//! Each directory under `tests/fixtures/golden/` holds an offline fixture
//! captured by `examples/capture_fixture.rs`:
//!
//! - `case.json`    bbox / width / height / zoom / optional layer filter
//!                  (`fixture_from` reuses another case's style + tiles;
//!                  `tiles_from` / `fonts_from` reuse only its tiles / glyphs), and
//!                  the case's fidelity limits `max_bad_pixel_frac` /
//!                  `max_mean_diff`
//! - `style.json`   style whose URLs point at `{{BASE}}/tiles|fonts|sprites/...`
//! - `tiles/z/x/y`  raw tile bytes; `fonts/<fontstack>/<range>.pbf` glyphs and
//!   `sprites/sprite.{json,png}` when the style uses them
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
//! `target/golden-output/<case>/`, and `check/manifest.js` (repo root,
//! gitignored) is refreshed for the `check/index.html` comparison viewer.

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

/// Percent-encode a URL path segment the way HTTP clients send it.
fn encode_segment(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b',' | b'!' | b'$' | b'&'
            | b'\'' | b'(' | b')' | b'*' | b'+' | b';' | b'=' | b':' | b'@' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Serve the fixture's `tiles/`, `fonts/` and `sprites/` trees at the same
/// relative paths (`/tiles/{z}/{x}/{y}`, `/fonts/{fontstack}/{range}.pbf`, ...).
fn serve_fixture(server: &Server, dir: &Path, tiles_dir: &Path, fonts_dir: &Path) {
    fn walk(server: &Server, root: &Path, dir: &Path) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(server, root, &p);
                continue;
            }
            let rel = p.strip_prefix(root).unwrap();
            let url_path: String = rel
                .components()
                .map(|c| format!("/{}", encode_segment(&c.as_os_str().to_string_lossy())))
                .collect();
            let bytes = std::fs::read(&p).unwrap();
            server.expect(
                Expectation::matching(method_path("GET", url_path))
                    .times(..)
                    .respond_with(status_code(200).body(bytes)),
            );
        }
    }
    if tiles_dir.join("tiles").is_dir() {
        walk(server, tiles_dir, &tiles_dir.join("tiles"));
    }
    if fonts_dir.join("fonts").is_dir() {
        walk(server, fonts_dir, &fonts_dir.join("fonts"));
    }
    if dir.join("sprites").is_dir() {
        walk(server, dir, &dir.join("sprites"));
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

/// Render one case; returns an error message on mismatch. Appends the
/// case's paths and scores to `manifest` for the `check/` viewer.
fn run_case(dir: &Path, manifest: &mut Vec<Value>) -> Result<String, String> {
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let case: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("case.json")).unwrap()).unwrap();

    let fixture_dir = match case["fixture_from"].as_str() {
        Some(other) => golden_root().join(other),
        None => dir.to_path_buf(),
    };
    let style_template = std::fs::read_to_string(fixture_dir.join("style.json")).unwrap();
    let server = Server::run();
    let tiles_dir = match case["tiles_from"].as_str() {
        Some(other) => golden_root().join(other),
        None => fixture_dir.clone(),
    };
    let fonts_dir = match case["fonts_from"].as_str() {
        Some(other) => golden_root().join(other),
        None => fixture_dir.clone(),
    };
    // Anything not in the fixture is a 404, as a real server would answer
    // (e.g. a glyph range MapLibre never requested). Registered first:
    // httptest prefers the most recently added matching expectation.
    server.expect(
        Expectation::matching(httptest::matchers::any())
            .times(..)
            .respond_with(status_code(404)),
    );
    serve_fixture(&server, &fixture_dir, &tiles_dir, &fonts_dir);
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
    let pass = diff.mean <= max_mean + 1e-9 && diff.bad_frac <= max_bad + 1e-9;

    // Optional geometry-only score against the no-labels reference.
    let nosym_path = dir.join("reference-maplibre-gl-nosymbols.png");
    let geometry = image::open(&nosym_path)
        .ok()
        .map(|r| r.to_rgba8())
        .filter(|r| r.dimensions() == actual.dimensions())
        .map(|r| {
            let d = compare(&actual, &r);
            serde_json::json!({ "bad": d.bad_frac, "mean": d.mean })
        });
    let mpp = (bbox[2] - bbox[0]) / width as f64;
    let gl_zoom = (40_075_016.685_578_49 / (512.0 * mpp)).log2();
    let group = name
        .rsplit_once("_z")
        .filter(|(_, z)| z.chars().all(|c| c.is_ascii_digit()))
        .map(|(g, _)| g.to_owned())
        .unwrap_or_else(|| name.trim_end_matches("_slc").to_owned());
    manifest.push(serde_json::json!({
        "name": name,
        "group": group,
        "zoom": (gl_zoom * 1000.0).round() / 1000.0,
        "width": width,
        "height": height,
        "ours": format!("../target/golden-output/{name}/actual.png"),
        "maplibre": format!("../core/tests/fixtures/golden/{name}/reference-maplibre-gl.png"),
        "diff": format!("../target/golden-output/{name}/diff.png"),
        "maplibre_nosymbols": nosym_path.exists()
            .then(|| format!("../core/tests/fixtures/golden/{name}/reference-maplibre-gl-nosymbols.png")),
        "bad": diff.bad_frac,
        "mean": diff.mean,
        "max_bad": max_bad,
        "max_mean": max_mean,
        "pass": pass,
        "geometry": geometry,
    }));
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
    let mut manifest = Vec::new();
    for dir in &cases {
        match run_case(dir, &mut manifest) {
            Ok(msg) => println!("ok   {msg}"),
            Err(msg) => failures.push(msg),
        }
    }
    write_check_manifest(&manifest);
    assert!(failures.is_empty(), "golden mismatches:\n{}", failures.join("\n"));
}

/// Write `check/manifest.js` (repo root, gitignored) for `check/index.html`,
/// the side-by-side / swipe viewer. A script rather than JSON so the page
/// works when opened straight from disk (file://).
fn write_check_manifest(cases: &[Value]) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../check");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let generated = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let body = serde_json::json!({ "generated": generated, "cases": cases });
    let _ = std::fs::write(
        dir.join("manifest.js"),
        format!("window.GOLDEN = {};\n", serde_json::to_string_pretty(&body).unwrap()),
    );
}
