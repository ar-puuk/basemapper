//! Styles for tile data: anything a user may pass as `style` that is not
//! itself a style. MapLibre always needs a style; when it is given a tile
//! URL, a TileJSON, a PMTiles archive or an ArcGIS service instead, the style
//! is built here, the way one would write it for MapLibre:
//!
//! | Input                                   | Detected by                       | Style                                   |
//! |-----------------------------------------|-----------------------------------|-----------------------------------------|
//! | `*.pmtiles` (URL, path) / `pmtiles://`  | the archive header's tile type    | raster layer / default vector style     |
//! | tile template (`{z}`, `{quadkey}`, ...) | extension, else one probed tile   | raster layer / default vector style     |
//! | TileJSON URL or file                    | `vector_layers`, `format`         | raster layer / default vector style     |
//! | `.../MapServer`                         | URL                               | raster layer over `tile/{z}/{y}/{x}`    |
//! | `.../VectorTileServer`                  | URL                               | the service's published style           |
//! | style URL or file                       | `layers` in the JSON              | the style, relative URLs made absolute  |
//!
//! The default vector style draws every source layer: polygons as
//! translucent fills, lines and outlines, points as circles, a colour per
//! source layer, over a transparent background (ready to stack).

use crate::error::BasemapError;
use crate::tile_fetcher::{local_path, TileCoord, TileUrls};
use serde_json::{json, Map, Value};

/// Raster or vector tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileKind {
    Raster,
    Vector,
}

impl TileKind {
    /// `"raster"` or `"vector"`.
    pub fn parse(s: &str) -> Result<TileKind, BasemapError> {
        match s {
            "raster" => Ok(TileKind::Raster),
            "vector" => Ok(TileKind::Vector),
            _ => Err(BasemapError::InvalidInput(format!("type must be \"raster\" or \"vector\", not {s:?}"))),
        }
    }

    /// `"raster"` or `"vector"`.
    pub fn name(self) -> &'static str {
        match self {
            TileKind::Raster => "raster",
            TileKind::Vector => "vector",
        }
    }
}

/// Overrides for [`crate::tile_style`]; unset fields are detected or left to
/// MapLibre's defaults.
#[derive(Clone, Debug, Default)]
pub struct TileStyleOptions {
    /// Raster or vector; detected when unset.
    pub kind: Option<TileKind>,
    /// Tile size in px (raster default 256 for tile URLs and archives).
    pub tile_size: Option<u32>,
    /// `"xyz"` or `"tms"`.
    pub scheme: Option<String>,
    pub minzoom: Option<u8>,
    pub maxzoom: Option<u8>,
    pub attribution: Option<String>,
    /// Vector: the source layers to draw (default: all).
    pub source_layers: Option<Vec<String>>,
    /// Vector: paint properties for the generated `fill-*`, `line-*` and
    /// `circle-*` layers, e.g. `{"fill-color": "#e8e0d8"}`.
    pub paint: Map<String, Value>,
}

/// What a `style` string names.
#[derive(Debug, PartialEq)]
enum Input {
    /// URL, `file://` URL or path, without `pmtiles://`.
    Pmtiles(String),
    Template(String),
    MapServer(String),
    VectorTileServer(String),
    /// A style or TileJSON, by URL or path.
    Resource(String),
}

fn classify(s: &str) -> Input {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix("pmtiles://") {
        return Input::Pmtiles(inner.to_owned());
    }
    let path = s.split(['?', '#']).next().unwrap_or(s).to_ascii_lowercase();
    if path.ends_with(".pmtiles") {
        return Input::Pmtiles(s.to_owned());
    }
    if ["{z}", "{quadkey}", "{bbox-epsg-3857}", "{TileMatrix}"].iter().any(|t| s.contains(t)) {
        return Input::Template(s.to_owned());
    }
    let trimmed = path.trim_end_matches('/');
    let base = || s.split(['?', '#']).next().unwrap_or(s).trim_end_matches('/').to_owned();
    if trimmed.ends_with("/mapserver") {
        return Input::MapServer(base());
    }
    if trimmed.ends_with("/vectortileserver") {
        return Input::VectorTileServer(base());
    }
    Input::Resource(s.to_owned())
}

/// The base URL when `s` names an ArcGIS VectorTileServer.
pub(crate) fn vector_tile_server(s: &str) -> Option<String> {
    match classify(s) {
        Input::VectorTileServer(base) => Some(base),
        _ => None,
    }
}

