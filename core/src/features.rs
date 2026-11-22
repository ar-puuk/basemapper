//! Labels and map features: select parts of a style to stack basemaps.
//!
//! Every style layer is either a label (a `symbol` layer: text and icons)
//! or not, and belongs to one *feature*: water, roads, buildings, ... (see
//! [`FEATURES`]). `labels` keeps all layers, drops the labels or keeps only
//! them; `features` keeps or drops features. The two combine:
//! `features = ["roads"]` with labels only is the road names.
//!
//! A layer's feature comes from its vector tile `source-layer`, through
//! tables for the common schemas (OpenMapTiles: OpenFreeMap, Stadia,
//! MapTiler; Protomaps: CARTO; Shortbread: VersaTiles), then keywords that
//! also cover Esri's descriptive layer names (`"Water area"`, `"Road"`),
//! then keywords in the layer id. Raster tiles have their labels drawn in;
//! catalogue basemaps with label-free or label-only tiles switch to those
//! (see [`crate::providers`]).

use crate::BasemapError;
use serde_json::Value;

/// The features layers are sorted into, with what each covers.
pub const FEATURES: &[(&str, &str)] = &[
    ("background", "the background colour and the land underneath everything"),
    ("water", "oceans, lakes, rivers and their names"),
    ("landcover", "land cover and land use: forest, grass, farmland, residential, industrial"),
    ("parks", "parks and protected areas"),
    ("buildings", "buildings and house numbers"),
    ("roads", "roads, paths, bridges, tunnels, ferries and runways, with their names and shields"),
    ("rail", "railways and transit lines"),
    ("boundaries", "country and region boundaries"),
    ("places", "place names: countries, regions, cities, towns, neighbourhoods"),
    ("poi", "points of interest: shops, schools, airports, landmarks"),
    ("terrain", "hillshade, relief, contours and elevations"),
    ("other", "layers that fit none of the above"),
];

/// Which labels to keep.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Labels {
    /// Every layer (the default).
    #[default]
    All,
    /// No labels: the map without text and icons.
    None,
    /// Only the labels, on a transparent background, to stack over another map.
    Only,
}

impl Labels {
    /// `"all"` (or `"true"`), `"none"` (or `"false"`), `"only"`.
    pub fn parse(s: &str) -> Result<Labels, BasemapError> {
        match s.to_ascii_lowercase().as_str() {
            "all" | "true" => Ok(Labels::All),
            "none" | "false" => Ok(Labels::None),
            "only" => Ok(Labels::Only),
            _ => Err(BasemapError::InvalidInput(format!("labels must be TRUE, FALSE or \"only\", not {s:?}"))),
        }
    }
}

fn exact(source_layer: &str) -> Option<&'static str> {
    Some(match source_layer {
        // OpenMapTiles, Protomaps, Shortbread
        "water" | "waterway" | "water_name" | "ocean" | "water_polygons" | "water_lines" | "water_polygons_labels"
        | "water_lines_labels" | "dam_lines" | "dam_polygons" => "water",
        "landcover" | "landuse" | "land" | "sites" | "globallandcover" => "landcover",
        "earth" => "background",
        "park" => "parks",
        "building" | "buildings" | "housenumber" | "addresses" => "buildings",
        "transportation" | "transportation_name" | "aeroway" | "roads" | "streets" | "street_polygons"
        | "streets_polygons_labels" | "street_labels" | "street_labels_points" | "bridges" | "pier_lines"
        | "pier_polygons" | "ferries" | "aerialways" => "roads",
        "transit" | "public_transport" => "rail",
        "boundary" | "boundaries" | "boundary_labels" => "boundaries",
        "place" | "places" | "place_labels" => "places",
        "poi" | "pois" | "aerodrome_label" => "poi",
        "mountain_peak" | "contour" | "contours" | "contour_line" | "hillshade" => "terrain",
        _ => return None,
    })
}

