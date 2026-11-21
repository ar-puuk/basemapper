//! Capture an offline golden-image fixture: a style plus exactly the tiles a
//! render of `--bbox` at `--zoom` needs.
//!
//! The style's tile source is rewritten to `{{BASE}}/{z}/{x}/{y}` and its
//! `sprite`/`glyphs` keys are removed, so `tests/golden.rs` can serve the
//! fixture from a local mock server with no network access.
//!
//! Usage:
//!   cargo run -p basemapper-core --example capture_fixture -- \
//!     --name ugrc_litebase_slc \
//!     --style "https://tiles.arcgis.com/.../VectorTileServer/resources/styles/root.json" \
//!     --bbox "-12459252,4963454,-12451608,4969187" --width 800 --height 600 --zoom 14
//!
//! Output goes to `core/tests/fixtures/golden/<name>/`.

use basemapper_core::tile_fetcher::{build_tile_coords, expand_url};
use clap::Parser;
use reqwest::Url;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    name: String,
    #[arg(long, help = "Style JSON URL or inline JSON")]
    style: String,
    #[arg(long, help = "xmin,ymin,xmax,ymax in EPSG:3857", allow_hyphen_values = true)]
    bbox: String,
    #[arg(long)]
    width: u32,
    #[arg(long)]
    height: u32,
    #[arg(long)]
    zoom: u8,
    #[arg(long, help = "Optional layer filter recorded in case.json (comma-separated)")]
    layers: Option<String>,
    #[arg(long, default_value = "OpenStreetMap contributors")]
    attribution: String,
}

fn get(client: &reqwest::blocking::Client, url: &str) -> Vec<u8> {
    let resp = client.get(url).send().unwrap_or_else(|e| panic!("GET {url}: {e}"));
    assert!(resp.status().is_success(), "GET {url}: HTTP {}", resp.status());
    resp.bytes().expect("read body").to_vec()
}

fn main() {
    let args = Args::parse();
    let bbox: Vec<f64> = args
        .bbox
        .split(',')
        .map(|s| s.trim().parse().expect("invalid bbox value"))
        .collect();
    assert_eq!(bbox.len(), 4, "bbox must have 4 values");
    let bbox = [bbox[0], bbox[1], bbox[2], bbox[3]];

    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("basemapper-fixture-capture/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap();

    // Inline JSON (starts with `{`) is accepted for raster styles that have no
    // hosted style URL; its tile URLs must then be absolute.
    let (style_url, mut style): (Url, Value) = if args.style.trim_start().starts_with('{') {
        (Url::parse("http://inline.invalid/").unwrap(), serde_json::from_str(&args.style).unwrap())
    } else {
        let u = Url::parse(&args.style).expect("style must be an absolute URL or inline JSON");
        let v = serde_json::from_slice(&get(&client, u.as_str())).unwrap();
        (u, v)
    };

    // Same source preference as core: first vector source, else first raster.
    let sources = style["sources"].as_object().expect("style has no sources").clone();
    let (src_name, src) = sources
        .iter()
        .find(|(_, s)| s["type"] == "vector")
        .or_else(|| sources.iter().find(|(_, s)| s["type"] == "raster"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .expect("no vector or raster source");

    let (template, maxzoom) = if let Some(t) = src["tiles"][0].as_str() {
        (style_url.join(t).unwrap().to_string(), src["maxzoom"].clone())
    } else {
        let tj_url = style_url.join(src["url"].as_str().expect("source has no tiles/url")).unwrap();
        let tj: Value = serde_json::from_slice(&get(&client, tj_url.as_str())).unwrap();
        let t = tj["tiles"][0].as_str().expect("TileJSON has no tiles");
        (tj_url.join(t).unwrap().to_string(), tj["maxzoom"].clone())
    };
    // Url::join percent-encodes braces; restore the placeholders.
    let template = template.replace("%7B", "{").replace("%7D", "}");

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/golden")
        .join(&args.name);
    std::fs::create_dir_all(out.join("tiles")).unwrap();

    let mut coords = build_tile_coords(bbox, args.zoom);
    // MapLibre GL treats vector sources as 512 px tiles, so a reference render
    // of the same view (tools/reference-render) requests tiles one zoom lower.
    if src["type"] == "vector" && args.zoom > 0 {
        coords.extend(build_tile_coords(bbox, args.zoom - 1));
    }
    println!("{} tiles from {template}", coords.len());
    for c in &coords {
        let bytes = get(&client, &expand_url(&template, *c));
        let dir = out.join("tiles").join(c.z.to_string()).join(c.x.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(c.y.to_string()), bytes).unwrap();
    }

    let mut new_src = json!({ "type": src["type"], "tiles": ["{{BASE}}/{z}/{x}/{y}"] });
    if !maxzoom.is_null() {
        new_src["maxzoom"] = maxzoom;
    }
    if let Some(ts) = src.get("tileSize") {
        new_src["tileSize"] = ts.clone();
    }
    style["sources"] = json!({ src_name: new_src });
    let obj = style.as_object_mut().unwrap();
    obj.remove("sprite");
    obj.remove("glyphs");
    std::fs::write(out.join("style.json"), serde_json::to_string_pretty(&style).unwrap()).unwrap();

    let layers: Option<Vec<String>> = args
        .layers
        .map(|l| l.split(',').map(|s| s.trim().to_owned()).collect());
    let case = json!({
        "source_style": args.style,
        "bbox": bbox,
        "width": args.width,
        "height": args.height,
        "zoom": args.zoom,
        "layers": layers,
        "attribution": args.attribution,
    });
    std::fs::write(out.join("case.json"), serde_json::to_string_pretty(&case).unwrap()).unwrap();
    println!("wrote {}", out.display());
}
