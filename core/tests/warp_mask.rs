//! Warped and clipped renders (implementation plan, phase 1d): a Web
//! Mercator render resampled into lon/lat and a transverse Mercator (UTM-like)
//! CRS through a mesh, and masked by polygons. MapLibre has no references for
//! these, so each case checks two things:
//!
//! - **alignment, against the formulas:** the style is a checkerboard of 1°
//!   squares; after the warp, square edges and corners must fall on the
//!   pixels the projection puts them at (the F1 bug, misplaced basemaps,
//!   is exactly this failing);
//! - **a reference image** (`fixtures/warp/*.png`), approved by eye, against
//!   regressions in the resampling. `BASEMAPPER_UPDATE_REFERENCES=1`
//!   rewrites them.
//!
//! No network: the style is inline.

use basemapper_core::mask::{alpha_mask, apply, Keep, Shape};
use basemapper_core::view::lonlat_to_mercator;
use basemapper_core::warp::{warp, Mesh, Source};
use basemapper_core::{Basemap, Crs, Rendering};
use std::path::PathBuf;

const R: f64 = 6_378_137.0;
const HALF: f64 = 20_037_508.342_789_244;
/// The checkerboard: 1° squares over lon -114..-106, lat 36..44.
const WEST: f64 = -114.0;
const SOUTH: f64 = 36.0;
const RED: [u8; 3] = [0xd0, 0x20, 0x20];
const BLUE: [u8; 3] = [0x20, 0x40, 0xd0];

fn checkerboard_style() -> String {
    let mut features = Vec::new();
    for i in 0..8 {
        for j in 0..8 {
            let (x0, y0) = (WEST + i as f64, SOUTH + j as f64);
            // Densified edges, so the squares stay squares in lon/lat after
            // the renderer's Web Mercator projection.
            let mut ring = Vec::new();
            for k in 0..=10 {
                ring.push(format!("[{},{}]", x0 + k as f64 / 10.0, y0));
            }
            for k in 0..=10 {
                ring.push(format!("[{},{}]", x0 + 1.0, y0 + k as f64 / 10.0));
            }
            for k in 0..=10 {
                ring.push(format!("[{},{}]", x0 + 1.0 - k as f64 / 10.0, y0 + 1.0));
            }
            for k in 0..=10 {
                ring.push(format!("[{},{}]", x0, y0 + 1.0 - k as f64 / 10.0));
            }
            features.push(format!(
                r#"{{"type":"Feature","properties":{{"c":{}}},"geometry":{{"type":"Polygon","coordinates":[[{}]]}}}}"#,
                (i + j) % 2,
                ring.join(",")
            ));
        }
    }
    format!(
        r##"{{"version":8,"sources":{{"g":{{"type":"geojson","data":{{"type":"FeatureCollection","features":[{}]}}}}}},
        "layers":[{{"id":"bg","type":"background","paint":{{"background-color":"#ffffff"}}}},
        {{"id":"sq","type":"fill","source":"g","paint":{{"fill-antialias":false,"fill-color":["match",["get","c"],0,"#d02020","#2040d0"]}}}}]}}"##,
        features.join(",")
    )
}

/// The checkerboard colour at a lon/lat inside it.
fn colour_at(lon: f64, lat: f64) -> [u8; 3] {
    let (i, j) = ((lon - WEST).floor() as i64, (lat - SOUTH).floor() as i64);
    if (i + j) % 2 == 0 { RED } else { BLUE }
}

/// The Web Mercator render covering `bounds_3857`, `px` pixels wide.
fn source_render(bounds_3857: [f64; 4], px: u32) -> Rendering {
    let aspect = (bounds_3857[3] - bounds_3857[1]) / (bounds_3857[2] - bounds_3857[0]);
    Basemap::new(checkerboard_style())
        .size(px, (px as f64 * aspect).ceil() as u32)
        .bounds(bounds_3857, Crs::WebMercator)
        .unwrap()
        .render()
        .unwrap()
}

/// The bounding box of a mesh's nodes, with a margin of `pad` of its size.
fn mesh_bounds(mesh: &Mesh, pad: f64) -> [f64; 4] {
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for &(x, y) in &mesh.nodes {
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    let (dx, dy) = ((b[2] - b[0]) * pad, (b[3] - b[1]) * pad);
    [b[0] - dx, b[1] - dy, b[2] + dx, b[3] + dy]
}

fn warped(mesh: &Mesh, w: u32, h: u32) -> Vec<u8> {
    let src = source_render(mesh_bounds(mesh, 0.02), w.max(h) * 2);
    warp(&Source { pixels: &src.pixels, width: src.width, height: src.height, bounds: src.bounds }, mesh, w, h).unwrap()
}

fn rgb(px: &[u8], w: u32, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * w + x) * 4) as usize;
    [px[i], px[i + 1], px[i + 2]]
}

