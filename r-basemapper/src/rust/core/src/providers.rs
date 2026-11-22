//! Named basemaps and OGC service styles.
//!
//! The catalogue behind `basemap_provider()` and `list_providers()` in R and
//! Python, so all three languages return the same style for the same name.
//! Names follow leaflet-providers (`CartoDB.Positron`, `Esri.WorldImagery`,
//! `OpenStreetMap.HOT`); basemaps leaflet lacks (OpenFreeMap) use the same
//! `Provider.Variant` form. An entry has a vector form (a MapLibre style),
//! a raster form (tiles), or both; the vector form is the default.
//!
//! Tile URLs, TileJSON, PMTiles archives and ArcGIS services need no entry:
//! they are accepted as `style` directly, and [`crate::tile_style`] builds
//! their style with overrides.

use crate::tile_style::{TileKind, TileStyleOptions};
use crate::BasemapError;
use serde_json::{json, Value};

/// The raster form of a basemap.
#[derive(Debug, Clone, Copy)]
pub struct Raster {
    /// Tile URL in leaflet form: `{s}` a subdomain, `{r}` `@2x` on
    /// high-density displays (MapLibre's `{ratio}`), `{key}` the API key.
    pub tiles: &'static str,
    /// One subdomain per character, filled into `{s}`.
    pub subdomains: &'static str,
    pub maxzoom: u8,
}

/// The API key a basemap needs.
#[derive(Debug, Clone, Copy)]
pub struct ApiKey {
    /// Environment variable read when no `api_key` is passed.
    pub env: &'static str,
    /// Where to get a key.
    pub signup: &'static str,
}

/// A catalogue entry.
#[derive(Debug, Clone, Copy)]
pub struct Provider {
    pub name: &'static str,
    /// MapLibre style URL (`{key}` for the API key).
    pub vector: Option<&'static str>,
    pub raster: Option<Raster>,
    /// Plain-text attribution the provider asks for.
    pub attribution: &'static str,
    pub key: Option<ApiKey>,
}

const fn raster(tiles: &'static str, subdomains: &'static str, maxzoom: u8) -> Option<Raster> {
    Some(Raster { tiles, subdomains, maxzoom })
}

const CARTO_KEY: Option<ApiKey> = Some(ApiKey { env: "CARTO_API_KEY", signup: "https://carto.com/basemaps/apikey" });
const MAPTILER: Option<ApiKey> = Some(ApiKey { env: "MAPTILER_API_KEY", signup: "https://cloud.maptiler.com" });
const STADIA: Option<ApiKey> = Some(ApiKey { env: "STADIA_API_KEY", signup: "https://client.stadiamaps.com" });

const OSM: &str = "© OpenStreetMap contributors";
const CARTO: &str = "© OpenStreetMap contributors © CARTO";
const OPENFREEMAP: &str = "OpenFreeMap © OpenMapTiles Data from OpenStreetMap";
const STADIA_ATTR: &str = "© Stadia Maps © OpenMapTiles © OpenStreetMap contributors";
const STAMEN: &str = "© Stadia Maps © Stamen Design © OpenMapTiles © OpenStreetMap contributors";
const MAPTILER_ATTR: &str = "© MapTiler © OpenStreetMap contributors";
const USGS: &str = "Tiles courtesy of the U.S. Geological Survey";

