//! A named basemap rendered to a PNG.
use basemapper_core::providers::{basemap_provider, ProviderOptions};
use basemapper_core::Basemap;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // "OpenFreeMap.Positron" is a leaflet-providers name; any MapLibre style
    // URL, style JSON or tile data works in its place.
    let style = basemap_provider("OpenFreeMap.Positron", &ProviderOptions::default())?;

    let m = Basemap::new(style)
        .size(720, 405)
        .center(-111.8887, 40.7556)
        .zoom(12.4)
        .pixel_ratio(1.0); // 2.0: a sharp 2x image of the same view
    m.render_png(Some(Path::new("quickstart.png")))?;

    let (w, h) = m.get_size();
    println!("{w} x {h} px at zoom {:.1}", m.get_zoom());
    println!("bounds (lon/lat): {:.4?}", m.get_bounds(true));
    println!("credit: {}", m.get_attribution()?);
    Ok(())
}
