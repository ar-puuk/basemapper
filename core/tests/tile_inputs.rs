//! Tile data accepted as `style`: tile URLs (probed when the extension does
//! not say raster or vector), TileJSON, local PMTiles archives and style
//! files, through `render`, `list_layers` and `tile_style`.

use basemapper_core::{list_layers, LayerFilter, render, tile_style, BasemapError, RenderRequest, StyleInput, TileKind, TileStyleOptions};
use httptest::{matchers::request::method_path, responders::status_code, Expectation, Server};
use serde_json::Value;
use std::io::Cursor;
use std::path::PathBuf;

const WORLD: f64 = 20_037_508.342_789_244;
const TILES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/golden/carto_positron_z1/tiles");

fn mvt(z: u8, x: u32, y: u32) -> Vec<u8> {
    std::fs::read(format!("{TILES}/{z}/{x}/{y}")).unwrap()
}

fn mvt_layer_names() -> Vec<String> {
    let mut n: Vec<String> = basemapper_core::vector::mvt::decode(&mvt(1, 0, 0)).unwrap().layers.into_keys().collect();
    n.sort();
    n
}

fn png(rgb: [u8; 3]) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn serve(server: &Server, path: &str, body: Vec<u8>) {
    server.expect(Expectation::matching(method_path("GET", path.to_owned())).times(1).respond_with(status_code(200).body(body)));
}

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("basemapper-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn style_json(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

#[test]
fn raster_template_without_extension_is_probed_and_rendered() {
    let server = Server::run();
    serve(&server, "/t/0/0/0", png([10, 20, 30])); // the probe
    for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        serve(&server, &format!("/t/1/{x}/{y}"), png([200, 100, 50]));
    }
    let tpl = server.url_str("/t/{z}/{x}/{y}");
    let mut req = RenderRequest::new([-WORLD, -WORLD, WORLD, WORLD], 512, 512, StyleInput::detect(&tpl));
    req.tile_zoom = Some(1);
    req.fail_on_tile_error = true;
    let r = render(req).unwrap();
    assert_eq!(&r.pixels[..4], &[200, 100, 50, 255]);
}

