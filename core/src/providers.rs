//! Style providers: MapLibre GL styles for common tile services.
//!
//! The single implementation behind `basemap_provider()`, `raster_provider()`
//! and friends in R and Python, so all three languages return the same style
//! for the same call. Each function returns a style that `Basemap` and the
//! renderers accept as `style`: a style URL (named GL styles, ArcGIS vector
//! tile servers) or inline style JSON (raster, vector, WMS, WMTS and PMTiles
//! sources).

use crate::BasemapError;
use serde_json::{json, Map, Value};

/// Paint properties `vector_provider()` accepts.
const FILL_KEYS: [&str; 3] = ["fill-color", "fill-opacity", "fill-outline-color"];
const LINE_KEYS: [&str; 3] = ["line-color", "line-width", "line-opacity"];

/// Every name `basemap_provider()` accepts.
pub const PROVIDER_NAMES: [&str; 25] = [
    "OpenFreeMap.liberty",
    "OpenFreeMap.bright",
    "OpenFreeMap.positron",
    "Carto.Positron",
    "Carto.DarkMatter",
    "Carto.Voyager",
    "ESRI.WorldBasemap",
    "MapTiler.Streets",
    "MapTiler.Satellite",
    "MapTiler.Topo",
    "MapTiler.Outdoor",
    "ESRI.WorldImagery",
    "ESRI.WorldStreetMap",
    "ESRI.WorldTopoMap",
    "ESRI.WorldShadedRelief",
    "ESRI.WorldGrayCanvas",
    "ESRI.NatGeoWorldMap",
    "OSM.Standard",
    "OSM.CyclOSM",
    "Stadia.AlidadeSmooth",
    "Stadia.AlidadeSmoothDark",
    "Stadia.OSMBright",
    "Stadia.StamenToner",
    "Stadia.StamenTerrain",
    "Stadia.StamenWatercolor",
];

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

/// An XYZ raster tile source (`{z}`, `{x}`, `{y}` placeholders).
pub fn raster_provider(url_template: &str, tile_size: u32) -> String {
    raster_style("raster-source", "raster-layer", url_template, tile_size)
}

/// An ArcGIS MapServer raster tile source (e.g. `.../MapServer/tile/{z}/{y}/{x}`).
pub fn esri_raster_provider(url_template: &str, tile_size: u32) -> String {
    raster_style("esri-raster-source", "esri-raster-layer", url_template, tile_size)
}

/// The published style of an ArcGIS VectorTileServer (its `root.json` URL).
pub fn esri_vector_provider(base_url: &str) -> String {
    format!("{}/resources/styles/root.json", base_url.trim_end_matches('/'))
}

/// An MVT vector tile source drawn as one fill and one line layer of
/// `source_layer`. `paint` may set `fill-color`, `fill-opacity`,
/// `fill-outline-color`, `line-color`, `line-width` and `line-opacity`;
/// other keys are dropped and returned so callers can warn about them.
pub fn vector_provider(url_template: &str, source_layer: &str, paint: &Map<String, Value>) -> (String, Vec<String>) {
    let dropped: Vec<String> =
        paint.keys().filter(|k| !FILL_KEYS.contains(&k.as_str()) && !LINE_KEYS.contains(&k.as_str())).cloned().collect();
    let pick = |keys: &[&str]| -> Map<String, Value> {
        keys.iter().filter_map(|k| paint.get(*k).map(|v| (k.to_string(), v.clone()))).collect()
    };
    let mut fill = pick(&FILL_KEYS);
    let mut line = pick(&LINE_KEYS);
    fill.entry("fill-color").or_insert(json!("#e8e0d8"));
    fill.entry("fill-opacity").or_insert(json!(1));
    line.entry("line-color").or_insert(json!("#aaaaaa"));
    line.entry("line-width").or_insert(json!(1));
    let style = json!({
        "version": 8,
        "sources": { "vector-source": { "type": "vector", "tiles": [url_template] } },
        "layers": [
            { "id": "vector-fill", "type": "fill", "source": "vector-source", "source-layer": source_layer, "paint": fill },
            { "id": "vector-line", "type": "line", "source": "vector-source", "source-layer": source_layer, "paint": line },
        ],
    });
    (style.to_string(), dropped)
}

