//! How a basemap fetches its data: R `fetch_options()`, Python
//! `FetchOptions`, Rust [`FetchOptions`] (F9).

use crate::BasemapError;
use std::time::Duration;

/// What to do when part of the data cannot be had.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum OnError {
    /// Draw what there is; each gap is a *problem* note (the default).
    #[default]
    Skip,
    /// Fail: a failed tile stops the render, and a rendering with a
    /// *problem* note is an error.
    Fail,
}

impl OnError {
    /// `"skip"` or `"fail"`.
    pub fn parse(s: &str) -> Result<OnError, BasemapError> {
        match s {
            "skip" => Ok(OnError::Skip),
            "fail" => Ok(OnError::Fail),
            _ => Err(BasemapError::InvalidInput(format!("on_error must be \"skip\" or \"fail\", not {s:?}"))),
        }
    }
}

/// Download settings: a timeout per request, the most tiles one style may
/// fetch, how many downloads run at once, and what a failure does.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FetchOptions {
    pub timeout: Duration,
    pub max_tiles: u32,
    pub concurrency: u32,
    pub on_error: OnError,
}

impl Default for FetchOptions {
    /// 10 s, 256 tiles, 16 downloads at once, failures skipped.
    fn default() -> Self {
        FetchOptions { timeout: Duration::from_secs(10), max_tiles: 256, concurrency: 16, on_error: OnError::Skip }
    }
}

impl FetchOptions {
    pub fn new() -> Self {
        FetchOptions::default()
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn max_tiles(mut self, n: u32) -> Self {
        self.max_tiles = n;
        self
    }

    pub fn concurrency(mut self, n: u32) -> Self {
        self.concurrency = n;
        self
    }

    pub fn on_error(mut self, on_error: OnError) -> Self {
        self.on_error = on_error;
        self
    }

    /// Check the settings: a timeout, and at least one tile and download.
    pub fn validate(&self) -> Result<(), BasemapError> {
        if self.timeout.is_zero() {
            return Err(BasemapError::InvalidInput("fetch timeout must be > 0".into()));
        }
        if self.max_tiles == 0 || self.concurrency == 0 {
            return Err(BasemapError::InvalidInput("fetch max_tiles and concurrency must be at least 1".into()));
        }
        Ok(())
    }

    /// The timeout in milliseconds, as render requests take it.
    pub(crate) fn timeout_ms(&self) -> u32 {
        self.timeout.as_millis().clamp(1, u32::MAX as u128) as u32
    }
}
