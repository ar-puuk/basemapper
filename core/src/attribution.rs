//! The attribution text styles ask maps to show.
//!
//! As MapLibre's attribution control: every source a style's layers draw
//! from contributes its `attribution`, read from the style, else from the
//! source's TileJSON (Esri's service JSON names it `copyrightText`) or
//! PMTiles metadata. HTML is reduced to plain text, duplicates are dropped
//! and the rest is joined with `" | "`.

use crate::style::StyleInput;
use crate::BasemapError;
use serde_json::Value;

/// Plain text from an attribution's HTML: tags dropped, entities decoded,
/// whitespace collapsed.
pub fn plain_text(html: &str) -> String {
    // Inline tags (<a>, <span>) vanish; line and block breaks become spaces.
    let mut text = String::with_capacity(html.len());
    let mut tag: Option<String> = None;
    for c in html.chars() {
        match (&mut tag, c) {
            (None, '<') => tag = Some(String::new()),
            (Some(t), '>') => {
                let name: String =
                    t.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                if matches!(name.to_ascii_lowercase().as_str(), "br" | "p" | "div" | "li" | "tr") {
                    text.push(' ');
                }
                tag = None;
            }
            (Some(t), c) => t.push(c),
            (None, c) => text.push(c),
        }
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest.find(';').filter(|&e| e <= 10);
        let decoded = end.and_then(|e| {
            let name = &rest[1..e];
            let c = match name {
                "copy" => Some('©'),
                "reg" => Some('®'),
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                _ => name
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A source's attribution: its own, else its TileJSON's or archive's.
async fn source_attribution(
    source: &Value,
    base: Option<&str>,
    client: &reqwest::Client,
    auth_token: Option<&str>,
) -> Option<String> {
    if let Some(a) = source["attribution"].as_str() {
        return Some(a.to_owned());
    }
    let url = source["url"].as_str()?;
    if let Some(archive) = url.strip_prefix("pmtiles://") {
        let reader = crate::open_pmtiles(&crate::resolve_url(archive, base), client).await.ok()?;
        let meta: Value = serde_json::from_str(&reader.get_metadata().await.ok()?).ok()?;
        return meta["attribution"].as_str().map(str::to_owned);
    }
    let url = crate::resolve_url(url, base);
    let mut req = client.get(&url);
    if let Some(token) = auth_token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let tilejson: Value = req.send().await.ok()?.error_for_status().ok()?.json().await.ok()?;
    tilejson["attribution"].as_str().or_else(|| tilejson["copyrightText"].as_str()).map(str::to_owned)
}

/// The attribution of `styles` (stacked styles, as [`crate::render`]
/// takes them), as plain text joined with `" | "`; empty when none is given.
pub(crate) async fn attribution(
    styles: &[String],
    client: &reqwest::Client,
    auth_token: Option<&str>,
) -> Result<String, BasemapError> {
    let mut parts: Vec<String> = Vec::new();
    for s in styles {
        let input = StyleInput::detect(s);
        let json = input.resolve(client, auth_token).await?;
        let base = match &input {
            StyleInput::Url(u) if u.starts_with("http") || u.starts_with("file://") => Some(u.as_str()),
            _ => None,
        };
        let style: Value = serde_json::from_str(&json).map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
        let mut used: Vec<&str> = Vec::new();
        for layer in style["layers"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if let Some(id) = layer["source"].as_str() {
                if !used.contains(&id) {
                    used.push(id);
                }
            }
        }
        for id in used {
            if let Some(a) = source_attribution(&style["sources"][id], base, client, auth_token).await {
                let a = plain_text(&a);
                // A credit another one already includes adds nothing.
                if !a.is_empty() && !parts.iter().any(|p| p.contains(&a)) {
                    parts.retain(|p| !a.contains(p.as_str()));
                    parts.push(a);
                }
            }
        }
    }
    Ok(parts.join(" | "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_becomes_plain_text() {
        let ofm = r#"<a href="https://openfreemap.org" target="_blank">OpenFreeMap</a> <a href="https://www.openmaptiles.org/" target="_blank">&copy; OpenMapTiles</a> Data from <a href="https://www.openstreetmap.org/copyright" target="_blank">OpenStreetMap</a>"#;
        assert_eq!(plain_text(ofm), "OpenFreeMap © OpenMapTiles Data from OpenStreetMap");
        assert_eq!(plain_text("&copy;&#169;&#xA9; A &amp; B &unknown; R&D"), "©©© A & B &unknown; R&D");
        assert_eq!(plain_text("  Tiles \n © Esri "), "Tiles © Esri");
        assert_eq!(
            plain_text("ESA WorldCover 2021 (<a href='x'>CC BY 4.0</a>)<br/>Next"),
            "ESA WorldCover 2021 (CC BY 4.0) Next"
        );
    }
}