/// Compare with `fixtures/warp/<name>.png`: at most 0.5% of pixels may
/// differ, by at most 8 per channel (platform rounding of the renderer).
fn matches_reference(name: &str, px: &[u8], w: u32, h: u32) {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "warp", &format!("{name}.png")].iter().collect();
    if std::env::var_os("BASEMAPPER_UPDATE_REFERENCES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::save_buffer(&path, px, w, h, image::ColorType::Rgba8).unwrap();
        return;
    }
    let want = image::open(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (BASEMAPPER_UPDATE_REFERENCES=1 writes it)", path.display()))
        .to_rgba8();
    assert_eq!((want.width(), want.height()), (w, h), "{name}: size");
    let mut off = 0usize;
    for (a, b) in px.chunks_exact(4).zip(want.as_raw().chunks_exact(4)) {
        let d = a.iter().zip(b).map(|(x, y)| (*x as i16 - *y as i16).unsigned_abs()).max().unwrap();
        if d > 8 {
            off += 1;
        }
    }
    let total = (w * h) as usize;
    assert!(off * 200 <= total, "{name}: {off} of {total} pixels differ from the reference");
}

#[test]
fn warped_to_lonlat_the_squares_land_where_they_are() {
    // An equirectangular image of lon -113.5..-106.5, lat 36.5..43.5: 40 px
    // per degree.
    let (w, h) = (280, 280);
    let (lon0, lat1, deg_px) = (-113.5, 43.5, 40.0);
    let mesh = Mesh::from_fn(15, 15, w, h, |x, y| lonlat_to_mercator(lon0 + x / deg_px, lat1 - y / deg_px)).unwrap();
    let out = warped(&mesh, w, h);
    // Every pixel whose centre is clear of an edge has its square's colour.
    let mut wrong = 0;
    for y in 0..h {
        for x in 0..w {
            let (lon, lat) = (lon0 + (x as f64 + 0.5) / deg_px, lat1 - (y as f64 + 0.5) / deg_px);
            let near_edge = |v: f64| (v - v.round()).abs() * deg_px < 1.5;
            if near_edge(lon) || near_edge(lat) {
                continue;
            }
            if rgb(&out, w, x, y) != colour_at(lon, lat) {
                wrong += 1;
            }
        }
    }
    assert_eq!(wrong, 0, "pixels of the wrong square: the warp misplaces the map");
    // And the edges are within a pixel: along a row, the colour changes at
    // each whole degree of longitude.
    let y = 100;
    for k in 1..7 {
        let edge = ((-113.0 + k as f64) - lon0) * deg_px;
        let (l, r) = (rgb(&out, w, edge as u32 - 1, y), rgb(&out, w, edge as u32 + 1, y));
        assert_ne!(l, r, "an edge at column {edge}");
    }
    matches_reference("lonlat", &out, w, h);
}

/// A spherical transverse Mercator about -111° (UTM zone 12's central
/// meridian, without UTM's scale factor and offsets): enough to test a
/// strongly non-Mercator grid.
fn tm_forward(lon: f64, lat: f64) -> (f64, f64) {
    let (l, p) = ((lon + 111.0).to_radians(), lat.to_radians());
    let b = p.cos() * l.sin();
    (R * b.atanh(), R * p.tan().atan2(l.cos()))
}

fn tm_inverse(x: f64, y: f64) -> (f64, f64) {
    let (x, y) = (x / R, y / R);
    let lat = (y.sin() / x.cosh()).asin().to_degrees();
    let lon = -111.0 + x.sinh().atan2(y.cos()).to_degrees();
    (lon, lat)
}