macro_rules! esri_tiles {
    ($service:literal) => {
        concat!("https://server.arcgisonline.com/ArcGIS/rest/services/", $service, "/MapServer/tile/{z}/{y}/{x}")
    };
}
macro_rules! esri_style {
    ($item:literal) => {
        concat!("https://www.arcgis.com/sharing/rest/content/items/", $item, "/resources/styles/root.json")
    };
}
macro_rules! carto {
    ($name:literal, $gl:literal, $raster:literal) => {
        Provider {
            name: $name,
            vector: Some(concat!("https://basemaps.cartocdn.com/gl/", $gl, "-gl-style/style.json?key={key}")),
            raster: raster(concat!("https://{s}.basemaps.cartocdn.com/", $raster, "/{z}/{x}/{y}{r}.png?key={key}"), "abcd", 20),
            attribution: CARTO,
            key: CARTO_KEY,
        }
    };
}
macro_rules! openfreemap {
    ($name:literal, $style:literal) => {
        Provider {
            name: $name,
            vector: Some(concat!("https://tiles.openfreemap.org/styles/", $style)),
            raster: None,
            attribution: OPENFREEMAP,
            key: None,
        }
    };
}
macro_rules! stadia {
    ($name:literal, $id:literal, $ext:literal, $maxzoom:literal, $attr:expr) => {
        Provider {
            name: $name,
            vector: Some(concat!("https://tiles.stadiamaps.com/styles/", $id, ".json?api_key={key}")),
            raster: raster(
                concat!("https://tiles.stadiamaps.com/tiles/", $id, "/{z}/{x}/{y}{r}.", $ext, "?api_key={key}"),
                "",
                $maxzoom,
            ),
            attribution: $attr,
            key: STADIA,
        }
    };
}
macro_rules! maptiler {
    ($name:literal, $id:literal, $ext:literal) => {
        Provider {
            name: $name,
            vector: Some(concat!("https://api.maptiler.com/maps/", $id, "/style.json?key={key}")),
            raster: raster(concat!("https://api.maptiler.com/maps/", $id, "/256/{z}/{x}/{y}{r}.", $ext, "?key={key}"), "", 22),
            attribution: MAPTILER_ATTR,
            key: MAPTILER,
        }
    };
}

