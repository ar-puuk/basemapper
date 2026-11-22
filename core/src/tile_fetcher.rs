use crate::bbox::mercator_to_tile;
use crate::error::BasemapError;
use lru::LruCache;
use once_cell::sync::Lazy;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::Duration;
use tokio::task::JoinSet;

// In-memory LRU cache shared across all render calls in a session.
// Keyed by fully-expanded tile URL; stores raw response bytes.
static TILE_CACHE: Lazy<Mutex<LruCache<String, Vec<u8>>>> =
    Lazy::new(|| Mutex::new(LruCache::new(NonZeroUsize::new(512).unwrap())));

/// A source's tile URL templates (MapLibre's `tiles`) and `scheme`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TileUrls {
    /// Templates; tiles are spread over them as MapLibre does.
    pub templates: Vec<String>,
    /// `scheme: "tms"`: tile rows count from the south.
    pub tms: bool,
}

impl TileUrls {
    /// One `scheme: "xyz"` template.
    pub fn xyz(template: impl Into<String>) -> Self {
        TileUrls { templates: vec![template.into()], tms: false }
    }

    /// The URL of a tile, as MapLibre's `CanonicalTileID.url()`.
    pub fn url(&self, coord: TileCoord, pixel_ratio: f64) -> String {
        tile_url(&self.templates, coord, pixel_ratio, self.tms)
    }
}

#[derive(Debug, Clone)]
pub enum TileSource {
    XyzRaster {
        urls: TileUrls,
        auth_header: Option<String>,
        /// Server-advertised maximum zoom from the source object or TileJSON.
        maxzoom: Option<u8>,
    },
    MapboxVectorTile {
        urls: TileUrls,
        api_key: Option<String>,
        /// Server-advertised maximum zoom from the source object or TileJSON.
        maxzoom: Option<u8>,
    },
    PMTiles {
        url: String,
        maxzoom: Option<u8>,
    },
}

