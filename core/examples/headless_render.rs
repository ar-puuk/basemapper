//! CLI example: render a basemap tile mosaic and write it to a PNG file.
//!
//! Usage:
//!   cargo run --example headless_render -- \
//!     --bbox "-13700000,4500000,-13600000,4600000" \
//!     --width 800 --height 600 \
//!     --style "https://demotiles.maplibre.org/style.json" \
//!     --output /tmp/basemap.png

use basemapper_core::{render, RenderRequest, StyleInput};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Render a headless basemap tile mosaic to a PNG file")]
struct Args {
    #[arg(long, help = "xmin,ymin,xmax,ymax in EPSG:3857", allow_hyphen_values = true)]
    bbox: String,
    #[arg(long, default_value = "800")]
    width: u32,
    #[arg(long, default_value = "600")]
    height: u32,
    #[arg(long, help = "Style URL or inline JSON")]
    style: String,
    #[arg(long, help = "Override zoom level (auto-computed if omitted)")]
    zoom: Option<u8>,
    #[arg(long, default_value = "basemap.png")]
    output: PathBuf,
}

fn main() {
    let args = Args::parse();

    let parts: Vec<f64> = args
        .bbox
        .split(',')
        .map(|s| s.trim().parse().expect("invalid bbox value"))
        .collect();
    assert_eq!(parts.len(), 4, "bbox must have 4 comma-separated values");

    let request = RenderRequest {
        bbox: [parts[0], parts[1], parts[2], parts[3]],
        width: args.width,
        height: args.height,
        style_input: StyleInput::detect(&args.style),
        zoom: args.zoom,
        tile_timeout_ms: 10_000,
        max_tiles: 256,
        tile_concurrency: 16,
        layers: None,
        auth_token: None,
        fail_on_tile_error: false,
    };

    let result = render(request).expect("render failed");

    let img = image::RgbaImage::from_raw(result.width, result.height, result.pixels)
        .expect("pixel buffer size mismatch");
    img.save(&args.output).expect("failed to save PNG");

    println!(
        "Saved {}×{} px basemap to {} (zoom {})",
        result.width,
        result.height,
        args.output.display(),
        result.zoom_used
    );
}
