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

    /// The style JSON: inline JSON as is; otherwise a style URL or file is
    /// loaded, or a style is built for tile data (see [`crate::tile_style()`]).
    pub async fn resolve(
        &self,
        client: &reqwest::Client,
        auth_token: Option<&str>,
    ) -> Result<String, BasemapError> {
        match self {
            StyleInput::InlineJson(json) => Ok(json.clone()),
            StyleInput::Url(url) => crate::tile_style::resolve_style_input(url, client, auth_token).await,
        }
    }
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
}
