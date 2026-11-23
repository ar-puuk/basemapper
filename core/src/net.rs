//! Network safety shared by every download: response size limits, secrets
//! kept out of messages, and the hosts an access token is sent to.

/// Most bytes read from one response, by what is fetched. Bodies are
/// counted after decompression, so a small compressed response cannot
/// expand without bound.
pub(crate) mod limit {
    /// A map tile (vector, raster or DEM) or a PMTiles range.
    pub const TILE: usize = 32 << 20;
    /// A style, TileJSON or service description.
    pub const JSON: usize = 64 << 20;
    /// A GeoJSON source's data.
    pub const GEOJSON: usize = 512 << 20;
    /// A glyph range, sprite sheet or font file.
    pub const ASSET: usize = 32 << 20;
}

/// The body of `resp`, or an error once it passes `max` bytes.
pub(crate) async fn read_limited(mut resp: reqwest::Response, max: usize) -> Result<Vec<u8>, String> {
    let too_big = || format!("response larger than {} MB", max >> 20);
    if resp.content_length().is_some_and(|n| n > max as u64) {
        return Err(too_big());
    }
    let mut out = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if out.len() + chunk.len() > max {
            return Err(too_big());
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

// ── In-memory cache of resources ────────────────────────────────────────────

/// A response kept in memory: its body and the URL it came from (after
/// redirects, for resolving relative URLs in it).
#[derive(Clone, Debug)]
pub(crate) struct Fetched {
    pub body: std::sync::Arc<Vec<u8>>,
    pub url: String,
}

/// Why [`get`] failed.
#[derive(Debug)]
pub(crate) enum FetchError {
    /// The server answered with this non-success status.
    Status(u16),
    /// No answer, or the body could not be read (message without the URL).
    Network(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Status(s) => write!(f, "HTTP {s}"),
            FetchError::Network(e) => f.write_str(e),
        }
    }
}

/// A byte-bounded LRU of responses with expiry times.
pub(crate) struct ResourceCache {
    entries: lru::LruCache<String, (std::time::Instant, Fetched)>,
    bytes: usize,
    max_bytes: usize,
}

impl ResourceCache {
    pub fn new(max_bytes: usize) -> Self {
        ResourceCache { entries: lru::LruCache::unbounded(), bytes: 0, max_bytes }
    }

    pub fn get(&mut self, key: &str) -> Option<Fetched> {
        let (expires, hit) = self.entries.get(key)?;
        if *expires > std::time::Instant::now() {
            return Some(hit.clone());
        }
        let (_, gone) = self.entries.pop(key)?;
        self.bytes -= gone.body.len();
        None
    }

    pub fn put(&mut self, key: String, ttl: std::time::Duration, value: Fetched) {
        let len = value.body.len();
        if len > self.max_bytes / 4 {
            return; // one huge response would evict everything else
        }
        if let Some((_, old)) = self.entries.put(key, (std::time::Instant::now() + ttl, value)) {
            self.bytes -= old.body.len();
        }
        self.bytes += len;
        while self.bytes > self.max_bytes {
            match self.entries.pop_lru() {
                Some((_, (_, v))) => self.bytes -= v.body.len(),
                None => break,
            }
        }
    }
}

/// Kept at most this long in memory, whatever the server allows, so a
/// long session still sees an updated style.
const MAX_MEMORY_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

/// Styles, TileJSON, glyphs, sprites and GeoJSON: fetched once per session
/// (within the server's caching headers), not on every render of a plot.
static RESOURCES: once_cell::sync::Lazy<std::sync::Mutex<ResourceCache>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(ResourceCache::new(96 << 20)));

/// The body of `url` (at most `max` bytes), from memory when an unexpired
/// copy is there. Successful responses are kept for as long as the server's
/// `Cache-Control` / `Expires` allow (an hour at most; never with
/// `no-store` or `no-cache`). `auth`'s token is sent when it may be.
pub(crate) async fn get(client: &reqwest::Client, url: &str, auth: &Auth, max: usize) -> Result<Fetched, FetchError> {
    let token = auth.token_for(url);
    // Responses can differ by token: the key includes it (in memory only).
    let key = format!("{url}\n{}", token.unwrap_or(""));
    if let Some(hit) = RESOURCES.lock().ok().and_then(|mut c| c.get(&key)) {
        return Ok(hit);
    }
    let resp = bearer(client.get(url), auth, url)
        .send()
        .await
        .map_err(|e| FetchError::Network(e.without_url().to_string()))?;
    if !resp.status().is_success() {
        return Err(FetchError::Status(resp.status().as_u16()));
    }
    let ttl = crate::cache::ttl(resp.headers()).map(|t| t.min(MAX_MEMORY_TTL));
    let final_url = resp.url().to_string();
    let body = read_limited(resp, max).await.map_err(FetchError::Network)?;
    let fetched = Fetched { body: std::sync::Arc::new(body), url: final_url };
    if let (Some(ttl), Ok(mut cache)) = (ttl, RESOURCES.lock()) {
        cache.put(key, ttl, fetched.clone());
    }
    Ok(fetched)
}

