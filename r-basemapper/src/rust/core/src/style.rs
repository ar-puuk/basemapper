use crate::error::BasemapError;
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum StyleInput {
    Url(String),
    InlineJson(String),
}

impl StyleInput {
    pub fn detect(s: &str) -> Self {
        if s.trim_start().starts_with('{') {
            StyleInput::InlineJson(s.to_owned())
        } else {
            StyleInput::Url(s.to_owned())
        }
    }

    /// The style string, as [`crate::net::Auth::for_input`] scopes a token to.
    pub(crate) fn as_input(&self) -> &str {
        match self {
            StyleInput::Url(u) | StyleInput::InlineJson(u) => u,
        }
    }

    /// The style JSON: inline JSON as is; otherwise a style URL or file is
    /// loaded, or a style is built for tile data (see [`crate::tile_style()`]).
    pub(crate) async fn resolve(
        &self,
        client: &reqwest::Client,
        auth: &crate::net::Auth,
    ) -> Result<String, BasemapError> {
        match self {
            StyleInput::InlineJson(json) => Ok(json.clone()),
            StyleInput::Url(url) => crate::tile_style::resolve_style_input(url, client, auth).await,
        }
    }
}

// ── Styles as values (docs/design/api-review.md §4.3) ──────────────────────

/// Where a style's map comes from. A plain string is auto-detected (a
/// catalogue name, a style URL or JSON, a tile URL, a PMTiles archive, an
/// ArcGIS service); [`Source::provider`], [`Source::tiles`], [`Source::wms`]
/// and [`Source::wmts`] take each kind of source's own options.
#[derive(Debug, Clone)]
pub enum Source {
    /// Detected from the string.
    Auto(String),
    /// A catalogue basemap in a chosen form, optionally drawing other tile
    /// data (of the same schema) with its style.
    Provider { name: String, form: Option<crate::TileKind>, tiles: Option<String> },
    /// Tile data, with a built style.
    Tiles { input: String, options: crate::TileStyleOptions },
    /// A WMS service.
    Wms { url: String, layers: String, styles: String, version: String, format: String, transparent: bool, extra: Option<String> },
    /// A WMTS service.
    Wmts { url: String, layer: String, tilematrixset: String, format: String, style: String, version: String, extra: Option<String> },
}

impl From<&str> for Source {
    fn from(s: &str) -> Self {
        Source::Auto(s.to_owned())
    }
}

impl From<String> for Source {
    fn from(s: String) -> Self {
        Source::Auto(s)
    }
}

impl Source {
    /// A catalogue basemap: `form` picks its vector or raster form (default:
    /// vector when it has one); `tiles` draws your own tile data (the same
    /// schema) with its style.
    pub fn provider(name: impl Into<String>, form: Option<crate::TileKind>, tiles: Option<String>) -> Source {
        Source::Provider { name: name.into(), form, tiles }
    }

    /// Tile data (a tile URL, TileJSON, PMTiles archive or ArcGIS service)
    /// with a built style.
    pub fn tiles(input: impl Into<String>, options: crate::TileStyleOptions) -> Source {
        Source::Tiles { input: input.into(), options }
    }

    /// A WMS service's `layers`, with the usual defaults (no styles, version
    /// 1.3.0, PNG, opaque); set the fields to change them.
    pub fn wms(url: impl Into<String>, layers: impl Into<String>) -> Source {
        Source::Wms {
            url: url.into(),
            layers: layers.into(),
            styles: String::new(),
            version: "1.3.0".into(),
            format: "image/png".into(),
            transparent: false,
            extra: None,
        }
    }

    /// A WMTS service's `layer`, with the usual defaults
    /// (GoogleMapsCompatible, PNG, the default style, version 1.0.0).
    pub fn wmts(url: impl Into<String>, layer: impl Into<String>) -> Source {
        Source::Wmts {
            url: url.into(),
            layer: layer.into(),
            tilematrixset: "GoogleMapsCompatible".into(),
            format: "image/png".into(),
            style: "default".into(),
            version: "1.0.0".into(),
            extra: None,
        }
    }
}

