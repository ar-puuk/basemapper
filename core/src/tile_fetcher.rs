use crate::bbox::{mercator_to_tile, SpatialBounds};
use crate::error::BasemapError;
use std::time::Duration;
use tokio::task::JoinSet;

#[derive(Debug, Clone)]
pub enum TileSource {
    XyzRaster {
        url_template: String,
        auth_header: Option<String>,
    },
    MapboxVectorTile {
        url_template: String,
        api_key: Option<String>,
    },
    EsriVectorTile {
        url_template: String,
        auth_header: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

pub type TileData = Vec<u8>;

/// Expand `{z}`, `{x}`, `{y}` placeholders in a URL template.
pub fn expand_url(template: &str, coord: TileCoord) -> String {
    template
        .replace("{z}", &coord.z.to_string())
        .replace("{x}", &coord.x.to_string())
        .replace("{y}", &coord.y.to_string())
}

/// Compute all tile coordinates that cover a given EPSG:3857 bbox at zoom.
pub fn build_tile_coords(bbox: [f64; 4], zoom: u8) -> Vec<TileCoord> {
    let (x0, y0) = mercator_to_tile(bbox[0], bbox[3], zoom);
    let (x1, y1) = mercator_to_tile(bbox[2], bbox[1], zoom);
    let mut coords = Vec::new();
    for tx in x0..=x1 {
        for ty in y0..=y1 {
            coords.push(TileCoord { z: zoom, x: tx, y: ty });
        }
    }
    coords
}

/// Build the URL list for a given tile source and set of coordinates.
pub fn build_tile_urls(source: &TileSource, coords: &[TileCoord]) -> Vec<(TileCoord, String)> {
    coords
        .iter()
        .map(|&coord| {
            let url = match source {
                TileSource::XyzRaster { url_template, .. } => expand_url(url_template, coord),
                TileSource::MapboxVectorTile { url_template, api_key } => {
                    let base = expand_url(url_template, coord);
                    if let Some(key) = api_key {
                        format!("{base}?access_token={key}")
                    } else {
                        base
                    }
                }
                TileSource::EsriVectorTile { url_template, .. } => expand_url(url_template, coord),
            };
            (coord, url)
        })
        .collect()
}

/// Fetch all tiles concurrently, respecting timeout and concurrency limits.
pub async fn fetch_all_tiles(
    client: &reqwest::Client,
    urls: Vec<(TileCoord, String)>,
    source: &TileSource,
    timeout_ms: u32,
    concurrency: u32,
) -> Result<Vec<(TileCoord, TileData)>, BasemapError> {
    let timeout = Duration::from_millis(timeout_ms as u64);
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency as usize));
    let mut set: JoinSet<Result<(TileCoord, TileData), BasemapError>> = JoinSet::new();

    for (coord, url) in urls {
        let client = client.clone();
        let sem = semaphore.clone();
        let url = url.clone();
        let auth = match source {
            TileSource::XyzRaster { auth_header, .. } => auth_header.clone(),
            TileSource::EsriVectorTile { auth_header, .. } => auth_header.clone(),
            TileSource::MapboxVectorTile { .. } => None,
        };

        set.spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let mut req = client.get(&url).timeout(timeout);
            if let Some(header) = auth {
                req = req.header("Authorization", header);
            }
            let resp = req.send().await.map_err(|_| BasemapError::TileFetchFailed {
                url: url.clone(),
                status: 0,
            })?;
            if !resp.status().is_success() {
                return Err(BasemapError::TileFetchFailed {
                    url: url.clone(),
                    status: resp.status().as_u16(),
                });
            }
            let bytes = resp.bytes().await.map_err(|_| BasemapError::TileFetchFailed {
                url: url.clone(),
                status: 0,
            })?;
            Ok((coord, bytes.to_vec()))
        });
    }

    let mut results = Vec::new();
    while let Some(res) = set.join_next().await {
        results.push(res.map_err(|e| BasemapError::RenderError(e.to_string()))??);
    }
    Ok(results)
}