/// Keywords in a source-layer name or layer id (lowercase), in order.
fn keywords(s: &str) -> Option<&'static str> {
    if s.is_empty() {
        return None;
    }
    let has = |words: &[&str]| words.iter().any(|w| s.contains(w));
    if has(&["contour", "hillshade", "relief", "elevation", "landform", "peak", "terrain"]) {
        return Some("terrain");
    }
    if has(&["point of interest", "poi", "landmark"]) || s.ends_with(" place") {
        return Some("poi");
    }
    if s == "land" || has(&["earth", "background"]) {
        return Some("background");
    }
    if has(&["rail", "transit", "subway", "tram", "metro"]) {
        return Some("rail");
    }
    if has(&["boundary", "border", "disputed"]) {
        return Some("boundaries");
    }
    if has(&["admin"]) {
        return Some(if has(&["park", "forest"]) { "parks" } else { "places" });
    }
    if has(&["water and wastewater"]) {
        return Some("landcover");
    }
    if has(&["water", "marine", "ocean", "river", "lake", "stream", "canal", "bathymetry", "coastline"]) {
        return Some("water");
    }
    if has(&["building", "parcel", "address", "housenumber"]) {
        return Some("buildings");
    }
    if has(&[
        "road", "street", "highway", "motorway", "trail", "path", "pedestrian", "ferry", "bridge", "tunnel",
        "transportation", "airport", "aeroway", "runway", "exit", "freight", "pavement", "shield",
    ]) {
        return Some("roads");
    }
    if has(&["city", "town", "village", "hamlet", "neighborhood", "neighbourhood", "suburb", "continent", "country", "place"]) {
        return Some("places");
    }
    if has(&["park", "golf", "cemetery", "zoo", "indigenous", "protected", "reserve", "garden"]) && !has(&["parking"]) {
        return Some("parks");
    }
    if has(&[
        "landcover", "landuse", "forest", "wood", "vegetation", "grass", "tree", "shrub", "beach", "sand", "urban",
        "residential", "industr", "farm", "glacier", "ice", "wetland", "retail", "commercial", "education", "medical",
        "government", "military", "emergency", "finance", "port", "special area", "openspace", "parking",
    ]) {
        return Some("landcover");
    }
    None
}

/// The feature a style layer belongs to (one of [`FEATURES`]).
pub fn layer_feature(layer: &Value) -> &'static str {
    let id = layer["id"].as_str().unwrap_or("").to_ascii_lowercase();
    match layer["type"].as_str().unwrap_or("") {
        "background" => return "background",
        "hillshade" | "color-relief" => return "terrain",
        "raster" => {
            let source = layer["source"].as_str().unwrap_or("").to_ascii_lowercase();
            let terrain = ["shade", "relief", "hill", "terrain", "dem"].iter().any(|k| id.contains(k) || source.contains(k));
            return if terrain { "terrain" } else { "other" };
        }
        _ => {}
    }
    let original = layer["source-layer"].as_str().unwrap_or("");
    if original == "Land" {
        return "background"; // Esri's land base (Shortbread's "land" is land use)
    }
    let sl = original.to_ascii_lowercase();
    let sl = sl.strip_suffix("/label").unwrap_or(&sl);
    let feature = exact(sl).or_else(|| keywords(sl)).or_else(|| keywords(&id)).unwrap_or("other");
    // OpenMapTiles and Protomaps keep rail with the roads, and parks in land use.
    let id_has = |words: &[&str]| words.iter().any(|w| id.contains(w));
    match feature {
        "roads" if id_has(&["rail", "transit", "subway", "tram"]) => "rail",
        "landcover" if id_has(&["park", "nature_reserve", "protected"]) && !id_has(&["parking"]) => "parks",
        f => f,
    }
}

/// Whether a style layer is a label (text or icons).
pub fn is_label(layer: &Value) -> bool {
    layer["type"].as_str() == Some("symbol")
}

/// A feature name as given (`"Roads"`, `"building"`) to its [`FEATURES`] name.
fn feature_name(name: &str) -> Result<&'static str, BasemapError> {
    let n = name.trim().to_ascii_lowercase();
    let n = n.as_str();
    FEATURES
        .iter()
        .map(|(f, _)| *f)
        .find(|f| *f == n || f.strip_suffix('s') == Some(n) || (*f == "boundaries" && n == "boundary") || (*f == "poi" && n == "pois"))
        .ok_or_else(|| {
            let all: Vec<&str> = FEATURES.iter().map(|(f, _)| *f).collect();
            BasemapError::InvalidInput(format!("unknown feature {name:?}; features are {}", all.join(", ")))
        })
}

/// A `features` filter: the feature names, and whether they are kept (plain
/// names) or dropped (`-`-prefixed).
fn parse_features(features: &[String]) -> Result<(Vec<&'static str>, bool), BasemapError> {
    let negative = features.iter().filter(|f| f.starts_with('-')).count();
    if negative != 0 && negative != features.len() {
        return Err(BasemapError::InvalidLayerFilter(
            "features mixes kept and dropped (-) features; use one mode only".into(),
        ));
    }
    let names = features.iter().map(|f| feature_name(f.trim_start_matches('-'))).collect::<Result<_, _>>()?;
    Ok((names, negative == 0))
}

