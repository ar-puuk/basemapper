//! Network safety: tile limits, where access tokens go, secrets kept out of
//! messages, local files out of reach of downloaded styles, and bounded
//! decompression.

use basemapper_core::{render, BasemapError, RenderRequest, StyleInput};
use httptest::matchers::{contains, key, not, request};
use httptest::{all_of, responders::status_code, Expectation, Server};
use std::io::{Cursor, Write};

const WORLD: f64 = 20_037_508.342_789_244;
const TILES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/golden/carto_positron_z1/tiles");

fn png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([10, 20, 30, 255]));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn raster_style(tiles: &str) -> String {
    serde_json::json!({
        "version": 8,
        "sources": {"r": {"type": "raster", "tiles": [tiles], "tileSize": 256}},
        "layers": [{"id": "r", "type": "raster", "source": "r"}]
    })
    .to_string()
}

fn world(style: StyleInput, width: u32) -> RenderRequest {
    RenderRequest::new([-WORLD, -WORLD, WORLD, WORLD], width, width, style)
}

#[test]
fn max_tiles_limits_vector_tiles() {
    // No expectations: any tile request fails the test when the server drops.
    let server = Server::run();
    let style = serde_json::json!({
        "version": 8,
        "sources": {"v": {"type": "vector", "tiles": [server.url_str("/v/{z}/{x}/{y}.pbf")], "maxzoom": 14}},
        "layers": [{"id": "water", "type": "fill", "source": "v", "source-layer": "water"}]
    });
    // A whole-world view 4096 px wide needs 8 x 8 vector tiles at zoom 3.
    let mut req = world(StyleInput::InlineJson(style.to_string()), 4096);
    req.max_tiles = 16;
    match render(req) {
        Err(BasemapError::MaxTilesExceeded { requested, limit: 16 }) => assert!(requested > 16, "{requested}"),
        other => panic!("expected MaxTilesExceeded, got {:?}", other.map(|r| r.width)),
    }
}

#[test]
fn max_tiles_lets_a_small_vector_view_through() {
    let server = Server::run();
    for (x, y) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        let body = std::fs::read(format!("{TILES}/1/{x}/{y}")).unwrap();
        server.expect(
            Expectation::matching(request::method_path("GET", format!("/v/1/{x}/{y}.pbf")))
                .respond_with(status_code(200).body(body)),
        );
    }
    let style = serde_json::json!({
        "version": 8,
        "sources": {"v": {"type": "vector", "tiles": [server.url_str("/v/{z}/{x}/{y}.pbf")], "maxzoom": 14}},
        "layers": [{"id": "water", "type": "fill", "source": "v", "source-layer": "water"}]
    });
    let mut req = world(StyleInput::InlineJson(style.to_string()), 1024);
    req.max_tiles = 4;
    assert!(render(req).is_ok());
}

#[test]
fn token_goes_to_the_style_origin_only() {
    let home = Server::run();
    let other = Server::run();
    // The style (from `home`) draws tiles from `other`: no token there.
    home.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/style.json"),
            request::headers(contains(key("authorization"))),
        ])
        .respond_with(status_code(200).body(raster_style(&other.url_str("/t/{z}/{x}/{y}.png")))),
    );
    other.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/t/0/0/0.png"),
            request::headers(not(contains(key("authorization")))),
        ])
        .respond_with(status_code(200).body(png())),
    );
    let mut req = world(StyleInput::Url(home.url_str("/style.json")), 256);
    req.auth_token = Some("SECRET".into());
    req.fail_on_tile_error = true;
    render(req).expect("renders");
}

#[test]
fn token_goes_to_tiles_on_the_style_origin() {
    let home = Server::run();
    let tiles = home.url_str("/t/{z}/{x}/{y}.png");
    home.expect(
        Expectation::matching(request::method_path("GET", "/style.json"))
            .respond_with(status_code(200).body(raster_style(&tiles))),
    );
    home.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/t/0/0/0.png"),
            request::headers(contains(("authorization", "Bearer SECRET"))),
        ])
        .respond_with(status_code(200).body(png())),
    );
    let mut req = world(StyleInput::Url(home.url_str("/style.json")), 256);
    req.auth_token = Some("SECRET".into());
    req.fail_on_tile_error = true;
    render(req).expect("renders");
}

#[test]
fn keys_in_urls_stay_out_of_error_messages() {
    let server = Server::run();
    server.expect(Expectation::matching(request::method_path("GET", "/t/0/0/0.png")).respond_with(status_code(500)));
    let style = raster_style(&format!("{}?key=SECRET123", server.url_str("/t/{z}/{x}/{y}.png")));
    let mut req = world(StyleInput::InlineJson(style), 256);
    req.fail_on_tile_error = true;
    let msg = render(req).unwrap_err().to_string();
    assert!(!msg.contains("SECRET123"), "{msg}");
    assert!(msg.contains("key=***"), "{msg}");
}

#[test]
fn downloaded_styles_cannot_read_local_archives() {
    let server = Server::run();
    let style = serde_json::json!({
        "version": 8,
        "sources": {"v": {"type": "vector", "url": "pmtiles://file:///etc/secret.pmtiles"}},
        "layers": [{"id": "x", "type": "fill", "source": "v", "source-layer": "water"}]
    });
    server.expect(
        Expectation::matching(request::method_path("GET", "/style.json"))
            .respond_with(status_code(200).body(style.to_string())),
    );
    let err = render(world(StyleInput::Url(server.url_str("/style.json")), 256)).unwrap_err();
    assert!(err.to_string().contains("may not read local archives"), "{err}");
}

#[test]
fn zero_concurrency_is_an_error_not_a_hang() {
    let mut req = world(StyleInput::InlineJson(raster_style("https://example.invalid/{z}/{x}/{y}.png")), 256);
    req.tile_concurrency = 0;
    assert!(matches!(render(req), Err(BasemapError::InvalidInput(_))));
}

#[test]
fn gzip_bombs_are_refused() {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    let zeros = vec![0u8; 1 << 20];
    for _ in 0..65 {
        gz.write_all(&zeros).unwrap();
    }
    let bomb = gz.finish().unwrap();
    assert!(bomb.len() < 1 << 20, "the bomb itself is small");
    let err = basemapper_core::vector::mvt::decode(&bomb).unwrap_err();
    assert!(err.contains("inflates past"), "{err}");
}
