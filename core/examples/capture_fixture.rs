//! Capture an offline golden-image fixture: a style plus exactly the tiles a
//! render of `--bbox` at `--zoom` needs.
//!
//! The style's tile source is rewritten to `{{BASE}}/tiles/{z}/{x}/{y}`; its
//! glyphs (every font stack used, every character range present in the
//! tiles) and 1x sprite are downloaded to `fonts/` and `sprites/` and the
//! style rewritten to `{{BASE}}/fonts/...` and `{{BASE}}/sprites/sprite`, so
//! `tests/golden.rs` and `tools/reference-render` can serve the fixture with
//! no network access.
//!
//! Usage:
//!   cargo run -p basemapper-core --example capture_fixture -- \
//!     --name ugrc_litebase_slc \
//!     --style "https://tiles.arcgis.com/.../VectorTileServer/resources/styles/root.json" \
//!     --bbox "-12459252,4963454,-12451608,4969187" --width 800 --height 600 --zoom 14
//!
//! Output goes to `core/tests/fixtures/golden/<name>/`.

use basemapper_core::tile_fetcher::expand_url;
use basemapper_core::vector::render::covering_tiles;
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
    #[arg(long, help = "Reuse the tiles of another case (same source and view); only style, glyphs and sprite are captured")]
    tiles_from: Option<String>,
    #[arg(long, help = "Store/reuse glyphs in another case's fonts/ directory (shared across styles with the same font stacks)")]
    fonts_from: Option<String>,
    #[arg(long, help = "Capture every vector/raster source the layers use, to tiles/<source>/z/x/y (styles with several sources)")]
    all_sources: bool,
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
    std::fs::create_dir_all(&out).unwrap();

    // Tiles MapLibre requests: floor(zoom) for 512 px vector tiles (one level
    // below the 256 px grid zoom basemapper uses), clamped to the source
    // maxzoom (overzoom), plus basemapper's own zoom for its legacy path.
    let maxz = maxzoom.as_u64().map(|m| m as u8).unwrap_or(22);
    let mut zooms = vec![args.zoom.min(maxz)];
    if src["type"] == "vector" && args.zoom > 0 {
        zooms.push((args.zoom - 1).min(maxz));
    }
    zooms.dedup();
    let mut coords = Vec::new();
    for z in zooms {
        // Including tiles of wrapped world copies (canonical coordinates).
        for (c, _) in covering_tiles(bbox, z) {
            if !coords.contains(&c) {
                coords.push(c);
            }
        }
    }
    let mut codepoints = std::collections::BTreeSet::new();
    // Characters of the label text each symbol layer would show (its
    // text-field evaluated on features passing its filter, at MapLibre's
    // zoom), so only the glyph ranges labels can actually request are kept.
    let gl_zoom = args.zoom.saturating_sub(1) as f64;
    let symbol_layers: Vec<Value> = style["layers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["type"] == "symbol" && l["layout"].get("text-field").is_some())
        .cloned()
        .collect();
    let mut note_chars = |bytes: &[u8]| {
        use basemapper_core::vector::expr::{eval_filter, eval_property, Ctx, Kind, Val};
        let Ok(t) = basemapper_core::vector::mvt::decode(bytes) else { return };
        for l in &symbol_layers {
            let Some(src) = l["source-layer"].as_str().and_then(|sl| t.layers.get(sl)) else { continue };
            for f in &src.features {
                let ctx = Ctx { zoom: gl_zoom, feature: Some(f) };
                if l.get("filter").map_or(false, |flt| !eval_filter(flt, &ctx)) {
                    continue;
                }
                let tf = &l["layout"]["text-field"];
                let text = match tf {
                    // Legacy "{token}" strings.
                    Value::String(s) if s.contains('{') => {
                        let mut out = String::new();
                        let mut rest = s.as_str();
                        while let Some(i) = rest.find('{') {
                            out.push_str(&rest[..i]);
                            let Some(j) = rest[i..].find('}') else { break };
                            let key = &rest[i + 1..i + j];
                            if let Some(basemapper_core::vector::mvt::Value::String(v)) = f.properties.get(key) {
                                out.push_str(v);
                            }
                            rest = &rest[i + j + 1..];
                        }
                        out.push_str(rest);
                        out
                    }
                    _ => match eval_property(tf, Kind::Enum, &ctx) {
                        Val::Str(s) => s,
                        Val::Num(n) => n.to_string(),
                        _ => String::new(),
                    },
                };
                codepoints.extend(text.chars().map(|ch| ch as u32));
            }
        }
    };
    if args.all_sources {
        assert!(args.tiles_from.is_none(), "--all-sources cannot reuse another case's tiles");
        use basemapper_core::tile_fetcher::{build_tile_coords, TileCoord};
        use basemapper_core::vector::render::{scale_zoom, tile_zoom};
        // The zooms MapLibre requests for each source at this view.
        let gl = scale_zoom(bbox, args.width);
        let mut used: Vec<String> = Vec::new();
        for l in style["layers"].as_array().unwrap() {
            if let Some(src) = l["source"].as_str() {
                if !used.iter().any(|u| u == src) {
                    used.push(src.to_owned());
                }
            }
        }
        let mut new_sources = serde_json::Map::new();
        for id in used {
            let Some(s) = sources.get(&id) else { continue };
            let kind = s["type"].as_str().unwrap_or("").to_owned();
            if kind != "vector" && kind != "raster" {
                new_sources.insert(id.clone(), s.clone());
                continue;
            }
            let (tpl, tj) = if let Some(t) = s["tiles"][0].as_str() {
                (style_url.join(t).unwrap().to_string(), s.clone())
            } else {
                let tj_url = style_url.join(s["url"].as_str().expect("source has no tiles/url")).unwrap();
                let tj: Value = serde_json::from_slice(&get(&client, tj_url.as_str())).unwrap();
                let t = tj["tiles"][0].as_str().expect("TileJSON has no tiles").to_owned();
                (tj_url.join(&t).unwrap().to_string(), tj)
            };
            let tpl = tpl.replace("%7B", "{").replace("%7D", "}");
            let pick = |k: &str| s.get(k).filter(|v| !v.is_null()).or_else(|| tj.get(k).filter(|v| !v.is_null())).cloned();
            let maxz = pick("maxzoom").and_then(|v| v.as_u64()).map(|m| m.min(22) as u8);
            let minz = pick("minzoom").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
            let ts = pick("tileSize").and_then(|v| v.as_f64()).unwrap_or(512.0);
            let mut coords: Vec<TileCoord> = if kind == "vector" {
                let tz = tile_zoom(gl, maxz);
                if tz < minz { Vec::new() } else { covering_tiles(bbox, tz).into_iter().map(|c| c.0).collect() }
            } else {
                let z = basemapper_core::bbox::raster_tile_zoom(bbox, args.width, ts);
                let z = maxz.map_or(z, |m| z.min(m));
                if z < minz { Vec::new() } else { build_tile_coords(bbox, z) }
            };
            coords.sort_by_key(|c| (c.z, c.x, c.y));
            coords.dedup();
            println!("source {id} ({kind}): {} tiles from {tpl}", coords.len());
            for c in &coords {
                let url = expand_url(&tpl, *c);
                let resp = client.get(&url).send().unwrap_or_else(|e| panic!("GET {url}: {e}"));
                if !resp.status().is_success() {
                    println!("  tile {}/{}/{}: HTTP {} (left out)", c.z, c.x, c.y, resp.status());
                    continue;
                }
                let bytes = resp.bytes().expect("read body").to_vec();
                if kind == "vector" {
                    note_chars(&bytes);
                }
                let dir = out.join("tiles").join(&id).join(c.z.to_string()).join(c.x.to_string());
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join(c.y.to_string()), bytes).unwrap();
            }
            let mut ns = json!({ "type": kind, "tiles": [format!("{{{{BASE}}}}/tiles/{id}/{{z}}/{{x}}/{{y}}")] });
            for k in ["minzoom", "maxzoom", "tileSize"] {
                if let Some(v) = pick(k) {
                    ns[k] = v;
                }
            }
            new_sources.insert(id.clone(), ns);
        }
        style["sources"] = Value::Object(new_sources);
    } else if let Some(other) = &args.tiles_from {
        let tdir = out.parent().unwrap().join(other).join("tiles");
        println!("reusing tiles from {}", tdir.display());
        for c in &coords {
            let p = tdir.join(c.z.to_string()).join(c.x.to_string()).join(c.y.to_string());
            let Ok(bytes) = std::fs::read(&p) else { continue }; // tile left out (404)
            if src["type"] == "vector" {
                note_chars(&bytes);
            }
        }
    } else {
        println!("{} tiles from {template}", coords.len());
        for c in &coords {
            let url = expand_url(&template, *c);
            let resp = client.get(&url).send().unwrap_or_else(|e| panic!("GET {url}: {e}"));
            if !resp.status().is_success() {
                // Sources with limited coverage return 404 outside it;
                // MapLibre treats those tiles as empty, so store nothing.
                println!("  tile {}/{}/{}: HTTP {} (left out)", c.z, c.x, c.y, resp.status());
                continue;
            }
            let bytes = resp.bytes().expect("read body").to_vec();
            if src["type"] == "vector" {
                note_chars(&bytes);
            }
            let dir = out.join("tiles").join(c.z.to_string()).join(c.x.to_string());
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(c.y.to_string()), bytes).unwrap();
        }
    }

    if !args.all_sources {
        let mut new_src = json!({ "type": src["type"], "tiles": ["{{BASE}}/tiles/{z}/{x}/{y}"] });
        if !maxzoom.is_null() {
            new_src["maxzoom"] = maxzoom;
        }
        if let Some(ts) = src.get("tileSize") {
            new_src["tileSize"] = ts.clone();
        }
        style["sources"] = json!({ src_name: new_src });
    }

    // Glyphs: every font stack used by a symbol layer, for range 0-255 plus
    // every range containing a character found in the tiles.
    if let Some(glyphs) = style["glyphs"].as_str().map(str::to_owned) {
        let glyph_tpl = style_url.join(&glyphs).unwrap().to_string().replace("%7B", "{").replace("%7D", "}");
        let mut stacks = std::collections::BTreeSet::new();
        for l in style["layers"].as_array().unwrap() {
            if l["type"] != "symbol" || l["layout"].get("text-field").is_none() {
                continue;
            }
            let fonts = l["layout"]
                .get("text-font")
                .and_then(|f| f.as_array().cloned())
                .unwrap_or_else(|| vec![json!("Open Sans Regular"), json!("Arial Unicode MS Regular")]);
            stacks.insert(fonts.iter().filter_map(|f| f.as_str()).collect::<Vec<_>>().join(","));
        }
        let mut ranges: std::collections::BTreeSet<u32> = codepoints.iter().map(|c| c / 256).collect();
        ranges.insert(0);
        let fonts_root = match &args.fonts_from {
            Some(other) => out.parent().unwrap().join(other),
            None => out.clone(),
        };
        for stack in &stacks {
            for r in &ranges {
                let range = format!("{}-{}", r * 256, r * 256 + 255);
                if fonts_root.join("fonts").join(stack).join(format!("{range}.pbf")).exists() {
                    continue;
                }
                let url = glyph_tpl.replace("{fontstack}", stack).replace("{range}", &range);
                match client.get(&url).send() {
                    Ok(resp) if resp.status().is_success() => {
                        let dir = fonts_root.join("fonts").join(stack);
                        std::fs::create_dir_all(&dir).unwrap();
                        std::fs::write(dir.join(format!("{range}.pbf")), resp.bytes().unwrap()).unwrap();
                    }
                    Ok(resp) => println!("  glyphs {stack} {range}: HTTP {}", resp.status()),
                    Err(e) => println!("  glyphs {stack} {range}: {e}"),
                }
            }
        }
        println!("{} font stacks x {} glyph ranges", stacks.len(), ranges.len());
        style["glyphs"] = json!("{{BASE}}/fonts/{fontstack}/{range}.pbf");
    }

    // Sprite: the 1x sheet, and the @2x sheet (used at pixel ratio > 1) when
    // the style has one.
    if let Some(sprite) = style["sprite"].as_str().map(str::to_owned) {
        let base = style_url.join(&sprite).unwrap().to_string();
        std::fs::create_dir_all(out.join("sprites")).unwrap();
        for ext in ["json", "png"] {
            let bytes = get(&client, &format!("{base}.{ext}"));
            std::fs::write(out.join("sprites").join(format!("sprite.{ext}")), bytes).unwrap();
        }
        let hi: Vec<Option<Vec<u8>>> = ["json", "png"]
            .iter()
            .map(|ext| {
                let resp = client.get(format!("{base}@2x.{ext}")).send().ok()?;
                resp.status().is_success().then(|| resp.bytes().ok().map(|b| b.to_vec())).flatten()
            })
            .collect();
        if let [Some(j), Some(p)] = hi.as_slice() {
            std::fs::write(out.join("sprites").join("sprite@2x.json"), j).unwrap();
            std::fs::write(out.join("sprites").join("sprite@2x.png"), p).unwrap();
        }
        style["sprite"] = json!("{{BASE}}/sprites/sprite");
    } else if let Some(list) = style["sprite"].as_array().cloned() {
        // Multi-sprite styles: [{id, url}], each saved as sprites/<id>.
        std::fs::create_dir_all(out.join("sprites")).unwrap();
        let mut new_list = Vec::new();
        for e in list {
            let (Some(id), Some(url)) = (e["id"].as_str(), e["url"].as_str()) else { continue };
            let base = style_url.join(url).unwrap().to_string();
            for suffix in ["", "@2x"] {
                for ext in ["json", "png"] {
                    match client.get(format!("{base}{suffix}.{ext}")).send() {
                        Ok(r) if r.status().is_success() => {
                            std::fs::write(out.join("sprites").join(format!("{id}{suffix}.{ext}")), r.bytes().unwrap()).unwrap()
                        }
                        _ => println!("  sprite {id}{suffix}.{ext}: not available"),
                    }
                }
            }
            new_list.push(json!({ "id": id, "url": format!("{{{{BASE}}}}/sprites/{id}") }));
        }
        style["sprite"] = Value::Array(new_list);
    }
    std::fs::write(out.join("style.json"), serde_json::to_string_pretty(&style).unwrap()).unwrap();

    let layers: Option<Vec<String>> = args
        .layers
        .map(|l| l.split(',').map(|s| s.trim().to_owned()).collect());
    // Keep fidelity limits (and other extra keys) from an existing case.json.
    let previous: Value = std::fs::read_to_string(out.join("case.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(json!({}));
    let mut case = json!({
        "source_style": args.style,
        "bbox": bbox,
        "width": args.width,
        "height": args.height,
        "zoom": args.zoom,
        "layers": layers,
        "attribution": args.attribution,
        "tiles_from": args.tiles_from,
        "fonts_from": args.fonts_from,
    });
    for k in ["max_bad_pixel_frac", "max_mean_diff"] {
        if let Some(v) = previous.get(k) {
            case[k] = v.clone();
        }
    }
    std::fs::write(out.join("case.json"), serde_json::to_string_pretty(&case).unwrap()).unwrap();
    println!("wrote {}", out.display());
}
