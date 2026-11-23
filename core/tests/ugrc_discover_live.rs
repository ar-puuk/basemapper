//! Live checks of UGRC Discover's basemaps, which need a quad word.
//!
//! Skipped unless `UGRC_QUAD_WORD` is set (CI sets it from a repository
//! secret: UGRC's test quad word, which opens the basemaps and NAIP). The
//! key is never printed: failures name the basemap, and any message from
//! the core has the key replaced by `<quad word>`.

use basemapper_core::providers::{basemap_provider, ProviderOptions};
use basemapper_core::{render, render_stack, Labels, RenderRequest, StyleInput};

/// Downtown Salt Lake City at zoom 14, 256 x 256 (one tile's extent), in
/// EPSG:3857 metres.
fn view(style: &str) -> RenderRequest {
    const R: f64 = 6_378_137.0;
    let (lon, lat) = (-111.891_f64, 40.769_f64);
    let x = R * lon.to_radians();
    let y = R * (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln();
    let half = std::f64::consts::PI * R / 2f64.powi(14); // half a zoom-14 tile
    RenderRequest::new([x - half, y - half, x + half, y + half], 256, 256, StyleInput::detect(style))
}

fn quad_word() -> Option<String> {
    std::env::var("UGRC_QUAD_WORD").ok().filter(|k| !k.trim().is_empty())
}

fn style(name: &str, labels: Labels) -> String {
    basemap_provider(name, &ProviderOptions { labels, ..Default::default() })
        .unwrap_or_else(|e| panic!("{name}: {}", hide(&e.to_string())))
}

/// `text` without the quad word.
fn hide(text: &str) -> String {
    match quad_word() {
        Some(k) => text.replace(&k, "<quad word>"),
        None => text.to_owned(),
    }
}

/// The spread of the drawn pixels' colours: near 0 for an empty or
/// one-colour image, tens for a real map or photo.
fn spread(pixels: &[u8]) -> f64 {
    let drawn: Vec<f64> = pixels.chunks_exact(4).filter(|p| p[3] > 0).flat_map(|p| p[..3].iter().map(|&c| c as f64)).collect();
    if drawn.is_empty() {
        return 0.0;
    }
    let mean = drawn.iter().sum::<f64>() / drawn.len() as f64;
    (drawn.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / drawn.len() as f64).sqrt()
}

#[test]
fn ugrc_discover_basemaps_render_with_a_quad_word() {
    if quad_word().is_none() {
        eprintln!("skipped: set UGRC_QUAD_WORD to check UGRC Discover's basemaps");
        return;
    }
    // Maps and imagery the test quad word opens.
    for name in ["UGRC.Terrain", "UGRC.Topo", "UGRC.Hillshade", "UGRC.NAIP2024", "UGRC.NAIP2024NRG"] {
        let mut req = view(&style(name, Labels::All));
        req.fail_on_tile_error = true;
        let r = render(req).unwrap_or_else(|e| panic!("{name}: {}", hide(&e.to_string())));
        assert!(spread(&r.pixels) > 3.0, "{name} drew an empty image");
    }

    // The overlay is transparent: over imagery, the imagery still shows.
    let imagery = render(view(&style("UGRC.NAIP2024", Labels::All))).expect("NAIP");
    let both = render_stack(vec![view(&style("UGRC.NAIP2024", Labels::All)), view(&style("UGRC.Overlay", Labels::All))])
        .unwrap_or_else(|e| panic!("NAIP + overlay: {}", hide(&e.to_string())));
    let white = both.pixels.chunks_exact(4).filter(|p| p[..3] == [255, 255, 255]).count();
    assert!(white < both.pixels.len() / 4 / 10, "the overlay hid the imagery ({white} white pixels)");
    assert_ne!(imagery.pixels, both.pixels, "the overlay drew nothing");

    // Labels for imagery are UGRC's overlay.
    let labels = style("UGRC.NAIP2024", Labels::Only);
    assert!(labels.contains("/overlay_basemap/"), "NAIP's labels are not UGRC's overlay");

    // A layer the quad word may not open: real imagery, or the refusal
    // error, never an empty map.
    match render(view(&style("UGRC.Imagery", Labels::All))) {
        Ok(r) => assert!(spread(&r.pixels) > 3.0, "UGRC.Imagery drew an empty image"),
        Err(e) => {
            let e = hide(&e.to_string());
            assert!(e.contains("refused every tile"), "UGRC.Imagery: {e}");
            assert!(!e.contains("discover.agrc"), "the refusal names the URL: {e}");
        }
    }
}