/// Every basemap `basemap_provider()` accepts.
pub const PROVIDERS: &[Provider] = &[
    // OpenStreetMap and OSM-based raster styles
    Provider { name: "OpenStreetMap.Mapnik", vector: None, raster: raster("https://tile.openstreetmap.org/{z}/{x}/{y}.png", "", 19), attribution: OSM, key: None },
    Provider { name: "OpenStreetMap.DE", vector: None, raster: raster("https://tile.openstreetmap.de/{z}/{x}/{y}.png", "", 18), attribution: OSM, key: None },
    Provider {
        name: "OpenStreetMap.France",
        vector: None,
        raster: raster("https://{s}.tile.openstreetmap.fr/osmfr/{z}/{x}/{y}.png", "abc", 20),
        attribution: "© OpenStreetMap France | © OpenStreetMap contributors",
        key: None,
    },
    Provider {
        name: "OpenStreetMap.HOT",
        vector: None,
        raster: raster("https://{s}.tile.openstreetmap.fr/hot/{z}/{x}/{y}.png", "abc", 19),
        attribution: "© OpenStreetMap contributors, Tiles style by Humanitarian OpenStreetMap Team hosted by OpenStreetMap France",
        key: None,
    },
    Provider {
        name: "OpenTopoMap",
        vector: None,
        raster: raster("https://{s}.tile.opentopomap.org/{z}/{x}/{y}.png", "abc", 17),
        attribution: "Map data: © OpenStreetMap contributors, SRTM | Map style: © OpenTopoMap (CC-BY-SA)",
        key: None,
    },
    Provider {
        name: "CyclOSM",
        vector: None,
        raster: raster("https://{s}.tile-cyclosm.openstreetmap.fr/cyclosm/{z}/{x}/{y}.png", "abc", 20),
        attribution: "CyclOSM | Map data: © OpenStreetMap contributors",
        key: None,
    },
    // OpenFreeMap (vector only, no key)
    openfreemap!("OpenFreeMap.Liberty", "liberty"),
    openfreemap!("OpenFreeMap.Bright", "bright"),
    openfreemap!("OpenFreeMap.Positron", "positron"),
    openfreemap!("OpenFreeMap.Dark", "dark"),
    openfreemap!("OpenFreeMap.Fiord", "fiord"),
    // CARTO (free API key since 2026)
    carto!("CartoDB.Positron", "positron", "light_all"),
    carto!("CartoDB.DarkMatter", "dark-matter", "dark_all"),
    carto!("CartoDB.Voyager", "voyager", "rastertiles/voyager"),
    // Esri
    Provider {
        name: "Esri.WorldStreetMap",
        vector: Some(esri_style!("de26a3cf4cc9451298ea173c4b324736")),
        raster: raster(esri_tiles!("World_Street_Map"), "", 19),
        attribution: "Tiles © Esri — Source: Esri, HERE, Garmin, USGS, Intermap, INCREMENT P, NRCan, Esri Japan, METI, Esri China (Hong Kong), Esri Korea, Esri (Thailand), NGCC, © OpenStreetMap contributors, and the GIS User Community",
        key: None,
    },
    Provider {
        name: "Esri.WorldTopoMap",
        vector: Some(esri_style!("7dc6cea0b1764a1f9af2e679f642f0f5")),
        raster: raster(esri_tiles!("World_Topo_Map"), "", 19),
        attribution: "Tiles © Esri — Esri, HERE, Garmin, Intermap, INCREMENT P, GEBCO, USGS, FAO, NPS, NRCAN, GeoBase, IGN, Kadaster NL, Ordnance Survey, Esri Japan, METI, Esri China (Hong Kong), © OpenStreetMap contributors, and the GIS User Community",
        key: None,
    },
    Provider {
        name: "Esri.WorldGrayCanvas",
        vector: Some(esri_style!("291da5eab3a0412593b66d384379f89f")),
        raster: raster(esri_tiles!("Canvas/World_Light_Gray_Base"), "", 16),
        attribution: "Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors, and the GIS User Community",
        key: None,
    },
    Provider {
        name: "Esri.WorldDarkGrayCanvas",
        vector: Some(esri_style!("5e9b3685f4c24d8781073dd928ebda50")),
        raster: raster(esri_tiles!("Canvas/World_Dark_Gray_Base"), "", 16),
        attribution: "Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors, and the GIS User Community",
        key: None,
    },
    Provider {
        name: "Esri.WorldImagery",
        vector: None,
        raster: raster(esri_tiles!("World_Imagery"), "", 19),
        attribution: "Tiles © Esri — Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community",
        key: None,
    },
    Provider {
        name: "Esri.WorldTerrain",
        vector: None,
        raster: raster(esri_tiles!("World_Terrain_Base"), "", 13),
        attribution: "Tiles © Esri — Source: USGS, Esri, TANA, DeLorme, and NPS",
        key: None,
    },
    Provider {
        name: "Esri.WorldShadedRelief",
        vector: None,
        raster: raster(esri_tiles!("World_Shaded_Relief"), "", 13),
        attribution: "Tiles © Esri — Source: Esri",
        key: None,
    },
    Provider {
        name: "Esri.WorldPhysical",
        vector: None,
        raster: raster(esri_tiles!("World_Physical_Map"), "", 8),
        attribution: "Tiles © Esri — Source: US National Park Service",
        key: None,
    },
    Provider {
        name: "Esri.OceanBasemap",
        vector: None,
        raster: raster(esri_tiles!("Ocean/World_Ocean_Base"), "", 13),
        attribution: "Tiles © Esri — Sources: GEBCO, NOAA, CHS, OSU, UNH, CSUMB, National Geographic, DeLorme, NAVTEQ, and Esri",
        key: None,
    },
    Provider {
        name: "Esri.NatGeoWorldMap",
        vector: None,
        raster: raster(esri_tiles!("NatGeo_World_Map"), "", 16),
        attribution: "Tiles © Esri — National Geographic, Esri, DeLorme, NAVTEQ, UNEP-WCMC, USGS, NASA, ESA, METI, NRCAN, GEBCO, NOAA, iPC",
        key: None,
    },
    // USGS The National Map
    Provider {
        name: "USGS.USTopo",
        vector: None,
        raster: raster("https://basemap.nationalmap.gov/arcgis/rest/services/USGSTopo/MapServer/tile/{z}/{y}/{x}", "", 16),
        attribution: USGS,
        key: None,
    },
    Provider {
        name: "USGS.USImagery",
        vector: None,
        raster: raster("https://basemap.nationalmap.gov/arcgis/rest/services/USGSImageryOnly/MapServer/tile/{z}/{y}/{x}", "", 16),
        attribution: USGS,
        key: None,
    },
    // Stadia Maps (API key)
    stadia!("Stadia.AlidadeSmooth", "alidade_smooth", "png", 20, STADIA_ATTR),
    stadia!("Stadia.AlidadeSmoothDark", "alidade_smooth_dark", "png", 20, STADIA_ATTR),
    stadia!("Stadia.AlidadeSatellite", "alidade_satellite", "jpg", 20, STADIA_ATTR),
    stadia!("Stadia.OSMBright", "osm_bright", "png", 20, STADIA_ATTR),
    stadia!("Stadia.Outdoors", "outdoors", "png", 20, STADIA_ATTR),
    stadia!("Stadia.StamenToner", "stamen_toner", "png", 20, STAMEN),
    stadia!("Stadia.StamenTonerLite", "stamen_toner_lite", "png", 20, STAMEN),
    stadia!("Stadia.StamenTerrain", "stamen_terrain", "png", 18, STAMEN),
    stadia!("Stadia.StamenWatercolor", "stamen_watercolor", "jpg", 16, STAMEN),
    // MapTiler (API key)
    maptiler!("MapTiler.Streets", "streets-v2", "png"),
    maptiler!("MapTiler.Basic", "basic-v2", "png"),
    maptiler!("MapTiler.Bright", "bright-v2", "png"),
    maptiler!("MapTiler.Outdoor", "outdoor-v2", "png"),
    maptiler!("MapTiler.Topo", "topo-v2", "png"),
    maptiler!("MapTiler.Dataviz", "dataviz", "png"),
    maptiler!("MapTiler.Satellite", "satellite", "jpg"),
];

