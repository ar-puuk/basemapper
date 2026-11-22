//! A disk cache for map tiles, so repeated renders of an area do not fetch
//! the same tiles again (tile usage policies such as OpenStreetMap's ask
//! clients to cache).
//!
//! Tiles are kept for as long as the server's `Cache-Control: max-age` or
//! `Expires` allows (a day when it says nothing), and never when it sends
//! `no-store` or `no-cache`. Tiles fetched with an `Authorization` header
//! and tiles from the local machine are not cached.
//!
//! The cache lives in `BASEMAPPER_CACHE_DIR` when that is set (`off`
//! disables it), else in the platform cache directory (`%LOCALAPPDATA%`,
//! `~/Library/Caches`, `$XDG_CACHE_HOME` or `~/.cache`), under `basemapper`.
//! [`set_dir`] changes it at run time.

use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static DIR: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(initial_dir()));

/// Kept this long when the server gives no caching headers.
const DEFAULT_TTL: Duration = Duration::from_secs(24 * 3600);

fn initial_dir() -> Option<PathBuf> {
    match std::env::var_os("BASEMAPPER_CACHE_DIR") {
        Some(v) if v.is_empty() || v.eq_ignore_ascii_case("off") => None,
        Some(v) => Some(PathBuf::from(v)),
        None => default_dir(),
    }
}

/// The platform cache directory for basemapper.
pub fn default_dir() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library").join("Caches"))
    } else {
        var("XDG_CACHE_HOME").or_else(|| var("HOME").map(|h| h.join(".cache")))
    };
    base.map(|b| b.join("basemapper"))
}

/// The cache directory, or `None` when caching is off.
pub fn dir() -> Option<PathBuf> {
    DIR.read().ok().and_then(|d| d.clone())
}

/// Move the cache to `dir`, or turn it off with `None`.
pub fn set_dir(dir: Option<PathBuf>) {
    if let Ok(mut d) = DIR.write() {
        *d = dir;
    }
}

/// Delete every cached tile. Only the cache's own `tiles` folder is removed.
pub fn clear() -> std::io::Result<()> {
    match dir() {
        Some(d) if d.join("tiles").exists() => std::fs::remove_dir_all(d.join("tiles")),
        _ => Ok(()),
    }
}