/// A WMS GetMap endpoint as 256 px raster tiles in EPSG:3857, through
/// MapLibre's `{bbox-epsg-3857}` token (filled in per tile). WMS 1.1.x names
/// the projection `SRS`, 1.3.0 `CRS`.
pub fn wms_provider(
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
pub fn wmts_provider(
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

/// A raster PMTiles archive served over HTTP (read with range requests).
pub fn pmtiles_provider(url: &str) -> String {
    raster_style("pmtiles", "pmtiles-layer", &format!("pmtiles://{url}"), 256)
}

/// A named basemap (see [`PROVIDER_NAMES`]): a style URL for GL styles, or
/// inline style JSON for raster services. MapTiler styles need `api_key`;
/// Stadia's work without one at low volume.
pub fn basemap_provider(name: &str, api_key: Option<&str>) -> Result<String, BasemapError> {
    let esri = "https://server.arcgisonline.com/ArcGIS/rest/services";
    let url = match name {
        "OpenFreeMap.liberty" => "https://tiles.openfreemap.org/styles/liberty".to_owned(),
        "OpenFreeMap.bright" => "https://tiles.openfreemap.org/styles/bright".to_owned(),
        "OpenFreeMap.positron" => "https://tiles.openfreemap.org/styles/positron".to_owned(),
        "Carto.Positron" => "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json".to_owned(),
        "Carto.DarkMatter" => "https://basemaps.cartocdn.com/gl/dark-matter-gl-style/style.json".to_owned(),
        "Carto.Voyager" => "https://basemaps.cartocdn.com/gl/voyager-gl-style/style.json".to_owned(),
        "ESRI.WorldBasemap" => {
            esri_vector_provider("https://basemaps.arcgis.com/arcgis/rest/services/World_Basemap_v2/VectorTileServer")
        }
        "MapTiler.Streets" | "MapTiler.Satellite" | "MapTiler.Topo" | "MapTiler.Outdoor" => {
            let key = api_key.ok_or_else(|| BasemapError::InvalidInput(format!("provider '{name}' requires an api_key")))?;
            let map = match name {
                "MapTiler.Streets" => "streets-v2",
                "MapTiler.Satellite" => "satellite",
                "MapTiler.Topo" => "topo-v2",
                _ => "outdoor-v2",
            };
            format!("https://api.maptiler.com/maps/{map}/style.json?key={key}")
        }
        "ESRI.WorldImagery" => esri_raster_provider(&format!("{esri}/World_Imagery/MapServer/tile/{{z}}/{{y}}/{{x}}"), 256),
        "ESRI.WorldStreetMap" => esri_raster_provider(&format!("{esri}/World_Street_Map/MapServer/tile/{{z}}/{{y}}/{{x}}"), 256),
        "ESRI.WorldTopoMap" => esri_raster_provider(&format!("{esri}/World_Topo_Map/MapServer/tile/{{z}}/{{y}}/{{x}}"), 256),
        "ESRI.WorldShadedRelief" => {
            esri_raster_provider(&format!("{esri}/World_Shaded_Relief/MapServer/tile/{{z}}/{{y}}/{{x}}"), 256)
        }
        "ESRI.WorldGrayCanvas" => {
            esri_raster_provider(&format!("{esri}/Canvas/World_Light_Gray_Base/MapServer/tile/{{z}}/{{y}}/{{x}}"), 256)
        }
        "ESRI.NatGeoWorldMap" => esri_raster_provider(&format!("{esri}/NatGeo_World_Map/MapServer/tile/{{z}}/{{y}}/{{x}}"), 256),
        "OSM.Standard" => raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png", 256),
        "OSM.CyclOSM" => raster_provider("https://tile-cyclosm.openstreetmap.fr/cyclosm/{z}/{x}/{y}.png", 256),
        "Stadia.AlidadeSmooth" | "Stadia.AlidadeSmoothDark" | "Stadia.OSMBright" | "Stadia.StamenToner"
        | "Stadia.StamenTerrain" | "Stadia.StamenWatercolor" => {
            let tiles = match name {
                "Stadia.AlidadeSmooth" => "alidade_smooth",
                "Stadia.AlidadeSmoothDark" => "alidade_smooth_dark",
                "Stadia.OSMBright" => "osm_bright",
                "Stadia.StamenToner" => "stamen_toner",
                "Stadia.StamenTerrain" => "stamen_terrain",
                _ => "stamen_watercolor",
            };
            let mut url = format!("https://tiles.stadiamaps.com/tiles/{tiles}/{{z}}/{{x}}/{{y}}.png");
            if let Some(key) = api_key {
                url.push_str("?api_key=");
                url.push_str(key);
            }
            raster_provider(&url, 256)
        }
        _ => {
            return Err(BasemapError::InvalidInput(format!(
                "unknown provider '{name}'; see list_providers() for the available names"
            )))
        }
    };
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_provider_resolves() {
        for name in PROVIDER_NAMES {
            let s = basemap_provider(name, Some("KEY")).unwrap();
            assert!(s.starts_with("https://") || s.starts_with('{'), "{name}: {s}");
        }
        assert!(basemap_provider("MapTiler.Streets", None).is_err());
        assert!(basemap_provider("Nope.Nothing", None).is_err());
    }

    #[test]
    fn provider_styles() {
        let v: Value = serde_json::from_str(&raster_provider("https://t/{z}/{x}/{y}.png", 512)).unwrap();
        assert_eq!(v["sources"]["raster-source"]["tileSize"], 512);
        let paint: Map<String, Value> = serde_json::from_str(r##"{"fill-color": "#123456", "bogus": 1}"##).unwrap();
        let (s, dropped) = vector_provider("https://t/{z}/{x}/{y}.pbf", "water", &paint);
        assert_eq!(dropped, vec!["bogus".to_owned()]);
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["layers"][0]["paint"]["fill-color"], "#123456");
        assert_eq!(v["layers"][1]["paint"]["line-width"], 1);
        let wms = wms_provider("https://w", "a b", "", "1.3.0", "image/png", true, Some("X=1"));
        assert!(wms.contains("LAYERS=a%20b") && wms.contains("FORMAT=image%2Fpng") && wms.contains("TRANSPARENT=TRUE&X=1"));
        assert!(wms.contains("&CRS=EPSG:3857&BBOX={bbox-epsg-3857}&"));
        assert!(wms_provider("https://w", "a", "", "1.1.1", "image/png", false, None).contains("&SRS=EPSG:3857&"));
        assert_eq!(esri_vector_provider("https://e/VectorTileServer/"), "https://e/VectorTileServer/resources/styles/root.json");
        assert!(pmtiles_provider("https://p/a.pmtiles").contains("pmtiles://https://p/a.pmtiles"));
    }
}