#[test]
fn warped_to_transverse_mercator_corners_keep_their_colours() {
    let (w, h) = (240, 320);
    let (_, y0) = tm_forward(-111.0, 37.0);
    let (_, y1) = tm_forward(-111.0, 43.0);
    let (xmin, xmax) = (-240_000.0, 240_000.0);
    let to_map = |x: f64, y: f64| (xmin + x / w as f64 * (xmax - xmin), y1 - y / h as f64 * (y1 - y0));
    let mesh = Mesh::from_fn(13, 17, w, h, |x, y| {
        let (mx, my) = to_map(x, y);
        let (lon, lat) = tm_inverse(mx, my);
        lonlat_to_mercator(lon, lat)
    })
    .unwrap();
    let out = warped(&mesh, w, h);
    let to_px = |lon: f64, lat: f64| {
        let (x, y) = tm_forward(lon, lat);
        ((x - xmin) / (xmax - xmin) * w as f64, (y1 - y) / (y1 - y0) * h as f64)
    };
    // Around each inner corner of the grid, 3 px diagonally into each square,
    // the square's own colour.
    let mut checked = 0;
    for lon in [-113.0, -112.0, -111.0, -110.0, -109.0] {
        for lat in [38.0, 39.0, 40.0, 41.0, 42.0] {
            let (u, v) = to_px(lon, lat);
            if u < 4.0 || v < 4.0 || u > w as f64 - 4.0 || v > h as f64 - 4.0 {
                continue;
            }
            for (dx, dy) in [(-3.0f64, -3.0f64), (3.0, -3.0), (-3.0, 3.0), (3.0, 3.0)] {
                // y down is south: (dx, dy) = (east, south) of the corner.
                let want = colour_at(lon + dx.signum() * 0.01, lat - dy.signum() * 0.01);
                let got = rgb(&out, w, (u + dx) as u32, (v + dy) as u32);
                assert_eq!(got, want, "corner ({lon}, {lat}) at pixel ({u:.1}, {v:.1}), offset ({dx}, {dy})");
            }
            checked += 1;
        }
    }
    assert!(checked >= 9, "{checked} corners checked");
    matches_reference("transverse_mercator", &out, w, h);
}

/// A render of the checkerboard in Web Mercator, and its pixels clipped.
fn clipped(polygons_lonlat: Vec<Vec<Vec<(f64, f64)>>>, keep: Keep) -> (Vec<u8>, u32, u32) {
    let (x0, y0) = lonlat_to_mercator(-113.5, 36.5);
    let (x1, y1) = lonlat_to_mercator(-106.5, 43.5);
    let r = source_render([x0, y0, x1, y1], 200);
    let polygons = polygons_lonlat
        .into_iter()
        .map(|p| p.into_iter().map(|ring| ring.into_iter().map(|(lon, lat)| lonlat_to_mercator(lon, lat)).collect()).collect())
        .collect();
    let shape = Shape::new(polygons, keep).unwrap();
    let px = shape.to_pixels(r.bounds, r.width as f64, r.height as f64, true);
    let mut pixels = r.pixels.clone();
    apply(&mut pixels, &alpha_mask(&px, r.width, r.height, keep));
    (pixels, r.width, r.height)
}

fn alpha(px: &[u8], w: u32, x: u32, y: u32) -> u8 {
    px[((y * w + x) * 4 + 3) as usize]
}

fn diamond(cx: f64, cy: f64, r: f64) -> Vec<(f64, f64)> {
    vec![(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)]
}

#[test]
fn clipped_inside_outside_and_with_holes() {
    let ring = diamond(-110.0, 40.0, 3.0);
    let hole = diamond(-110.0, 40.0, 1.0);
    // Inside: the centre ring kept, the corners transparent.
    let (inside, w, h) = clipped(vec![vec![ring.clone()]], Keep::Inside);
    assert_eq!((alpha(&inside, w, w / 2, h / 2 + 20), alpha(&inside, w, 2, 2)), (255, 0));
    matches_reference("clip_inside", &inside, w, h);
    // Outside: the reverse.
    let (outside, ..) = clipped(vec![vec![ring.clone()]], Keep::Outside);
    assert_eq!((alpha(&outside, w, w / 2, h / 2 + 20), alpha(&outside, w, 2, 2)), (0, 255));
    matches_reference("clip_outside", &outside, w, h);
    // A hole: the centre is cut out of the kept ring.
    let (holed, ..) = clipped(vec![vec![ring, hole]], Keep::Inside);
    assert_eq!((alpha(&holed, w, w / 2, h / 2), alpha(&holed, w, w / 2, h / 2 + 60)), (0, 255));
    matches_reference("clip_holes", &holed, w, h);
}

#[test]
fn the_web_mercator_constant_is_the_cores() {
    assert!((lonlat_to_mercator(180.0, 0.0).0 - HALF).abs() < 1e-6);
}