#[test]
fn vector_template_is_probed_for_its_source_layers() {
    let server = Server::run();
    serve(&server, "/v/0/0/0", mvt(1, 0, 0));
    let (s, dropped) = tile_style(&server.url_str("/v/{z}/{x}/{y}"), &TileStyleOptions::default(), None).unwrap();
    assert!(dropped.is_empty());
    let v = style_json(&s);
    assert_eq!(v["sources"]["tiles"]["type"], "vector");
    let names = mvt_layer_names();
    assert_eq!(names, ["boundaries", "earth", "landcover", "places", "water"]);
    let ids: Vec<&str> = v["layers"].as_array().unwrap().iter().map(|l| l["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), names.len() * 3);
    assert!(ids.contains(&"water-fill") && ids.contains(&"water-line"), "{ids:?}");
}

/// A vector PMTiles archive of the z1 fixture tiles, with `vector_layers`.
fn vector_archive() -> PathBuf {
    use pmtiles::{PmTilesWriter, TileCoord, TileType};
    let meta = serde_json::json!({
        "vector_layers": mvt_layer_names().iter().map(|n| serde_json::json!({"id": n})).collect::<Vec<_>>(),
        "attribution": "© test",
    });
    let mut buf = Vec::new();
    let mut w = PmTilesWriter::new(TileType::Mvt)
        .min_zoom(1)
        .max_zoom(1)
        .metadata(&meta.to_string())
        .create(Cursor::new(&mut buf))
        .unwrap();
    for (x, y) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        w.add_tile(TileCoord::new(1, x, y).unwrap(), &mvt(1, x, y)).unwrap();
    }
    w.finalize().unwrap();
    // One file per call: tests run in parallel.
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = temp_dir("pmtiles").join(format!("area-{n}.pmtiles"));
    std::fs::write(&path, buf).unwrap();
    path
}

#[test]
fn local_pmtiles_archive_lists_and_renders() {
    let path = vector_archive();
    let p = path.to_str().unwrap();

    let layers = list_layers(p, None, &LayerFilter::default()).unwrap();
    assert!(layers.iter().any(|l| l.id == "water-fill" && l.kind == "fill"));

    let (s, _) = tile_style(p, &TileStyleOptions::default(), None).unwrap();
    let v = style_json(&s);
    assert!(v["sources"]["tiles"]["url"].as_str().unwrap().starts_with("pmtiles://file://"));
    assert_eq!(v["sources"]["tiles"]["attribution"], "© test");

    // Asking for raster tiles from a vector archive is an error.
    let raster = TileStyleOptions { kind: Some(TileKind::Raster), ..Default::default() };
    assert!(matches!(tile_style(p, &raster, None), Err(BasemapError::InvalidInput(_))));

    // Rendered from the file: the default style draws the fixture's features.
    let req = RenderRequest::new([-WORLD, -WORLD, WORLD, WORLD], 1024, 1024, StyleInput::detect(p));
    let r = render(req).unwrap();
    let drawn = r.pixels.chunks(4).filter(|px| px[3] > 0).count();
    assert!(drawn > r.pixels.len() / 4 / 10, "only {drawn} pixels drawn");
}

#[test]
fn tilejson_becomes_a_source_url() {
    let server = Server::run();
    let tj = serde_json::json!({"tilejson": "3.0.0", "tiles": [server.url_str("/t/{z}/{x}/{y}.png")], "maxzoom": 9});
    serve(&server, "/tiles.json", tj.to_string().into_bytes());
    let url = server.url_str("/tiles.json");
    let (s, _) = tile_style(&url, &TileStyleOptions::default(), None).unwrap();
    let v = style_json(&s);
    assert_eq!(v["sources"]["tiles"]["type"], "raster");
    assert_eq!(v["sources"]["tiles"]["url"], url.as_str());
    assert_eq!(v["layers"][0]["type"], "raster");
}

#[test]
fn local_style_file_gets_absolute_urls() {
    let dir = temp_dir("style");
    let style = serde_json::json!({
        "version": 8,
        "sources": {"s": {"type": "raster", "tiles": ["tiles/{z}/{x}/{y}.png"]}},
        "layers": [{"id": "r", "type": "raster", "source": "s"}],
    });
    let path = dir.join("style.json");
    std::fs::write(&path, style.to_string()).unwrap();
    let (s, _) = tile_style(path.to_str().unwrap(), &TileStyleOptions::default(), None).unwrap();
    let tile = style_json(&s)["sources"]["s"]["tiles"][0].as_str().unwrap().to_owned();
    assert!(tile.starts_with("file://") && tile.ends_with("/tiles/{z}/{x}/{y}.png"), "{tile}");
}

#[test]
fn unknown_input_is_a_clear_error() {
    let e = tile_style("no/such/style.json", &TileStyleOptions::default(), None).unwrap_err();
    assert!(matches!(e, BasemapError::InvalidInput(ref m) if m.contains("not a style")), "{e}");
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
                _ => http::Response::builder().status(416).body(hyper::Body::empty()).unwrap(),
            }
        })
    }
}

#[test]
fn remote_pmtiles_reads_ranges_of_the_raw_file() {
    use httptest::matchers::{contains, request};
    let bytes = std::fs::read(vector_archive()).unwrap();
    let server = Server::run();
    // Only range reads asking for the uncompressed file are answered: a
    // server free to gzip would otherwise send ranges of the gzipped file.
    server.expect(
        Expectation::matching(httptest::all_of![
            method_path("GET", "/area.pmtiles"),
            request::headers(contains(("accept-encoding", "identity"))),
        ])
        .times(1..)
        .respond_with(RangeFile(std::sync::Arc::new(bytes))),
    );
    let url = server.url_str("/area.pmtiles");
    let layers = list_layers(&url, None, &LayerFilter::default()).unwrap();
    assert!(layers.iter().any(|l| l.id == "water-fill"));
    let r = render(RenderRequest::new([-WORLD, -WORLD, WORLD, WORLD], 1024, 1024, StyleInput::detect(&url))).unwrap();
    assert!(r.pixels.chunks(4).any(|px| px[3] > 0));
}

#[test]
fn catalogue_names_work_as_style() {
    // Raster-only entries resolve without the network.
    let layers = list_layers("Esri.WorldImagery", None, &LayerFilter::default()).unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!((layers[0].id.as_str(), layers[0].kind.as_str(), layers[0].feature.as_str()), ("raster", "raster", "other"));
    assert_eq!(list_layers("esri.worldimagery", None, &LayerFilter::default()).unwrap(), layers);
    // A near miss is an error with the name it was probably meant to be.
    let e = list_layers("Esri.WorldImagry", None, &LayerFilter::default()).unwrap_err().to_string();
    assert!(e.contains("did you mean Esri.WorldImagery?"), "{e}");
}