/// A style: a source, plus what every style can have: filters and a
/// credential. Each style of a stack is filtered on its own; a catalogue
/// stack (`"UGRC.Lite"`) is one style, its services filtered as one map.
#[derive(Debug, Clone)]
pub struct Style {
    pub source: Source,
    /// Layer ids or patterns to keep, or (`-`-prefixed) to drop.
    pub layers: Option<Vec<String>>,
    /// Feature classes to keep or (`-`-prefixed) drop. `"labels"` keeps only
    /// labels and `"-labels"` drops them; they combine with the others:
    /// `["roads", "labels"]` keeps the road labels.
    pub features: Option<Vec<String>>,
    /// The credential: put into the URL for catalogue basemaps, sent as a
    /// Bearer token (to the style's own origin) otherwise.
    pub key: Option<String>,
}

impl From<&str> for Style {
    fn from(s: &str) -> Self {
        Style::new(s)
    }
}

impl From<String> for Style {
    fn from(s: String) -> Self {
        Style::new(s)
    }
}

impl From<Source> for Style {
    fn from(source: Source) -> Self {
        Style::new(source)
    }
}

impl Style {
    /// A style for `source` (a string is auto-detected).
    pub fn new(source: impl Into<Source>) -> Style {
        Style { source: source.into(), layers: None, features: None, key: None }
    }

    pub fn layers(mut self, layers: Vec<String>) -> Self {
        self.layers = Some(layers);
        self
    }

