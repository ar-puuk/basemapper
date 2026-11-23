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
//! `~/Library/Caches`, `$XDG_CACHE_HOME` or `~/.cache`), under `basemapper`
//! (the R package uses `tools::R_user_dir("basemapper", "cache")`, as CRAN
//! asks). [`set_dir`] changes it at run time.
//!
//! The cache is kept small: expired tiles are deleted, and past its size
//! limit (250 MB, or `BASEMAPPER_CACHE_MAX_SIZE` in MB; [`set_max_size`])
//! the least recently used tiles go first. This clean-up runs in the
//! background, on the first tile stored in a session and every
//! [`PRUNE_EVERY`] tiles after.

use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static DIR: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(initial_dir()));

/// Kept this long when the server gives no caching headers.
const DEFAULT_TTL: Duration = Duration::from_secs(24 * 3600);

/// The size limit when `BASEMAPPER_CACHE_MAX_SIZE` is not set: 250 MB.
pub const DEFAULT_MAX_SIZE: u64 = 250 * 1024 * 1024;
/// Clean up after this many stored tiles (and on the first in a session).
pub const PRUNE_EVERY: u64 = 200;
static MAX_SIZE: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(initial_max_size()));
static STORED: AtomicU64 = AtomicU64::new(0);
static PRUNING: AtomicBool = AtomicBool::new(false);

fn initial_max_size() -> u64 {
    std::env::var("BASEMAPPER_CACHE_MAX_SIZE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|mb| *mb > 0.0)
        .map_or(DEFAULT_MAX_SIZE, |mb| (mb * 1024.0 * 1024.0) as u64)
}

/// The cache's size limit in bytes.
pub fn max_size() -> u64 {
    MAX_SIZE.load(Ordering::Relaxed)
}

/// Set the cache's size limit in bytes, and trim the cache to it.
pub fn set_max_size(bytes: u64) {
    MAX_SIZE.store(bytes.max(1), Ordering::Relaxed);
    prune_in_background();
}

/// What a clean-up did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pruned {
    /// Tiles deleted (expired, or to keep under the size limit).
    pub removed: usize,
    /// The bytes the cache holds afterwards.
    pub size: u64,
}

/// Delete expired tiles, then the least recently used ones until the cache
/// is under 80% of its size limit (so clean-ups are not needed on every
/// store). Unfinished writes older than an hour go too.
pub fn prune() -> std::io::Result<Pruned> {
    match dir() {
        Some(d) => prune_in(&d, max_size()),
        None => Ok(Pruned::default()),
    }
}

/// [`prune`] for the cache in `dir`, to `limit` bytes.
fn prune_in(dir: &Path, limit: u64) -> std::io::Result<Pruned> {
    let tiles = dir.join("tiles");
    if !tiles.exists() {
        return Ok(Pruned::default());
    }
    let (now_s, now_t) = (now(), SystemTime::now());
    let mut kept: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    let mut out = Pruned::default();
    for sub in std::fs::read_dir(&tiles)?.flatten() {
        let Ok(entries) = std::fs::read_dir(sub.path()) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            let Ok(meta) = e.metadata() else { continue };
            let modified = meta.modified().unwrap_or(now_t);
            let unfinished = path.extension().is_some_and(|x| x.to_string_lossy().starts_with("tmp"));
            let stale = now_t.duration_since(modified).is_ok_and(|age| age > Duration::from_secs(3600));
            if unfinished {
                if stale && std::fs::remove_file(&path).is_ok() {
                    out.removed += 1;
                }
                continue;
            }
            if expires_at(&path).is_some_and(|t| t <= now_s) {
                if std::fs::remove_file(&path).is_ok() {
                    out.removed += 1;
                }
                continue;
            }
            kept.push((modified, meta.len(), path));
        }
    }
    let mut size: u64 = kept.iter().map(|k| k.1).sum();
    if size > limit {
        kept.sort_by_key(|k| k.0); // least recently used first
        let target = limit / 10 * 8;
        for (_, len, path) in &kept {
            if size <= target {
                break;
            }
            if std::fs::remove_file(path).is_ok() {
                size -= len;
                out.removed += 1;
            }
        }
    }
    out.size = size;
    Ok(out)
}

/// When a cached tile expires, from its first eight bytes.
fn expires_at(path: &Path) -> Option<u64> {
    use std::io::Read;
    let mut head = [0u8; 8];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    Some(u64::from_le_bytes(head))
}

/// [`prune`] on a background thread, unless one is already running.
fn prune_in_background() {
    if PRUNING.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(|| {
        if let Err(e) = prune() {
            log::debug!("tile cache: clean-up failed: {e}");
        }
        PRUNING.store(false, Ordering::Release);
    });
}

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
    // Mark the tile as just used, for least-recently-used clean-up.
    if let Ok(f) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ = f.set_modified(SystemTime::now());
    }
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
        return;
    }
    if STORED.fetch_add(1, Ordering::Relaxed) % PRUNE_EVERY == 0 {
        prune_in_background();
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
    fn prune_deletes_expired_then_least_recently_used_tiles() {
        let dir = std::env::temp_dir().join(format!("basemapper-prune-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let old = SystemTime::now() - Duration::from_secs(600);
        let entry = |name: &str, expires: u64, used: SystemTime| {
            let p = path_in(&dir, name);
            write_entry(&p, expires, &[0u8; 1000]).unwrap();
            std::fs::OpenOptions::new().write(true).open(&p).unwrap().set_modified(used).unwrap();
            p
        };
        let expired = entry("https://a/expired", now() - 1, SystemTime::now());
        let lru = entry("https://a/least-recent", now() + 60, old);
        let fresh = entry("https://a/fresh", now() + 60, SystemTime::now());
        let fresh2 = entry("https://a/fresh2", now() + 60, SystemTime::now());

        let r = prune_in(&dir, 2_600).unwrap(); // room for two of the three live tiles

        assert!(!expired.exists(), "expired tiles are deleted");
        assert!(!lru.exists(), "the least recently used tile goes first");
        assert!(fresh.exists() && fresh2.exists());
        assert_eq!(r, Pruned { removed: 2, size: 2016 });
        std::fs::remove_dir_all(&dir).unwrap();
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
