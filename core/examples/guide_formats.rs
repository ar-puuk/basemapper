//! Raster and vector basemaps, each as a PNG and as an SVG.
use basemapper_core::{Basemap, Context, SvgOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for (name, style) in [("raster", "OpenStreetMap.Mapnik"), ("vector", "OpenFreeMap.Bright")] {
        let m = Basemap::new(style).center(-111.8910, 40.7608).zoom(11.5).size(480, 300);

        let png = m.render(&Context::Standalone)?.to_png()?;
        std::fs::write(format!("{name}.png"), &png)?;
        // SVG: vector layers as paths and text; raster tiles as embedded images.
        let svg = m.render_svg(&Context::Standalone, &SvgOptions::default())?;
        std::fs::write(format!("{name}.svg"), &svg)?;

        println!("{name}: {name}.png {} KB, {name}.svg {} KB ({} paths, {} images)",
            png.len() / 1024, svg.len() / 1024, svg.matches("<path").count(), svg.matches("<image").count());
    }
    Ok(())
}
