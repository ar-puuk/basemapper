use thiserror::Error;

#[derive(Debug, Error)]
pub enum BasemapError {
    #[error("invalid bbox: {0}")]
    InvalidBbox(String),
    #[error("invalid dimensions {0}×{1}")]
    InvalidDimensions(u32, u32),
    /// The URL is shown with credentials redacted ([`crate::net::redact`]).
    #[error("style fetch failed for {}: HTTP {status}", crate::net::redact(.url))]
    StyleFetchFailed { url: String, status: u16 },
    #[error("style parse error: {0}")]
    StyleParseError(String),
    /// The URL is shown with credentials redacted ([`crate::net::redact`]).
    #[error("tile fetch failed for {}: HTTP {status}", crate::net::redact(.url))]
    TileFetchFailed { url: String, status: u16 },
    /// A tile URL answered with a web page (a sign-in or error page), as
    /// some services do for a wrong or missing key. The message leaves the
    /// URL out: it may carry the key.
    #[error("the tile server sent a web page instead of a tile: check the API key (or quad word) and that it gives access to this basemap")]
    TileIsWebPage { url: String },
    #[error("tile decode error: {0}")]
    TileDecodeError(String),
    #[error("render error: {0}")]
    RenderError(String),
    #[error("max tiles exceeded: requested {requested}, limit {limit}")]
    MaxTilesExceeded { requested: u32, limit: u32 },
    #[error("invalid layer filter: {0}")]
    InvalidLayerFilter(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl BasemapError {
    /// Return a stable, coarse category string for mapping to typed Python exceptions.
    ///
    /// Variants:
    /// - `"validation"` → `ValidationError` (invalid input before any I/O)
    /// - `"style"`      → `StyleError` (style fetch or parse failure)
    /// - `"network"`    → `NetworkError` (tile fetch failure)
    /// - `"render"`     → `BasemapError` (everything else — GPU / decode)
    pub fn kind(&self) -> &'static str {
        match self {
            BasemapError::InvalidBbox(_)
            | BasemapError::InvalidDimensions(_, _)
            | BasemapError::InvalidLayerFilter(_)
            | BasemapError::InvalidInput(_)
            | BasemapError::MaxTilesExceeded { .. } => "validation",
            BasemapError::StyleFetchFailed { .. } | BasemapError::StyleParseError(_) => "style",
            BasemapError::TileFetchFailed { .. } | BasemapError::TileIsWebPage { .. } => "network",
            BasemapError::RenderError(_) | BasemapError::TileDecodeError(_) => "render",
        }
    }
}
