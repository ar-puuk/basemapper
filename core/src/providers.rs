//! Named basemaps and OGC service styles.
//!
//! The single implementation behind `basemap_provider()`, `wms_style()` and
//! `wmts_style()` in R and Python, so all three languages return the same
//! style for the same call. Tile URLs, TileJSON, PMTiles archives and ArcGIS
//! services need no builder: they are accepted as `style` directly, and
//! [`crate::tile_style`] builds their style with overrides.

use crate::BasemapError;
use serde_json::json;

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
fn raster_provider(url_template: &str, tile_size: u32) -> String {
    raster_style("raster-source", "raster-layer", url_template, tile_size)
}

/// An ArcGIS MapServer raster tile source (e.g. `.../MapServer/tile/{z}/{y}/{x}`).
fn esri_raster_provider(url_template: &str, tile_size: u32) -> String {
    raster_style("esri-raster-source", "esri-raster-layer", url_template, tile_size)
}

/// The published style of an ArcGIS VectorTileServer (its `root.json` URL).
fn esri_vector_provider(base_url: &str) -> String {
    format!("{}/resources/styles/root.json", base_url.trim_end_matches('/'))
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
        let v: serde_json::Value = serde_json::from_str(&raster_provider("https://t/{z}/{x}/{y}.png", 512)).unwrap();
        assert_eq!(v["sources"]["raster-source"]["tileSize"], 512);
        let wms = wms_style("https://w", "a b", "", "1.3.0", "image/png", true, Some("X=1"));
        assert!(wms.contains("LAYERS=a%20b") && wms.contains("FORMAT=image%2Fpng") && wms.contains("TRANSPARENT=TRUE&X=1"));
        assert!(wms.contains("&CRS=EPSG:3857&BBOX={bbox-epsg-3857}&"));
        assert!(wms_style("https://w", "a", "", "1.1.1", "image/png", false, None).contains("&SRS=EPSG:3857&"));
        let wmts = wmts_style("https://w", "lyr", "GoogleMapsCompatible", "image/png", "default", "1.0.0", None);
        assert!(wmts.contains("LAYER=lyr") && wmts.contains("TILEMATRIX={z}&TILEROW={y}&TILECOL={x}"));
        assert_eq!(esri_vector_provider("https://e/VectorTileServer/"), "https://e/VectorTileServer/resources/styles/root.json");
    }
}
