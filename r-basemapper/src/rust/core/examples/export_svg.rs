//! Export a map view as a vector SVG (and, optionally, a PNG of the same
//! view), from a centre and a MapLibre zoom, which may be fractional. Several
//! `--style` flags are stacked bottom to top, e.g. UGRC's basemap services:
//!
//!   cargo run -p basemapper-core --example export_svg -- \
//!     --style https://tiles.arcgis.com/.../VectorHillshade/VectorTileServer/resources/styles/root.json \
//!     --style https://tiles.arcgis.com/.../LiteBase/VectorTileServer/resources/styles/root.json \
//!     --style https://tiles.arcgis.com/.../LiteLabels/VectorTileServer/resources/styles/root.json \
//!     --center=-111.89,40.76 --zoom 12.4 --width 800 --height 600 \
//!     --output ugrc.svg --png ugrc.png

use basemapper_core::{render, render_svg_stack_with, FontEmbed, RenderRequest, StyleInput, SvgOptions, SvgText};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Export a vector SVG of a map view (styles stacked bottom to top)")]
struct Args {
    #[arg(long, required = true, help = "Style URL or inline JSON; repeat to stack, bottom first")]
    style: Vec<String>,
    #[arg(long, help = "lon,lat of the view centre", allow_hyphen_values = true)]
    center: String,
    #[arg(long, help = "MapLibre zoom (512 px tiles); fractional values are rendered natively")]
    zoom: f64,
    #[arg(long, default_value = "800")]
    width: u32,
    #[arg(long, default_value = "600")]
    height: u32,
    #[arg(long, default_value = "basemap.svg")]
    output: PathBuf,
    #[arg(long, help = "Also write a PNG of the same view")]
    png: Option<PathBuf>,
    #[arg(long, default_value = "editable", help = "Label text: editable (<text> with embedded fonts) or paths (traced outlines)")]
    text: String,
    #[arg(long, default_value = "subset", help = "Embedded fonts: subset, full or none")]
    embed_fonts: String,
    #[arg(long, help = "Folder of font files to use before downloading (repeatable)")]
    font_dir: Vec<PathBuf>,
}

const R: f64 = 6_378_137.0;

fn main() {
    let args = Args::parse();
    let c: Vec<f64> = args.center.split(',').map(|s| s.trim().parse().expect("invalid centre")).collect();
    assert_eq!(c.len(), 2, "--center must be lon,lat");
    let cx = c[0].to_radians() * R;
    let cy = (std::f64::consts::FRAC_PI_4 + c[1].to_radians() / 2.0).tan().ln() * R;
    let mpp = 2.0 * std::f64::consts::PI * R / (512.0 * 2f64.powf(args.zoom));
    let (hw, hh) = (args.width as f64 * mpp / 2.0, args.height as f64 * mpp / 2.0);
    let bbox = [cx - hw, cy - hh, cx + hw, cy + hh];

    let request = |style: &str| RenderRequest {
        bbox,
        width: args.width,
        height: args.height,
        style_input: StyleInput::detect(style),
        zoom: None,
        tile_timeout_ms: 15_000,
        max_tiles: 256,
        tile_concurrency: 16,
        layers: None,
        auth_token: None,
        fail_on_tile_error: false,
        pixel_ratio: 1.0,
    };

    let mut opts = SvgOptions::default();
    opts.text = if args.text == "paths" { SvgText::Paths } else { SvgText::Editable };
    opts.embed_fonts = match args.embed_fonts.as_str() {
        "full" => FontEmbed::Full,
        "none" => FontEmbed::None,
        _ => FontEmbed::Subset,
    };
    opts.font_dirs.extend(args.font_dir.iter().cloned());
    let svg = render_svg_stack_with(args.style.iter().map(|s| request(s)).collect(), &opts).expect("SVG render failed");
    std::fs::write(&args.output, &svg).expect("failed to write SVG");
    println!("Saved {} x {} SVG ({} KB) to {}", args.width, args.height, svg.len() / 1024, args.output.display());

    if let Some(png) = &args.png {
        let mut acc: Option<image::RgbaImage> = None;
        for s in &args.style {
            let r = render(request(s)).expect("render failed");
            let img = image::RgbaImage::from_raw(r.width, r.height, r.pixels).expect("pixel buffer size mismatch");
            acc = Some(match acc {
                None => img,
                Some(dst) => over(&dst, &img),
            });
        }
        acc.unwrap().save(png).expect("failed to save PNG");
        println!("Saved PNG of the same view to {}", png.display());
    }
}

/// Straight-alpha source-over (as the golden tests composite stacks).
fn over(dst: &image::RgbaImage, src: &image::RgbaImage) -> image::RgbaImage {
    let mut out = dst.clone();
    for (o, s) in out.pixels_mut().zip(src.pixels()) {
        let (sa, da) = (s[3] as f64 / 255.0, o[3] as f64 / 255.0);
        let a = sa + da * (1.0 - sa);
        if a <= 0.0 {
            *o = image::Rgba([0, 0, 0, 0]);
            continue;
        }
        for i in 0..3 {
            let v = (s[i] as f64 / 255.0 * sa + o[i] as f64 / 255.0 * da * (1.0 - sa)) / a;
            o[i] = (v * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        o[3] = (a * 255.0).round() as u8;
    }
    out
}