    pub fn features(mut self, features: Vec<String>) -> Self {
        self.features = Some(features);
        self
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Check the filters without fetching anything.
    pub fn validate(&self) -> Result<(), BasemapError> {
        let (_, features) = split_label_filter(self.features.as_deref().unwrap_or(&[]))?;
        crate::features::validate_features(&features)
    }

    /// The style's parts, bottom to top (several for a catalogue stack), each
    /// a style input with this style's filters and credential. Building a
    /// style for tile data may fetch a tile or an archive header.
    pub fn parts(&self) -> Result<Vec<StylePart>, BasemapError> {
        let (labels, features) = split_label_filter(self.features.as_deref().unwrap_or(&[]))?;
        crate::features::validate_features(&features)?;
        let features = (!features.is_empty()).then_some(features);
        let part = |input: StyleInput, auth_token: Option<String>| StylePart {
            input,
            layers: self.layers.clone(),
            labels,
            features: features.clone(),
            auth_token,
            token_scope: None,
        };
        let provider = |name: &str, form: Option<crate::TileKind>, tiles: Option<String>| {
            // A catalogue basemap applies its key itself, and its labels by
            // switching tiles; its parts get the feature filter as layers do.
            let opts = crate::providers::ProviderOptions { kind: form, api_key: self.key.clone(), tiles, ..Default::default() };
            crate::providers::basemap_provider_styles(name, &opts)
                .map(|styles| styles.into_iter().map(|s| part(StyleInput::detect(&s), None)).collect::<Vec<_>>())
        };
        match &self.source {
            Source::Auto(s) if crate::providers::find_provider(s).is_some() => provider(s, None, None),
            Source::Auto(s) => Ok(vec![part(StyleInput::detect(s), self.key.clone())]),
            Source::Provider { name, form, tiles } => provider(name, *form, tiles.clone()),
            Source::Tiles { input, options } => {
                let (json, _unmatched_paint) = crate::tile_style(input, options, self.key.as_deref())?;
                // The style is built inline; its token goes to its tile data's origin.
                let mut p = part(StyleInput::InlineJson(json), self.key.clone());
                p.token_scope = Some(input.clone());
                Ok(vec![p])
            }
            Source::Wms { url, layers, styles, version, format, transparent, extra } => Ok(vec![part(
                StyleInput::detect(&crate::providers::wms_style(url, layers, styles, version, format, *transparent, extra.as_deref())),
                self.key.clone(),
            )]),
            Source::Wmts { url, layer, tilematrixset, format, style, version, extra } => Ok(vec![part(
                StyleInput::detect(&crate::providers::wmts_style(url, layer, tilematrixset, format, style, version, extra.as_deref())),
                self.key.clone(),
            )]),
        }
    }
}

/// One drawn style of a [`Style`]: what a render needs of it.
#[derive(Debug, Clone)]
pub struct StylePart {
    pub input: StyleInput,
    pub layers: Option<Vec<String>>,
    pub labels: crate::Labels,
    pub features: Option<Vec<String>>,
    pub auth_token: Option<String>,
    /// The input a token is scoped to when it is not `input` (a built style's
    /// tile data).
    pub token_scope: Option<String>,
}

/// `features` split into the label filter (`"labels"` keeps only labels,
/// `"-labels"` drops them) and the feature classes.
pub fn split_label_filter(features: &[String]) -> Result<(crate::Labels, Vec<String>), BasemapError> {
    let mut labels = crate::Labels::All;
    let mut rest = Vec::new();
    for f in features {
        let next = match f.as_str() {
            "labels" => crate::Labels::Only,
            "-labels" => crate::Labels::None,
            _ => {
                rest.push(f.clone());
                continue;
            }
        };
        if labels != crate::Labels::All && labels != next {
            return Err(BasemapError::InvalidLayerFilter("features has both \"labels\" and \"-labels\"".into()));
        }
        labels = next;
    }
    Ok((labels, rest))
}

/// Validate a resolved MapLibre GL style JSON string.
pub fn validate_maplibre_style(json: &str) -> Result<(), BasemapError> {
    let v: Value =
        serde_json::from_str(json).map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
    let version = v.get("version").and_then(Value::as_u64);
    if version != Some(8) {
        return Err(BasemapError::StyleParseError(
            "MapLibre GL style must have \"version\": 8".into(),
        ));
    }
    if v.get("sources").is_none() {
        return Err(BasemapError::StyleParseError(
            "MapLibre GL style missing required \"sources\" field".into(),
        ));
    }
    let layers = v.get("layers").and_then(Value::as_array).ok_or_else(|| {
        BasemapError::StyleParseError("MapLibre GL style missing \"layers\" array".into())
    })?;
    for (i, layer) in layers.iter().enumerate() {
        if layer.get("id").is_none() {
            return Err(BasemapError::StyleParseError(format!(
                "layer[{i}]: missing required field 'id'"
            )));
        }
        if layer.get("type").is_none() {
            return Err(BasemapError::StyleParseError(format!(
                "layer[{i}]: missing required field 'type'"
            )));
        }
    }
    Ok(())
}

/// Whether layer id `id` matches `pattern`: `*` matches any run of
/// characters, `?` any one; anything else matches itself.
pub fn layer_matches(pattern: &str, id: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), id.chars().collect());
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// Filter the style JSON's "layers" array: plain ids or patterns (`*`, `?`)
/// keep the layers they match, `-`-prefixed ones drop them. Returns the
/// style and the patterns that matched no layer. Sources, sprite and glyphs
/// are kept as they are.
pub fn filter_layers(style_json: &str, layers: &[String]) -> Result<(String, Vec<String>), BasemapError> {
    if layers.is_empty() {
        return Ok((style_json.to_owned(), Vec::new()));
    }
    let mut style: Value = serde_json::from_str(style_json).map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
    let all_negative = layers.iter().all(|l| l.starts_with('-'));
    let patterns: Vec<&str> = layers.iter().map(|l| l.strip_prefix('-').unwrap_or(l.as_str())).collect();
    let mut used = vec![false; patterns.len()];
    if let Some(arr) = style.get_mut("layers").and_then(Value::as_array_mut) {
        arr.retain(|layer| {
            let id = layer.get("id").and_then(Value::as_str).unwrap_or("");
            let mut hit = false;
            for (i, p) in patterns.iter().enumerate() {
                if layer_matches(p, id) {
                    used[i] = true;
                    hit = true;
                }
            }
            hit != all_negative
        });
    }
    let unmatched = layers.iter().zip(&used).filter(|(_, u)| !**u).map(|(l, _)| l.clone()).collect();
    let json = serde_json::to_string(&style).map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
    Ok((json, unmatched))
}

/// As [`filter_layers`], without the unmatched patterns.
pub fn filter_style_layers(style_json: &str, layers: &[String]) -> Result<String, BasemapError> {
    filter_layers(style_json, layers).map(|(s, _)| s)
}

