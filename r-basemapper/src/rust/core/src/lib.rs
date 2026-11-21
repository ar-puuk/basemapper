pub mod bbox;
pub mod error;
pub mod renderer;
pub mod style;
pub mod tile_fetcher;
pub mod vector;

pub use bbox::{compute_zoom, RenderRequest, RenderResult, SpatialBounds};
pub use error::BasemapError;
pub use style::{filter_style_layers, StyleInput};
pub use tile_fetcher::TileSource;

use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

// Thread-local tokio runtime shared across all render calls from binding crates.
static RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("failed to create basemapper tokio runtime"));

/// Render a basemap tile mosaic to a flat RGBA byte array.
///
/// Supports anti-meridian crossing bboxes (xmin > xmax in EPSG:3857).
/// Such bboxes are split internally into west and east halves, each rendered
/// at proportional pixel width, then stitched horizontally.
pub fn render(request: RenderRequest) -> Result<RenderResult, BasemapError> {
    request.validate()?;
    RUNTIME.block_on(render_async(request))
}

// ──────────────────────────────────────────────────────────────────────────────
// Async rendering pipeline
// ──────────────────────────────────────────────────────────────────────────────

async fn render_async(request: RenderRequest) -> Result<RenderResult, BasemapError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(
            request.tile_timeout_ms as u64,
        ))
        .use_rustls_tls()
        .user_agent(concat!("basemapper/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| BasemapError::RenderError(e.to_string()))?;

    // Resolve style once — shared across both halves for anti-meridian renders.
    let raw_json = request
        .style_input
        .resolve(&client, request.auth_token.as_deref())
        .await?;
    style::validate_maplibre_style(&raw_json)?;

    let style_json = if let Some(ref layers) = request.layers {
        filter_style_layers(&raw_json, layers)?
    } else {
        raw_json.clone()
    };
    let transparent = request.layers.is_some();

    // Extract tile source (may fetch TileJSON).
    let style_val: serde_json::Value = serde_json::from_str(&style_json)
        .map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
    let style_base_url = match &request.style_input {
        StyleInput::Url(u) => Some(u.as_str()),
        StyleInput::InlineJson(_) => None,
    };
    let tile_source = extract_tile_source_from_style(
        &style_val,
        &client,
        request.auth_token.as_deref(),
        style_base_url,
    )
    .await?;

    // Compute zoom for the full bbox — handles anti-meridian via compute_zoom.
    let zoom = {
        let raw = request.zoom.unwrap_or_else(|| {
            let mut z = bbox::compute_zoom(request.bbox, request.width);
            while z > 0 && bbox::tile_count(request.bbox, z) > request.max_tiles {
                z -= 1;
            }
            z
        });
        tile_source.maxzoom().map_or(raw, |mz| raw.min(mz))
    };

    // Guard: tile_count handles anti-meridian correctly now.
    let total_tiles = bbox::tile_count(request.bbox, zoom);
    if total_tiles > request.max_tiles {
        return Err(BasemapError::MaxTilesExceeded {
            requested: total_tiles,
            limit: request.max_tiles,
        });
    }

    let [xmin, _ymin, xmax, _ymax] = request.bbox;
    if xmin > xmax {
        render_antimeridian_split(&request, &client, &style_json, &tile_source, transparent, zoom)
            .await
    } else {
        render_single_bbox(
            &request,
            &client,
            &style_json,
            &tile_source,
            transparent,
            request.bbox,
            request.width,
            request.height,
            zoom,
        )
        .await
    }
}

/// Render one non-crossing bbox at the given zoom level and pixel dimensions.
///
/// Called both for normal bboxes and for each half of an anti-meridian split.
async fn render_single_bbox(
    request: &RenderRequest,
    client: &reqwest::Client,
    style_json: &str,
    tile_source: &TileSource,
    transparent: bool,
    bbox: [f64; 4],
    width: u32,
    height: u32,
    zoom: u8,
) -> Result<RenderResult, BasemapError> {
    let bounds = SpatialBounds {
        xmin: bbox[0],
        ymin: bbox[1],
        xmax: bbox[2],
        ymax: bbox[3],
        crs_epsg: 3857,
    };

    // Vector tiles: in-house CPU renderer (default) or the legacy maplibre-rs
    // engine (BASEMAPPER_VECTOR_ENGINE=maplibre) while it is being retired.
    let legacy_engine = std::env::var("BASEMAPPER_VECTOR_ENGINE").as_deref() == Ok("maplibre");
    if matches!(tile_source, TileSource::MapboxVectorTile { .. }) && !legacy_engine {
        // MapLibre convention: style evaluated at the true scale zoom; vector
        // tiles (512 px) fetched at floor(scale zoom), clamped to the source
        // maxzoom. An explicit request zoom is on the 256 px raster grid, which
        // is one level above the equivalent 512 px tile zoom.
        let sz = vector::render::scale_zoom(bbox, width);
        let tz = match request.zoom {
            Some(z) => z.saturating_sub(1),
            None => vector::render::tile_zoom(sz, None),
        };
        let tz = tile_source.maxzoom().map_or(tz, |m| tz.min(m));
        let coords = tile_fetcher::build_tile_coords(bbox, tz);
        let urls = tile_fetcher::build_tile_urls(tile_source, &coords);
        let tiles = tile_fetcher::fetch_all_tiles(
            client,
            urls,
            tile_source,
            request.tile_timeout_ms,
            request.tile_concurrency,
            request.fail_on_tile_error,
        )
        .await?;
        let style_val: serde_json::Value = serde_json::from_str(style_json)
            .map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
        let glyphs = fetch_glyphs(client, &style_val, style_base_url_of(request), &tiles, sz).await;
        let out = vector::render::render(&style_val, &tiles, &glyphs, bbox, width, height)
            .map_err(BasemapError::RenderError)?;
        if !out.skipped_layer_types.is_empty() {
            log::warn!(
                "vector renderer does not draw these layer types yet: {}",
                out.skipped_layer_types.join(", ")
            );
        }
        return Ok(RenderResult {
            bounds,
            zoom_used: tz.saturating_add(1),
            width,
            height,
            pixels: out.pixels,
        });
    }
    if let TileSource::MapboxVectorTile { ref url_template, .. } = tile_source {
        let pixels = renderer::render_vector_tiles(
            client,
            style_json,
            url_template,
            request.auth_token.as_deref(),
            bbox,
            zoom,
            width,
            height,
            request.tile_timeout_ms,
            request.tile_concurrency,
            request.fail_on_tile_error,
            transparent,
        )
        .await?;
        return Ok(RenderResult { bounds, zoom_used: zoom, width, height, pixels });
    }

    // PMTiles: fetch via HTTP range requests.
    if let TileSource::PMTiles { ref url, .. } = tile_source {
        let coords = tile_fetcher::build_tile_coords(bbox, zoom);
        let tiles = fetch_pmtiles_tiles(url, &coords, client).await?;
        let canvas =
            renderer::composite_raster_tiles(&tiles, bbox, zoom, width, height, transparent)?;
        return Ok(RenderResult {
            bounds,
            zoom_used: zoom,
            width,
            height,
            pixels: canvas.into_raw(),
        });
    }

    // Raster XYZ (also handles WMS via bbox placeholder expansion).
    let coords = tile_fetcher::build_tile_coords(bbox, zoom);
    let urls = tile_fetcher::build_tile_urls(tile_source, &coords);
    let tiles = tile_fetcher::fetch_all_tiles(
        client,
        urls,
        tile_source,
        request.tile_timeout_ms,
        request.tile_concurrency,
        request.fail_on_tile_error,
    )
    .await?;
    let canvas =
        renderer::composite_raster_tiles(&tiles, bbox, zoom, width, height, transparent)?;
    Ok(RenderResult {
        bounds,
        zoom_used: zoom,
        width,
        height,
        pixels: canvas.into_raw(),
    })
}

/// Split an anti-meridian bbox into west and east halves, render each, then
/// stitch the pixel arrays horizontally (west first).
async fn render_antimeridian_split(
    request: &RenderRequest,
    client: &reqwest::Client,
    style_json: &str,
    tile_source: &TileSource,
    transparent: bool,
    zoom: u8,
) -> Result<RenderResult, BasemapError> {
    const EARTH_HALF: f64 = 20_037_508.342_789_244;
    let [xmin, ymin, xmax, ymax] = request.bbox;

    // West half: from xmin → +EARTH_HALF (right edge of the tile grid).
    // East half: from −EARTH_HALF (left edge) → xmax.
    let west_bbox = [xmin, ymin, EARTH_HALF, ymax];
    let east_bbox = [-EARTH_HALF, ymin, xmax, ymax];

    let west_m = EARTH_HALF - xmin;
    let east_m = xmax + EARTH_HALF;
    let total_m = west_m + east_m;

    // Proportional pixel widths; guarantee at least 1 px per half.
    let west_px = ((request.width as f64 * west_m / total_m).round() as u32).max(1);
    let east_px = request.width.saturating_sub(west_px).max(1);

    let west = render_single_bbox(
        request, client, style_json, tile_source, transparent,
        west_bbox, west_px, request.height, zoom,
    )
    .await?;
    let east = render_single_bbox(
        request, client, style_json, tile_source, transparent,
        east_bbox, east_px, request.height, zoom,
    )
    .await?;

    // Stitch row by row: west pixels then east pixels per scanline.
    let row_w = west.width as usize * 4;
    let row_e = east.width as usize * 4;
    let row_out = row_w + row_e;
    let mut pixels = vec![0u8; row_out * request.height as usize];
    for row in 0..request.height as usize {
        let out = &mut pixels[row * row_out..][..row_out];
        out[..row_w].copy_from_slice(&west.pixels[row * row_w..][..row_w]);
        out[row_w..].copy_from_slice(&east.pixels[row * row_e..][..row_e]);
    }

    Ok(RenderResult {
        bounds: SpatialBounds { xmin, ymin, xmax, ymax, crs_epsg: 3857 },
        zoom_used: zoom,
        width: west.width + east.width,
        height: request.height,
        pixels,
    })
}

// ──────────────────────────────────────────────────────────────────────────────
// Style and tile source helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Extract a `TileSource` from the first raster or vector source in the style.
///
/// Skips non-tile source types (geojson, image, video).  If the source has a
/// "url" field (TileJSON reference) instead of an inline "tiles" array, the
/// TileJSON endpoint is fetched to obtain the tile URL template.
async fn extract_tile_source_from_style(
    style: &serde_json::Value,
    client: &reqwest::Client,
    auth_token: Option<&str>,
    style_base_url: Option<&str>,
) -> Result<TileSource, BasemapError> {
    let sources = style
        .get("sources")
        .and_then(|s| s.as_object())
        .ok_or_else(|| BasemapError::StyleParseError("missing \"sources\" object".into()))?;

    // Prefer vector sources over raster; prevents multi-source styles from
    // routing through the raster pipeline when a vector source is present.
    let vector_entry = sources.values().find_map(|s| {
        if s.get("type").and_then(|t| t.as_str()) == Some("vector") {
            Some(("vector", s))
        } else {
            None
        }
    });
    let (src_type, source) = if let Some(entry) = vector_entry {
        entry
    } else {
        sources
            .values()
            .find_map(|s| {
                let t = s.get("type").and_then(|t| t.as_str()).unwrap_or("raster");
                if matches!(t, "geojson" | "image" | "video") {
                    None
                } else {
                    Some((t, s))
                }
            })
            .ok_or_else(|| {
                BasemapError::StyleParseError("no raster or vector tile source found".into())
            })?
    };

    // Resolve tile URL template and maxzoom.
    let (tiles, source_maxzoom): (String, Option<u8>) = if let Some(t) = source
        .get("tiles")
        .and_then(|t| t.as_array())
        .and_then(|a| a.first())
        .and_then(|u| u.as_str())
    {
        let mz = source
            .get("maxzoom")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(22) as u8);
        (t.to_owned(), mz)
    } else if let Some(tilejson_url_raw) = source.get("url").and_then(|u| u.as_str()) {
        let tilejson_url = resolve_url(tilejson_url_raw, style_base_url);
        let mut tilejson_req = client.get(&tilejson_url);
        if let Some(token) = auth_token {
            tilejson_req = tilejson_req.header("Authorization", format!("Bearer {token}"));
        }
        let resp = tilejson_req
            .send()
            .await
            .map_err(|_| BasemapError::StyleFetchFailed {
                url: tilejson_url.clone(),
                status: 0,
            })?;
        if !resp.status().is_success() {
            return Err(BasemapError::StyleFetchFailed {
                url: tilejson_url.clone(),
                status: resp.status().as_u16(),
            });
        }
        let tilejson: serde_json::Value = resp.json().await.map_err(|_| {
            BasemapError::StyleParseError("could not parse TileJSON response".into())
        })?;
        let tile_url_raw = tilejson
            .get("tiles")
            .and_then(|t| t.as_array())
            .and_then(|a| a.first())
            .and_then(|u| u.as_str())
            .ok_or_else(|| {
                BasemapError::StyleParseError("TileJSON missing \"tiles\" array".into())
            })?;
        let mz = tilejson
            .get("maxzoom")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(22) as u8);
        (resolve_url(tile_url_raw, Some(&tilejson_url)), mz)
    } else {
        return Err(BasemapError::StyleParseError(
            "source has neither \"tiles\" array nor \"url\" TileJSON reference".into(),
        ));
    };

    if tiles.starts_with("pmtiles://") {
        return Ok(TileSource::PMTiles {
            url: tiles.trim_start_matches("pmtiles://").to_owned(),
            maxzoom: source_maxzoom,
        });
    }

    match src_type {
        "vector" => Ok(TileSource::MapboxVectorTile {
            url_template: tiles,
            api_key: auth_token.map(str::to_owned),
            maxzoom: source_maxzoom,
        }),
        _ => Ok(TileSource::XyzRaster {
            url_template: tiles,
            auth_header: auth_token.map(|t| format!("Bearer {t}")),
            maxzoom: source_maxzoom,
        }),
    }
}

