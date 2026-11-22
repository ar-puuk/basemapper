//! Raster and vector basemaps, each as a PNG and as an SVG.
use basemapper_core::providers::{basemap_provider, ProviderOptions};
use basemapper_core::{Basemap, SvgOptions};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raster = basemap_provider("OpenStreetMap.Mapnik", &ProviderOptions::default())?;
    let vector = basemap_provider("OpenFreeMap.Bright", &ProviderOptions::default())?;

    for (name, style) in [("raster", raster), ("vector", vector)] {
        let m = Basemap::new(style).size(480, 300).center(-111.8910, 40.7608).zoom(11.5);

        let png = m.render_png(Some(Path::new(&format!("{name}.png"))))?;
        // SVG: vector layers as paths and text; raster tiles as embedded images.
        let svg = m.render_svg(Some(Path::new(&format!("{name}.svg"))), &SvgOptions::default())?;

        println!("{name}: {name}.png {} KB, {name}.svg {} KB ({} paths, {} images)",
            png.len() / 1024, svg.len() / 1024, svg.matches("<path").count(), svg.matches("<image").count());
    }
    Ok(())
}