#[cfg(test)]
mod pattern_tests {
    use super::*;

    #[test]
    fn wildcards_match_layer_ids() {
        assert!(layer_matches("label_*", "label_city"));
        assert!(layer_matches("*_casing", "highway_motorway_casing"));
        assert!(layer_matches("Road/*", "Road/Major/1"));
        assert!(layer_matches("road_?", "road_1"));
        assert!(layer_matches("water", "water"));
        assert!(!layer_matches("water", "waterway"));
        assert!(!layer_matches("label_*", "road_label"));
        assert!(layer_matches("*", ""));
    }

    #[test]
    fn filter_reports_unmatched_patterns() {
        let s = r#"{"version":8,"sources":{},"layers":[{"id":"label_city","type":"symbol"},{"id":"water","type":"fill"}]}"#;
        let strs = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let (out, unmatched) = filter_layers(s, &strs(&["-label_*", "-roads*"])).unwrap();
        assert!(!out.contains("label_city") && out.contains("water"));
        assert_eq!(unmatched, ["-roads*"]);
        let (out, unmatched) = filter_layers(s, &strs(&["water", "lab?l_*"])).unwrap();
        assert!(out.contains("label_city") && out.contains("water") && unmatched.is_empty());
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn labels_fold_into_features() {
        use crate::Labels;
        let split = |f: &[&str]| split_label_filter(&strs(f));
        assert_eq!(split(&[]).unwrap(), (Labels::All, vec![]));
        assert_eq!(split(&["roads", "labels"]).unwrap(), (Labels::Only, strs(&["roads"])));
        assert_eq!(split(&["-labels", "-water"]).unwrap(), (Labels::None, strs(&["-water"])));
        assert_eq!(split(&["labels", "labels"]).unwrap().0, Labels::Only);
        assert!(split(&["labels", "-labels"]).unwrap_err().to_string().contains("both"));
        assert!(Style::new("x").features(strs(&["no-such-feature"])).validate().is_err());
        assert!(Style::new("x").features(strs(&["roads", "-labels"])).validate().is_ok());
    }

    #[test]
    fn each_source_kind_resolves_to_parts() {
        use crate::Labels;
        // A URL: one part, the key sent as a token, the filters carried.
        let p = Style::new("https://example.com/style.json").key("k").features(strs(&["roads", "labels"])).parts().unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].input.as_input(), "https://example.com/style.json");
        assert_eq!((p[0].auth_token.as_deref(), p[0].labels), (Some("k"), Labels::Only));
        assert_eq!(p[0].features, Some(strs(&["roads"])));
        // A catalogue name: its style URL, the key used by the catalogue (not
        // sent as a token).
        let p = Style::new("CartoDB.Positron").key("k").parts().unwrap();
        assert_eq!(p.len(), 1);
        assert!(p[0].input.as_input().starts_with("https://") && p[0].auth_token.is_none(), "{:?}", p[0]);
        // The same through provider(), in its raster form.
        let r = Style::new(Source::provider("CartoDB.Positron", Some(crate::TileKind::Raster), None)).key("k").parts().unwrap();
        assert_ne!(r[0].input.as_input(), p[0].input.as_input());
        // A catalogue stack: one style, several parts, one filter for all.
        let p = Style::new("UGRC.Lite").features(strs(&["-labels"])).parts().unwrap();
        assert!(p.len() > 1 && p.iter().all(|q| q.labels == Labels::None));
        // WMS / WMTS: one generated style each, the key a token.
        let p = Style::new(Source::wms("https://example.com/wms", "roads")).key("k").parts().unwrap();
        assert!(p[0].input.as_input().contains("LAYERS=roads") && p[0].auth_token.as_deref() == Some("k"), "{:?}", p[0]);
        let p = Style::new(Source::wmts("https://example.com/wmts", "base")).parts().unwrap();
        assert!(p[0].input.as_input().contains("LAYER=base"));
        // A bad filter fails before anything is fetched.
        assert!(Style::new("CartoDB.Positron").features(strs(&["labels", "-labels"])).parts().is_err());
    }
}
