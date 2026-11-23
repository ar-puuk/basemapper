//! PMTiles archives, read over HTTP with range requests (a local path works
//! too): a raster archive as it is, and a vector archive drawn with the
//! Protomaps basemap style, light and dark.
use basemapper_core::{Basemap, Crs};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A raster archive draws as it is: a USGS topographic map, fitted to
    // the archive's extent.
    let topo = Basemap::new("https://pmtiles.io/usgs-mt-whitney-8-15-webp-512.pmtiles")
        .size(720, 405)
        .bounds([-118.3198, 36.5611, -118.2607, 36.5930], Crs::LonLat)?;
    topo.render_png(Some(Path::new("pmtiles-raster.png")))?;
    println!("raster: fitted at zoom {:.2}", topo.get_zoom());

    // A vector archive with a full basemap style: the Protomaps styles in
    // guide/styles/, whose source is `pmtiles://` and the archive's URL.
    let styles = concat!(env!("CARGO_MANIFEST_DIR"), "/guide/styles");
    for flavor in ["light", "dark"] {
        let m = Basemap::new(format!("{styles}/protomaps-{flavor}.json"))
            .size(720, 405)
            .center(11.2551, 43.7710)
            .zoom(15.2);
        m.render_png(Some(Path::new(&format!("pmtiles-{flavor}.png"))))?;
        println!("{flavor}: {}", m.get_attribution()?);
    }
    Ok(())
}