/// Check a `features` filter without applying it.
pub fn validate_features(features: &[String]) -> Result<(), BasemapError> {
    parse_features(features).map(|_| ())
}

/// `style_json` with only the layers `labels` and `features` select. A
/// catalogue raster basemap switches to its label-free or label-only tiles
/// instead. An error when nothing would be left to draw.
pub fn select_layers(style_json: &str, labels: Labels, features: &[String]) -> Result<String, BasemapError> {
    if labels == Labels::All && features.is_empty() {
        return Ok(style_json.to_owned());
    }
    let (names, keep) = parse_features(features)?;
    let mut style: Value = serde_json::from_str(style_json).map_err(|e| BasemapError::StyleParseError(e.to_string()))?;

    if let Some(variants) = style.pointer("/metadata/basemapper:raster").cloned() {
        let name = variants["provider"].as_str().unwrap_or("this basemap");
        if !features.is_empty() {
            return Err(BasemapError::InvalidInput(format!(
                "features select parts of vector styles; {name} is raster tiles (use its vector form or another                  basemap, or filter one style of a stack with basemap_provider(..., features =))"
            )));
        }
        let tiles = match labels {
            Labels::All => return Ok(style_json.to_owned()),
            Labels::None if variants["has_labels"] == false => return Ok(style_json.to_owned()),
            Labels::None => variants.get("nolabels").ok_or_else(|| {
                BasemapError::InvalidInput(format!(
                    "{name} raster tiles have their labels drawn in, with no label-free version; use its vector form or another basemap"
                ))
            })?,
            Labels::Only => variants.get("labels").ok_or_else(|| {
                BasemapError::InvalidInput(format!("{name} has no label-only raster tiles"))
            })?,
        };
        style["sources"]["raster"]["tiles"] = tiles.clone();
        if let Some(m) = style["metadata"].as_object_mut() {
            m.remove("basemapper:raster");
            if labels == Labels::Only {
                // Labels to stack over another map: no background fill.
                m.insert("basemapper:transparent".into(), Value::Bool(true));
            }
        }
        return serde_json::to_string(&style).map_err(|e| BasemapError::StyleParseError(e.to_string()));
    }

    let Some(layers) = style.get_mut("layers").and_then(Value::as_array_mut) else {
        return Ok(style_json.to_owned());
    };
    let mut present: Vec<&'static str> = Vec::new();
    for l in layers.iter() {
        let f = layer_feature(l);
        if !present.contains(&f) {
            present.push(f);
        }
    }
    layers.retain(|l| {
        let label_ok = match labels {
            Labels::All => true,
            Labels::None => !is_label(l),
            Labels::Only => is_label(l),
        };
        label_ok && (names.is_empty() || names.contains(&layer_feature(l)) == keep)
    });
    if layers.is_empty() {
        return Err(BasemapError::InvalidLayerFilter(format!(
            "labels and features leave nothing of this style to draw; its features are {}",
            present.join(", ")
        )));
    }
    serde_json::to_string(&style).map_err(|e| BasemapError::StyleParseError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn f(ty: &str, id: &str, sl: &str) -> &'static str {
        layer_feature(&json!({"id": id, "type": ty, "source-layer": sl}))
    }

    #[test]
    fn schemas_map_to_features() {
        // OpenMapTiles (OpenFreeMap, Stadia, MapTiler)
        assert_eq!(f("fill", "water", "water"), "water");
        assert_eq!(f("line", "road_minor", "transportation"), "roads");
        assert_eq!(f("line", "railway_transit", "transportation"), "rail");
        assert_eq!(f("symbol", "highway-name-major", "transportation_name"), "roads");
        assert_eq!(f("fill", "landuse_residential", "landuse"), "landcover");
        assert_eq!(f("fill", "park", "park"), "parks");
        assert_eq!(f("symbol", "label_city", "place"), "places");
        assert_eq!(f("symbol", "airport", "aerodrome_label"), "poi");
        // Protomaps (CARTO)
        assert_eq!(f("fill", "earth", "earth"), "background");
        assert_eq!(f("fill", "landuse_park", "landuse"), "parks");
        assert_eq!(f("line", "roads_rail", "roads"), "rail");
        assert_eq!(f("symbol", "places_locality", "places"), "places");
        // Shortbread (VersaTiles)
        assert_eq!(f("fill", "water-ocean", "ocean"), "water");
        assert_eq!(f("line", "street-primary", "streets"), "roads");
        assert_eq!(f("symbol", "label-place-city", "place_labels"), "places");
        // Esri
        assert_eq!(f("fill", "Water area/1", "Water area large scale"), "water");
        assert_eq!(f("line", "Road/Major/1", "Road"), "roads");
        assert_eq!(f("line", "Railroad/1", "Railroad"), "rail");
        assert_eq!(f("fill", "Building/1", "Building"), "buildings");
        assert_eq!(f("fill", "Admin0 forest or park/1", "Admin0 forest or park"), "parks");
        assert_eq!(f("symbol", "City large scale/x", "City large scale"), "places");
        assert_eq!(f("symbol", "Transportation place/x", "Transportation place"), "poi");
        assert_eq!(f("line", "Boundary line/Admin0", "Boundary line"), "boundaries");
        assert_eq!(f("fill", "Land/1", "Land"), "background");
        assert_eq!(f("fill", "Industry/1", "Industry"), "landcover");
        assert_eq!(f("symbol", "Water area/label/x", "Water area/label"), "water");
        assert_eq!(f("fill", "Water and wastewater/1", "Water and wastewater"), "landcover");
        // Layer types
        assert_eq!(layer_feature(&json!({"id": "bg", "type": "background"})), "background");
        assert_eq!(layer_feature(&json!({"id": "natural_earth", "type": "raster", "source": "ne2_shaded"})), "terrain");
        assert_eq!(layer_feature(&json!({"id": "raster", "type": "raster", "source": "raster"})), "other");
        assert_eq!(f("fill", "zcta-fill", "zcta"), "other");
    }

    fn style() -> String {
        json!({"version": 8, "sources": {}, "layers": [
            {"id": "bg", "type": "background"},
            {"id": "water", "type": "fill", "source": "s", "source-layer": "water"},
            {"id": "road", "type": "line", "source": "s", "source-layer": "transportation"},
            {"id": "road-name", "type": "symbol", "source": "s", "source-layer": "transportation_name"},
            {"id": "city", "type": "symbol", "source": "s", "source-layer": "place"},
        ]})
        .to_string()
    }

    fn ids(s: &str) -> Vec<String> {
        let v: Value = serde_json::from_str(s).unwrap();
        v["layers"].as_array().unwrap().iter().map(|l| l["id"].as_str().unwrap().to_owned()).collect()
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn labels_and_features_combine() {
        let s = style();
        assert_eq!(select_layers(&s, Labels::All, &[]).unwrap(), s);
        assert_eq!(ids(&select_layers(&s, Labels::None, &[]).unwrap()), ["bg", "water", "road"]);
        assert_eq!(ids(&select_layers(&s, Labels::Only, &[]).unwrap()), ["road-name", "city"]);
        assert_eq!(ids(&select_layers(&s, Labels::All, &strs(&["roads"])).unwrap()), ["road", "road-name"]);
        assert_eq!(ids(&select_layers(&s, Labels::Only, &strs(&["Road"])).unwrap()), ["road-name"]);
        assert_eq!(ids(&select_layers(&s, Labels::None, &strs(&["-water", "-background"])).unwrap()), ["road"]);
        assert!(select_layers(&s, Labels::All, &strs(&["roads", "-water"])).is_err());
        let e = select_layers(&s, Labels::All, &strs(&["highways"])).unwrap_err().to_string();
        assert!(e.contains("features are background, water"), "{e}");
        let e = select_layers(&s, Labels::Only, &strs(&["water"])).unwrap_err().to_string();
        assert!(e.contains("its features are background, water, roads, places"), "{e}");
    }

    #[test]
    fn raster_variants_switch_tiles() {
        let s = json!({"version": 8, "metadata": {"basemapper:raster": {
                "provider": "X.Y", "has_labels": true, "nolabels": ["n"], "labels": ["l"]}},
            "sources": {"raster": {"type": "raster", "tiles": ["a"]}},
            "layers": [{"id": "raster", "type": "raster", "source": "raster"}]})
        .to_string();
        let tiles = |s: &str| serde_json::from_str::<Value>(s).unwrap()["sources"]["raster"]["tiles"].clone();
        assert_eq!(tiles(&select_layers(&s, Labels::None, &[]).unwrap()), json!(["n"]));
        let only: Value = serde_json::from_str(&select_layers(&s, Labels::Only, &[]).unwrap()).unwrap();
        assert_eq!(only["sources"]["raster"]["tiles"], json!(["l"]));
        assert_eq!(only["metadata"]["basemapper:transparent"], true, "label tiles stack without a background");
        assert!(select_layers(&s, Labels::All, &strs(&["water"])).unwrap_err().to_string().contains("raster tiles"));
        let baked = s.replace(r#""nolabels":["n"],"#, "");
        assert!(select_layers(&baked, Labels::None, &[]).unwrap_err().to_string().contains("drawn in"));
    }
}
