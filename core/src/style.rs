use crate::error::BasemapError;
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum StyleInput {
    Url(String),
    InlineJson(String),
}

impl StyleInput {
    pub fn from_str(s: &str) -> Self {
        if s.trim_start().starts_with('{') {
            StyleInput::InlineJson(s.to_owned())
        } else {
            StyleInput::Url(s.to_owned())
        }
    }

    /// Fetch (if URL) or parse (if inline JSON) and return the raw JSON string.
    pub async fn resolve(&self, client: &reqwest::Client) -> Result<String, BasemapError> {
        match self {
            StyleInput::InlineJson(json) => Ok(json.clone()),
            StyleInput::Url(url) => {
                let resp = client.get(url).send().await.map_err(|e| {
                    BasemapError::StyleFetchFailed { url: url.clone(), status: 0 }
                })?;
                if !resp.status().is_success() {
                    return Err(BasemapError::StyleFetchFailed {
                        url: url.clone(),
                        status: resp.status().as_u16(),
                    });
                }
                resp.text().await.map_err(|_| BasemapError::StyleParseError(
                    "could not read style response body".into(),
                ))
            }
        }
    }
}

/// Validate a resolved MapLibre GL style JSON string.
pub fn validate_maplibre_style(json: &str) -> Result<(), BasemapError> {
    let v: Value = serde_json::from_str(json)
        .map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
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

/// Filter the style JSON's "layers" array.
///
/// H3 SAFETY: Only the "layers" array is modified. Sources, sprite, and glyphs
/// are preserved exactly as-is to avoid maplibre-rs parse-time crashes.
pub fn filter_style_layers(style_json: &str, layers: &[String]) -> Result<String, BasemapError> {
    if layers.is_empty() {
        return Ok(style_json.to_owned());
    }
    let mut style: Value = serde_json::from_str(style_json)
        .map_err(|e| BasemapError::StyleParseError(e.to_string()))?;

    let all_negative = layers.iter().all(|l| l.starts_with('-'));
    let filter_ids: Vec<&str> = layers
        .iter()
        .map(|l| if l.starts_with('-') { &l[1..] } else { l.as_str() })
        .collect();

    if let Some(arr) = style.get_mut("layers").and_then(Value::as_array_mut) {
        *arr = arr
            .iter()
            .filter(|layer| {
                let id = layer.get("id").and_then(Value::as_str).unwrap_or("");
                if all_negative {
                    !filter_ids.contains(&id)
                } else {
                    filter_ids.contains(&id)
                }
            })
            .cloned()
            .collect();
    }

    serde_json::to_string(&style).map_err(|e| BasemapError::StyleParseError(e.to_string()))
}
