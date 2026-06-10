pub mod bbox;
pub mod error;
pub mod renderer;
pub mod style;
pub mod tile_fetcher;

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
/// This is a synchronous function. It drives async work (tile fetching, style
/// resolution) on a dedicated thread-local tokio Runtime so binding crates
/// can call it from a blocking context.
pub fn render(request: RenderRequest) -> Result<RenderResult, BasemapError> {
    request.validate()?;

    RUNTIME.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(
                request.tile_timeout_ms as u64,
            ))
            .use_rustls_tls()
            .build()
            .map_err(|e| BasemapError::RenderError(e.to_string()))?;

        // Resolve and validate the style JSON.
        let raw_json = request.style_input.resolve(&client).await?;
        style::validate_maplibre_style(&raw_json)?;

        // Apply layer filter if requested (H3: touches only "layers" array).
        let style_json = if let Some(ref layers) = request.layers {
            filter_style_layers(&raw_json, layers)?
        } else {
            raw_json.clone()
        };

        let transparent = request.layers.is_some();

        // Determine effective zoom.
        let zoom = request.zoom.unwrap_or_else(|| {
            let mut z = bbox::compute_zoom(request.bbox, request.width);
            // Reduce zoom until tile count fits within max_tiles.
            while z > 0 && bbox::tile_count(request.bbox, z) > request.max_tiles {
                z -= 1;
            }
            z
        });

        let tile_count = bbox::tile_count(request.bbox, zoom);
        if tile_count > request.max_tiles {
            return Err(BasemapError::MaxTilesExceeded {
                requested: tile_count,
                limit: request.max_tiles,
            });
        }

        // Fetch tiles.
        let style_val: serde_json::Value = serde_json::from_str(&style_json)
            .map_err(|e| BasemapError::StyleParseError(e.to_string()))?;

        // Extract TileSource from the first raster/vector source in the style.
        let tile_source = extract_tile_source_from_style(&style_val, &client).await?;
        let coords = tile_fetcher::build_tile_coords(request.bbox, zoom);
        let urls = tile_fetcher::build_tile_urls(&tile_source, &coords);

        let tiles = tile_fetcher::fetch_all_tiles(
            &client,
            urls,
            &tile_source,
            request.tile_timeout_ms,
            request.tile_concurrency,
        )
        .await?;

        // Composite raster tiles (vector tile rendering via maplibre-rs is
        // handled here for XyzRaster; MVT/ESRI paths require maplibre-rs).
        let canvas = renderer::composite_raster_tiles(
            &tiles,
            request.bbox,
            zoom,
            request.width,
            request.height,
            transparent,
        )?;

        let pixels = canvas.into_raw();

        Ok(RenderResult {
            bounds: SpatialBounds {
                xmin: request.bbox[0],
                ymin: request.bbox[1],
                xmax: request.bbox[2],
                ymax: request.bbox[3],
                crs_epsg: 3857,
            },
            zoom_used: zoom,
            width: request.width,
            height: request.height,
            pixels,
        })
    })
}

/// Extract a `TileSource` from the first raster or vector source in the style.
///
/// Skips non-tile source types (geojson, image, video).  If the source has a
/// "url" field (TileJSON reference) instead of an inline "tiles" array, the
/// TileJSON endpoint is fetched to obtain the tile URL template.
async fn extract_tile_source_from_style(
    style: &serde_json::Value,
    client: &reqwest::Client,
) -> Result<TileSource, BasemapError> {
    let sources = style
        .get("sources")
        .and_then(|s| s.as_object())
        .ok_or_else(|| BasemapError::StyleParseError("missing \"sources\" object".into()))?;

    let (src_type, source) = sources
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
        })?;

    // Prefer inline "tiles" array; fall back to resolving a "url" TileJSON endpoint.
    let tiles = if let Some(t) = source
        .get("tiles")
        .and_then(|t| t.as_array())
        .and_then(|a| a.first())
        .and_then(|u| u.as_str())
    {
        t.to_owned()
    } else if let Some(tilejson_url) = source.get("url").and_then(|u| u.as_str()) {
        let resp = client
            .get(tilejson_url)
            .send()
            .await
            .map_err(|_| BasemapError::StyleFetchFailed {
                url: tilejson_url.to_owned(),
                status: 0,
            })?;
        if !resp.status().is_success() {
            return Err(BasemapError::StyleFetchFailed {
                url: tilejson_url.to_owned(),
                status: resp.status().as_u16(),
            });
        }
        let tilejson: serde_json::Value = resp.json().await.map_err(|_| {
            BasemapError::StyleParseError("could not parse TileJSON response".into())
        })?;
        tilejson
            .get("tiles")
            .and_then(|t| t.as_array())
            .and_then(|a| a.first())
            .and_then(|u| u.as_str())
            .ok_or_else(|| {
                BasemapError::StyleParseError("TileJSON missing \"tiles\" array".into())
            })?
            .to_owned()
    } else {
        return Err(BasemapError::StyleParseError(
            "source has neither \"tiles\" array nor \"url\" TileJSON reference".into(),
        ));
    };

    match src_type {
        "vector" => Ok(TileSource::MapboxVectorTile {
            url_template: tiles,
            api_key: None,
        }),
        _ => Ok(TileSource::XyzRaster {
            url_template: tiles,
            auth_header: None,
        }),
    }
}