/// Fetch tiles from a PMTiles archive over HTTP using range requests.
///
/// Opens the archive (header + root directory via two range requests), then
/// fetches each requested tile.  Tiles absent from the archive (sparse
/// coverage) are silently skipped.
async fn fetch_pmtiles_tiles(
    url: &str,
    coords: &[tile_fetcher::TileCoord],
    client: &reqwest::Client,
) -> Result<Vec<(tile_fetcher::TileCoord, tile_fetcher::TileData)>, BasemapError> {
    use pmtiles::AsyncPmTilesReader;
    use tile_fetcher::ReqwestBackend;

    let backend = ReqwestBackend::new(url.to_owned(), client.clone());
    let reader = AsyncPmTilesReader::try_from_source(backend)
        .await
        .map_err(|e| BasemapError::RenderError(format!("PMTiles open failed: {e}")))?;

    let mut results = Vec::new();
    for &coord in coords {
        let pm_coord = pmtiles::TileCoord::new(coord.z, coord.x, coord.y)
            .map_err(|e| BasemapError::RenderError(format!("PMTiles coord error: {e}")))?;
        match reader.get_tile(pm_coord).await {
            Ok(Some(bytes)) => results.push((coord, bytes.to_vec())),
            Ok(None) => {}
            Err(e) => {
                log::warn!(
                    "PMTiles tile {}/{}/{} fetch error (skipped): {e}",
                    coord.z,
                    coord.x,
                    coord.y,
                );
            }
        }
    }
    Ok(results)
}

