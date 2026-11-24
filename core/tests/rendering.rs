//! A `Rendering` carries its image's place, detail and resolution, and the
//! notes of what happened making it (docs/design/api-review.md §4.8). No
//! network: inline styles, and a local server for tiles.

use basemapper_core::{Basemap, Context, FetchOptions, Level, Style};
use httptest::{matchers::request, responders::status_code, Expectation, Server};

/// A vector style using what the renderer does not support: terrain,
/// clustering, an unknown source type.
const UNSUPPORTED: &str = r##"{
  "version": 8,
  "terrain": {"source": "dem"},
  "sources": {
    "points": {"type": "geojson", "cluster": true, "data": {"type": "FeatureCollection", "features": []}},
    "film": {"type": "video", "urls": []}
  },
  "layers": [
    {"id": "bg", "type": "background", "paint": {"background-color": "#336699"}},
    {"id": "dots", "type": "circle", "source": "points"},
    {"id": "movie", "type": "raster", "source": "film"}
  ]
}"##;

fn kinds(r: &basemapper_core::Rendering) -> Vec<(&'static str, &'static str)> {
    r.notes.iter().map(|n| (n.level.name(), n.kind)).collect()
}

#[test]
fn a_rendering_knows_where_it_is_and_what_happened() {
    let m = Basemap::new(Style::new(UNSUPPORTED).layers(["bg", "dots", "movie", "no-such-layer"].map(String::from).to_vec()))
        .size(400, 200)
        .center(-111.89, 40.76)
        .zoom(10.0)
        .dpi(192.0);
    let r = m.render(&Context::Standalone).expect("renders");
    assert_eq!((r.width, r.height, r.dpi, r.zoom), (400, 200, 192.0, 10.0));
    assert_eq!(r.crs, "EPSG:3857");
    assert_eq!(r.bounds, m.view(&Context::Standalone).unwrap().bounds);
    assert_eq!(&r.pixels[..4], &[0x33, 0x66, 0x99, 0xff]);
    let k = kinds(&r);
    for want in [
        ("adjusted", "terrain_unsupported"),
        ("adjusted", "clustering_unsupported"),
        ("problem", "source_unsupported"),
        ("problem", "filter_unmatched"),
    ] {
        assert!(k.contains(&want), "{want:?} missing from {k:?}");
    }
    assert!(r.notes_at(Level::Problem).any(|n| n.message.contains("no-such-layer")));
}

#[test]
fn scale_follows_zoom_and_latitude_not_dpi() {
    let style = r##"{"version":8,"sources":{},"layers":[{"id":"bg","type":"background","paint":{"background-color":"#fff"}}]}"##;
    let at = |lat: f64, ratio: f64| Basemap::new(style).size(64, 64).center(0.0, lat).zoom(4.0).dpi(96.0 * ratio).render(&Context::Standalone).unwrap();
    // 512 px tiles: zoom 0 is 78 271.5 m per CSS pixel at the equator, a
    // CSS pixel 1/96 inch.
    let equator = at(0.0, 1.0).scale();
    let want = 2.0 * 20_037_508.342_789_244 / 512.0 / 16.0 * 96.0 / 0.0254;
    assert!((equator / want - 1.0).abs() < 1e-9, "{equator} vs {want}");
    assert!((at(0.0, 3.0).scale() / equator - 1.0).abs() < 1e-9, "dpi does not change the scale");
    let north = at(60.0, 1.0).scale();
    assert!((north / equator - 0.5).abs() < 1e-3, "half the scale number at 60°: {north} vs {equator}");
}

#[test]
fn failed_and_coarsened_tiles_are_one_note_each() {
    let server = Server::run();
    server.expect(Expectation::matching(request::method("GET")).times(..).respond_with(status_code(404)));
    let tiles = server.url_str("/{z}/{x}/{y}.png");
    // The source has nothing past zoom 3, and at most 4 tiles may be fetched.
    let style = format!(
        r#"{{"version":8,"sources":{{"s":{{"type":"raster","tiles":["{tiles}"],"tileSize":256,"maxzoom":3}}}},"layers":[{{"id":"r","type":"raster","source":"s"}}]}}"#
    );
    let wide = Basemap::new(style.as_str()).size(1024, 512).center(0.0, 0.0).zoom(2.0).fetch(FetchOptions::new().max_tiles(4));
    let r = wide.render(&Context::Standalone).expect("failed tiles leave gaps, not an error");
    let k = kinds(&r);
    assert!(k.contains(&("adjusted", "tiles_coarsened")), "{k:?}");
    let skipped: Vec<_> = r.notes.iter().filter(|n| n.kind == "tile_skipped").collect();
    assert_eq!(skipped.len(), 1, "one note for all the failed tiles: {k:?}");
    assert!(skipped[0].count >= 2 && skipped[0].details.len() as u32 == skipped[0].count, "{:?}", skipped[0]);

    let deep = Basemap::new(style.as_str()).size(256, 256).center(0.0, 0.0).zoom(8.0);
    let k = kinds(&deep.render(&Context::Standalone).unwrap());
    assert!(k.contains(&("adjusted", "tiles_overzoomed")), "{k:?}");
    assert!(!k.contains(&("adjusted", "tiles_coarsened")), "{k:?}");
}

#[test]
fn a_stack_collects_the_notes_of_every_style() {
    // Stacked styles render in their own tasks; their notes still arrive.
    let plain = r##"{"version":8,"sources":{},"layers":[{"id":"bg","type":"background","paint":{"background-color":"#fff"}}]}"##;
    let r = Basemap::stack(vec![Style::new(plain), Style::new(UNSUPPORTED)]).center(0.0, 0.0).zoom(2.0).size(64, 32).render(&Context::Standalone).unwrap();
    let k = kinds(&r);
    assert!(k.contains(&("adjusted", "terrain_unsupported")) && k.contains(&("problem", "source_unsupported")), "{k:?}");
}
