// Crate documentation: the guide in guide/, with each example's source
// (examples/guide_*.rs) and its output (guide/output/, written by
// scripts/build-docs.ps1). Only read when building docs, so builds
// without the guide folder (the vendored R copy) are unaffected.
#![cfg_attr(doc, doc = include_str!("../guide/intro.md"))]
#![cfg_attr(doc, doc = include_str!("../guide/quickstart.md"))]
//! ```no_run
#![cfg_attr(doc, doc = include_str!("../examples/guide_quickstart.rs"))]
//! ```
//!
//! Output:
//!
//! ```text
#![cfg_attr(doc, doc = include_str!("../guide/output/quickstart.txt"))]
//! ```
#![cfg_attr(doc, doc = include_str!("../guide/quickstart_after.md"))]
//!
#![cfg_attr(doc, doc = include_str!("../guide/formats.md"))]
//! ```no_run
#![cfg_attr(doc, doc = include_str!("../examples/guide_formats.rs"))]
//! ```
//!
//! Output:
//!
//! ```text
#![cfg_attr(doc, doc = include_str!("../guide/output/formats.txt"))]
//! ```
#![cfg_attr(doc, doc = include_str!("../guide/formats_after.md"))]
//!
#![cfg_attr(doc, doc = include_str!("../guide/stack.md"))]
//! ```no_run
#![cfg_attr(doc, doc = include_str!("../examples/guide_stack.rs"))]
//! ```
//!
//! Output:
//!
//! ```text
#![cfg_attr(doc, doc = include_str!("../guide/output/stack.txt"))]
//! ```
#![cfg_attr(doc, doc = include_str!("../guide/stack_after.md"))]
//!
#![cfg_attr(doc, doc = include_str!("../guide/layers.md"))]
//! ```no_run
#![cfg_attr(doc, doc = include_str!("../examples/guide_layers.rs"))]
//! ```
//!
//! Output:
//!
//! ```text
#![cfg_attr(doc, doc = include_str!("../guide/output/layers.txt"))]
//! ```
#![cfg_attr(doc, doc = include_str!("../guide/layers_after.md"))]
//!
#![cfg_attr(doc, doc = include_str!("../guide/pmtiles.md"))]
//! ```no_run
#![cfg_attr(doc, doc = include_str!("../examples/guide_pmtiles.rs"))]
//! ```
//!
//! Output:
//!
//! ```text
#![cfg_attr(doc, doc = include_str!("../guide/output/pmtiles.txt"))]
//! ```
#![cfg_attr(doc, doc = include_str!("../guide/pmtiles_after.md"))]
//!
#![cfg_attr(doc, doc = include_str!("../guide/more.md"))]

#[doc(hidden)]
pub mod attribution;
pub mod basemap;
#[doc(hidden)]
pub mod bbox;
pub mod cache;
pub mod error;
pub mod features;
pub mod fetch;
#[doc(hidden)]
pub mod net;
pub mod mask;
pub mod notes;
pub mod plot;
pub mod rendering;
pub mod providers;
#[doc(hidden)]
pub mod renderer;
pub mod style;
#[doc(hidden)]
pub mod tile_fetcher;
pub mod tile_style;
#[doc(hidden)]
pub mod vector;
pub mod view;
pub mod warp;

pub use basemap::{Basemap, Crs};
pub use fetch::{FetchOptions, OnError};
pub use mask::{Keep, Shape};
pub use view::{Context, Device, Units, View, ViewInputs};
pub use warp::{Mesh, Warp};
#[doc(hidden)]
pub use bbox::{bbox_from_center, compute_zoom, constrain_view, fit_bounds, RenderRequest, RenderResult, SpatialBounds};
pub use error::BasemapError;
pub use notes::{Level, Note};
pub use plot::{Panel, PanelCrs};
pub use rendering::Rendering;
pub use style::{Source, Style};
#[doc(hidden)]
pub use style::{filter_style_layers, StyleInput, StylePart};
/// The version of basemapper (the Rust core, and the R and Python packages
/// built on it, share it; the release tag is `v` and this).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use features::Labels;
#[doc(hidden)]
pub use tile_fetcher::{TileSource, TileUrls};
pub use tile_style::{TileKind, TileStyleOptions};
pub use vector::fonts::{FontEmbed, SvgOptions, SvgText};

/// The pre-0.3 entry points and building blocks, for the R and Python
/// bindings (until they are adapters over [`Basemap`]) and the test suites.
/// Not part of the API: they change without notice.
#[doc(hidden)]
pub mod internal {
    pub use crate::bbox::{bbox_from_center, compute_zoom, constrain_view, fit_bounds, RenderRequest, RenderResult, SpatialBounds};
    pub use crate::style::{filter_style_layers, StyleInput, StylePart};
    pub use crate::tile_fetcher::{TileSource, TileUrls};
    pub use crate::{
        attribution, encode_png, render, render_stack, render_svg, render_svg_stack, render_svg_stack_with, render_svg_with,
        tile_style,
    };
}

use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

// Thread-local tokio runtime shared across all render calls from binding crates.
pub(crate) static RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("failed to create basemapper tokio runtime"));

/// The User-Agent sent with every request, identifying the app as tile
/// usage policies ask (e.g. OpenStreetMap's).
pub(crate) const USER_AGENT: &str =
    concat!("basemapper/", env!("CARGO_PKG_VERSION"), " (+https://github.com/ar-puuk/basemapper)");

/// The HTTP client for styles, tiles, glyphs and sprites, with `timeout`
/// per request.
///
/// Clients are built once per timeout and shared by every render (a
/// `reqwest::Client` is a handle to one connection pool): renders reuse
/// open connections and TLS sessions instead of building a client (and
/// loading the TLS root store) and connecting again each time.
pub(crate) fn http_client(timeout: std::time::Duration) -> Result<reqwest::Client, BasemapError> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static CLIENTS: Lazy<Mutex<HashMap<std::time::Duration, reqwest::Client>>> = Lazy::new(Default::default);
    let mut clients = CLIENTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = clients.get(&timeout) {
        return Ok(c.clone());
    }
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .use_rustls_tls()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| BasemapError::RenderError(e.to_string()))?;
    clients.insert(timeout, client.clone());
    Ok(client)
}

/// The request's access token, scoped to its style's origin (see
/// [`net::Auth`]).
fn request_auth(request: &RenderRequest) -> net::Auth {
    let scope = request.auth_scope.as_deref().unwrap_or_else(|| request.style_input.as_input());
    net::Auth::for_input(request.auth_token.as_deref(), scope)
}

/// Render a basemap tile mosaic to a flat RGBA byte array.
///
/// Supports anti-meridian crossing bboxes (xmin > xmax in EPSG:3857).
/// Such bboxes are split internally into west and east halves, each rendered
/// at proportional pixel width, then stitched horizontally.
#[doc(hidden)]
pub fn render(request: RenderRequest) -> Result<RenderResult, BasemapError> {
    let mut requests = expand_stacked(vec![request]);
    if requests.len() > 1 {
        return render_stack(requests);
    }
    let request = requests.remove(0);
    request.validate()?;
    RUNTIME.block_on(render_async(request))
}