/// Query parameters whose values are credentials.
const SECRET_PARAMS: &[&str] =
    &["key", "apikey", "api_key", "access_token", "token", "auth", "signature", "sig", "password", "client_secret"];

/// `url` with credentials replaced by `***`: secret query parameters,
/// user info (`user:pass@`), and UGRC Discover's quad word path segment.
/// For error messages and logs, which end up in consoles, CI logs and
/// bug reports.
pub fn redact(url: &str) -> String {
    let (head, query) = match url.split_once('?') {
        Some((h, q)) => (h, Some(q)),
        None => (url, None),
    };
    let mut head = head.to_owned();
    // user:pass@host
    if let Some(scheme_end) = head.find("://") {
        let auth_start = scheme_end + 3;
        let host_end = head[auth_start..].find('/').map_or(head.len(), |i| auth_start + i);
        if let Some(at) = head[auth_start..host_end].rfind('@') {
            head.replace_range(auth_start..auth_start + at, "***");
        }
    }
    // UGRC Discover: .../login/path/<quad-word>/tiles/...
    if let Some(i) = head.find("/login/path/") {
        let start = i + "/login/path/".len();
        let end = head[start..].find('/').map_or(head.len(), |j| start + j);
        if end > start {
            head.replace_range(start..end, "***");
        }
    }
    let Some(query) = query else { return head };
    let query: Vec<String> = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) if !v.is_empty() && SECRET_PARAMS.contains(&k.to_ascii_lowercase().as_str()) => format!("{k}=***"),
            _ => pair.to_owned(),
        })
        .collect();
    format!("{head}?{}", query.join("&"))
}

/// The origin (scheme, host and port, as browsers scope credentials) of an
/// http(s) URL: `https://tiles.example.com`, `http://127.0.0.1:8080`.
pub(crate) fn origin_of(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    Some(u.origin().ascii_serialization())
}

/// An access token and the origins it may be sent to.
///
/// A token is sent to the origin the style was loaded from (or the tile
/// URL, TileJSON or service given instead of a style), and to the tile
/// origins that a TileJSON from there names. Origins that a style merely
/// lists (a third-party overlay, say) do not get it. A style given inline or
/// as a local file is the user's own: every origin in it is trusted.
#[derive(Clone, Debug, Default)]
pub(crate) struct Auth {
    token: Option<String>,
    /// `None`: every origin (no remote origin to scope to).
    hosts: Option<Vec<String>>,
}

impl Auth {
    /// No token.
    pub fn none() -> Self {
        Auth::default()
    }

    /// `token` scoped to the origin of `input`, the `style` string a user
    /// gave (a catalogue name is resolved to its URL first).
    pub fn for_input(token: Option<&str>, input: &str) -> Self {
        let Some(token) = token else { return Auth::none() };
        let resolved = crate::providers::bare_name_style(input).ok().flatten();
        let target = resolved.as_deref().unwrap_or(input).trim();
        let target = target.strip_prefix("pmtiles://").unwrap_or(target);
        let hosts = if target.starts_with('{') {
            None
        } else if target.starts_with("http://") || target.starts_with("https://") {
            // A template's `{s}`-free host still parses; one that does not
            // parse gets no token rather than every host.
            Some(origin_of(target).into_iter().collect())
        } else {
            None // a local file
        };
        Auth { token: Some(token.to_owned()), hosts }
    }

    /// The token, if `url` may receive it.
    pub fn token_for(&self, url: &str) -> Option<&str> {
        let token = self.token.as_deref()?;
        match &self.hosts {
            None => Some(token),
            Some(hosts) => origin_of(url).filter(|h| hosts.contains(h)).map(|_| token),
        }
    }