fn is_http(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// A style for `input` (a style itself, or tile data), as [`crate::tile_style`].
pub(crate) async fn tile_style(
    input: &str,
    opts: &TileStyleOptions,
    client: &reqwest::Client,
    auth_token: Option<&str>,
) -> Result<(String, Vec<String>), BasemapError> {
    let want = |found: TileKind, what: &str| match opts.kind {
        Some(k) if k != found => Err(BasemapError::InvalidInput(format!(
            "{what} holds {} tiles, not {}",
            found.name(),
            k.name()
        ))),
        _ => Ok(()),
    };
    match classify(input) {
        Input::Pmtiles(url) => {
            let url = archive_url(&url)?;
            let reader = crate::open_pmtiles(&url, client).await?;
            let header = reader.get_header();
            let kind = match header.tile_type {
                pmtiles::TileType::Mvt => TileKind::Vector,
                pmtiles::TileType::Png | pmtiles::TileType::Jpeg | pmtiles::TileType::Webp | pmtiles::TileType::Avif => {
                    TileKind::Raster
                }
                pmtiles::TileType::Mlt => {
                    return Err(BasemapError::InvalidInput(format!(
                        "{input}: MapLibre Tiles (MLT) are not supported yet"
                    )))
                }
                pmtiles::TileType::Unknown => {
                    return Err(BasemapError::InvalidInput(format!("{input}: unknown tile type in the archive")))
                }
            };
            want(kind, "the archive")?;
            let meta: Value = reader
                .get_metadata()
                .await
                .ok()
                .and_then(|m| serde_json::from_str(&m).ok())
                .unwrap_or_default();
            let mut source = json!({"type": kind.name(), "url": format!("pmtiles://{url}")});
            if kind == TileKind::Raster {
                source["tileSize"] = json!(opts.tile_size.unwrap_or(256));
            }
            if let Some(a) = meta["attribution"].as_str() {
                source["attribution"] = json!(a);
            }
            Ok(build_style(kind, source, vector_layer_ids(&meta), opts))
        }
        Input::Template(template) => {
            let urls = TileUrls { templates: vec![template.clone()], tms: opts.scheme.as_deref() == Some("tms") };
            let (kind, names) = match opts.kind.or_else(|| kind_from_template(&template)) {
                Some(TileKind::Raster) => (TileKind::Raster, Vec::new()),
                Some(TileKind::Vector) if opts.source_layers.is_some() => (TileKind::Vector, Vec::new()),
                _ => probe(&urls, opts, client, auth_token).await?,
            };
            want(kind, "the tile URL")?;
            let mut source = json!({"type": kind.name(), "tiles": [template]});
            if kind == TileKind::Raster {
                source["tileSize"] = json!(opts.tile_size.unwrap_or(256));
            }
            Ok(build_style(kind, source, names, opts))
        }
        Input::MapServer(base) => {
            want(TileKind::Raster, "an ArcGIS MapServer")?;
            let source = json!({
                "type": "raster",
                "tiles": [format!("{base}/tile/{{z}}/{{y}}/{{x}}")],
                "tileSize": opts.tile_size.unwrap_or(256),
            });
            Ok(build_style(TileKind::Raster, source, Vec::new(), opts))
        }
        Input::VectorTileServer(base) => {
            want(TileKind::Vector, "an ArcGIS VectorTileServer")?;
            let (style, url) = load_json(&format!("{base}/resources/styles/root.json"), client, auth_token).await?;
            Ok((absolutize(style, &url).to_string(), Vec::new()))
        }
        Input::Resource(r) => {
            let (v, url) = load_json(&r, client, auth_token).await?;
            if v.get("layers").is_some() {
                return Ok((absolutize(v, &url).to_string(), Vec::new()));
            }
            if v.get("tiles").is_none() {
                return Err(BasemapError::StyleParseError(format!(
                    "{r} is neither a MapLibre style (no \"layers\") nor a TileJSON (no \"tiles\")"
                )));
            }
            // TileJSON: the source references it, as MapLibre's `url`.
            let format = v["format"].as_str().unwrap_or("");
            let found = if v.get("vector_layers").is_some() || matches!(format, "pbf" | "mvt") {
                TileKind::Vector
            } else {
                TileKind::Raster
            };
            let kind = opts.kind.unwrap_or(found);
            let source_url = if is_http(&url) { url.clone() } else { r.clone() };
            let mut source = json!({"type": kind.name(), "url": source_url});
            if !is_http(&url) {
                // A local TileJSON: inline it (the renderer fetches TileJSON over HTTP).
                source = v.clone();
                source["type"] = json!(kind.name());
                if let Some(o) = source.as_object_mut() {
                    o.retain(|k, _| {
                        matches!(k.as_str(), "type" | "tiles" | "minzoom" | "maxzoom" | "bounds" | "scheme" | "tileSize" | "attribution")
                    });
                }
            }
            Ok(build_style(kind, source, vector_layer_ids(&v), opts))
        }
    }
}

/// The style for a `style` string the renderer is given: a style URL or file
/// is loaded (relative URLs made absolute); tile data gets a built style; a
/// catalogue name (`"CartoDB.Positron"`) is that basemap's default form.
pub(crate) async fn resolve_style_input(
    input: &str,
    client: &reqwest::Client,
    auth_token: Option<&str>,
) -> Result<String, BasemapError> {
    let named = crate::providers::bare_name_style(input)?;
    let input = match &named {
        Some(style) if style.trim_start().starts_with('{') => return Ok(style.clone()),
        Some(url) => url.as_str(),
        None => input,
    };
    tile_style(input, &TileStyleOptions::default(), client, auth_token).await.map(|(s, _)| s)
}

/// An archive location the backend reads: URLs as they are, paths as
/// absolute `file://` URLs.
fn archive_url(url: &str) -> Result<String, BasemapError> {
    if is_http(url) || url.starts_with("file://") {
        return Ok(url.to_owned());
    }
    let path = std::fs::canonicalize(url)
        .map_err(|e| BasemapError::InvalidInput(format!("PMTiles archive {url}: {e}")))?;
    reqwest::Url::from_file_path(&path)
        .map(|u| u.to_string())
        .map_err(|_| BasemapError::InvalidInput(format!("PMTiles archive {url}: not an absolute path")))
}

fn kind_from_template(t: &str) -> Option<TileKind> {
    if t.contains("{bbox-epsg-3857}") {
        return Some(TileKind::Raster);
    }
    let path = t.split('?').next().unwrap_or(t).to_ascii_lowercase();
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "png" | "jpg" | "jpeg" | "webp" | "avif" | "gif" => Some(TileKind::Raster),
        "pbf" | "mvt" => Some(TileKind::Vector),
        _ => None,
    }
}