impl TileSource {
    pub fn maxzoom(&self) -> Option<u8> {
        match self {
            TileSource::XyzRaster { maxzoom, .. } => *maxzoom,
            TileSource::MapboxVectorTile { maxzoom, .. } => *maxzoom,
            TileSource::PMTiles { maxzoom, .. } => *maxzoom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

pub type TileData = Vec<u8>;

/// Where a PMTiles archive is read from: HTTP range requests (with our
/// reqwest 0.12 client, avoiding the reqwest 0.13 pulled in by pmtiles'
/// `http-async` feature), or a local file (`file://` URL or a path).
pub struct ArchiveBackend {
    location: ArchiveLocation,
}

enum ArchiveLocation {
    Http { url: String, client: reqwest::Client },
    File(std::path::PathBuf),
}

impl ArchiveBackend {
    /// `url`: `http(s)://...`, `file://...` or a file path.
    pub fn new(url: String, client: reqwest::Client) -> Self {
        let location = if url.starts_with("http://") || url.starts_with("https://") {
            ArchiveLocation::Http { url, client }
        } else {
            ArchiveLocation::File(local_path(&url))
        };
        Self { location }
    }
}

/// The file a `file://` URL or a path names.
pub fn local_path(url: &str) -> std::path::PathBuf {
    if url.starts_with("file://") {
        if let Ok(p) = reqwest::Url::parse(url).map(|u| u.to_file_path()) {
            if let Ok(p) = p {
                return p;
            }
        }
    }
    std::path::PathBuf::from(url)
}

fn read_error(msg: String) -> pmtiles::PmtError {
    pmtiles::PmtError::Reading(std::io::Error::other(msg))
}

impl pmtiles::AsyncBackend for ArchiveBackend {
    fn read(
        &self,
        offset: usize,
        length: usize,
    ) -> impl std::future::Future<Output = pmtiles::PmtResult<pmtiles::BackendResponse>> + Send {
        let location = match &self.location {
            ArchiveLocation::Http { url, client } => Ok((url.clone(), client.clone())),
            ArchiveLocation::File(path) => Err(path.clone()),
        };
        async move {
            match location {
                Ok((url, client)) => {
                    let end = offset + length - 1;
                    // Ranges of the file itself: a server free to compress
                    // (GitHub Pages) would otherwise return a range of the
                    // gzipped file, which is not the archive's bytes.
                    let resp = client
                        .get(&url)
                        .header("Range", format!("bytes={offset}-{end}"))
                        .header("Accept-Encoding", "identity")
                        .send()
                        .await
                        .map_err(|e| read_error(e.to_string()))?;
                    if !resp.status().is_success() {
                        return Err(read_error(format!("HTTP {} fetching PMTiles range from {url}", resp.status())));
                    }
                    let bytes = resp.bytes().await.map_err(|e| read_error(e.to_string()))?;
                    Ok(pmtiles::BackendResponse::new(bytes))
                }
                Err(path) => {
                    use std::io::{Read, Seek, SeekFrom};
                    let mut f = std::fs::File::open(&path)
                        .map_err(|e| read_error(format!("{}: {e}", path.display())))?;
                    f.seek(SeekFrom::Start(offset as u64)).map_err(|e| read_error(e.to_string()))?;
                    // A range past the end (the header read asks for more
                    // than a small archive holds) returns what there is.
                    let mut buf = Vec::with_capacity(length);
                    f.take(length as u64).read_to_end(&mut buf).map_err(|e| read_error(e.to_string()))?;
                    Ok(pmtiles::BackendResponse::new(bytes::Bytes::from(buf)))
                }
            }
        }
    }
}

/// The URL of a tile from a source's templates, as MapLibre's
/// `CanonicalTileID.url()`: the template is picked by `(x + y) % n` (spreading
/// tiles over subdomains), then `{prefix}`, `{z}`, `{x}`, `{y}` (flipped for
/// `scheme: "tms"`), `{ratio}` (`@2x` above pixel ratio 1), `{quadkey}` and
/// `{bbox-epsg-3857}` (for WMS) are replaced. WMTS's `{TileMatrix}`,
/// `{TileCol}` and `{TileRow}` work too (an extension: MapLibre has none).
pub fn tile_url(templates: &[String], coord: TileCoord, pixel_ratio: f64, tms: bool) -> String {
    let Some(first) = templates.first() else { return String::new() };
    let template = templates
        .get(((coord.x as u64 + coord.y as u64) % templates.len() as u64) as usize)
        .unwrap_or(first);
    let n = 1u64 << coord.z;
    let y = if tms { n - 1 - coord.y as u64 } else { coord.y as u64 };
    let mut s = template
        .replace("{prefix}", &format!("{:x}{:x}", coord.x % 16, coord.y % 16))
        .replace("{z}", &coord.z.to_string())
        .replace("{x}", &coord.x.to_string())
        .replace("{y}", &y.to_string())
        .replace("{ratio}", if pixel_ratio > 1.0 { "@2x" } else { "" });
    // WMTS ResourceURL templates (GoogleMapsCompatible matrix sets), so a
    // template copied from GetCapabilities works as is.
    if s.contains("{Tile") {
        s = s
            .replace("{TileMatrix}", &coord.z.to_string())
            .replace("{TileCol}", &coord.x.to_string())
            .replace("{TileRow}", &y.to_string());
    }
    if s.contains("{quadkey}") {
        s = s.replace("{quadkey}", &quadkey(coord));
    }
    if s.contains("{bbox-epsg-3857}") {
        s = s.replace("{bbox-epsg-3857}", &tile_bbox(coord));
    }
    s
}

/// [`tile_url`] for one `scheme: "xyz"` template at pixel ratio 1.
pub fn expand_url(template: &str, coord: TileCoord) -> String {
    tile_url(&[template.to_owned()], coord, 1.0, false)
}

/// Bing-style quadkey of a tile (MapLibre's `getQuadkey`).
fn quadkey(c: TileCoord) -> String {
    (1..=c.z)
        .rev()
        .map(|i| {
            let mask = 1u32 << (i - 1);
            char::from(b'0' + u8::from(c.x & mask != 0) + 2 * u8::from(c.y & mask != 0))
        })
        .collect()
}

/// `xmin,ymin,xmax,ymax` of a tile in EPSG:3857 metres, computed and printed
/// exactly as MapLibre's `{bbox-epsg-3857}` (whoots-js `getTileBBox`, numbers
/// as JavaScript prints them), so WMS requests match MapLibre's byte for byte.
fn tile_bbox(c: TileCoord) -> String {
    use std::f64::consts::PI;
    let y = 2f64.powi(c.z as i32) - c.y as f64 - 1.0;
    let merc = |px: f64, py: f64| {
        let resolution = (2.0 * PI * 6378137.0 / 256.0) / 2f64.powi(c.z as i32);
        let half = 2.0 * PI * 6378137.0 / 2.0;
        (px * resolution - half, py * resolution - half)
    };
    let x = c.x as f64;
    let (x0, y0) = merc(x * 256.0, y * 256.0);
    let (x1, y1) = merc((x + 1.0) * 256.0, (y + 1.0) * 256.0);
    format!("{},{},{},{}", js_number(x0), js_number(y0), js_number(x1), js_number(y1))
}

/// A number as JavaScript's `String(n)` prints it (shortest round trip;
/// exponent form below 1e-6). Values here stay well under 1e21.
fn js_number(v: f64) -> String {
    if v == 0.0 {
        "0".to_owned()
    } else if v.abs() < 1e-6 {
        format!("{v:e}")
    } else {
        format!("{v}")
    }
}

/// Compute all tile coordinates that cover a given EPSG:3857 bbox at zoom.
pub fn build_tile_coords(bbox: [f64; 4], zoom: u8) -> Vec<TileCoord> {
    let (x0, y0) = mercator_to_tile(bbox[0], bbox[3], zoom);
    let (x1, y1) = mercator_to_tile(bbox[2], bbox[1], zoom);
    let mut coords = Vec::new();
    for tx in x0..=x1 {
        for ty in y0..=y1 {
            coords.push(TileCoord {
                z: zoom,
                x: tx,
                y: ty,
            });
        }
    }
    coords
}

/// Build the URL list for a given tile source and set of coordinates;
/// `pixel_ratio` fills `{ratio}`.
pub fn build_tile_urls(source: &TileSource, coords: &[TileCoord], pixel_ratio: f64) -> Vec<(TileCoord, String)> {
    coords
        .iter()
        .map(|&coord| {
            let url = match source {
                TileSource::XyzRaster { urls, .. } => urls.url(coord, pixel_ratio),
                TileSource::MapboxVectorTile { urls, api_key, .. } => {
                    let base = urls.url(coord, pixel_ratio);
                    match api_key {
                        Some(key) => {
                            let sep = if base.contains('?') { '&' } else { '?' };
                            format!("{base}{sep}access_token={key}")
                        }
                        None => base,
                    }
                }
                // PMTiles fetch bypasses the URL template system.
                TileSource::PMTiles { .. } => String::new(),
            };
            (coord, url)
        })
        .collect()
}

/// Fetch all tiles concurrently, respecting timeout and concurrency limits.
///
/// Hits the in-process LRU cache before issuing network requests. Cache misses
/// are populated after a successful fetch so subsequent renders re-use bytes.
///
/// When `fail_on_tile_error` is `false`, individual tile failures are logged
/// and skipped; the render continues with whatever tiles succeeded.
pub async fn fetch_all_tiles(
    client: &reqwest::Client,
    urls: Vec<(TileCoord, String)>,
    source: &TileSource,
    timeout_ms: u32,
    concurrency: u32,
    fail_on_tile_error: bool,
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
            TileSource::MapboxVectorTile { .. } => None,
            TileSource::PMTiles { .. } => None,
        };

        set.spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            // Cache hit — skip network entirely.
            if let Ok(mut cache) = TILE_CACHE.lock() {
                if let Some(cached) = cache.get(&url) {
                    return Ok((coord, cached.clone()));
                }
            }

            let mut req = client.get(&url).timeout(timeout);
            if let Some(header) = auth {
                req = req.header("Authorization", header);
            }
            let resp = req
                .send()
                .await
                .map_err(|_| BasemapError::TileFetchFailed {
                    url: url.clone(),
                    status: 0,
                })?;
            if !resp.status().is_success() {
                return Err(BasemapError::TileFetchFailed {
                    url: url.clone(),
                    status: resp.status().as_u16(),
                });
            }
            let bytes = resp
                .bytes()
                .await
                .map_err(|_| BasemapError::TileFetchFailed {
                    url: url.clone(),
                    status: 0,
                })?;
            let data = bytes.to_vec();

            // Populate cache for future renders.
            if let Ok(mut cache) = TILE_CACHE.lock() {
                cache.put(url, data.clone());
            }

            Ok((coord, data))
        });
    }

    let mut results = Vec::new();
    while let Some(res) = set.join_next().await {
        let tile_result = res.map_err(|e| BasemapError::RenderError(e.to_string()))?;
        match tile_result {
            Ok(tile) => results.push(tile),
            Err(e) => {
                if fail_on_tile_error {
                    return Err(e);
                }
                log::warn!("tile fetch failed (skipped): {e}");
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod url_tests {
    use super::*;

    fn c(z: u8, x: u32, y: u32) -> TileCoord {
        TileCoord { z, x, y }
    }

    #[test]
    fn tokens_expand_as_maplibre() {
        let t = |tpl: &str, coord, ratio, tms| tile_url(&[tpl.to_owned()], coord, ratio, tms);
        assert_eq!(t("https://t/{z}/{x}/{y}.png", c(5, 3, 7), 1.0, false), "https://t/5/3/7.png");
        // scheme "tms": rows from the south.
        assert_eq!(t("https://t/{z}/{x}/{y}.png", c(5, 3, 7), 1.0, true), "https://t/5/3/24.png");
        assert_eq!(t("https://t/{z}/{x}/{y}{ratio}.png", c(1, 0, 0), 2.0, false), "https://t/1/0/0@2x.png");
        assert_eq!(t("https://t/{z}/{x}/{y}{ratio}.png", c(1, 0, 0), 1.0, false), "https://t/1/0/0.png");
        // Bing's documented example: tile (3, 5) at level 3 is "213".
        assert_eq!(t("https://t/{quadkey}.jpeg", c(3, 3, 5), 1.0, false), "https://t/213.jpeg");
        assert_eq!(t("https://t/{prefix}/{z}", c(5, 17, 30), 1.0, false), "https://t/1e/5");
        assert_eq!(
            t("https://t/{TileMatrix}/{TileRow}/{TileCol}.png", c(5, 3, 7), 1.0, false),
            "https://t/5/7/3.png"
        );
    }

    #[test]
    fn templates_are_picked_by_x_plus_y() {
        let urls = TileUrls { templates: vec!["a/{x}".into(), "b/{x}".into(), "c/{x}".into()], tms: false };
        assert_eq!(urls.url(c(4, 0, 0), 1.0), "a/0");
        assert_eq!(urls.url(c(4, 1, 0), 1.0), "b/1");
        assert_eq!(urls.url(c(4, 1, 1), 1.0), "c/1");
        assert_eq!(urls.url(c(4, 2, 1), 1.0), "a/2");
    }

    #[test]
    fn bbox_matches_maplibre_strings() {
        // Strings from MapLibre GL JS 5.24 (whoots-js getTileBBox, String(n)).
        let b = |coord| tile_url(&["{bbox-epsg-3857}".to_owned()], coord, 1.0, false);
        assert_eq!(b(c(0, 0, 0)), "-20037508.342789244,-20037508.342789244,20037508.342789244,20037508.342789244");
        assert_eq!(b(c(1, 1, 0)), "0,0,20037508.342789244,20037508.342789244");
        assert_eq!(b(c(1, 0, 1)), "-20037508.342789244,-20037508.342789244,0,0");
        assert_eq!(b(c(3, 4, 2)), "0,5009377.085697312,5009377.085697312,10018754.17139462");
        assert_eq!(
            b(c(19, 154308, 197167)),
            "-8242663.382160267,4966572.349857613,-8242586.945131982,4966648.786885899"
        );
    }
}