#[test]
fn provider_tiles_can_be_replaced_with_your_own() {
    use basemapper_core::providers::{basemap_provider, ProviderOptions};
    let raster = |tiles: &str| ProviderOptions {
        kind: Some(TileKind::Raster),
        tiles: Some(tiles.to_owned()),
        ..Default::default()
    };
    // The style is kept, the source draws the given tiles; a template keeps
    // the provider's zoom range, and the provider's attribution goes.
    let s = basemap_provider("Esri.WorldGrayCanvas", &raster("https://my.server/{z}/{x}/{y}.png")).unwrap();
    let src = &style_json(&s)["sources"]["raster"];
    assert_eq!(src["tiles"], serde_json::json!(["https://my.server/{z}/{x}/{y}.png"]));
    assert_eq!(src["maxzoom"], 16);
    assert!(src.get("attribution").is_none());
    // Tiles of the other kind are an error from the archive's header.
    let archive = vector_archive();
    let e = basemap_provider("Esri.WorldGrayCanvas", &raster(archive.to_str().unwrap())).unwrap_err().to_string();
    assert!(e.contains("vector"), "{e}");
}

#[test]
fn arcgis_service_json_tiles_resolve_against_the_service() {
    use httptest::matchers::{matches, request};
    // An Esri item style references the service by a URL without a trailing
    // slash; its JSON lists tiles relative to the service.
    let server = Server::run();
    let svc = server.url_str("/rest/services/Basemap/VectorTileServer");
    let style = serde_json::json!({
        "version": 8,
        "sources": {"esri": {"type": "vector", "url": svc}},
        "layers": [{"id": "water", "type": "fill", "source": "esri", "source-layer": "water",
                    "paint": {"fill-color": "#0000ff"}}],
    });
    server.expect(
        Expectation::matching(method_path("GET", "/rest/services/Basemap/VectorTileServer"))
            .times(1..)
            .respond_with(status_code(200).body(r#"{"tiles": ["tile/{z}/{y}/{x}.pbf"], "maxzoom": 1}"#)),
    );
    server.expect(
        Expectation::matching(request::path(matches(r"^/rest/services/Basemap/VectorTileServer/tile/1/[01]/[01]\.pbf$")))
            .times(1..)
            .respond_with(status_code(200).body(mvt(1, 0, 0))),
    );
    let mut req = RenderRequest::new([-WORLD, -WORLD, WORLD, WORLD], 1024, 1024, StyleInput::detect(&style.to_string()));
    req.fail_on_tile_error = true;
    render(req).unwrap();
}

#[test]
fn attribution_collects_every_source_as_plain_text() {
    use basemapper_core::attribution;
    // Inline (HTML), from a TileJSON (Esri's copyrightText), from a PMTiles archive.
    let server = Server::run();
    server.expect(
        Expectation::matching(method_path("GET", "/svc/VectorTileServer"))
            .times(1..)
            .respond_with(status_code(200).body(r#"{"tiles": ["tile/{z}/{y}/{x}.pbf"], "copyrightText": "Sources: Esri"}"#)),
    );
    let style = serde_json::json!({
        "version": 8,
        "sources": {
            "a": {"type": "raster", "tiles": ["https://t/{z}/{x}/{y}.png"], "attribution": "<a href=\"x\">&copy; A</a>"},
            "b": {"type": "vector", "url": server.url_str("/svc/VectorTileServer")},
            "unused": {"type": "raster", "tiles": ["https://u/{z}/{x}/{y}.png"], "attribution": "never shown"},
        },
        "layers": [
            {"id": "a", "type": "raster", "source": "a"},
            {"id": "b", "type": "fill", "source": "b", "source-layer": "water"},
        ],
    })
    .to_string();
    let archive = vector_archive();
    let styles = vec![style.clone(), archive.to_str().unwrap().to_owned(), style, "Esri.WorldImagery".to_owned()];
    let a = attribution(&styles, None).unwrap();
    assert_eq!(
        a,
        "© A | Sources: Esri | © test | Tiles © Esri — Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, \
         Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community"
    );
    // A credit included in another is dropped, whichever comes first.
    let two = |first: &str, second: &str| {
        serde_json::json!({"version": 8, "sources": {
            "a": {"type": "raster", "tiles": ["https://a/{z}/{x}/{y}.png"], "attribution": first},
            "b": {"type": "raster", "tiles": ["https://b/{z}/{x}/{y}.png"], "attribution": second}},
            "layers": [{"id": "a", "type": "raster", "source": "a"}, {"id": "b", "type": "raster", "source": "b"}]})
        .to_string()
    };
    assert_eq!(attribution(&[two("© OSM · ESA", "© OSM")], None).unwrap(), "© OSM · ESA");
    assert_eq!(attribution(&[two("© OSM", "© OSM · ESA")], None).unwrap(), "© OSM · ESA");
    let none = r#"{"version": 8, "sources": {}, "layers": [{"id": "bg", "type": "background"}]}"#;
    assert_eq!(attribution(&[none.to_owned()], None).unwrap(), "");
}

fn background_style(id: &str) -> String {
    serde_json::json!({"version": 8, "sources": {}, "layers": [
        {"id": id, "type": "background", "paint": {"background-color": "#336699"}}]})
    .to_string()
}

#[test]
fn unmatched_filter_names_warn_only_when_no_style_has_them() {
    use basemapper_core::render_stack;
    let req = |style: &str| {
        let mut r = RenderRequest::new([-WORLD, -WORLD, WORLD, WORLD], 16, 16, StyleInput::detect(style));
        r.layers = Some(vec!["-bg-a".into()]);
        r.features = Some(vec!["-water".into()]);
        r
    };
    // Alone, the second style matches neither name.
    let alone = render(req(&background_style("bg-b"))).unwrap();
    assert_eq!(alone.warnings, [r#"layers: "-bg-a" matches no layer"#, r#"features: "water" matches no layer"#]);
    // In a stack, "-bg-a" matches the first style: only "water" is unmatched everywhere.
    let stack = render_stack(vec![req(&background_style("bg-a")), req(&background_style("bg-b"))]).unwrap();
    assert_eq!(stack.warnings, [r#"features: "water" matches no layer"#]);
}

#[test]
fn list_layers_previews_filters() {
    let style = serde_json::json!({"version": 8, "sources": {"s": {"type": "vector", "tiles": ["https://t/{z}/{x}/{y}.pbf"]}},
        "layers": [
            {"id": "bg", "type": "background"},
            {"id": "water", "type": "fill", "source": "s", "source-layer": "water"},
            {"id": "road_major", "type": "line", "source": "s", "source-layer": "transportation"},
            {"id": "road_name", "type": "symbol", "source": "s", "source-layer": "transportation_name"},
        ]})
    .to_string();
    let ids = |f: LayerFilter| -> Vec<String> { list_layers(&style, None, &f).unwrap().into_iter().map(|l| l.id).collect() };
    let all = list_layers(&style, None, &LayerFilter::default()).unwrap();
    assert_eq!(all[1].source_layer.as_deref(), Some("water"));
    assert_eq!(all[0].source_layer, None);
    assert_eq!(ids(LayerFilter { layers: Some(vec!["road_*".into()]), ..Default::default() }), ["road_major", "road_name"]);
    assert_eq!(ids(LayerFilter { labels: basemapper_core::Labels::Only, ..Default::default() }), ["road_name"]);
    assert_eq!(ids(LayerFilter { features: Some(vec!["-roads".into()]), ..Default::default() }), ["bg", "water"]);
    // A filter that leaves nothing previews as no layers, not an error.
    assert!(ids(LayerFilter { labels: basemapper_core::Labels::Only, features: Some(vec!["water".into()]), ..Default::default() }).is_empty());
    assert!(list_layers(&style, None, &LayerFilter { features: Some(vec!["highways".into()]), ..Default::default() }).is_err());
}

#[test]
fn raster_styles_render_to_svg_as_embedded_images() {
    let server = Server::run();
    for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        serve(&server, &format!("/r/1/{x}/{y}.png"), png([200, 100, 50]));
    }
    let style = serde_json::json!({
        "version": 8,
        "sources": {"r": {"type": "raster", "tiles": [server.url_str("/r/{z}/{x}/{y}.png")], "tileSize": 256}},
        "layers": [{"id": "r", "type": "raster", "source": "r"}],
    })
    .to_string();
    let mut req = RenderRequest::new([-WORLD, -WORLD, WORLD, WORLD], 512, 512, StyleInput::detect(&style));
    req.fail_on_tile_error = true;
    let svg = basemapper_core::render_svg(req).unwrap();
    assert!(svg.starts_with("<svg"), "{}", &svg[..svg.len().min(200)]);
    assert_eq!(svg.matches("<image").count(), 1, "the raster layer is one embedded image");
}