/// The requests with each stacked basemap's name (`UGRC.Lite`) replaced by
/// one request per style of its stack, bottom to top.
fn expand_stacked(requests: Vec<RenderRequest>) -> Vec<RenderRequest> {
    let mut out = Vec::with_capacity(requests.len());
    for request in requests {
        match &request.style_input {
            StyleInput::Url(name) if providers::stack_provider(name).is_some() => {
                for style in providers::expand_stacks(std::slice::from_ref(name)) {
                    out.push(RenderRequest { style_input: StyleInput::detect(&style), ..request.clone() });
                }
            }
            _ => out.push(request),
        }
    }
    out
}

/// Render several styles over the same view and composite them bottom to
/// top (straight-alpha source-over), e.g. a basemap published as separate
/// services. All requests must share bbox, size and pixel ratio.
///
/// Each request's filters apply to its [`group`](RenderRequest::group) as
/// one map (a catalogue basemap published as several services is one
/// group): a part they leave nothing of (the hillshade under `labels =
/// Only`, the labels over `features = ["terrain"]`) is skipped, and it is an
/// error only when they leave nothing of a whole group. A name warns only
/// when it matches nothing in any part of its group.
#[doc(hidden)]
pub fn render_stack(requests: Vec<RenderRequest>) -> Result<RenderResult, BasemapError> {
    let mut requests = expand_stacked(requests);
    let first = requests.first().ok_or_else(|| BasemapError::RenderError("no styles to render".into()))?;
    let (bbox, w, h, r) = (first.bbox, first.width, first.height, first.pixel_ratio);
    if requests.iter().any(|q| q.bbox != bbox || q.width != w || q.height != h || q.pixel_ratio != r) {
        return Err(BasemapError::RenderError("all styles in a stack must share bbox, size and pixel ratio".into()));
    }
    if requests.len() == 1 {
        return render(requests.remove(0));
    }
    for q in &requests {
        q.validate()?;
    }
    // The styles download and render concurrently, then composite in
    // order (the first error in stack order is the one reported).
    let parts: Vec<(u32, Result<StackPart, BasemapError>)> = RUNTIME.block_on(async {
        let mut set = tokio::task::JoinSet::new();
        for (i, q) in requests.into_iter().enumerate() {
            set.spawn(notes::propagate(async move { (i, q.group, render_part(q).await) }));
        }
        let mut parts: Vec<(usize, u32, Result<StackPart, BasemapError>)> = Vec::new();
        while let Some(joined) = set.join_next().await {
            parts.push(joined.map_err(|e| BasemapError::RenderError(format!("stack render task failed: {e}")))?);
        }
        parts.sort_by_key(|p| p.0);
        Ok::<_, BasemapError>(parts.into_iter().map(|p| (p.1, p.2)).collect::<Vec<_>>())
    })?;
    /// What the parts of one group have drawn and found, so far.
    struct Group {
        id: u32,
        drawn: bool,
        warnings: Option<Vec<String>>,
        present: Vec<&'static str>,
    }
    let mut groups: Vec<Group> = Vec::new();
    let mut acc: Option<RenderResult> = None;
    for (id, part) in parts {
        let part = part?;
        let g = match groups.iter().position(|g| g.id == id) {
            Some(i) => &mut groups[i],
            None => {
                groups.push(Group { id, drawn: false, warnings: None, present: Vec::new() });
                groups.last_mut().expect("just pushed")
            }
        };
        g.warnings = Some(match g.warnings.take() {
            None => part.warnings,
            Some(prev) => prev.into_iter().filter(|w| part.warnings.contains(w)).collect(),
        });
        for f in part.present {
            if !g.present.contains(&f) {
                g.present.push(f);
            }
        }
        let Some(top) = part.result else { continue };
        g.drawn = true;
        acc = Some(match acc {
            None => top,
            Some(mut below) => {
                if below.pixels.len() != top.pixels.len() {
                    return Err(BasemapError::RenderError("stacked renders differ in size".into()));
                }
                composite_over(&mut below.pixels, &top.pixels);
                below.zoom_used = below.zoom_used.max(top.zoom_used);
                below
            }
        });
    }
    if let Some(g) = groups.iter().find(|g| !g.drawn) {
        let what = if groups.len() == 1 { "stack" } else { "style of the stack" };
        return Err(empty_filter_error(&g.present, what));
    }
    let mut out = acc.ok_or_else(|| BasemapError::RenderError("no styles to render".into()))?;
    let mut warnings: Vec<String> = Vec::new();
    for w in groups.into_iter().flat_map(|g| g.warnings.unwrap_or_default()) {
        if !warnings.contains(&w) {
            warnings.push(w);
        }
    }
    out.warnings = warnings;
    Ok(out)
}