/// Earlier names, kept working.
const ALIASES: &[(&str, &str)] = &[
    ("OSM.Standard", "OpenStreetMap.Mapnik"),
    ("OpenStreetMap", "OpenStreetMap.Mapnik"),
    ("OSM.CyclOSM", "CyclOSM"),
    ("ESRI.WorldBasemap", "Esri.WorldStreetMap"),
];

/// Family names people use for the same provider.
fn normalize(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    let (family, rest) = lower.split_once('.').unwrap_or((lower.as_str(), ""));
    let family = match family {
        "carto" => "cartodb",
        "osm" => "openstreetmap",
        f => f,
    };
    if rest.is_empty() {
        family.to_owned()
    } else {
        format!("{family}.{rest}")
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// The catalogue entry for `name` (case-insensitive; `Carto.` and `OSM.`
/// work for `CartoDB.` and `OpenStreetMap.`, and earlier names still work).
pub fn find_provider(name: &str) -> Option<&'static Provider> {
    let key = normalize(name);
    let target = ALIASES
        .iter()
        .find(|(alias, _)| normalize(alias) == key)
        .map_or(key, |(_, to)| normalize(to));
    PROVIDERS.iter().find(|p| normalize(p.name) == target)
}

/// Names close to an unknown `name`, best first.
fn suggestions(name: &str) -> Vec<&'static str> {
    let key = normalize(name);
    let limit = (key.len() / 4).max(2);
    let mut scored: Vec<(usize, &'static str)> = PROVIDERS
        .iter()
        .map(|p| (levenshtein(&key, &normalize(p.name)), p.name))
        .filter(|(d, _)| *d <= limit)
        .collect();
    scored.sort();
    let mut out: Vec<&'static str> = scored.into_iter().take(3).map(|(_, n)| n).collect();
    if out.is_empty() {
        // Same family, e.g. "CartoDB.Light": list its basemaps.
        let family = key.split('.').next().unwrap_or("");
        out = PROVIDERS
            .iter()
            .filter(|p| normalize(p.name).split('.').next() == Some(family))
            .map(|p| p.name)
            .collect();
    }
    out
}

fn unknown(name: &str) -> BasemapError {
    let s = suggestions(name);
    let hint = match s.len() {
        0 => String::new(),
        1 => format!("did you mean {}? ", s[0]),
        _ => format!("did you mean one of {}? ", s.join(", ")),
    };
    BasemapError::InvalidInput(format!("unknown basemap '{name}'; {hint}See list_providers() for every name"))
}

/// The default style of a catalogue name given as `style`, e.g.
/// `"CartoDB.Positron"`. `None` for anything that may be a path or URL; an
/// error for a near miss of a name (with suggestions).
pub(crate) fn bare_name_style(input: &str) -> Result<Option<String>, BasemapError> {
    let s = input.trim();
    let file_like = [".json", ".pmtiles", ".mbtiles", ".geojson", ".png", ".jpg", ".pbf", ".mvt"]
        .iter()
        .any(|ext| s.to_ascii_lowercase().ends_with(ext));
    if s.is_empty() || file_like || s.contains(['/', '\\', ':', '{', '?']) || std::path::Path::new(s).exists() {
        return Ok(None);
    }
    match find_provider(s) {
        Some(p) => basemap_provider(p.name, &ProviderOptions::default()).map(Some),
        None if !suggestions(s).is_empty() => Err(unknown(s)),
        None => Ok(None),
    }
}

impl Provider {
    /// The forms this basemap comes in, vector first.
    pub fn kinds(&self) -> Vec<TileKind> {
        let mut k = Vec::new();
        if self.vector.is_some() {
            k.push(TileKind::Vector);
        }
        if self.raster.is_some() {
            k.push(TileKind::Raster);
        }
        k
    }
}

/// The basemaps that come in `kind` (all with `None`), in catalogue order.
pub fn list_providers(kind: Option<TileKind>) -> Vec<&'static Provider> {
    PROVIDERS.iter().filter(|p| kind.is_none_or(|k| p.kinds().contains(&k))).collect()
}