    /// Whether every one of `urls` may receive the token.
    pub fn allows_all<'a>(&self, mut urls: impl Iterator<Item = &'a str>) -> bool {
        self.token.is_some() && urls.all(|u| self.token_for(u).is_some())
    }

    /// This scope widened to the hosts of `urls` (the tiles a trusted
    /// TileJSON names).
    pub fn with_hosts<'a>(&self, urls: impl Iterator<Item = &'a str>) -> Self {
        let mut out = self.clone();
        if let Some(hosts) = out.hosts.as_mut() {
            for h in urls.filter_map(origin_of) {
                if !hosts.contains(&h) {
                    hosts.push(h);
                }
            }
        }
        out
    }

    /// The bare token, unscoped (for sources whose every URL is trusted).
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }
}

/// `req` with `Authorization: Bearer` when `auth` allows `url`.
pub(crate) fn bearer(req: reqwest::RequestBuilder, auth: &Auth, url: &str) -> reqwest::RequestBuilder {
    match auth.token_for(url) {
        Some(t) => req.header("Authorization", format!("Bearer {t}")),
        None => req,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_redacted() {
        assert_eq!(
            redact("https://api.maptiler.com/maps/streets/style.json?key=SECRET"),
            "https://api.maptiler.com/maps/streets/style.json?key=***"
        );
        assert_eq!(
            redact("https://t/1/0/0.png?API_KEY=s&fmt=png&access_token=t"),
            "https://t/1/0/0.png?API_KEY=***&fmt=png&access_token=***"
        );
        assert_eq!(
            redact("https://discover.agrc.utah.gov/login/path/one-two-three-four/tiles/lite/1/0/0"),
            "https://discover.agrc.utah.gov/login/path/***/tiles/lite/1/0/0"
        );
        assert_eq!(redact("https://user:pw@host/x"), "https://***@host/x");
        assert_eq!(redact("https://host/{z}/{x}/{y}.png"), "https://host/{z}/{x}/{y}.png");
        assert_eq!(redact("https://host/a?keyword=x"), "https://host/a?keyword=x");
    }

    #[test]
    fn resource_cache_is_bounded_by_bytes_and_expires() {
        let f = |n: usize| Fetched { body: std::sync::Arc::new(vec![0; n]), url: String::new() };
        let hour = std::time::Duration::from_secs(3600);
        let mut c = ResourceCache::new(1000);
        c.put("a".into(), hour, f(200));
        c.put("b".into(), hour, f(200));
        assert!(c.get("a").is_some()); // a is now the most recently used
        c.put("c".into(), hour, f(200));
        c.put("d".into(), hour, f(200));
        c.put("e".into(), hour, f(200)); // 1000 bytes: still fits
        c.put("f".into(), hour, f(200)); // evicts the least recently used: b
        assert!(c.get("b").is_none() && c.get("a").is_some() && c.get("f").is_some());
        assert!(c.bytes <= 1000);
        c.put("huge".into(), hour, f(600)); // over a quarter of the cache: not kept
        assert!(c.get("huge").is_none());
        c.put("old".into(), std::time::Duration::ZERO, f(10));
        assert!(c.get("old").is_none(), "expired entries are not served");
    }

    #[test]
    fn tokens_stay_with_their_origin() {
        let a = Auth::for_input(Some("T"), "https://maps.example.com/style.json");
        assert_eq!(a.token_for("https://maps.example.com/tiles/1/0/0.pbf"), Some("T"));
        assert_eq!(a.token_for("https://MAPS.example.com/x"), Some("T"));
        assert_eq!(a.token_for("https://overlay.other.org/1/0/0.png"), None);
        assert_eq!(a.token_for("http://maps.example.com/x"), None, "another scheme is another origin");
        assert_eq!(a.token_for("https://maps.example.com:8443/x"), None, "another port is another origin");
        let b = a.with_hosts(["https://tiles.example.net/{z}/{x}/{y}.pbf"].into_iter());
        assert_eq!(b.token_for("https://tiles.example.net/1/0/0.pbf"), Some("T"));
        // Inline styles and local files are the user's own: no scoping.
        let inline = Auth::for_input(Some("T"), r#"{"version":8}"#);
        assert_eq!(inline.token_for("https://anywhere.org/x"), Some("T"));
        let local = Auth::for_input(Some("T"), "styles/mine.json");
        assert_eq!(local.token_for("https://anywhere.org/x"), Some("T"));
        assert_eq!(Auth::for_input(None, "https://a/s.json").token_for("https://a/x"), None);
    }
}