/// Resolve a potentially-relative URL against a base URL.
///
/// `reqwest::Url::join` percent-encodes `{` and `}` in tile template
/// placeholders, so we decode them back after joining.
fn resolve_url(url: &str, base: Option<&str>) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_owned();
    }
    if let Some(base_str) = base {
        if let Ok(base_parsed) = reqwest::Url::parse(base_str) {
            if let Ok(resolved) = base_parsed.join(url) {
                return resolved
                    .to_string()
                    .replace("%7B", "{")
                    .replace("%7D", "}")
                    .replace("%7b", "{")
                    .replace("%7d", "}");
            }
        }
    }
    url.to_owned()
}

fn style_base_url_of(request: &RenderRequest) -> Option<&str> {
    match &request.style_input {
        StyleInput::Url(u) => Some(u.as_str()),
        StyleInput::InlineJson(_) => None,
    }
}

/// Fetch the glyph PBF ranges the visible text labels need, from the style's
/// `glyphs` URL template. Failures are logged and skipped (labels whose glyphs
/// are missing are not drawn), as MapLibre does.
async fn fetch_glyphs(
    client: &reqwest::Client,
    style: &serde_json::Value,
    style_base_url: Option<&str>,
    tiles: &[(tile_fetcher::TileCoord, tile_fetcher::TileData)],
    zoom: f64,
) -> vector::symbol::GlyphMap {
    let mut map = vector::symbol::GlyphMap::default();
    let Some(template) = style["glyphs"].as_str() else { return map };
    let template = resolve_url(template, style_base_url);
    let decoded: Vec<vector::mvt::Tile> = tiles.iter().filter_map(|(_, b)| vector::mvt::decode(b).ok()).collect();
    let mut needed: Vec<(String, u32)> =
        vector::symbol::required_glyph_ranges(style, decoded.iter(), zoom).into_iter().collect();
    needed.sort();
    let mut set = tokio::task::JoinSet::new();
    for (stack, start) in needed {
        let url = template
            .replace("{fontstack}", &stack)
            .replace("{range}", &format!("{}-{}", start, start + 255));
        let client = client.clone();
        set.spawn(async move {
            let resp = client.get(&url).send().await.ok()?;
            if !resp.status().is_success() {
                log::warn!("glyphs {url}: HTTP {}", resp.status());
                return None;
            }
            Some((stack, resp.bytes().await.ok()?.to_vec()))
        });
    }
    let mut results = Vec::new();
    while let Some(r) = set.join_next().await {
        if let Ok(Some(x)) = r {
            results.push(x);
        }
    }
    results.sort_by(|a, b| a.0.cmp(&b.0));
    for (stack, bytes) in results {
        if let Err(e) = map.add_pbf(&stack, &bytes) {
            log::warn!("glyphs for {stack}: {e}");
        }
    }
    map
}