/// `dst = src over dst`, both straight-alpha RGBA.
fn composite_over(dst: &mut [u8], src: &[u8]) {
    for (o, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let (sa, da) = (s[3] as f64 / 255.0, o[3] as f64 / 255.0);
        let a = sa + da * (1.0 - sa);
        if a <= 0.0 {
            o.copy_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        for i in 0..3 {
            let c = (s[i] as f64 / 255.0 * sa + o[i] as f64 / 255.0 * da * (1.0 - sa)) / a;
            o[i] = (c * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        o[3] = (a * 255.0).round() as u8;
    }
}

/// A style for tile data, with overrides: a tile URL template, TileJSON,
/// PMTiles archive (URL or path) or ArcGIS service, as raster tiles or a
/// default vector style over its source layers (see the
/// [`tile_style`](mod@tile_style) module). A style URL or file is returned as is (relative URLs made
/// absolute). Detecting raster or vector may fetch a tile or the archive
/// header. Returns the style JSON and the `paint` keys that matched no
/// generated layer.
#[doc(hidden)]
pub fn tile_style(
    input: &str,
    options: &TileStyleOptions,
    auth_token: Option<&str>,
) -> Result<(String, Vec<String>), BasemapError> {
    RUNTIME.block_on(async {
        let client = http_client(std::time::Duration::from_secs(30))?;
        tile_style::tile_style(input, options, &client, &net::Auth::for_input(auth_token, input)).await
    })
}

/// A style layer, as [`list_layers`] lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerInfo {
    /// What the `layers` filter matches.
    pub id: String,
    /// The layer type (`symbol` layers are the labels).
    pub kind: String,
    /// The vector tile layer it draws, if any.
    pub source_layer: Option<String>,
    /// What the `features` filter matches (see [`features`]).
    pub feature: String,
}

/// Filters to preview with [`list_layers`], as the renderers take them.
#[derive(Debug, Clone, Default)]
pub struct LayerFilter {
    pub layers: Option<Vec<String>>,
    pub labels: Labels,
    pub features: Option<Vec<String>>,
}

/// A style's layers in drawing order; with a filter, the layers a render
/// with that filter draws (none when it leaves nothing).
///
/// `style` is anything [`render`] accepts: a style, or tile data (a tile
/// URL, TileJSON, PMTiles archive or ArcGIS service) with its built style.
pub fn list_layers(style: &str, auth_token: Option<&str>, filter: &LayerFilter) -> Result<Vec<LayerInfo>, BasemapError> {
    if providers::stack_provider(style).is_some() {
        // A stacked basemap: every style's layers, bottom to top.
        let mut all = Vec::new();
        for part in providers::expand_stacks(&[style.to_owned()]) {
            all.extend(list_layers(&part, auth_token, filter)?);
        }
        return Ok(all);
    }
    let json = RUNTIME.block_on(async {
        let client = http_client(std::time::Duration::from_secs(30))?;
        StyleInput::detect(style).resolve(&client, &net::Auth::for_input(auth_token, style)).await
    })?;
    if let Some(f) = &filter.features {
        features::validate_features(f)?;
    }
    let (selection, _) = apply_filters(&json, filter.layers.as_deref(), filter.labels, filter.features.as_deref())?;
    let v: serde_json::Value =
        serde_json::from_str(&selection.style).map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
    Ok(v["layers"]
        .as_array()
        .map(|ls| {
            ls.iter()
                .map(|l| LayerInfo {
                    id: l["id"].as_str().unwrap_or("").to_owned(),
                    kind: l["type"].as_str().unwrap_or("").to_owned(),
                    source_layer: l["source-layer"].as_str().map(str::to_owned),
                    feature: features::layer_feature(l).to_owned(),
                })
                .collect()
        })
        .unwrap_or_default())
}

/// The attribution text of `styles` (stacked styles, the strings
/// [`Basemap`] takes): every source the styles draw from contributes its
/// attribution (from the style, its TileJSON or PMTiles metadata), as plain
/// text, without duplicates, joined with `" | "`. Empty when none is given.
#[doc(hidden)]
pub fn attribution(styles: &[String], auth_token: Option<&str>) -> Result<String, BasemapError> {
    RUNTIME.block_on(async {
        let client = http_client(std::time::Duration::from_secs(30))?;
        attribution::attribution(styles, &client, auth_token).await
    })
}

/// Encode straight-alpha RGBA pixels as a PNG file.
#[doc(hidden)]
pub fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, BasemapError> {
    use image::ImageEncoder;
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(pixels, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| BasemapError::RenderError(format!("PNG encoding failed: {e}")))?;
    Ok(buf)
}

// ──────────────────────────────────────────────────────────────────────────────
// Async rendering pipeline
// ──────────────────────────────────────────────────────────────────────────────

/// The style with `layers`, `labels` and `features` applied, and warnings
/// for the names that matched no layer of it.
fn apply_filters(
    raw_json: &str,
    layers: Option<&[String]>,
    labels: Labels,
    features: Option<&[String]>,
) -> Result<(features::Selection, Vec<String>), BasemapError> {
    let (json, unmatched_layers) = match layers {
        Some(l) => style::filter_layers(raw_json, l)?,
        None => (raw_json.to_owned(), Vec::new()),
    };
    let selection = features::select(&json, labels, features.unwrap_or(&[]))?;
    let mut warnings: Vec<String> =
        unmatched_layers.iter().map(|l| format!("layers: {l:?} matches no layer")).collect();
    warnings.extend(selection.unmatched.iter().map(|f| format!("features: {f:?} matches no layer")));
    Ok((selection, warnings))
}

/// The error for `labels` / `features` leaving nothing to draw, listing the
/// features there are (`what`: "style" or "stack").
fn empty_filter_error(present: &[&str], what: &str) -> BasemapError {
    BasemapError::InvalidLayerFilter(format!(
        "labels and features leave nothing of this {what} to draw; its features are {}",
        present.join(", ")
    ))
}

/// The style a request draws: its filters applied (an error when they leave
/// nothing to draw), with warnings for names that matched no layer.
fn select_style_layers(raw_json: &str, request: &RenderRequest) -> Result<(String, Vec<String>), BasemapError> {
    let (selection, warnings) =
        apply_filters(raw_json, request.layers.as_deref(), request.labels, request.features.as_deref())?;
    // `layers` may leave nothing (as before); `labels` / `features` emptying a
    // style that had layers is an error.
    if selection.empty && !selection.present.is_empty() {
        return Err(empty_filter_error(&selection.present, "style"));
    }
    Ok((selection.style, warnings))
}

/// One style of a stack, rendered with its filters: no result when
/// `labels` / `features` leave nothing of it, with the warnings and the
/// features it has either way.
struct StackPart {
    result: Option<RenderResult>,
    warnings: Vec<String>,
    present: Vec<&'static str>,
}

async fn render_part(request: RenderRequest) -> Result<StackPart, BasemapError> {
    let client = http_client(std::time::Duration::from_millis(request.tile_timeout_ms as u64))?;
    let raw_json = request.style_input.resolve(&client, &request_auth(&request)).await?;
    style::validate_maplibre_style(&raw_json)?;
    let (selection, warnings) =
        apply_filters(&raw_json, request.layers.as_deref(), request.labels, request.features.as_deref())?;
    if selection.empty && !selection.present.is_empty() {
        return Ok(StackPart { result: None, warnings, present: selection.present });
    }
    let mut result = render_style(request, client, selection.style).await?;
    result.warnings = warnings.clone();
    Ok(StackPart { result: Some(result), warnings, present: selection.present })
}

async fn render_async(request: RenderRequest) -> Result<RenderResult, BasemapError> {
    let client = http_client(std::time::Duration::from_millis(request.tile_timeout_ms as u64))?;

    // Resolve style once — shared across both halves for anti-meridian renders.
    let raw_json = request.style_input.resolve(&client, &request_auth(&request)).await?;
    style::validate_maplibre_style(&raw_json)?;

    let (style_json, warnings) = select_style_layers(&raw_json, &request)?;
    let mut result = render_style(request, client, style_json).await?;
    result.warnings = warnings;
    Ok(result)
}

/// Render a request with its resolved, filtered style.
async fn render_style(request: RenderRequest, client: reqwest::Client, style_json: String) -> Result<RenderResult, BasemapError> {
    // Extract tile source (may fetch TileJSON).
    let style_val: serde_json::Value = serde_json::from_str(&style_json)
        .map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
    // Filtered styles and label-only tiles go over other maps: no background fill.
    let transparent = request.layers.is_some()
        || request.labels != Labels::All
        || request.features.is_some()
        || style_val.pointer("/metadata/basemapper:transparent") == Some(&serde_json::Value::Bool(true));
    let style_base_url = match &request.style_input {
        StyleInput::Url(u) => Some(u.as_str()),
        StyleInput::InlineJson(_) => None,
    };
    let tile_source = extract_tile_source_from_style(&style_val, &client, &request_auth(&request), style_base_url).await?;

    let is_vector = matches!(tile_source, TileSource::MapboxVectorTile { .. });
    // Compute zoom for the full bbox — handles anti-meridian via compute_zoom.
    // (Clamping to the source's zoom before coarsening picks the same zoom as
    // after, and notes only the step that changed it.)
    let zoom = {
        let wanted = request
            .tile_zoom
            .unwrap_or_else(|| bbox::raster_tile_zoom(request.bbox, request.width, raster_tile_size(&style_val)));
        let available = tile_source.maxzoom().map_or(wanted, |mz| wanted.min(mz));
        let z = match request.tile_zoom {
            Some(_) => available,
            None => coarsen_to_fit(request.bbox, available, 0, request.max_tiles),
        };
        if !is_vector {
            note_overzoomed(wanted, available);
            note_coarsened(available, z, request.max_tiles);
        }
        z
    };

    // Raster sources: tiles on this zoom's grid (handles anti-meridian
    // bboxes). The vector path checks each source's own tiles as it
    // fetches them (fetch_vector_inputs).
    if !is_vector {
        check_max_tiles(bbox::tile_count(request.bbox, zoom), request.max_tiles)?;
    }

    let [xmin, _ymin, xmax, _ymax] = request.bbox;
    let half = 20_037_508.342_789_244;
    let wraps = xmin < xmax && (xmin < -half || xmax > half);
    if xmin > xmax {
        render_antimeridian_split(&request, &client, &style_json, &tile_source, transparent, zoom)
            .await
    } else if wraps && !is_vector {
        // Raster sources: express a view past ±180° as an anti-meridian bbox.
        // (Vector sources draw the wrapped world copy directly.)
        let mut req = request.clone();
        if req.bbox[0] < -half {
            req.bbox[0] += 2.0 * half;
        }
        if req.bbox[2] > half {
            req.bbox[2] -= 2.0 * half;
        }
        render_antimeridian_split(&req, &client, &style_json, &tile_source, transparent, zoom).await
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

/// The most detailed tile zoom from `zoom` down to `min` whose tiles for
/// `bbox` number at most `max_tiles` (`min` when none does).
fn coarsen_to_fit(bbox: [f64; 4], zoom: u8, min: u8, max_tiles: u32) -> u8 {
    let mut z = zoom;
    while z > min && bbox::tile_count(bbox, z) > max_tiles {
        z -= 1;
    }
    z
}

/// Note raster tiles drawn coarser than the view's zoom to stay within
/// `max_tiles`.
fn note_coarsened(wanted: u8, used: u8, max_tiles: u32) {
    if used < wanted {
        notes::report(
            Level::Adjusted,
            "tiles_coarsened",
            format!(
                "the view needs more than max_tiles ({max_tiles}) raster tiles at zoom {wanted}: drawn with coarser zoom-{used} tiles"
            ),
        );
    }
}

/// Note raster tiles enlarged past the source's most detailed zoom.
fn note_overzoomed(wanted: u8, available: u8) {
    if available < wanted {
        notes::report(
            Level::Adjusted,
            "tiles_overzoomed",
            format!(
                "the view wants zoom-{wanted} raster tiles; the source's most detailed are zoom {available}, enlarged (less sharp)"
            ),
        );
    }
}

/// Error when a source needs more than `limit` tiles for the view.
fn check_max_tiles(requested: u32, limit: u32) -> Result<(), BasemapError> {
    if requested > limit {
        return Err(BasemapError::MaxTilesExceeded { requested, limit });
    }
    Ok(())
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

    // Vector tiles: in-house CPU renderer (core/src/vector/).
    if matches!(tile_source, TileSource::MapboxVectorTile { .. }) {
        // MapLibre convention: style evaluated at the true scale zoom; vector
        // tiles (512 px) fetched at floor(scale zoom), clamped to the source
        // maxzoom. An explicit request zoom is on the 256 px raster grid, which
        // is one level above the equivalent 512 px tile zoom.
        let (style_val, placed, assets, tz) =
            fetch_vector_inputs(request, client, style_json, tile_source, bbox, width).await?;
        let out = vector::render::render(&style_val, &placed, &assets, bbox, width, height, request.pixel_ratio)
            .map_err(BasemapError::RenderError)?;
        if !out.skipped_layer_types.is_empty() {
            notes::report(
                Level::Problem,
                "layer_types_unsupported",
                format!("these layer types are not drawn yet: {}", out.skipped_layer_types.join(", ")),
            );
        }
        return Ok(RenderResult {
            bounds,
            zoom_used: tz.saturating_add(1),
            warnings: Vec::new(),
            width: out.width,
            height: out.height,
            pixels: out.pixels,
        });
    }
    // Raster sources: tiles chosen for the CSS size, composited at device size.
    let (width, height) = (
        vector::render::device_px(width, request.pixel_ratio),
        vector::render::device_px(height, request.pixel_ratio),
    );
    // PMTiles: fetch via HTTP range requests.
    if let TileSource::PMTiles { ref url, .. } = tile_source {
        let coords = tile_fetcher::build_tile_coords(bbox, zoom);
        let tiles =
            fetch_pmtiles_tiles(url, &coords, client, request.tile_concurrency, request.fail_on_tile_error).await?;
        let canvas =
            renderer::composite_raster_tiles(&tiles, bbox, zoom, width, height, transparent, request.pixel_ratio)?;
        return Ok(RenderResult {
            bounds,
            zoom_used: zoom,
            warnings: Vec::new(),
            width,
            height,
            pixels: canvas.into_raw(),
        });
    }

    // Raster XYZ (also WMS, through {bbox-epsg-3857}).
    let coords = tile_fetcher::build_tile_coords(bbox, zoom);
    let urls = tile_fetcher::build_tile_urls(tile_source, &coords, request.pixel_ratio);
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
        renderer::composite_raster_tiles(&tiles, bbox, zoom, width, height, transparent, request.pixel_ratio)?;
    Ok(RenderResult {
        bounds,
        zoom_used: zoom,
        warnings: Vec::new(),
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

    let (west, east) = tokio::join!(
        render_single_bbox(request, client, style_json, tile_source, transparent, west_bbox, west_px, request.height, zoom),
        render_single_bbox(request, client, style_json, tile_source, transparent, east_bbox, east_px, request.height, zoom),
    );
    let (west, east) = (west?, east?);

    // Stitch row by row: west pixels then east pixels per scanline.
    let row_w = west.width as usize * 4;
    let row_e = east.width as usize * 4;
    let row_out = row_w + row_e;
    let rows = west.height.min(east.height) as usize;
    let mut pixels = vec![0u8; row_out * rows];
    for row in 0..rows {
        let out = &mut pixels[row * row_out..][..row_out];
        out[..row_w].copy_from_slice(&west.pixels[row * row_w..][..row_w]);
        out[row_w..].copy_from_slice(&east.pixels[row * row_e..][..row_e]);
    }

    Ok(RenderResult {
        bounds: SpatialBounds { xmin, ymin, xmax, ymax, crs_epsg: 3857 },
        zoom_used: zoom,
        warnings: Vec::new(),
        width: west.width + east.width,
        height: rows as u32,
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
    auth: &net::Auth,
    style_base_url: Option<&str>,
) -> Result<TileSource, BasemapError> {
    let sources = style
        .get("sources")
        .and_then(|s| s.as_object())
        .ok_or_else(|| BasemapError::StyleParseError("missing \"sources\" object".into()))?;
    static GEOJSON_ONLY: Lazy<serde_json::Value> = Lazy::new(|| serde_json::json!({"type": "vector", "tiles": [""], "maxzoom": 22}));

    // Prefer vector sources over raster; prevents multi-source styles from
    // routing through the raster pipeline when a vector source is present.
    let vector_entry = sources.values().find_map(|s| {
        if s.get("type").and_then(|t| t.as_str()) == Some("vector") {
            Some(("vector", s))
        } else {
            None
        }
    });
    // Sources only the vector path draws (GeoJSON, DEM for hillshade): such
    // styles render through it even without a vector source.
    let needs_vector_path = sources.values().any(|s| matches!(s["type"].as_str(), Some("geojson" | "raster-dem")));
    let (src_type, source) = if let Some(entry) = vector_entry {
        entry
    } else if needs_vector_path {
        ("vector", &*GEOJSON_ONLY)
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
            .or_else(|| {
                // GeoJSON-only (or source-less) styles render through the
                // vector path, which reads every source the layers use.
                (sources.is_empty() || sources.values().any(|s| s["type"] == "geojson"))
                    .then(|| ("vector", &*GEOJSON_ONLY))
            })
            .ok_or_else(|| {
                BasemapError::StyleParseError("no raster or vector tile source found".into())
            })?
    };

    let resolved = resolve_source(source, src_type, client, auth, style_base_url).await?.source;
    Ok(match resolved {
        // A vector archive: the vector path, which fetches every source
        // itself (from the archive), with the archive's max zoom.
        TileSource::PMTiles { maxzoom, .. } if src_type == "vector" => {
            TileSource::MapboxVectorTile { urls: TileUrls::default(), api_key: None, maxzoom }
        }
        other => other,
    })
}

/// A style source resolved to its tile URL template (inline `tiles` or
/// TileJSON), with the zoom range and tile size MapLibre uses for it.
struct ResolvedSource {
    source: TileSource,
    minzoom: u8,
    /// Raster tile size in px (MapLibre's default for raster sources: 512).
    tile_size: f64,
}

/// The URL a TileJSON's relative `tiles` resolve against. An ArcGIS
/// service's JSON (`.../VectorTileServer`) lists them relative to the
/// service itself (`tile/{z}/{y}/{x}.pbf`), as if its URL ended in `/`.
fn tilejson_base(url: &str) -> String {
    let (path, query) = url.split_once('?').map_or((url, None), |(p, q)| (p, Some(q)));
    let lower = path.to_ascii_lowercase();
    if lower.ends_with("/vectortileserver") || lower.ends_with("/mapserver") {
        match query {
            Some(q) => format!("{path}/?{q}"),
            None => format!("{path}/"),
        }
    } else {
        url.to_owned()
    }
}

async fn resolve_source(
    source: &serde_json::Value,
    src_type: &str,
    client: &reqwest::Client,
    auth: &net::Auth,
    style_base_url: Option<&str>,
) -> Result<ResolvedSource, BasemapError> {
    let num = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_f64());
    let tile_list = |v: &serde_json::Value| -> Vec<String> {
        v.get("tiles")
            .and_then(|t| t.as_array())
            .map(|a| a.iter().filter_map(|u| u.as_str().map(str::to_owned)).collect())
            .unwrap_or_default()
    };
    let mut minzoom = num(source, "minzoom");
    let mut tile_size = num(source, "tileSize");
    // `scheme` from the source, else from its TileJSON (as MapLibre's
    // loadTileJson, where explicit source options take precedence).
    let mut scheme = source.get("scheme").and_then(|v| v.as_str()).map(str::to_owned);
    // The scope the tiles' token is checked against: widened to the tile
    // hosts of a TileJSON that came from a trusted host.
    let mut tile_auth = auth.clone();
    // Resolve the tile URL templates and maxzoom.
    let (tiles, source_maxzoom): (Vec<String>, Option<u8>) = if !tile_list(source).is_empty() {
        let mz = source
            .get("maxzoom")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(22) as u8);
        (tile_list(source), mz)
    } else if let Some(archive) = source.get("url").and_then(|u| u.as_str()).and_then(|u| u.strip_prefix("pmtiles://")) {
        // The pmtiles protocol's TileJSON: zoom range from the archive
        // header; the source's own minzoom / maxzoom take precedence.
        let url = resolve_url(archive, style_base_url);
        check_archive_origin(&url, style_base_url)?;
        let reader = open_pmtiles(&url, client).await?;
        let header = reader.get_header();
        minzoom = minzoom.or(Some(header.min_zoom as f64));
        let mz = source
            .get("maxzoom")
            .and_then(|v| v.as_u64())
            .map_or(header.max_zoom, |v| v as u8)
            .min(22);
        (vec![format!("pmtiles://{url}")], Some(mz))
    } else if let Some(tilejson_url_raw) = source.get("url").and_then(|u| u.as_str()) {
        let tilejson_url = resolve_url(tilejson_url_raw, style_base_url);
        let body = net::get(client, &tilejson_url, auth, net::limit::JSON).await.map_err(|e| match e {
            net::FetchError::Status(status) => BasemapError::StyleFetchFailed { url: tilejson_url.clone(), status },
            net::FetchError::Network(_) => BasemapError::StyleFetchFailed { url: tilejson_url.clone(), status: 0 },
        })?;
        let tilejson: serde_json::Value = serde_json::from_slice(&body.body).map_err(|_| {
            BasemapError::StyleParseError("could not parse TileJSON response".into())
        })?;
        let base = tilejson_base(&tilejson_url);
        let tiles: Vec<String> = tile_list(&tilejson).iter().map(|t| resolve_url(t, Some(&base))).collect();
        if auth.token_for(&tilejson_url).is_some() {
            tile_auth = auth.with_hosts(tiles.iter().map(String::as_str));
        }
        if tiles.is_empty() {
            return Err(BasemapError::StyleParseError("TileJSON missing \"tiles\" array".into()));
        }
        let mz = source
            .get("maxzoom")
            .or_else(|| tilejson.get("maxzoom"))
            .and_then(|v| v.as_u64())
            .map(|v| v.min(22) as u8);
        minzoom = minzoom.or_else(|| num(&tilejson, "minzoom"));
        tile_size = tile_size.or_else(|| num(&tilejson, "tileSize"));
        scheme = scheme.or_else(|| tilejson.get("scheme").and_then(|v| v.as_str()).map(str::to_owned));
        (tiles, mz)
    } else {
        return Err(BasemapError::StyleParseError(
            "source has neither \"tiles\" array nor \"url\" TileJSON reference".into(),
        ));
    };

    let urls = TileUrls { templates: tiles, tms: scheme.as_deref() == Some("tms") };
    // The token goes with the tiles only when every tile host may have it.
    let token = if tile_auth.allows_all(urls.templates.iter().map(String::as_str)) { tile_auth.token() } else { None };
    let source = if let Some(archive) = urls.templates.first().and_then(|t| t.strip_prefix("pmtiles://")) {
        check_archive_origin(archive, style_base_url)?;
        TileSource::PMTiles { url: archive.to_owned(), maxzoom: source_maxzoom }
    } else if src_type == "vector" {
        TileSource::MapboxVectorTile { urls, api_key: token.map(str::to_owned), maxzoom: source_maxzoom }
    } else {
        TileSource::XyzRaster { urls, auth_header: token.map(|t| format!("Bearer {t}")), maxzoom: source_maxzoom }
    };
    Ok(ResolvedSource {
        source,
        minzoom: minzoom.unwrap_or(0.0).clamp(0.0, 22.0) as u8,
        tile_size: tile_size.unwrap_or(512.0),
    })
}

/// A style downloaded from the web may name only remote archives: a local
/// path or `file://` URL in it would let any web page read the user's files.
fn check_archive_origin(archive: &str, style_base_url: Option<&str>) -> Result<(), BasemapError> {
    let remote_style = style_base_url.is_some_and(|b| b.starts_with("http://") || b.starts_with("https://"));
    let remote_archive = archive.starts_with("http://") || archive.starts_with("https://");
    if remote_style && !remote_archive {
        return Err(BasemapError::StyleParseError(format!(
            "a style loaded from the web may not read local archives ({archive})"
        )));
    }
    Ok(())
}

/// Fetch tiles from a PMTiles archive over HTTP using range requests.
///
/// Opens the archive (header + root directory via two range requests), then
/// fetches each requested tile, `concurrency` at a time. Tiles absent from
/// the archive (sparse coverage) are skipped; tiles that fail to load are
/// skipped too, unless `fail_on_tile_error`.
async fn fetch_pmtiles_tiles(
    url: &str,
    coords: &[tile_fetcher::TileCoord],
    client: &reqwest::Client,
    concurrency: u32,
    fail_on_tile_error: bool,
) -> Result<Vec<(tile_fetcher::TileCoord, tile_fetcher::TileData)>, BasemapError> {
    let reader = std::sync::Arc::new(open_pmtiles(url, client).await?);
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency.max(1) as usize));
    // Tiles are fetched concurrently and decompressed (tile_compression).
    let mut set = tokio::task::JoinSet::new();
    for (i, &coord) in coords.iter().enumerate() {
        let pm_coord = pmtiles::TileCoord::new(coord.z, coord.x, coord.y)
            .map_err(|e| BasemapError::RenderError(format!("PMTiles coord error: {e}")))?;
        let reader = reader.clone();
        let semaphore = semaphore.clone();
        set.spawn(notes::propagate(async move {
            let _permit = semaphore.acquire_owned().await;
            (i, coord, reader.get_tile_decompressed(pm_coord).await)
        }));
    }
    let mut results = Vec::new();
    while let Some(joined) = set.join_next().await {
        let (i, coord, res) = joined.map_err(|e| BasemapError::RenderError(format!("PMTiles fetch task failed: {e}")))?;
        match res {
            Ok(Some(bytes)) => results.push((i, coord, bytes.to_vec())),
            Ok(None) => {}
            Err(e) => {
                let msg = format!("PMTiles tile {}/{}/{} from {}: {e}", coord.z, coord.x, coord.y, net::redact(url));
                if fail_on_tile_error {
                    return Err(BasemapError::TileDecodeError(msg));
                }
                notes::report(Level::Problem, "tile_skipped", format!("{msg} (skipped)"));
            }
        }
    }
    results.sort_by_key(|r| r.0);
    Ok(results.into_iter().map(|(_, c, d)| (c, d)).collect())
}

/// Open a PMTiles archive over HTTP (header and root directory).
pub(crate) async fn open_pmtiles(
    url: &str,
    client: &reqwest::Client,
) -> Result<pmtiles::AsyncPmTilesReader<tile_fetcher::ArchiveBackend>, BasemapError> {
    let backend = tile_fetcher::ArchiveBackend::new(url.to_owned(), client.clone());
    pmtiles::AsyncPmTilesReader::try_from_source(backend)
        .await
        .map_err(|e| BasemapError::RenderError(format!("PMTiles open failed ({}): {e}", net::redact(url))))
}

/// Resolve a potentially-relative URL against a base URL.
///
/// `reqwest::Url::join` percent-encodes `{` and `}` in tile template
/// placeholders, so we decode them back after joining.
pub(crate) fn resolve_url(url: &str, base: Option<&str>) -> String {
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

/// `tileSize` of the style's first raster source (MapLibre's default: 512).
fn raster_tile_size(style: &serde_json::Value) -> f64 {
    style["sources"]
        .as_object()
        .and_then(|s| s.values().find(|v| v["type"] == "raster"))
        .and_then(|v| v["tileSize"].as_f64())
        .unwrap_or(512.0)
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
    // Decoded once for the whole render: the renderer gets the same tiles
    // from the shared cache.
    let bytes: Vec<&[u8]> = tiles.iter().map(|(_, b)| b.as_slice()).collect();
    let decoded: Vec<std::sync::Arc<vector::mvt::Tile>> =
        vector::mvt::decode_shared(&bytes).into_iter().filter_map(Result::ok).collect();
    let mut needed: Vec<(String, u32)> =
        vector::symbol::required_glyph_ranges(style, decoded.iter().map(|t| t.as_ref()), zoom).into_iter().collect();
    needed.sort();
    let mut set = tokio::task::JoinSet::new();
    for (stack, start) in needed {
        let url = template
            .replace("{fontstack}", &stack)
            .replace("{range}", &format!("{}-{}", start, start + 255));
        let client = client.clone();
        set.spawn(notes::propagate(async move {
            match net::get(&client, &url, &net::Auth::none(), net::limit::ASSET).await {
                Ok(f) => Some((stack, f.body)),
                Err(e) => {
                    notes::report(Level::Problem, "glyphs_missing", format!("glyphs {}: {e}", net::redact(&url)));
                    None
                }
            }
        }));
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
            notes::report(Level::Problem, "glyphs_missing", format!("glyphs for {stack}: {e}"));
        }
    }
    map
}

/// Fetch the style's 1x sprite (`<sprite>.json` + `<sprite>.png`). A missing
/// or broken sprite only disables icons.
/// The style's sprite; above pixel ratio 1 the `@2x` sheet (as MapLibre
/// loads it), falling back to 1x. Multi-sprite styles (`[{id, url}, ...]`)
/// load every sheet; images of sprites other than `default` are named
/// `id:name`, as MapLibre addresses them.
async fn fetch_sprite(
    client: &reqwest::Client,
    style: &serde_json::Value,
    style_base_url: Option<&str>,
    pixel_ratio: f64,
) -> vector::symbol::Sprite {
    let entries: Vec<(String, String)> = match &style["sprite"] {
        serde_json::Value::String(u) => vec![("default".to_owned(), u.clone())],
        serde_json::Value::Array(list) => list
            .iter()
            .filter_map(|e| Some((e["id"].as_str()?.to_owned(), e["url"].as_str()?.to_owned())))
            .collect(),
        _ => Vec::new(),
    };
    let mut sprite = vector::symbol::Sprite::default();
    for (id, url) in entries {
        let base = resolve_url(&url, style_base_url);
        let get = |url: String| {
            let client = client.clone();
            async move {
                net::get(&client, &url, &net::Auth::none(), net::limit::ASSET)
                    .await
                    .map_err(|e| notes::report(Level::Problem, "sprite_missing", format!("sprite {}: {e}", net::redact(&url))))
                    .ok()
                    .map(|f| f.body)
            }
        };
        let mut fetched = None;
        if pixel_ratio > 1.0 {
            let (json, png) = tokio::join!(get(format!("{base}@2x.json")), get(format!("{base}@2x.png")));
            if let (Some(j), Some(p)) = (json, png) {
                fetched = Some((j, p));
            }
        }
        if fetched.is_none() {
            let (json, png) = tokio::join!(get(format!("{base}.json")), get(format!("{base}.png")));
            if let (Some(j), Some(p)) = (json, png) {
                fetched = Some((j, p));
            }
        }
        let Some((j, p)) = fetched else { continue };
        match vector::symbol::Sprite::from_json_png(&j, &p) {
            Ok(sheet) => {
                for (name, img) in sheet.images {
                    let key = if id == "default" { name } else { format!("{id}:{name}") };
                    sprite.images.insert(key, img);
                }
            }
            Err(e) => notes::report(Level::Problem, "sprite_missing", e.to_string()),
        }
    }
    sprite.device_pixel_ratio = pixel_ratio;
    sprite
}

/// Tiles for every tile source the style's layers use, plus glyphs and
/// sprite, fetched as MapLibre would: the style is evaluated at the true
/// scale zoom; vector sources (512 px tiles) at floor(zoom), raster sources
/// at round(zoom + log2(512 / tileSize)), each clamped to its zoom range.
/// `primary` is the style's main vector source: its tile zoom is returned.
/// An explicit request zoom is on the 256 px raster grid, one level above
/// the equivalent 512 px vector tile zoom.
#[allow(clippy::type_complexity)]
async fn fetch_vector_inputs(
    request: &RenderRequest,
    client: &reqwest::Client,
    style_json: &str,
    primary: &TileSource,
    bbox: [f64; 4],
    width: u32,
) -> Result<(serde_json::Value, vector::render::SourceTiles, vector::symbol::SymbolAssets, u8), BasemapError> {
    let style_val: serde_json::Value =
        serde_json::from_str(style_json).map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
    // 3D terrain is out of scope for flat basemaps (with a straight-down
    // camera MapLibre's terrain only resamples and displaces the map).
    if style_val.get("terrain").is_some_and(|t| !t.is_null()) {
        notes::report(Level::Adjusted, "terrain_unsupported", "style terrain is not supported; the map is drawn flat");
    }
    let sz = vector::render::scale_zoom(bbox, width);
    let vector_tz = |maxzoom: Option<u8>, minzoom: u8| -> Option<u8> {
        let tz = match request.tile_zoom {
            Some(z) => z.saturating_sub(1),
            None => vector::render::tile_zoom(sz, None),
        };
        let tz = maxzoom.map_or(tz, |m| tz.min(m));
        (tz >= minzoom).then_some(tz)
    };
    let primary_tz = vector_tz(primary.maxzoom(), 0).unwrap_or(0);

    // Sources used by layers visible at this zoom, in style order.
    let mut used: Vec<String> = Vec::new();
    for l in style_val["layers"].as_array().map(|v| v.as_slice()).unwrap_or(&[]) {
        if !vector::symbol::layer_visible(l, sz) {
            continue;
        }
        if let Some(src) = l["source"].as_str() {
            if !used.iter().any(|u| u == src) {
                used.push(src.to_owned());
            }
        }
    }
    // The sprite does not depend on the tiles: it downloads meanwhile.
    let tiles = async {
        let mut sources = vector::render::SourceTiles::default();
        let mut all_vector: Vec<(tile_fetcher::TileCoord, tile_fetcher::TileData)> = Vec::new();
        for id in used {
            let src = &style_val["sources"][&id];
            let kind = src["type"].as_str().unwrap_or("");
            if kind == "geojson" {
                if src["cluster"].as_bool() == Some(true) {
                    notes::report(
                        Level::Adjusted,
                        "clustering_unsupported",
                        format!("source {id}: GeoJSON clustering is not supported; drawing unclustered features"),
                    );
                }
                let data = match &src["data"] {
                    serde_json::Value::String(url) => {
                        let url = resolve_url(url, style_base_url_of(request));
                        let shown = net::redact(&url);
                        match net::get(client, &url, &net::Auth::none(), net::limit::GEOJSON).await {
                            Ok(f) => serde_json::from_slice(&f.body).unwrap_or_default(),
                            Err(e) => {
                                notes::report(Level::Problem, "geojson_failed", format!("source {id}: GeoJSON {shown}: {e}"));
                                continue;
                            }
                        }
                    }
                    other => other.clone(),
                };
                let opts = vector::geojson::GeoJsonOptions::from_source(src);
                let index = vector::geojson::GeoJsonIndex::new(&data, opts);
                let tz = match vector_tz(Some(opts.max_zoom), 0) {
                    Some(z) => z,
                    None => continue,
                };
                let tiles: Vec<_> = vector::render::covering_tiles(bbox, tz)
                    .into_iter()
                    .map(|(c, wrap)| (c, wrap, index.tile(c.z, c.x, c.y)))
                    .collect();
                all_vector.extend(tiles.iter().map(|(c, _, b)| (*c, b.clone())));
                sources.vector.insert(id, tiles);
                continue;
            }
            if kind != "vector" && kind != "raster" && kind != "raster-dem" {
                if !kind.is_empty() {
                    notes::report(Level::Problem, "source_unsupported", format!("source {id}: type {kind} is not supported yet"));
                }
                continue;
            }
            let resolved = resolve_source(src, kind, client, &request_auth(request), style_base_url_of(request)).await?;
            let source = &resolved.source;
            let fetch = |coords: Vec<tile_fetcher::TileCoord>| async move {
                if let TileSource::PMTiles { url, .. } = source {
                    return fetch_pmtiles_tiles(url, &coords, client, request.tile_concurrency, request.fail_on_tile_error)
                        .await;
                }
                let urls = tile_fetcher::build_tile_urls(source, &coords, request.pixel_ratio);
                tile_fetcher::fetch_all_tiles(
                    client,
                    urls,
                    source,
                    request.tile_timeout_ms,
                    request.tile_concurrency,
                    request.fail_on_tile_error,
                )
                .await
            };
            if kind == "vector" {
                let Some(tz) = vector_tz(resolved.source.maxzoom(), resolved.minzoom) else { continue };
                let covering = vector::render::covering_tiles(bbox, tz);
                let mut coords: Vec<tile_fetcher::TileCoord> = covering.iter().map(|c| c.0).collect();
                coords.sort_by_key(|c| (c.z, c.x, c.y));
                coords.dedup();
                check_max_tiles(coords.len().try_into().unwrap_or(u32::MAX), request.max_tiles)?;
                let tiles = fetch(coords).await?;
                let by_coord: std::collections::HashMap<(u8, u32, u32), &tile_fetcher::TileData> =
                    tiles.iter().map(|(c, d)| ((c.z, c.x, c.y), d)).collect();
                let placed: Vec<_> = covering
                    .iter()
                    .filter_map(|(c, wrap)| by_coord.get(&(c.z, c.x, c.y)).map(|d| (*c, *wrap, (*d).clone())))
                    .collect();
                all_vector.extend(tiles);
                sources.vector.insert(id, placed);
            } else {
                let wanted = bbox::raster_tile_zoom(bbox, width, resolved.tile_size);
                let fit = resolved.source.maxzoom().map_or(wanted, |m| wanted.min(m));
                note_overzoomed(wanted, fit);
                // As raster basemaps: coarser tiles rather than more than max_tiles.
                let z = coarsen_to_fit(bbox, fit, resolved.minzoom, request.max_tiles);
                note_coarsened(fit, z, request.max_tiles);
                if z < resolved.minzoom {
                    continue;
                }
                let coords = tile_fetcher::build_tile_coords(bbox, z);
                check_max_tiles(coords.len().try_into().unwrap_or(u32::MAX), request.max_tiles)?;
                let tiles = fetch(coords).await?;
                if kind == "raster-dem" {
                    let unpack = vector::hillshade::unpack_vector(src);
                    sources.dem.insert(id, vector::hillshade::DemTiles { zoom: z, tiles, unpack });
                } else {
                    sources.raster.insert(id, vector::render::RasterTiles { zoom: z, tiles });
                }
            }
        }
        Ok::<_, BasemapError>((sources, all_vector))
    };
    let (tiles, sprite) =
        tokio::join!(tiles, fetch_sprite(client, &style_val, style_base_url_of(request), request.pixel_ratio));
    let (sources, all_vector) = tiles?;
    let assets = vector::symbol::SymbolAssets {
        glyphs: fetch_glyphs(client, &style_val, style_base_url_of(request), &all_vector, sz).await,
        sprite,
    };
    Ok((style_val, sources, assets, primary_tz))
}

// ──────────────────────────────────────────────────────────────────────────────
// SVG output
// ──────────────────────────────────────────────────────────────────────────────

/// Render a vector style as an SVG document (see [`render_svg_stack`]).
#[doc(hidden)]
pub fn render_svg(request: RenderRequest) -> Result<String, BasemapError> {
    render_svg_stack(vec![request])
}

/// [`render_svg`] with options (see [`render_svg_stack_with`]).
#[doc(hidden)]
pub fn render_svg_with(request: RenderRequest, options: &SvgOptions) -> Result<String, BasemapError> {
    render_svg_stack_with(vec![request], options)
}

/// Render several styles over the same view as one SVG document, bottom to
/// top, each style in its own group (e.g. a basemap built from separate
/// services). Fills, lines and circles are vector paths; labels are glyph
/// outlines traced from the style's SDF glyphs; non-SDF icons are embedded
/// sprite images. All requests must share bbox, width and height, and use
/// vector tile sources.
#[doc(hidden)]
pub fn render_svg_stack(requests: Vec<RenderRequest>) -> Result<String, BasemapError> {
    render_svg_stack_with(requests, &SvgOptions::default())
}

/// [`render_svg_stack`] with options: label text as editable `<text>` with
/// embedded fonts (default) or as traced glyph outlines.
#[doc(hidden)]
pub fn render_svg_stack_with(requests: Vec<RenderRequest>, options: &SvgOptions) -> Result<String, BasemapError> {
    render_svg_document(requests, options, None)
}

/// [`render_svg_stack_with`], clipped to `clip` (polygons in Web Mercator):
/// an SVG `<clipPath>` around the whole map.
pub(crate) fn render_svg_document(
    requests: Vec<RenderRequest>,
    options: &SvgOptions,
    clip: Option<&mask::Shape>,
) -> Result<String, BasemapError> {
    use std::fmt::Write as _;
    let requests = expand_stacked(requests);
    let first = requests.first().ok_or_else(|| BasemapError::RenderError("no styles to render".into()))?;
    let (bbox, width, height) = (first.bbox, first.width, first.height);
    if bbox[0] > bbox[2] {
        return Err(BasemapError::InvalidBbox("SVG output does not support anti-meridian (xmin > xmax) bboxes".into()));
    }
    for request in &requests {
        if request.bbox != bbox || request.width != width || request.height != height {
            return Err(BasemapError::RenderError("all styles in an SVG stack must share bbox and size".into()));
        }
        request.validate()?;
    }
    let timeout = requests.iter().map(|r| r.tile_timeout_ms).max().unwrap_or(10_000);
    RUNTIME.block_on(async {
        let client = http_client(std::time::Duration::from_millis(timeout.max(15_000) as u64))?;
        // Every style's tiles and assets first, so the fonts of the whole
        // document are resolved (and embedded) once.
        // As render_stack: filters apply to each group (style) as one map.
        let stacked = requests.len() > 1;
        let mut groups: Vec<(u32, bool, Vec<&'static str>)> = Vec::new();
        let mut inputs = Vec::new();
        for request in &requests {
            let raw_json = request.style_input.resolve(&client, &request_auth(request)).await?;
            style::validate_maplibre_style(&raw_json)?;
            let style_json = if stacked {
                let (selection, _) =
                    apply_filters(&raw_json, request.layers.as_deref(), request.labels, request.features.as_deref())?;
                let g = match groups.iter().position(|g| g.0 == request.group) {
                    Some(i) => i,
                    None => {
                        groups.push((request.group, false, Vec::new()));
                        groups.len() - 1
                    }
                };
                for f in &selection.present {
                    if !groups[g].2.contains(f) {
                        groups[g].2.push(f);
                    }
                }
                if selection.empty && !selection.present.is_empty() {
                    continue;
                }
                groups[g].1 = true;
                selection.style
            } else {
                select_style_layers(&raw_json, request)?.0
            };
            let style_val: serde_json::Value =
                serde_json::from_str(&style_json).map_err(|e| BasemapError::StyleParseError(e.to_string()))?;
            let tile_source =
                extract_tile_source_from_style(&style_val, &client, &request_auth(request), style_base_url_of(request)).await?;
            // Raster-only styles take the same path: raster layers are
            // embedded as images (vector layers are paths and text).
            let (style_val, sources, assets, _) =
                fetch_vector_inputs(request, &client, &style_json, &tile_source, bbox, width).await?;
            inputs.push((style_val, sources, assets));
        }
        if let Some(g) = groups.iter().find(|g| !g.1) {
            let what = if groups.len() == 1 { "stack" } else { "style of the stack" };
            return Err(empty_filter_error(&g.2, what));
        }
        let mut fonts = if options.text == SvgText::Editable {
            let mut stacks: Vec<String> = inputs.iter().flat_map(|i| i.2.glyphs.stacks.keys().cloned()).collect();
            stacks.sort();
            stacks.dedup();
            Some(vector::fonts::resolve(&client, &stacks, options).await)
        } else {
            None
        };
        let mut defs = String::new();
        let mut body = String::new();
        for (k, (style_val, sources, assets)) in inputs.iter().enumerate() {
            let parts = vector::render::render_svg(style_val, sources, assets, bbox, width, height, &format!("s{k}_"), options.text, fonts.as_ref())
                .map_err(BasemapError::RenderError)?;
            if !parts.skipped_layer_types.is_empty() {
                notes::report(
                    Level::Problem,
                    "layer_types_unsupported",
                    format!("SVG output does not draw these layer types yet: {}", parts.skipped_layer_types.join(", ")),
                );
            }
            defs.push_str(&parts.defs);
            let _ = write!(body, r#"<g id="style{k}">{}</g>"#, parts.body);
        }
        if let Some(f) = fonts.as_mut() {
            f.finish(&client, options).await;
            let css = f.font_face_css(options.embed_fonts);
            if !css.is_empty() {
                defs = format!("<style>{css}</style>{defs}");
            }
        }
        if let Some(shape) = clip {
            let polygons = shape.to_pixels(bbox, width as f64, height as f64, true);
            defs.push_str(&mask::svg_clip_path("basemapper-clip", &polygons, width as f64, height as f64, shape.keep));
            body = format!(r#"<g clip-path="url(#basemapper-clip)">{body}</g>"#);
        }
        Ok(format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" "#,
                r#"width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
                r#"<defs>{defs}</defs>{body}</svg>"#
            ),
            w = width,
            h = height,
            defs = defs,
            body = body
        ))
    })
}