/// Options for [`basemap_provider`].
#[derive(Debug, Clone, Default)]
pub struct ProviderOptions {
    /// Vector style or raster tiles; default the vector form when there is one.
    pub kind: Option<TileKind>,
    /// API key; default from the provider's environment variable.
    pub api_key: Option<String>,
    /// Tile data to draw instead of the provider's own (a `.pmtiles` archive,
    /// tile URL or TileJSON, by URL or path), keeping its style. It must use
    /// the same schema (source layers and fields) as the provider's tiles.
    pub tiles: Option<String>,
}

fn other_kind(kind: TileKind) -> TileKind {
    match kind {
        TileKind::Vector => TileKind::Raster,
        TileKind::Raster => TileKind::Vector,
    }
}

/// A named basemap's style: a style URL for the vector form, inline style
/// JSON for the raster form (or for either with `tiles`). See [`PROVIDERS`].
pub fn basemap_provider(name: &str, options: &ProviderOptions) -> Result<String, BasemapError> {
    let p = find_provider(name).ok_or_else(|| unknown(name))?;
    let kind = options.kind.unwrap_or(p.kinds()[0]);
    if !p.kinds().contains(&kind) {
        let other = other_kind(kind);
        let form = |k: TileKind| if k == TileKind::Vector { "a vector style" } else { "raster tiles" };
        return Err(BasemapError::InvalidInput(format!(
            "{} comes only as {}; use type \"{}\", or see list_providers(\"{}\") for basemaps with {}",
            p.name,
            form(other),
            other.name(),
            kind.name(),
            form(kind),
        )));
    }
    let key = match p.key {
        None => String::new(),
        Some(k) => options
            .api_key
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var(k.env).ok().filter(|s| !s.is_empty()))
            .ok_or_else(|| {
                BasemapError::InvalidInput(format!(
                    "{} needs an API key: pass api_key or set the {} environment variable (keys: {})",
                    p.name, k.env, k.signup
                ))
            })?,
    };
    let style = match kind {
        TileKind::Vector => p.vector.unwrap_or_default().replace("{key}", &key),
        TileKind::Raster => raster_style_for(p, &key),
    };
    match &options.tiles {
        None => Ok(style),
        Some(tiles) => crate::RUNTIME.block_on(async {
            let client = crate::http_client(std::time::Duration::from_secs(30))?;
            replace_tiles(&style, kind, tiles, &client).await
        }),
    }
}

