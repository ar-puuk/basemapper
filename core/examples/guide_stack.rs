//! Styles stacked bottom to top: satellite imagery under a vector
//! basemap's labels.
use basemapper_core::providers::{basemap_provider, ProviderOptions};
use basemapper_core::{Basemap, Labels};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let imagery = basemap_provider("Esri.WorldImagery", &ProviderOptions::default())?;
    let labels = basemap_provider(
        "OpenFreeMap.Liberty",
        &ProviderOptions { labels: Labels::Only, ..Default::default() },
    )?;

    let m = Basemap::stack(vec![imagery, labels])
        .size(720, 405)
        .center(-111.8887, 40.7556)
        .zoom(13.0);
    m.render_png(Some(Path::new("hybrid.png")))?;
    println!("credit: {}", m.get_attribution()?);
    Ok(())
}
