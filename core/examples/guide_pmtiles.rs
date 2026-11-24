//! PMTiles archives, read over HTTP with range requests (a local path works
//! too): a raster archive as it is, and a vector archive drawn with the
//! Protomaps basemap style, light and dark.
use basemapper_core::{Basemap, Context, Crs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A raster archive draws as it is: a USGS topographic map, fitted to
    // the archive's extent (the zoom follows from the extent and size).
    let topo = Basemap::new("https://pmtiles.io/usgs-mt-whitney-8-15-webp-512.pmtiles")
        .bounds([-118.3198, 36.5611, -118.2607, 36.5930], Crs::LonLat)
        .size(720, 405);
    let r = topo.render(&Context::Standalone)?;
    std::fs::write("pmtiles-raster.png", r.to_png()?)?;
    println!("raster: fitted at zoom {:.2}", r.zoom);

    // A vector archive with a full basemap style: the Protomaps styles in
    // guide/styles/, whose source is `pmtiles://` and the archive's URL.
    let styles = concat!(env!("CARGO_MANIFEST_DIR"), "/guide/styles");
    for flavor in ["light", "dark"] {
        let m = Basemap::new(format!("{styles}/protomaps-{flavor}.json")).center(11.2551, 43.7710).zoom(15.2).size(720, 405);
        std::fs::write(format!("pmtiles-{flavor}.png"), m.render(&Context::Standalone)?.to_png()?)?;
        println!("{flavor}: {}", m.attribution()?);
    }
    Ok(())
}
