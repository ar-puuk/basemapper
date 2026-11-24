//! CLI example: render a basemap view to a PNG file with [`Basemap`], the
//! Rust counterpart of R's and Python's `basemap()`.
//!
//! Usage:
//!   cargo run --example headless_render -- \
//!     --bounds "-111.95,40.70,-111.80,40.80" --crs EPSG:4326 \
//!     --width 1600 --height 1200 --dpi 192 \
//!     --style "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json" \
//!     --path /tmp/basemap.png
//!
//! The zoom follows from the bounds, the image size and the dpi.

use basemapper_core::{Basemap, Context, Crs};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Render a headless basemap view to a PNG file")]
struct Args {
    #[arg(long, help = "xmin,ymin,xmax,ymax in --crs", allow_hyphen_values = true)]
    bounds: String,
    #[arg(long, default_value = "EPSG:4326", help = "CRS of --bounds (EPSG:4326 or EPSG:3857)")]
    crs: String,
    #[arg(long, default_value = "800")]
    width: u32,
    #[arg(long, default_value = "600")]
    height: u32,
    #[arg(long, default_value = "96", help = "image pixels per inch (sharpness)")]
    dpi: f64,
    #[arg(long, help = "Style URL or inline JSON")]
    style: String,
    #[arg(long, default_value = "basemap.png")]
    path: PathBuf,
}

fn main() {
    let args = Args::parse();

    let parts: Vec<f64> = args
        .bounds
        .split(',')
        .map(|s| s.trim().parse().expect("invalid bounds value"))
        .collect();
    assert_eq!(parts.len(), 4, "bounds must have 4 comma-separated values");
    let crs = Crs::parse(&args.crs).expect("unsupported crs");

    let m = Basemap::new(args.style)
        .bounds([parts[0], parts[1], parts[2], parts[3]], crs)
        .size(args.width, args.height)
        .dpi(args.dpi);
    let r = m.render(&Context::Standalone).expect("render failed");
    std::fs::write(&args.path, r.to_png().expect("PNG encoding")).expect("writing the PNG");
    println!("Saved {}×{} px basemap to {} (zoom {:.3})", r.width, r.height, args.path.display(), r.zoom);
}