/// Fetch one tile (0, 0 at the minimum zoom) and tell raster from vector by
/// its bytes; for vector tiles, the layer names in it.
async fn probe(
    urls: &TileUrls,
    opts: &TileStyleOptions,
    client: &reqwest::Client,
    auth_token: Option<&str>,
) -> Result<(TileKind, Vec<String>), BasemapError> {
    let url = urls.url(TileCoord { z: opts.minzoom.unwrap_or(0), x: 0, y: 0 }, 1.0);
    let mut req = client.get(&url);
    if let Some(t) = auth_token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let hint = "; give type = \"raster\" or \"vector\" (and source_layers for vector tiles)";
    let resp = req
        .send()
        .await
        .map_err(|e| BasemapError::InvalidInput(format!("could not fetch a tile to detect its type ({url}: {e}){hint}")))?;
    if !resp.status().is_success() {
        return Err(BasemapError::InvalidInput(format!(
            "could not fetch a tile to detect its type ({url}: HTTP {}){hint}",
            resp.status()
        )));
    }
    let bytes = resp.bytes().await.map_err(|e| BasemapError::InvalidInput(format!("{url}: {e}")))?;
    let b = &bytes[..];
    let image = b.starts_with(&[0x89, b'P', b'N', b'G'])
        || b.starts_with(&[0xff, 0xd8, 0xff])
        || b.starts_with(b"GIF8")
        || (b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP")
        || (b.len() > 12 && &b[4..8] == b"ftyp");
    if image {
        return Ok((TileKind::Raster, Vec::new()));
    }
    let tile = crate::vector::mvt::decode(b)
        .map_err(|e| BasemapError::InvalidInput(format!("{url}: neither an image nor a vector tile ({e}){hint}")))?;
    let mut names: Vec<String> = tile.layers.into_keys().collect();
    names.sort();
    if names.is_empty() {
        return Err(BasemapError::InvalidInput(format!("{url}: the probed vector tile is empty{hint}")));
    }
    Ok((TileKind::Vector, names))
}

fn vector_layer_ids(v: &Value) -> Vec<String> {
    v["vector_layers"]
        .as_array()
        .map(|a| a.iter().filter_map(|l| l["id"].as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

/// Tableau 10: a colour per source layer.
const PALETTE: [&str; 10] =
    ["#4e79a7", "#f28e2b", "#e15759", "#76b7b2", "#59a14f", "#edc948", "#b07aa1", "#ff9da7", "#9c755f", "#bab0ac"];

/// A style with one source ("tiles"): a raster layer, or the default vector
/// style over `names` (or `opts.source_layers`). Returns the style and the
/// paint keys that applied to no generated layer.
fn build_style(kind: TileKind, mut source: Value, names: Vec<String>, opts: &TileStyleOptions) -> (String, Vec<String>) {
    if let Some(s) = &opts.scheme {
        source["scheme"] = json!(s);
    }
    if let Some(z) = opts.minzoom {
        source["minzoom"] = json!(z);
    }
    if let Some(z) = opts.maxzoom {
        source["maxzoom"] = json!(z);
    }
    if let Some(t) = opts.tile_size {
        source["tileSize"] = json!(t);
    }
    if let Some(a) = &opts.attribution {
        source["attribution"] = json!(a);
    }
    let mut dropped = Vec::new();
    let layers = match kind {
        TileKind::Raster => {
            dropped.extend(opts.paint.keys().cloned());
            vec![json!({"id": "raster", "type": "raster", "source": "tiles"})]
        }
        TileKind::Vector => {
            let names = opts.source_layers.clone().unwrap_or(names);
            let paint_for = |prefix: &str, defaults: Value| {
                let mut p = defaults;
                for (k, v) in &opts.paint {
                    if k.starts_with(prefix) {
                        p[k] = v.clone();
                    }
                }
                p
            };
            dropped.extend(
                opts.paint.keys().filter(|k| !["fill-", "line-", "circle-"].iter().any(|p| k.starts_with(p))).cloned(),
            );
            let colour = |i: usize| PALETTE[i % PALETTE.len()];
            let mut layers = Vec::new();
            for (i, n) in names.iter().enumerate() {
                layers.push(json!({
                    "id": format!("{n}-fill"), "type": "fill", "source": "tiles", "source-layer": n,
                    "filter": ["==", "$type", "Polygon"],
                    "paint": paint_for("fill-", json!({"fill-color": colour(i), "fill-opacity": 0.4})),
                }));
            }
            for (i, n) in names.iter().enumerate() {
                layers.push(json!({
                    "id": format!("{n}-line"), "type": "line", "source": "tiles", "source-layer": n,
                    "filter": ["!=", "$type", "Point"],
                    "paint": paint_for("line-", json!({"line-color": colour(i), "line-width": 1})),
                }));
            }
            for (i, n) in names.iter().enumerate() {
                layers.push(json!({
                    "id": format!("{n}-circle"), "type": "circle", "source": "tiles", "source-layer": n,
                    "filter": ["==", "$type", "Point"],
                    "paint": paint_for("circle-", json!({
                        "circle-color": colour(i), "circle-radius": 3,
                        "circle-stroke-color": "#ffffff", "circle-stroke-width": 1,
                    })),
                }));
            }
            layers
        }
    };
    let style = json!({"version": 8, "sources": {"tiles": source}, "layers": layers});
    (style.to_string(), dropped)
}

/// Load JSON from a URL or a local file; returns it with the URL relative
/// references resolve against (after redirects, or the file's `file://` URL).
async fn load_json(
    input: &str,
    client: &reqwest::Client,
    auth_token: Option<&str>,
) -> Result<(Value, String), BasemapError> {
    if is_http(input) {
        let mut req = client.get(input);
        if let Some(token) = auth_token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        let resp = req
            .send()
            .await
            .map_err(|_| BasemapError::StyleFetchFailed { url: input.to_owned(), status: 0 })?;
        if !resp.status().is_success() {
            return Err(BasemapError::StyleFetchFailed { url: input.to_owned(), status: resp.status().as_u16() });
        }
        let url = resp.url().to_string();
        let text = resp
            .text()
            .await
            .map_err(|_| BasemapError::StyleParseError("could not read style response body".into()))?;
        let v = serde_json::from_str(&text).map_err(|e| BasemapError::StyleParseError(format!("{input}: {e}")))?;
        return Ok((v, url));
    }
    let path = local_path(input);
    let text = std::fs::read_to_string(&path).map_err(|_| {
        BasemapError::InvalidInput(format!(
            "style {input:?} is not a style, a tile URL, a .pmtiles archive or an existing file"
        ))
    })?;
    let v = serde_json::from_str(&text).map_err(|e| BasemapError::StyleParseError(format!("{input}: {e}")))?;
    let url = std::fs::canonicalize(&path)
        .ok()
        .and_then(|p| reqwest::Url::from_file_path(p).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| input.to_owned());
    Ok((v, url))
}

/// Make a style's relative URLs (sources, tiles, GeoJSON data, sprite,
/// glyphs) absolute against `base`, the URL the style was loaded from.
fn absolutize(mut style: Value, base: &str) -> Value {
    let abs = |u: &str| match u.strip_prefix("pmtiles://") {
        Some(inner) => format!("pmtiles://{}", crate::resolve_url(inner, Some(base))),
        None => crate::resolve_url(u, Some(base)),
    };
    if let Some(sources) = style["sources"].as_object_mut() {
        for src in sources.values_mut() {
            for key in ["url", "data"] {
                if let Some(u) = src[key].as_str().map(abs) {
                    src[key] = json!(u);
                }
            }
            if let Some(tiles) = src["tiles"].as_array_mut() {
                for t in tiles.iter_mut() {
                    if let Some(u) = t.as_str().map(abs) {
                        *t = json!(u);
                    }
                }
            }
        }
    }
    if let Some(u) = style["glyphs"].as_str().map(abs) {
        style["glyphs"] = json!(u);
    }
    match &mut style["sprite"] {
        Value::String(u) => *u = abs(u),
        Value::Array(sprites) => {
            for s in sprites.iter_mut() {
                if let Some(u) = s["url"].as_str().map(abs) {
                    s["url"] = json!(u);
                }
            }
        }
        _ => {}
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inputs_are_classified() {
        assert_eq!(classify("pmtiles://https://x/a.pmtiles"), Input::Pmtiles("https://x/a.pmtiles".into()));
        assert_eq!(classify("data/area.PMTiles"), Input::Pmtiles("data/area.PMTiles".into()));
        assert_eq!(classify("https://x/a.pmtiles?v=2"), Input::Pmtiles("https://x/a.pmtiles?v=2".into()));
        assert_eq!(classify("https://t/{z}/{x}/{y}.png"), Input::Template("https://t/{z}/{x}/{y}.png".into()));
        assert_eq!(
            classify("https://s/arcgis/rest/services/World_Imagery/MapServer/"),
            Input::MapServer("https://s/arcgis/rest/services/World_Imagery/MapServer".into())
        );
        assert_eq!(
            classify("https://s/arcgis/rest/services/B/VectorTileServer?f=json"),
            Input::VectorTileServer("https://s/arcgis/rest/services/B/VectorTileServer".into())
        );
        assert_eq!(classify("https://x/style.json"), Input::Resource("https://x/style.json".into()));
    }

    #[test]
    fn template_kind_from_extension() {
        assert_eq!(kind_from_template("https://t/{z}/{x}/{y}.png"), Some(TileKind::Raster));
        assert_eq!(kind_from_template("https://t/{z}/{x}/{y}.pbf?key=1"), Some(TileKind::Vector));
        assert_eq!(kind_from_template("https://w?BBOX={bbox-epsg-3857}"), Some(TileKind::Raster));
        assert_eq!(kind_from_template("https://t/{z}/{x}/{y}"), None);
    }

    #[test]
    fn default_vector_style_and_paint() {
        let mut opts = TileStyleOptions::default();
        opts.paint.insert("fill-color".into(), json!("#123456"));
        opts.paint.insert("text-color".into(), json!("#000"));
        let (s, dropped) =
            build_style(TileKind::Vector, json!({"type": "vector", "tiles": ["t"]}), vec!["roads".into(), "water".into()], &opts);
        let v: Value = serde_json::from_str(&s).unwrap();
        let ids: Vec<&str> = v["layers"].as_array().unwrap().iter().map(|l| l["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["roads-fill", "water-fill", "roads-line", "water-line", "roads-circle", "water-circle"]);
        assert_eq!(v["layers"][0]["paint"]["fill-color"], "#123456");
        assert_eq!(v["layers"][2]["paint"]["line-color"], PALETTE[0]);
        assert_eq!(dropped, ["text-color"]);
        crate::style::validate_maplibre_style(&s).unwrap();
    }

    #[test]
    fn relative_urls_become_absolute() {
        let style = json!({
            "sources": {"esri": {"type": "vector", "url": "../../"}, "p": {"type": "vector", "url": "pmtiles://a.pmtiles"}},
            "sprite": "../sprites/sprite", "glyphs": "../fonts/{fontstack}/{range}.pbf",
        });
        let v = absolutize(style, "https://h/s/VectorTileServer/resources/styles/root.json");
        assert_eq!(v["sources"]["esri"]["url"], "https://h/s/VectorTileServer/");
        assert_eq!(v["sources"]["p"]["url"], "pmtiles://https://h/s/VectorTileServer/resources/styles/a.pmtiles");
        assert_eq!(v["sprite"], "https://h/s/VectorTileServer/resources/sprites/sprite");
        assert_eq!(v["glyphs"], "https://h/s/VectorTileServer/resources/fonts/{fontstack}/{range}.pbf");
    }
}
