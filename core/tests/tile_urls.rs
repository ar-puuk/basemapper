//! Tile URLs requested by the renderer, as MapLibre GL JS requests them:
//! every `tiles` template (spread by `(x + y) % n`), `scheme: "tms"`, and the
//! `{ratio}`, `{quadkey}`, `{prefix}` and `{bbox-epsg-3857}` tokens. Each test
//! serves only the URLs MapLibre would ask for; httptest fails the test on any
//! other request, or if one of them is never made.

use basemapper_core::{render, RenderRequest, StyleInput};
use httptest::{matchers::request::method_path, responders::status_code, Expectation, Server};
use std::io::Cursor;

const WORLD: f64 = 20_037_508.342_789_244;

fn png(rgb: [u8; 3]) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// Expect exactly one GET of `path`, answered with a solid tile.
fn expect(server: &Server, path: &str, rgb: [u8; 3]) {
    server.expect(Expectation::matching(method_path("GET", path.to_owned())).times(1).respond_with(
        status_code(200).insert_header("Content-Type", "image/png").body(png(rgb)),
    ));
}

/// A raster style over `tiles`, with extra source properties.
fn style(tiles: &[String], extra: serde_json::Value) -> String {
    let mut source = serde_json::json!({"type": "raster", "tiles": tiles, "tileSize": 256});
    if let (Some(s), Some(e)) = (source.as_object_mut(), extra.as_object()) {
        s.extend(e.clone());
    }
    serde_json::json!({
        "version": 8,
        "sources": {"s": source},
        "layers": [{"id": "r", "type": "raster", "source": "s"}],
    })
    .to_string()
}

/// Render the whole world at tile zoom 1 (2 x 2 tiles of 256 px).
fn render_world(style: String, pixel_ratio: f64) -> basemapper_core::RenderResult {
    let mut req = RenderRequest::new([-WORLD, -WORLD, WORLD, WORLD], 512, 512, StyleInput::detect(&style));
    req.tile_zoom = Some(1);
    req.pixel_ratio = pixel_ratio;
    req.fail_on_tile_error = true;
    render(req).expect("render")
}

fn pixel(r: &basemapper_core::RenderResult, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * r.width + x) * 4) as usize;
    [r.pixels[i], r.pixels[i + 1], r.pixels[i + 2]]
}

#[test]
fn tms_scheme_counts_rows_from_the_south() {
    let server = Server::run();
    // XYZ row 0 (north) is TMS row 1 at zoom 1.
    let (nw, ne, sw, se) = ([200, 0, 0], [0, 200, 0], [0, 0, 200], [200, 200, 0]);
    expect(&server, "/1/0/1.png", nw);
    expect(&server, "/1/1/1.png", ne);
    expect(&server, "/1/0/0.png", sw);
    expect(&server, "/1/1/0.png", se);
    let tpl = server.url_str("/{z}/{x}/{y}.png");
    let r = render_world(style(&[tpl], serde_json::json!({"scheme": "tms"})), 1.0);
    assert_eq!(pixel(&r, 128, 128), nw);
    assert_eq!(pixel(&r, 384, 128), ne);
    assert_eq!(pixel(&r, 128, 384), sw);
    assert_eq!(pixel(&r, 384, 384), se);
}

#[test]
fn tiles_are_spread_over_every_template() {
    let server = Server::run();
    // (x + y) % 2: tiles (0,0) and (1,1) from the first template, the others
    // from the second.
    expect(&server, "/a/1/0/0.png", [1, 1, 1]);
    expect(&server, "/a/1/1/1.png", [1, 1, 1]);
    expect(&server, "/b/1/1/0.png", [1, 1, 1]);
    expect(&server, "/b/1/0/1.png", [1, 1, 1]);
    let tiles = [server.url_str("/a/{z}/{x}/{y}.png"), server.url_str("/b/{z}/{x}/{y}.png")];
    render_world(style(&tiles, serde_json::json!({})), 1.0);
}

#[test]
fn ratio_quadkey_and_prefix_tokens() {
    let server = Server::run();
    for (x, y, quadkey, prefix) in [(0, 0, "0", "00"), (1, 0, "1", "10"), (0, 1, "2", "01"), (1, 1, "3", "11")] {
        expect(&server, &format!("/{prefix}/{quadkey}/{x}/{y}@2x.png"), [1, 1, 1]);
    }
    let tpl = server.url_str("/{prefix}/{quadkey}/{x}/{y}{ratio}.png");
    render_world(style(&[tpl], serde_json::json!({})), 2.0);
}

#[test]
fn wms_bbox_token_matches_maplibre() {
    let server = Server::run();
    // Strings MapLibre GL JS 5.24 produces for the zoom-1 tiles.
    let h = "20037508.342789244";
    // Tiles (0,0), (1,0), (0,1), (1,1).
    for bbox in [format!("-{h},0,0,{h}"), format!("0,0,{h},{h}"), format!("-{h},-{h},0,0"), format!("0,-{h},{h},0")] {
        server.expect(
            Expectation::matching(httptest::all_of![
                method_path("GET", "/wms"),
                httptest::matchers::request::query(httptest::matchers::url_decoded(httptest::matchers::contains((
                    "BBOX",
                    bbox
                ))))
            ])
            .times(1)
            .respond_with(status_code(200).body(png([1, 1, 1]))),
        );
    }
    let tpl = server.url_str("/wms?SERVICE=WMS&REQUEST=GetMap&BBOX={bbox-epsg-3857}&WIDTH=256&HEIGHT=256");
    render_world(style(&[tpl], serde_json::json!({})), 1.0);
}