/// Two FNV-1a passes with different seeds: a stable 128-bit file name.
fn key(url: &str) -> String {
    let fnv = |seed: u64| {
        url.bytes().fold(seed, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
    };
    format!("{:016x}{:016x}", fnv(0xcbf2_9ce4_8422_2325), fnv(0x6c62_272e_07bb_0142))
}

fn path_in(dir: &Path, url: &str) -> PathBuf {
    let k = key(url);
    dir.join("tiles").join(&k[..2]).join(k)
}

/// Whether tiles from `url` may be cached: remote http(s) only.
fn cacheable(url: &str) -> bool {
    let Ok(u) = reqwest::Url::parse(url) else { return false };
    if !matches!(u.scheme(), "http" | "https") {
        return false;
    }
    match u.host_str() {
        Some(h) => match h.trim_start_matches('[').trim_end_matches(']').parse::<std::net::IpAddr>() {
            Ok(ip) => !ip.is_loopback(),
            Err(_) => !h.eq_ignore_ascii_case("localhost"),
        },
        None => false,
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// A cached, unexpired tile.
pub(crate) fn get(url: &str) -> Option<Vec<u8>> {
    let dir = dir()?;
    if !cacheable(url) {
        return None;
    }
    read_entry(&path_in(&dir, url))
}

fn read_entry(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = std::fs::read(path).ok()?;
    let expires = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    if expires <= now() {
        let _ = std::fs::remove_file(path);
        return None;
    }
    bytes.drain(..8);
    Some(bytes)
}

/// How long the server lets a response be cached; `None` for not at all.
fn ttl(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    use reqwest::header::{CACHE_CONTROL, EXPIRES};
    if let Some(cc) = headers.get(CACHE_CONTROL).and_then(|v| v.to_str().ok()) {
        let cc = cc.to_ascii_lowercase();
        let directives: Vec<&str> = cc.split(',').map(str::trim).collect();
        if directives.iter().any(|d| matches!(*d, "no-store" | "no-cache" | "private")) {
            return None;
        }
        if let Some(age) = directives.iter().find_map(|d| d.strip_prefix("max-age=")) {
            return age.trim_matches('"').parse::<u64>().ok().filter(|&s| s > 0).map(Duration::from_secs);
        }
    }
    if let Some(expires) = headers.get(EXPIRES).and_then(|v| v.to_str().ok()) {
        let at = httpdate::parse_http_date(expires).ok()?;
        return at.duration_since(SystemTime::now()).ok().filter(|d| !d.is_zero());
    }
    Some(DEFAULT_TTL)
}

/// Store a fetched tile, if the server allows caching it.
pub(crate) fn put(url: &str, headers: &reqwest::header::HeaderMap, body: &[u8]) {
    let (Some(dir), Some(ttl)) = (dir(), ttl(headers)) else { return };
    if !cacheable(url) {
        return;
    }
    if let Err(e) = write_entry(&path_in(&dir, url), now() + ttl.as_secs(), body) {
        log::debug!("tile cache: could not write {url}: {e}");
    }
}

fn write_entry(path: &Path, expires: u64, body: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().expect("cache path has a parent"))?;
    let mut data = Vec::with_capacity(body.len() + 8);
    data.extend_from_slice(&expires.to_le_bytes());
    data.extend_from_slice(body);
    // Write then rename, so a concurrent reader never sees half a tile.
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, &data)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue, CACHE_CONTROL, EXPIRES};

    fn headers(pairs: &[(reqwest::header::HeaderName, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(k.clone(), HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn ttl_follows_the_server() {
        assert_eq!(ttl(&headers(&[])), Some(DEFAULT_TTL));
        assert_eq!(ttl(&headers(&[(CACHE_CONTROL, "public, max-age=604800")])), Some(Duration::from_secs(604800)));
        assert_eq!(ttl(&headers(&[(CACHE_CONTROL, "max-age=0")])), None);
        assert_eq!(ttl(&headers(&[(CACHE_CONTROL, "no-store")])), None);
        assert_eq!(ttl(&headers(&[(CACHE_CONTROL, "No-Cache, max-age=60")])), None);
        assert_eq!(ttl(&headers(&[(EXPIRES, "Thu, 01 Jan 1970 00:00:00 GMT")])), None);
        let later = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(7200));
        let t = ttl(&headers(&[(EXPIRES, &later)])).unwrap();
        assert!(t > Duration::from_secs(7000) && t <= Duration::from_secs(7200), "{t:?}");
    }

    #[test]
    fn only_remote_http_is_cached() {
        assert!(cacheable("https://tile.openstreetmap.org/1/0/0.png"));
        assert!(!cacheable("http://127.0.0.1:8080/1/0/0.png"));
        assert!(!cacheable("http://localhost/1/0/0.png"));
        assert!(!cacheable("http://[::1]/t.png"));
        assert!(!cacheable("file:///tmp/t.png"));
        assert!(!cacheable("pmtiles://https://x/a.pmtiles"));
    }

    #[test]
    fn entries_round_trip_and_expire() {
        let dir = std::env::temp_dir().join(format!("basemapper-cache-test-{}", std::process::id()));
        let p = path_in(&dir, "https://a/1/0/0.png");
        assert_ne!(p, path_in(&dir, "https://a/1/0/1.png"));
        write_entry(&p, now() + 60, b"tile").unwrap();
        assert_eq!(read_entry(&p).as_deref(), Some(&b"tile"[..]));
        write_entry(&p, now() - 1, b"old").unwrap();
        assert_eq!(read_entry(&p), None);
        assert!(!p.exists(), "expired entries are removed");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
