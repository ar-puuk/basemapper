//! Styles stacked bottom to top: satellite imagery under a vector
//! basemap's labels.
use basemapper_core::{Basemap, Context, Style};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Each style has its own filters: all of the imagery, only the labels of
    // the vector basemap.
    let imagery = Style::new("Esri.WorldImagery");
    let labels = Style::new("OpenFreeMap.Liberty").features(vec!["labels".into()]);

    let m = Basemap::stack(vec![imagery, labels]).center(-111.8887, 40.7556).zoom(13.0).size(720, 405);
    std::fs::write("hybrid.png", m.render(&Context::Standalone)?.to_png()?)?;
    println!("credit: {}", m.attribution()?);
    Ok(())
}