/// The raster form as a style: one raster source, `{s}` expanded into one
/// tile URL per subdomain as MapLibre spreads requests over `tiles`.
fn raster_style_for(p: &Provider, key: &str) -> String {
    let r = p.raster.expect("raster form");
    let template = r.tiles.replace("{r}", "{ratio}").replace("{key}", key);
    let tiles: Vec<String> = if r.subdomains.is_empty() {
        vec![template]
    } else {
        r.subdomains.chars().map(|s| template.replace("{s}", &s.to_string())).collect()
    };
    json!({
        "version": 8,
        "sources": { "raster": {
            "type": "raster",
            "tiles": tiles,
            "tileSize": 256,
            "maxzoom": r.maxzoom,
            "attribution": p.attribution,
        }},
        "layers": [{ "id": "raster", "type": "raster", "source": "raster" }],
    })
    .to_string()
}

/// `style` with every source of `kind` drawing `tiles` instead.
async fn replace_tiles(
    style: &str,
    kind: TileKind,
    tiles: &str,
    client: &reqwest::Client,
) -> Result<String, BasemapError> {
    let new = match crate::tile_style::vector_tile_server(tiles) {
        Some(base) => json!({"type": "vector", "url": base}),
        None => {
            // The source tile_style() builds for this data, without probing.
            let opts = TileStyleOptions { kind: Some(kind), source_layers: Some(Vec::new()), ..Default::default() };
            let (built, _) = crate::tile_style::tile_style(tiles, &opts, client, None).await?;
            let v: Value = serde_json::from_str(&built).map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
            match v.pointer("/sources/tiles") {
                Some(s) => s.clone(),
                None => {
                    return Err(BasemapError::InvalidInput(format!(
                        "tiles {tiles:?} is a style; pass tile data (a .pmtiles archive, tile URL or TileJSON)"
                    )))
                }
            }
        }
    };
    let mut v: Value = if style.trim_start().starts_with('{') {
        serde_json::from_str(style).map_err(|e| BasemapError::StyleParseError(e.to_string()))?
    } else {
        let resolved = crate::tile_style::resolve_style_input(style, client, None).await?;
        serde_json::from_str(&resolved).map_err(|e| BasemapError::StyleParseError(e.to_string()))?
    };
    let by_url = new.get("url").is_some();
    let mut replaced = 0;
    if let Some(sources) = v["sources"].as_object_mut() {
        for src in sources.values_mut() {
            let Some(o) = src.as_object_mut() else { continue };
            if o.get("type").and_then(Value::as_str) != Some(kind.name()) {
                continue;
            }
            for k in ["url", "tiles", "scheme", "bounds", "attribution"] {
                o.remove(k);
            }
            if by_url {
                // A TileJSON or archive carries its own zoom range.
                o.remove("minzoom");
                o.remove("maxzoom");
            }
            for (k, val) in new.as_object().into_iter().flatten() {
                if k != "type" {
                    o.insert(k.clone(), val.clone());
                }
            }
            replaced += 1;
        }
    }
    if replaced == 0 {
        return Err(BasemapError::InvalidInput(format!("the style has no {} source to draw tiles from", kind.name())));
    }
    Ok(v.to_string())
}

/// A style with one raster source and layer.
fn raster_style(source: &str, layer: &str, tile_url: &str, tile_size: u32) -> String {
    json!({
        "version": 8,
        "sources": { source: { "type": "raster", "tiles": [tile_url], "tileSize": tile_size } },
        "layers": [{ "id": layer, "type": "raster", "source": source }],
    })
    .to_string()
}

