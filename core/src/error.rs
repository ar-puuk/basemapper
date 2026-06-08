use thiserror::Error;

#[derive(Debug, Error)]
pub enum BasemapError {
    #[error("invalid bbox: {0}")]
    InvalidBbox(String),
    #[error("invalid dimensions {0}×{1}")]
    InvalidDimensions(u32, u32),
    #[error("style fetch failed for {url}: HTTP {status}")]
    StyleFetchFailed { url: String, status: u16 },
    #[error("style parse error: {0}")]
    StyleParseError(String),
    #[error("tile fetch failed for {url}: HTTP {status}")]
    TileFetchFailed { url: String, status: u16 },
    #[error("tile decode error: {0}")]
    TileDecodeError(String),
    #[error("render error: {0}")]
    RenderError(String),
    #[error("max tiles exceeded: requested {requested}, limit {limit}")]
    MaxTilesExceeded { requested: u32, limit: u32 },
    #[error("invalid layer filter: {0}")]
    InvalidLayerFilter(String),
}
