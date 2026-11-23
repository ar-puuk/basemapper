//! Golden fixtures served from a local mock server (shared by the golden
//! test and the render benchmark): a case's tiles, glyphs, sprites,
//! GeoJSON and packed PMTiles archives, and the request that renders it.

#![allow(dead_code)]

use basemapper_core::{RenderRequest, StyleInput};
use httptest::{matchers::request::method_path, responders::status_code, Expectation, Server};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Requests served (2xx answers), counted across a fixture's files.
#[derive(Clone, Default)]
pub struct Hits(Arc<AtomicUsize>);

impl Hits {
    pub fn get(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

/// A responder that counts the requests it answers.
struct Counted<R>(Hits, R);

impl<R: httptest::responders::Responder> httptest::responders::Responder for Counted<R> {
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = http::Response<hyper::Body>> + Send + 'a>> {
        self.0 .0.fetch_add(1, Ordering::Relaxed);
        self.1.respond(req)
    }
}

/// A case served and ready to render: keep it alive while rendering.
pub struct Fixture {
    pub name: String,
    pub server: Server,
    pub request: RenderRequest,
    /// Requests the server has answered so far.
    pub hits: Hits,
}

pub fn golden_root() -> PathBuf {
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
fn serve_fixture(server: &Server, dir: &Path, tiles_dir: &Path, fonts_dir: &Path, sprites_dir: &Path, hits: &Hits) {
    fn walk(server: &Server, root: &Path, dir: &Path, hits: &Hits) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(server, root, &p, hits);
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
                    .respond_with(Counted(hits.clone(), status_code(200).body(bytes))),
            );
        }
    }
    if tiles_dir.join("tiles").is_dir() {
        walk(server, tiles_dir, &tiles_dir.join("tiles"), hits);
    }
    if fonts_dir.join("fonts").is_dir() {
        walk(server, fonts_dir, &fonts_dir.join("fonts"), hits);
    }
    if sprites_dir.join("sprites").is_dir() {
        walk(server, sprites_dir, &sprites_dir.join("sprites"), hits);
    }
    if dir.join("geojson").is_dir() {
        walk(server, dir, &dir.join("geojson"), hits);
    }
}

/// case.json `pmtiles`: `{"<name>.pmtiles": {"tiles": "<dir under the
/// tiles fixture>", "type": "mvt" | "png" | "jpg" | "webp", "min_zoom": 0,
/// "max_zoom": 14}}`. Each archive is packed from the directory's
/// `z/x/y[.ext]` files (MVT gzip-compressed, as tile archives usually are),
/// covering the world, and returned as (name, bytes).
fn build_pmtiles(tiles_dir: &Path, spec: &Value) -> Vec<(String, Vec<u8>)> {
    use pmtiles::{PmTilesWriter, TileCoord, TileId, TileType};
    let Some(spec) = spec.as_object() else { return Vec::new() };
    spec.iter()
        .map(|(name, a)| {
            let root = tiles_dir.join(a["tiles"].as_str().unwrap());
            let mut tiles: Vec<(u64, TileCoord, Vec<u8>)> = Vec::new();
            for z in std::fs::read_dir(&root).unwrap().flatten() {
                for x in std::fs::read_dir(z.path()).unwrap().flatten() {
                    for y in std::fs::read_dir(x.path()).unwrap().flatten() {
                        let num = |p: &Path| -> u32 {
                            p.file_stem().unwrap().to_string_lossy().parse().unwrap()
                        };
                        let c = TileCoord::new(num(&z.path()) as u8, num(&x.path()), num(&y.path())).unwrap();
                        tiles.push((TileId::from(c).value(), c, std::fs::read(y.path()).unwrap()));
                    }
                }
            }
            tiles.sort_by_key(|t| t.0);
            let tile_type = match a["type"].as_str().unwrap() {
                "mvt" => TileType::Mvt,
                "png" => TileType::Png,
                "jpg" => TileType::Jpeg,
                "webp" => TileType::Webp,
                t => panic!("{name}: unknown tile type {t}"),
            };
            let mut buf = Vec::new();
            let mut w = PmTilesWriter::new(tile_type)
                .min_zoom(a["min_zoom"].as_u64().unwrap_or(0) as u8)
                .max_zoom(a["max_zoom"].as_u64().unwrap() as u8)
                .bounds(-180.0, -85.051129, 180.0, 85.051129)
                .create(std::io::Cursor::new(&mut buf))
                .unwrap();
            for (_, c, data) in &tiles {
                w.add_tile(*c, data).unwrap();
            }
            w.finalize().unwrap();
            (name.clone(), buf)
        })
        .collect()
}

/// Serves a file with HTTP range requests (206 Partial Content).
#[derive(Clone)]
struct RangeFile(std::sync::Arc<Vec<u8>>);

impl httptest::responders::Responder for RangeFile {
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = http::Response<hyper::Body>> + Send + 'a>> {
        let data = self.0.clone();
        let range = req
            .headers()
            .get("range")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("bytes="))
            .and_then(|v| v.split_once('-'))
            .and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?)));
        Box::pin(async move {
            match range {
                Some((a, b)) if a < data.len() => {
                    let b = b.min(data.len() - 1);
                    http::Response::builder()
                        .status(206)
                        .header("content-range", format!("bytes {a}-{b}/{}", data.len()))
                        .body(hyper::Body::from(data[a..=b].to_vec()))
                        .unwrap()
                }
                Some(_) => http::Response::builder().status(416).body(hyper::Body::empty()).unwrap(),
                None => http::Response::builder().status(200).body(hyper::Body::from(data.to_vec())).unwrap(),
            }
        })
    }
}

/// Serve the case in `dir` (with its `case.json` already read as `case`).
pub fn load(dir: &Path, case: &Value) -> Fixture {
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
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
    let sprites_dir = match case["sprites_from"].as_str() {
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
    let hits = Hits::default();
    serve_fixture(&server, &fixture_dir, &tiles_dir, &fonts_dir, &sprites_dir, &hits);
    // PMTiles archives, also written next to the outputs for
    // tools/reference-render (which serves the same bytes to MapLibre).
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/golden-output").join(&name);
    for (file, bytes) in build_pmtiles(&tiles_dir, &case["pmtiles"]) {
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join(&file), &bytes).unwrap();
        server.expect(
            Expectation::matching(method_path("GET", format!("/{file}")))
                .times(..)
                .respond_with(Counted(hits.clone(), RangeFile(std::sync::Arc::new(bytes)))),
        );
    }
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
        // Automatic zoom, as users get by default (case.json "zoom" records
        // the capture zoom only).
        tile_zoom: None,
        tile_timeout_ms: 10_000,
        max_tiles: 256,
        tile_concurrency: 8,
        layers,
        labels: basemapper_core::Labels::All,
        features: None,
        auth_token: None,
        // Tiles outside a source's coverage are left out of fixtures (404);
        // MapLibre renders them as empty, so skip rather than fail.
        fail_on_tile_error: false,
        pixel_ratio: case["pixel_ratio"].as_f64().unwrap_or(1.0),
    };
    Fixture { name, server, request, hits }
}
