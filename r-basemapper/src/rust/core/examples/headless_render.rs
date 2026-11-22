//! CLI example: render a basemap view to a PNG file with [`Basemap`], the
//! Rust counterpart of R's and Python's `basemap()`.
//!
//! Usage:
//!   cargo run --example headless_render -- \
//!     --bounds "-111.95,40.70,-111.80,40.80" --crs EPSG:4326 \
//!     --width 800 --height 600 --pixel-ratio 2 \
//!     --style "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json" \
//!     --path /tmp/basemap.png
//!
//! The same render in R and Python:
//!   m <- basemap(style, 800, 600, bounds = c(-111.95, 40.70, -111.80, 40.80), pixel_ratio = 2)
//!   render_png(m, path = "/tmp/basemap.png")
//!
//!   m = basemapper.basemap(style, 800, 600, bounds=[-111.95, 40.70, -111.80, 40.80], pixel_ratio=2)
//!   m.render_png(path="/tmp/basemap.png")

use basemapper_core::{Basemap, Crs};
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
    #[arg(long, default_value = "1")]
    pixel_ratio: f64,
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
        .size(args.width, args.height)
        .pixel_ratio(args.pixel_ratio)
        .bounds([parts[0], parts[1], parts[2], parts[3]], crs)
        .expect("invalid bounds");
    m.render_png(Some(&args.path)).expect("render failed");

    let (w, h) = m.get_size();
    println!("Saved {w}×{h} px basemap to {} (zoom {:.3})", args.path.display(), m.get_zoom());
}