/// Percent-encode a query value (everything but unreserved characters).
fn encode_query(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A WMS GetMap endpoint as 256 px raster tiles in EPSG:3857, through
/// MapLibre's `{bbox-epsg-3857}` token (filled in per tile). WMS 1.1.x names
/// the projection `SRS`, 1.3.0 `CRS`.
pub fn wms_style(
    url: &str,
    layers: &str,
    styles: &str,
    version: &str,
    format: &str,
    transparent: bool,
    extra_params: Option<&str>,
) -> String {
    let crs = if version.starts_with("1.0") || version.starts_with("1.1") { "SRS" } else { "CRS" };
    let mut tile_url = format!(
        "{url}?SERVICE=WMS&VERSION={version}&REQUEST=GetMap&LAYERS={}&STYLES={}&{crs}=EPSG:3857\
         &BBOX={{bbox-epsg-3857}}&WIDTH=256&HEIGHT=256&FORMAT={}&TRANSPARENT={}",
        encode_query(layers),
        encode_query(styles),
        encode_query(format),
        if transparent { "TRUE" } else { "FALSE" },
    );
    if let Some(extra) = extra_params.filter(|e| !e.is_empty()) {
        tile_url.push('&');
        tile_url.push_str(extra);
    }
    raster_style("wms", "wms-layer", &tile_url, 256)
}

/// A WMTS GetTile (KVP) endpoint as 256 px raster tiles.
pub fn wmts_style(
    url: &str,
    layer: &str,
    tilematrixset: &str,
    format: &str,
    style: &str,
    version: &str,
    extra_params: Option<&str>,
) -> String {
    let mut tile_url = format!(
        "{url}?SERVICE=WMTS&REQUEST=GetTile&VERSION={version}&LAYER={}&STYLE={}&FORMAT={}&TILEMATRIXSET={}\
         &TILEMATRIX={{z}}&TILEROW={{y}}&TILECOL={{x}}",
        encode_query(layer),
        encode_query(style),
        encode_query(format),
        encode_query(tilematrixset),
    );
    if let Some(extra) = extra_params.filter(|e| !e.is_empty()) {
        tile_url.push('&');
        tile_url.push_str(extra);
    }
    raster_style("wmts", "wmts-layer", &tile_url, 256)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn opts(kind: Option<TileKind>, key: Option<&str>) -> ProviderOptions {
        ProviderOptions { kind, api_key: key.map(str::to_owned), tiles: None }
    }

    #[test]
    fn every_form_of_every_provider_resolves() {
        for p in PROVIDERS {
            assert!(!p.kinds().is_empty() && !p.attribution.is_empty(), "{}", p.name);
            for kind in p.kinds() {
                let s = basemap_provider(p.name, &opts(Some(kind), Some("KEY"))).unwrap();
                assert!(!s.contains("{key}") && !s.contains("{s}") && !s.contains("{r}"), "{}: {s}", p.name);
                match kind {
                    TileKind::Vector => assert!(s.starts_with("https://"), "{}: {s}", p.name),
                    TileKind::Raster => {
                        let v: Value = serde_json::from_str(&s).unwrap();
                        assert_eq!(v["sources"]["raster"]["attribution"], p.attribution);
                    }
                }
            }
        }
    }

    #[test]
    fn names_are_unique_and_lookup_is_forgiving() {
        for (i, p) in PROVIDERS.iter().enumerate() {
            assert!(PROVIDERS[..i].iter().all(|q| normalize(q.name) != normalize(p.name)), "{}", p.name);
        }
        assert_eq!(find_provider("cartodb.positron").unwrap().name, "CartoDB.Positron");
        assert_eq!(find_provider("Carto.Voyager").unwrap().name, "CartoDB.Voyager");
        assert_eq!(find_provider("OSM.HOT").unwrap().name, "OpenStreetMap.HOT");
        assert_eq!(find_provider("OSM.Standard").unwrap().name, "OpenStreetMap.Mapnik");
        assert_eq!(find_provider("ESRI.WorldImagery").unwrap().name, "Esri.WorldImagery");
        assert!(find_provider("Nope").is_none());
    }

    #[test]
    fn forms_keys_and_errors() {
        // Vector by default, raster on request.
        let gray = basemap_provider("Esri.WorldGrayCanvas", &opts(None, None)).unwrap();
        assert!(gray.ends_with("291da5eab3a0412593b66d384379f89f/resources/styles/root.json"));
        let gray = basemap_provider("Esri.WorldGrayCanvas", &opts(Some(TileKind::Raster), None)).unwrap();
        assert!(gray.contains("World_Light_Gray_Base/MapServer/tile/{z}/{y}/{x}"));
        // Raster-only gets its raster form; asking for vector is an error naming the fix.
        assert!(basemap_provider("Esri.WorldImagery", &opts(None, None)).unwrap().starts_with('{'));
        let e = basemap_provider("Esri.WorldImagery", &opts(Some(TileKind::Vector), None)).unwrap_err().to_string();
        assert!(e.contains("only as raster tiles") && e.contains("type \"raster\""), "{e}");
        let e = basemap_provider("OpenFreeMap.Liberty", &opts(Some(TileKind::Raster), None)).unwrap_err().to_string();
        assert!(e.contains("only as a vector style"), "{e}");
        // Subdomains spread over `tiles`, {r} becomes MapLibre's {ratio}.
        let v: Value =
            serde_json::from_str(&basemap_provider("CartoDB.Positron", &opts(Some(TileKind::Raster), Some("K"))).unwrap())
                .unwrap();
        let tiles = v["sources"]["raster"]["tiles"].as_array().unwrap();
        assert_eq!(tiles.len(), 4);
        assert_eq!(tiles[3], "https://d.basemaps.cartocdn.com/light_all/{z}/{x}/{y}{ratio}.png?key=K");
        // API keys: argument, else environment variable, else an error naming both.
        assert!(basemap_provider("MapTiler.Streets", &opts(None, Some("K1"))).unwrap().ends_with("style.json?key=K1"));
        let e = basemap_provider("MapTiler.Topo", &opts(None, None));
        if std::env::var("MAPTILER_API_KEY").is_err() {
            let e = e.unwrap_err().to_string();
            assert!(e.contains("api_key") && e.contains("MAPTILER_API_KEY"), "{e}");
        }
        // Unknown names suggest close ones.
        let e = basemap_provider("CartoDB.Positon", &opts(None, None)).unwrap_err().to_string();
        assert!(e.contains("did you mean CartoDB.Positron?"), "{e}");
        let e = basemap_provider("Esri.Nothing", &opts(None, None)).unwrap_err().to_string();
        assert!(e.contains("Esri.WorldImagery"), "{e}");
    }

    #[test]
    fn list_by_kind() {
        let vector = list_providers(Some(TileKind::Vector));
        assert!(vector.iter().all(|p| p.vector.is_some()) && vector.iter().any(|p| p.name == "OpenFreeMap.Liberty"));
        assert!(list_providers(Some(TileKind::Raster)).iter().all(|p| p.raster.is_some()));
        assert_eq!(list_providers(None).len(), PROVIDERS.len());
    }

    #[test]
    fn ogc_styles() {
        let wms = wms_style("https://w", "a b", "", "1.3.0", "image/png", true, Some("X=1"));
        assert!(wms.contains("LAYERS=a%20b") && wms.contains("FORMAT=image%2Fpng") && wms.contains("TRANSPARENT=TRUE&X=1"));
        assert!(wms.contains("&CRS=EPSG:3857&BBOX={bbox-epsg-3857}&"));
        assert!(wms_style("https://w", "a", "", "1.1.1", "image/png", false, None).contains("&SRS=EPSG:3857&"));
        let wmts = wmts_style("https://w", "lyr", "GoogleMapsCompatible", "image/png", "default", "1.0.0", None);
        assert!(wmts.contains("LAYER=lyr") && wmts.contains("TILEMATRIX={z}&TILEROW={y}&TILECOL={x}"));
    }
}
