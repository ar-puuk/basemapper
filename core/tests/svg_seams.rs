//! SVG output is vector art without seams: tiles meet with no hairline when
//! the SVG is viewed or rasterised by a renderer that antialiases clip
//! edges (every PDF viewer: poppler, pdf.js, Acrobat), and it matches the
//! PNG render.
//!
//! Each vector tile's paths are clipped to the tile, as MapLibre's stencil
//! clips them. Where two tiles meet, both clip edges are antialiased, so an
//! area both tiles fill shows the background through a faint line unless
//! the clips overlap. These tests render an offline fixture that spans
//! several tiles, rasterise it with resvg at 3x without
//! `shape-rendering` hints (as a PDF viewer draws it), and look for lighter
//! pixels along the tile boundaries.

#[path = "support/fixture.rs"]
mod fixture;

use basemapper_core::{render, render_svg_with, RenderRequest, SvgOptions, SvgText};
use fixture::golden_root;

const HALF: f64 = 20_037_508.342_789_244;

/// A UGRC fixture at zoom 12.4 (`ugrc_litebase_z12_4`: translucent and
/// opaque fills and lines; `ugrc_hillshade_z12_4`: shading fills at
/// opacity 0.8): about 1.2 x 1.1 tiles of zoom 12, with a vertical and a
/// horizontal tile boundary.
fn fixture_named(name: &str) -> fixture::Fixture {
    let dir = golden_root().join(name);
    let case: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("case.json")).unwrap()).unwrap();
    fixture::load(&dir, &case)
}

/// `svg` rasterised at `scale`, composited over white, as luminance
/// (0-255) rows. With `pdf_like`, `shape-rendering` hints are dropped: PDF
/// has no such hint, so a PDF viewer antialiases every edge and clip.
fn rasterise(svg: &str, scale: f32, pdf_like: bool) -> (usize, usize, Vec<f64>) {
    let svg = if pdf_like { svg.replace(r#" shape-rendering="crispEdges""#, "") } else { svg.to_owned() };
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).expect("the SVG parses");
    let size = tree.size().to_int_size();
    let (w, h) = ((size.width() as f32 * scale).round() as u32, (size.height() as f32 * scale).round() as u32);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).unwrap();
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let lum = pixmap
        .pixels()
        .iter()
        .map(|p| {
            let c = p.demultiply();
            0.299 * c.red() as f64 + 0.587 * c.green() as f64 + 0.114 * c.blue() as f64
        })
        .collect();
    (w as usize, h as usize, lum)
}

/// The tile boundaries of zoom `tz` inside `bbox`, in CSS pixels of a
/// `width x height` view: (vertical lines' x, horizontal lines' y).
fn tile_boundaries(bbox: [f64; 4], width: u32, height: u32, tz: u32) -> (Vec<f64>, Vec<f64>) {
    let tile = 2.0 * HALF / 2f64.powi(tz as i32);
    let (kx, ky) = (width as f64 / (bbox[2] - bbox[0]), height as f64 / (bbox[3] - bbox[1]));
    let xs = (0..2u64.pow(tz)).map(|k| (-HALF + k as f64 * tile - bbox[0]) * kx).filter(|&x| x > 4.0 && x < width as f64 - 4.0).collect();
    let ys = (0..2u64.pow(tz)).map(|k| (bbox[3] - (HALF - k as f64 * tile)) * ky).filter(|&y| y > 4.0 && y < height as f64 - 4.0).collect();
    (xs, ys)
}

/// Seams along a line: of the samples where the image is flat across the
/// line (the pixels 2 and 3 px either side agree to within 2 levels, so no
/// map edge runs along it), the share whose pixels on the line (from 1 px
/// before to 1 px after) are lighter than that by more than 4 levels.
/// `at(i, d)` is the luminance at sample `i`, offset `d` px across the line.
fn seam_share(n: usize, at: impl Fn(usize, i64) -> f64) -> (f64, usize) {
    let (mut flat, mut seams) = (0usize, 0usize);
    for i in 0..n {
        let side = [at(i, -3), at(i, -2), at(i, 2), at(i, 3)];
        let (lo, hi) = side.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        if hi - lo > 2.0 || lo < 30.0 {
            continue;
        }
        flat += 1;
        let on = at(i, -1).max(at(i, 0)).max(at(i, 1));
        if on > hi + 4.0 {
            seams += 1;
        }
    }
    (if flat == 0 { 0.0 } else { seams as f64 / flat as f64 }, flat)
}

/// Offsets (device pixels at 3x) of the lines that are not tile boundaries
/// the boundaries are compared with.
const CONTROL: [i64; 6] = [-40, -25, -13, 13, 25, 40];

fn fixture() -> fixture::Fixture {
    fixture_named("ugrc_litebase_z12_4")
}

fn svg_of(request: &RenderRequest) -> String {
    render_svg_with(request.clone(), &SvgOptions { text: SvgText::Paths, ..SvgOptions::default() }).expect("renders to SVG")
}

#[test]
fn tiles_meet_without_seams_when_clip_edges_are_antialiased() {
    for name in ["ugrc_litebase_z12_4", "ugrc_hillshade_z12_4"] {
        no_seams(name);
    }
}

