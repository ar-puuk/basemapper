//! A named basemap rendered to a PNG.
use basemapper_core::{Basemap, Context};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // "OpenFreeMap.Positron" is a catalogue name; any MapLibre style URL,
    // style JSON or tile data works in its place.
    let m = Basemap::new("OpenFreeMap.Positron")
        .center(-111.8887, 40.7556)
        .zoom(12.4)
        .size(720, 405); // pixels; with no .size(), .dpi(192.0) gives the default view, sharper
    let r = m.render(&Context::Standalone)?;
    std::fs::write("quickstart.png", r.to_png()?)?;

    println!("{} x {} px at zoom {:.1}, 1:{:.0}", r.width, r.height, r.zoom, r.scale());
    println!("bounds ({}): {:.0?}", r.crs, r.bounds);
    for note in &r.notes {
        println!("{}: {}", note.level.name(), note.message);
    }
    println!("credit: {}", m.attribution()?);
    Ok(())
}