fn no_seams(name: &str) {
    let f = fixture_named(name);
    let svg = svg_of(&f.request);
    let (bbox, width, height) = (f.request.bbox, f.request.width, f.request.height);
    let (xs, ys) = tile_boundaries(bbox, width, height, 12);
    assert!(!xs.is_empty() && !ys.is_empty(), "the view crosses tile boundaries: {xs:?} {ys:?}");
    let scale = 3.0;
    let (w, h, lum) = rasterise(&svg, scale as f32, true);
    let px = |x: i64, y: i64| lum[(y.clamp(0, h as i64 - 1) as usize) * w + x.clamp(0, w as i64 - 1) as usize];
    let mut checked = 0;
    for &x in &xs {
        let c = (x * scale).floor() as i64;
        let (share, flat) = seam_share(h, |i, d| px(c + d, i as i64));
        assert!(flat > 100, "{name}: enough flat rows across x = {x}: {flat}");
        // Map edges fall on any column now and then: compare with columns
        // that are not tile boundaries.
        let control = CONTROL.iter().map(|k| seam_share(h, |i, d| px(c + k + d, i as i64)).0).fold(0.0, f64::max);
        assert!(share <= control + 0.005, "{name}: a seam along the tile edge at x = {x:.1}: {:.1}% of {flat} flat rows are lighter on the edge (elsewhere at most {:.1}%)", share * 100.0, control * 100.0);
        checked += 1;
    }
    for &y in &ys {
        let r = (y * scale).floor() as i64;
        let (share, flat) = seam_share(w, |i, d| px(i as i64, r + d));
        assert!(flat > 100, "{name}: enough flat columns across y = {y}: {flat}");
        let control = CONTROL.iter().map(|k| seam_share(w, |i, d| px(i as i64, r + k + d)).0).fold(0.0, f64::max);
        assert!(share <= control + 0.005, "{name}: a seam along the tile edge at y = {y:.1}: {:.1}% of {flat} flat columns are lighter on the edge (elsewhere at most {:.1}%)", share * 100.0, control * 100.0);
        checked += 1;
    }
    assert!(checked >= 2);
    if std::env::var("SEAMS_VERBOSE").is_ok() {
        for &x in &xs {
            let c = (x * scale).floor() as i64;
            let at = |k: i64| seam_share(h, |i, d| px(c + k + d, i as i64)).0 * 100.0;
            eprintln!("{name} x = {x:.1}: edge {:.2}%, elsewhere {:?}", at(0), CONTROL.map(|k| (at(k) * 100.0).round() / 100.0));
        }
        for &y in &ys {
            let r = (y * scale).floor() as i64;
            let at = |k: i64| seam_share(w, |i, d| px(i as i64, r + k + d)).0 * 100.0;
            eprintln!("{name} y = {y:.1}: edge {:.2}%, elsewhere {:?}", at(0), CONTROL.map(|k| (at(k) * 100.0).round() / 100.0));
        }
    }
}

#[test]
fn the_svg_matches_the_png_render() {
    let f = fixture();
    let svg = svg_of(&f.request);
    let png = render(f.request.clone()).expect("renders");
    let (w, h, svg_lum) = rasterise(&svg, 1.0, false);
    assert_eq!((w, h), (png.width as usize, png.height as usize));
    // The PNG over white, as luminance.
    let png_lum: Vec<f64> = png
        .pixels
        .chunks_exact(4)
        .map(|p| {
            let a = p[3] as f64 / 255.0;
            let c = |v: u8| v as f64 * a + 255.0 * (1.0 - a);
            0.299 * c(p[0]) + 0.587 * c(p[1]) + 0.114 * c(p[2])
        })
        .collect();
    let diffs: Vec<f64> = svg_lum.iter().zip(&png_lum).map(|(a, b)| (a - b).abs()).collect();
    let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
    let bad = diffs.iter().filter(|&&d| d > 24.0).count() as f64 / diffs.len() as f64;
    assert!(mean < 2.0 && bad < 0.01, "SVG vs PNG: mean luminance difference {mean:.3}, {:.3}% of pixels differ by more than 24", bad * 100.0);
}

#[test]
fn raster_layers_are_embedded_at_the_output_dpi() {
    // The hillshade fixture: one raster-dem layer, embedded as an image.
    let dir = golden_root().join("pmtiles_hillshade");
    let case: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("case.json")).unwrap()).unwrap();
    let f = fixture::load(&dir, &case);
    let request = RenderRequest { pixel_ratio: 1.75, ..f.request.clone() };
    let svg = svg_of(&request);
    let (w, h) = (request.width as f64, request.height as f64);
    // The embedded PNG is the device size (1.75x), drawn at the CSS size.
    let at = svg.find("data:image/png;base64,").expect("an embedded image");
    let b64 = &svg[at + "data:image/png;base64,".len()..];
    let b64 = &b64[..b64.find('"').unwrap()];
    let bytes = base64_decode(b64);
    let img = image::load_from_memory(&bytes).unwrap();
    assert_eq!((img.width(), img.height()), ((w * 1.75).round() as u32, (h * 1.75).round() as u32));
    assert!(svg.contains(&format!(r#"<image width="{}" height="{}" preserveAspectRatio="none""#, request.width, request.height)), "drawn at the CSS size");
}

fn base64_decode(s: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    } as u32;
    let bytes: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, &c)| acc | val(c) << (18 - 6 * i));
        for k in 0..chunk.len().saturating_sub(1) {
            out.push((n >> (16 - 8 * k)) as u8);
        }
    }
    out
}

