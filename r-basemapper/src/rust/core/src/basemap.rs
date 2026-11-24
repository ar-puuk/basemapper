//! `Basemap`: what to draw and how to fetch it, the Rust counterpart of R's
//! and Python's `basemap()` (docs/design/api-review.md §4.1, D9).
//!
//! A basemap is a value: styles, the view inputs (any of extent, zoom,
//! image size, dpi), fetch options and an optional clip. Rendering it in a
//! [`Context`] resolves the view ([`crate::view::resolve`]) and returns a
//! [`Rendering`]:
//!
//! ```no_run
//! use basemapper_core::{Basemap, Context};
//! let r = Basemap::new("OpenFreeMap.Liberty").center(-111.89, 40.76).zoom(12.0).render(&Context::Standalone)?;
//! std::fs::write("map.png", r.to_png()?)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! A map in another CRS is two steps, because its mesh depends on the image
//! size: [`Basemap::view`], then [`Basemap::render_view`] with a [`Warp`].

use crate::bbox::RenderRequest;
use crate::fetch::{FetchOptions, OnError};
use crate::mask::Shape;
use crate::notes::Level;
use crate::view::{self, Context, Units, View, ViewInputs, CSS_DPI};
use crate::warp::{self, Image, Mesh, Warp};
use crate::{BasemapError, Rendering, Style, SvgOptions};

/// The CRS of bounds given to [`Basemap::bounds`]: the core reads
/// longitude / latitude and Web Mercator (R and Python transform others).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Crs {
    /// EPSG:4326 longitude / latitude degrees.
    LonLat,
    /// EPSG:3857 Web Mercator metres.
    WebMercator,
}

impl Crs {
    /// `"EPSG:4326"` / `4326` / `"WGS84"` or `"EPSG:3857"` / `3857`.
    pub fn parse(s: &str) -> Result<Crs, BasemapError> {
        match s.trim().to_ascii_uppercase().as_str() {
            "EPSG:4326" | "4326" | "WGS84" | "CRS84" | "OGC:CRS84" => Ok(Crs::LonLat),
            "EPSG:3857" | "3857" | "EPSG:900913" => Ok(Crs::WebMercator),
            other => Err(BasemapError::InvalidInput(format!(
                "unsupported crs '{other}': the Rust core reads EPSG:4326 or EPSG:3857"
            ))),
        }
    }
}

/// A basemap: one style, or several drawn bottom to top, with what is given
/// of its view, how to fetch it, and an optional clip.
#[derive(Clone, Debug)]
pub struct Basemap {
    styles: Vec<Style>,
    inputs: ViewInputs,
    fetch: FetchOptions,
    clip: Option<Shape>,
}

impl Basemap {
    /// A basemap of one style (a [`Style`], or a string: a catalogue name,
    /// a style URL or JSON, tile data), its view all derived or defaulted.
    pub fn new(style: impl Into<Style>) -> Self {
        Self::stack(vec![style.into()])
    }

    /// Several styles drawn bottom to top, each with its own filters.
    pub fn stack(styles: Vec<Style>) -> Self {
        Basemap { styles, inputs: ViewInputs::new(), fetch: FetchOptions::default(), clip: None }
    }

    // ── What is given of the view ───────────────────────────────────────────

    /// The area to show, `[xmin, ymin, xmax, ymax]` in `crs` (west > east
    /// across the date line).
    pub fn bounds(mut self, bounds: [f64; 4], crs: Crs) -> Self {
        let b = match crs {
            Crs::WebMercator => bounds,
            Crs::LonLat => {
                let (x0, y0) = view::lonlat_to_mercator(bounds[0], bounds[1]);
                let (x1, y1) = view::lonlat_to_mercator(bounds[2], bounds[3]);
                [x0, y0, x1, y1]
            }
        };
        self.inputs = self.inputs.bounds(b);
        self
    }

    /// The camera's centre, longitude / latitude (needs a zoom).
    pub fn center(mut self, lon: f64, lat: f64) -> Self {
        self.inputs = self.inputs.center(lon, lat);
        self
    }

    /// The MapLibre zoom: the detail of the map.
    pub fn zoom(mut self, zoom: f64) -> Self {
        self.inputs = self.inputs.zoom(zoom);
        self
    }

    /// The image size in pixels.
    pub fn size(self, width: u32, height: u32) -> Self {
        self.size_in(width as f64, height as f64, Units::Px)
    }

    /// The image size in `units` (pixels, or inches / cm / mm at the dpi).
    pub fn size_in(mut self, width: f64, height: f64, units: Units) -> Self {
        self.inputs = self.inputs.size(width, height, units);
        self
    }

    /// Image pixels per inch: the sharpness (96 = one pixel per CSS pixel).
    pub fn dpi(mut self, dpi: f64) -> Self {
        self.inputs = self.inputs.dpi(dpi);
        self
    }

    /// Replace everything given of the view at once.
    pub fn with_inputs(mut self, inputs: ViewInputs) -> Self {
        self.inputs = inputs;
        self
    }

    /// How to fetch the data.
    pub fn fetch(mut self, fetch: FetchOptions) -> Self {
        self.fetch = fetch;
        self
    }

    /// Draw only inside (or outside) polygons, given in the map's CRS. An
    /// inside clip sets the default extent (its bounding box, which the core
    /// reads as Web Mercator: for a map in another CRS, give the bounds).
    pub fn clip(mut self, clip: Shape) -> Self {
        self.clip = Some(clip);
        self
    }

    pub fn styles(&self) -> &[Style] {
        &self.styles
    }

    pub fn inputs(&self) -> &ViewInputs {
        &self.inputs
    }

    pub fn fetch_options(&self) -> &FetchOptions {
        &self.fetch
    }

    // ── Views and renderings ────────────────────────────────────────────────

    /// The view in `context`: every value given, derived or defaulted.
    pub fn view(&self, context: &Context) -> Result<View, BasemapError> {
        let mut inputs = self.inputs.clone();
        if let (Some(extent), Context::Standalone) = (self.clip.as_ref().and_then(Shape::extent), context) {
            if inputs.bounds.is_none() && inputs.center.is_none() {
                inputs = inputs.bounds(extent);
            }
        }
        view::resolve(&inputs, context)
    }

    /// Render in `context`, in Web Mercator.
    pub fn render(&self, context: &Context) -> Result<Rendering, BasemapError> {
        self.render_view(&self.view(context)?, None)
    }

    /// Render a resolved `view` (from [`view`](Self::view)), in Web Mercator
    /// or, with `warp`, in the warp's CRS: the map is drawn in Web Mercator
    /// over the mesh's area at the view's zoom and dpi, then resampled.
    pub fn render_view(&self, view: &View, warp: Option<&Warp>) -> Result<Rendering, BasemapError> {
        self.fetch.validate()?;
        let ratio = view.dpi / CSS_DPI;
        let mut before = view.notes.clone();
        before.extend(view.decided());
        let mut r = match warp {
            None => {
                // CSS pixels are whole numbers. A size that is not a whole
                // number of them is drawn a little larger on the same pixel
                // grid, from the view's top-left corner, and cropped: the
                // pixels are the renderer's own, never resampled.
                if let Some((bbox, css)) = covering(view, ratio) {
                    let r = crate::rendering::render_collecting(|| self.requests(bbox, css, ratio), view.zoom, view.dpi, before)?;
                    cropped(r, view)?
                } else {
                    // A view too close to a whole world (or the poles) to
                    // draw larger: resampled, pixel centres to centres.
                    let css = css_px(view.width, view.height, ratio);
                    let r = crate::rendering::render_collecting(|| self.requests(view.bounds, css, ratio), view.zoom, view.dpi, before)?;
                    let b = view.bounds;
                    // The image covers its device pixels, from the top-left
                    // corner, not exactly the bounds asked for.
                    let r = Rendering { bounds: drawn_bounds(b, css, ratio, r.width, r.height), ..r };
                    let mesh = Mesh::from_fn(2, 2, view.width, view.height, |x, y| {
                        (b[0] + x / view.width as f64 * span_x(b), b[3] - y / view.height as f64 * (b[3] - b[1]))
                    })?;
                    Rendering { bounds: b, web_mercator_y: (b[1] + b[3]) / 2.0, ..resampled(r, &mesh, view.width, view.height)? }
                }
            }
            Some(w) => {
                let src = w.mesh.bounds(0.02).ok_or_else(|| {
                    BasemapError::InvalidInput("the map's extent has no point in Web Mercator's area".into())
                })?;
                let m = view::metres_per_px(view.zoom);
                let width_m = if src[0] > src[2] { src[2] + 2.0 * view::HALF - src[0] } else { src[2] - src[0] };
                let css = ((width_m / m).ceil().max(1.0), ((src[3] - src[1]) / m).ceil().max(1.0));
                if css.0 * ratio > MAX_SOURCE_PX || css.1 * ratio > MAX_SOURCE_PX {
                    return Err(BasemapError::InvalidInput(format!(
                        "drawing this extent at zoom {:.2} needs a {:.0} x {:.0} px image before warping; use a lower zoom or a smaller extent",
                        view.zoom,
                        css.0 * ratio,
                        css.1 * ratio
                    )));
                }
                let css = (css.0 as u32, css.1 as u32);
                let r = crate::rendering::render_collecting(|| self.requests(src, css, ratio), view.zoom, view.dpi, before)?;
                let r = Rendering { bounds: drawn_bounds(src, css, ratio, r.width, r.height), ..r };
                let mut out = resampled(r, &w.mesh, view.width, view.height)?;
                out.bounds = w.bounds;
                out.crs = w.crs.clone();
                out
            }
        };
        if let Some(shape) = &self.clip {
            let polygons = match warp {
                Some(w) => shape.to_pixels(w.bounds, r.width as f64, r.height as f64, false),
                None => shape.to_pixels(r.bounds, r.width as f64, r.height as f64, true),
            };
            crate::mask::apply(&mut r.pixels, &crate::mask::alpha_mask(&polygons, r.width, r.height, shape.keep));
        }
        if self.fetch.on_error == OnError::Fail {
            let problems: Vec<&str> = r.notes_at(Level::Problem).map(|n| n.message.as_str()).collect();
            if !problems.is_empty() {
                return Err(BasemapError::RenderError(format!("on_error = \"fail\": {}", problems.join("; "))));
            }
        }
        Ok(r)
    }

    /// The view and warp for a plot `panel` (see [`crate::plot`]): the plot
    /// gives the extent, size and dpi; the basemap may give a zoom or a dpi.
    /// A clip is only a mask in a plot.
    pub fn plan(&self, panel: &crate::Panel) -> Result<(View, Option<Warp>), BasemapError> {
        crate::plot::plan(panel, &self.inputs)
    }

    /// Render for a plot `panel`, in the plot's CRS: the image to draw over
    /// the panel's limits.
    pub fn render_panel(&self, panel: &crate::Panel) -> Result<Rendering, BasemapError> {
        let (view, warp) = self.plan(panel)?;
        self.render_view(&view, warp.as_ref())
    }

    /// An SVG document of the map in `context` (Web Mercator, CSS pixels).
    pub fn render_svg(&self, context: &Context, options: &SvgOptions) -> Result<String, BasemapError> {
        self.render_svg_view(&self.view(context)?, None, options)
    }

    /// An SVG document of a resolved `view`. A map in another CRS (a `warp`)
    /// is an error until paths are warped: SVG output is Web Mercator.
    pub fn render_svg_view(&self, view: &View, warp: Option<&Warp>, options: &SvgOptions) -> Result<String, BasemapError> {
        if let Some(w) = warp {
            return Err(BasemapError::InvalidInput(format!(
                "SVG output is in Web Mercator only for now, not {}: render an image, or use the Web Mercator CRS",
                w.crs
            )));
        }
        self.fetch.validate()?;
        let css = css_px(view.width, view.height, view.dpi / CSS_DPI);
        crate::render_svg_document(self.requests(view.bounds, css, 1.0)?, options, self.clip.as_ref())
    }

    /// The attribution text the styles ask maps to show.
    pub fn attribution(&self) -> Result<String, BasemapError> {
        let mut credits: Vec<String> = Vec::new();
        for style in &self.styles {
            for part in style.parts()? {
                let input = part.input.as_input().to_owned();
                let text = crate::attribution(std::slice::from_ref(&input), part.auth_token.as_deref())?;
                for c in text.split(" | ").filter(|c| !c.is_empty()) {
                    if !credits.iter().any(|k| k == c) {
                        credits.push(c.to_owned());
                    }
                }
            }
        }
        Ok(credits.join(" | "))
    }

    /// One request per part of each style, over `bbox` at `css` pixels.
    fn requests(&self, bbox: [f64; 4], css: (u32, u32), ratio: f64) -> Result<Vec<RenderRequest>, BasemapError> {
        let mut requests = Vec::new();
        for (group, style) in self.styles.iter().enumerate() {
            for part in style.parts()? {
                requests.push(RenderRequest {
                    layers: part.layers,
                    labels: part.labels,
                    features: part.features,
                    auth_token: part.auth_token,
                    auth_scope: part.token_scope,
                    group: group as u32,
                    tile_timeout_ms: self.fetch.timeout_ms(),
                    max_tiles: self.fetch.max_tiles,
                    tile_concurrency: self.fetch.concurrency,
                    fail_on_tile_error: self.fetch.on_error == OnError::Fail,
                    pixel_ratio: ratio,
                    ..RenderRequest::new(bbox, css.0, css.1, part.input)
                });
            }
        }
        if requests.is_empty() {
            return Err(BasemapError::InvalidInput("a basemap needs at least one style".into()));
        }
        Ok(requests)
    }
}

/// The largest image drawn before a warp, per side.
const MAX_SOURCE_PX: f64 = 16_384.0;

/// Whole CSS pixels for an image of `width x height` at `ratio`.
fn css_px(width: u32, height: u32, ratio: f64) -> (u32, u32) {
    (((width as f64 / ratio).round() as u32).max(1), ((height as f64 / ratio).round() as u32).max(1))
}

/// West-to-east width of `b` (across the date line when `xmin > xmax`).
fn span_x(b: [f64; 4]) -> f64 {
    if b[0] > b[2] { b[2] + 2.0 * view::HALF - b[0] } else { b[2] - b[0] }
}

/// A render of the view's pixels without resampling: a bbox and a whole
/// CSS size whose device pixels are the view's own pixel grid from its
/// top-left corner, at least as large as the view (cropped by
/// [`cropped`]). The renderer lays device pixels `1 / ratio` CSS pixel wide
/// from the bbox's top-left corner, and the zoom it derives from the bbox
/// and CSS size is the view's. The view's own bounds when its size is whole
/// CSS pixels; `None` when the larger bbox would leave Web Mercator's area
/// (a view of nearly the whole world, or reaching the poles).
fn covering(view: &View, ratio: f64) -> Option<([f64; 4], (u32, u32))> {
    let b = view.bounds;
    let (w, h) = (view.width as f64, view.height as f64);
    let css = ((w / ratio).ceil().max(1.0), (h / ratio).ceil().max(1.0));
    if css.0 * ratio == w && css.1 * ratio == h {
        return Some((b, (css.0 as u32, css.1 as u32)));
    }
    // Metres per device pixel, and the bbox those CSS pixels span.
    let (mx, my) = (span_x(b) / w, (b[3] - b[1]) / h);
    let (bw, bh) = (css.0 * ratio * mx, css.1 * ratio * my);
    if bw >= 2.0 * view::HALF || b[3] - bh < -view::HALF {
        return None;
    }
    let mut xmax = b[0] + bw;
    if b[0] > b[2] {
        xmax -= 2.0 * view::HALF;
    }
    Some(([b[0], b[3] - bh, xmax, b[3]], (css.0 as u32, css.1 as u32)))
}

/// The top-left `view.width x view.height` pixels of `r` (from
/// [`covering`]), which are the view's.
fn cropped(r: Rendering, view: &View) -> Result<Rendering, BasemapError> {
    let (w, h) = (view.width as usize, view.height as usize);
    if (r.width as usize, r.height as usize) == (w, h) {
        return Ok(Rendering { bounds: view.bounds, ..r });
    }
    if (r.width as usize) < w || (r.height as usize) < h {
        return Err(BasemapError::RenderError(format!(
            "internal: a {} x {} px render is smaller than the {w} x {h} px view",
            r.width, r.height
        )));
    }
    let stride = r.width as usize * 4;
    let pixels = r.pixels.chunks_exact(stride).take(h).flat_map(|row| &row[..w * 4]).copied().collect();
    let b = view.bounds;
    Ok(Rendering { pixels, width: view.width, height: view.height, bounds: b, web_mercator_y: (b[1] + b[3]) / 2.0, ..r })
}

/// What a render of `b` at `css` CSS pixels and `ratio` covers with its
/// `dw x dh` device pixels, laid from the top-left corner.
fn drawn_bounds(b: [f64; 4], css: (u32, u32), ratio: f64, dw: u32, dh: u32) -> [f64; 4] {
    let (mx, my) = (span_x(b) / (css.0 as f64 * ratio), (b[3] - b[1]) / (css.1 as f64 * ratio));
    let mut xmax = b[0] + dw as f64 * mx;
    if b[0] > b[2] {
        xmax -= 2.0 * view::HALF;
    }
    [b[0], b[3] - dh as f64 * my, xmax, b[3]]
}

/// `r` resampled through `mesh` into `width x height` pixels.
fn resampled(r: Rendering, mesh: &Mesh, width: u32, height: u32) -> Result<Rendering, BasemapError> {
    let pixels = warp::warp(&Image { pixels: &r.pixels, width: r.width, height: r.height, bounds: r.bounds }, mesh, width, height)?;
    Ok(Rendering { pixels, width, height, ..r })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mask::Keep;
    use crate::view::{Device, Provenance};

    const WORLD: f64 = view::HALF;

    fn plain(color: &str) -> String {
        format!(r##"{{"version":8,"sources":{{}},"layers":[{{"id":"bg","type":"background","paint":{{"background-color":"{color}"}}}}]}}"##)
    }

    /// A style with a background and, with `label`, an (empty) label layer.
    fn inline(color: &str, label: bool) -> String {
        let symbol = if label { r#",{"id":"names","type":"symbol","source":"g","layout":{"text-field":"x"}}"# } else { "" };
        format!(
            r#"{{"version":8,"sources":{{"g":{{"type":"geojson","data":{{"type":"FeatureCollection","features":[]}}}}}},"layers":[{{"id":"bg","type":"background","paint":{{"background-color":"{color}"}}}}{symbol}]}}"#
        )
    }

    #[test]
    fn the_view_comes_from_what_is_given() {
        let m = Basemap::new("x").center(-111.89, 40.76).zoom(12.5).size(800, 600).dpi(192.0);
        let v = m.view(&Context::Standalone).unwrap();
        assert_eq!((v.width, v.height, v.zoom, v.dpi), (800, 600, 12.5, 192.0));
        // Fitting that view's own bounds gives the same zoom back.
        let n = Basemap::new("x").bounds(v.bounds, Crs::WebMercator).size(800, 600).dpi(192.0);
        assert!((n.view(&Context::Standalone).unwrap().zoom - 12.5).abs() < 1e-9);
        // Errors name what to drop.
        let e = Basemap::new("x").center(0.0, 0.0).view(&Context::Standalone).unwrap_err().to_string();
        assert!(e.contains("zoom"), "{e}");
        assert_eq!(Crs::parse("epsg:3857").unwrap(), Crs::WebMercator);
        assert!(Crs::parse("EPSG:2163").is_err());
    }

    #[test]
    fn the_default_view_renders_the_world_and_says_so() {
        let r = Basemap::new(plain("#abcdef")).render(&Context::Standalone).expect("renders");
        assert_eq!((r.width, r.height, &r.pixels[..4]), (800, 600, &[0xab, 0xcd, 0xef, 0xff][..]));
        assert!((r.bounds[2] - r.bounds[0] - 2.0 * WORLD).abs() < 1e-3, "{:?}", r.bounds);
        let decided: Vec<&str> = r.notes_at(Level::Decided).map(|n| n.kind).collect();
        assert_eq!(decided, ["zoom_auto", "dpi_default", "size_default", "extent_default"]);
    }

    #[test]
    fn sizes_that_are_not_whole_css_pixels_are_exact() {
        // 1001 px at 300 dpi is 320.32 CSS px.
        let r = Basemap::new(plain("#123456")).center(0.0, 0.0).zoom(3.0).size(1001, 333).dpi(300.0).render(&Context::Standalone).unwrap();
        assert_eq!((r.width, r.height, r.dpi), (1001, 333, 300.0));
        assert_eq!(&r.pixels[(333 / 2 * 1001 + 500) * 4..][..4], &[0x12, 0x34, 0x56, 0xff]);
    }

    #[test]
    fn a_clip_masks_and_an_inside_clip_frames() {
        let d = 1_000_000.0;
        let square = vec![vec![vec![(-d, -d), (d, -d), (d, d), (-d, d)]]];
        let inside = Shape::new(square.clone(), Keep::Inside).unwrap();
        let m = Basemap::new(plain("#ff0000")).size(100, 100).clip(inside.clone());
        let v = m.view(&Context::Standalone).unwrap();
        assert_eq!(v.provenance.extent, Provenance::Given, "the clip gives the extent");
        let r = m.render(&Context::Standalone).unwrap();
        assert_eq!((r.pixels[(50 * 100 + 50) * 4 + 3], r.pixels[3]), (255, 255), "the square fills the frame");
        // Outside: no extent from the clip; the square is a hole.
        let out = Basemap::new(plain("#ff0000")).size(100, 100).clip(Shape::new(square, Keep::Outside).unwrap());
        assert_eq!(out.view(&Context::Standalone).unwrap().provenance.extent, Provenance::Default);
        let r = out.render(&Context::Standalone).unwrap();
        assert_eq!((r.pixels[(50 * 100 + 50) * 4 + 3], r.pixels[3]), (0, 255));
        // In a plot, a clip is only a mask: the plot gives the extent.
        let plot = Context::Plot { extent: [-4.0 * d, -4.0 * d, 4.0 * d, 4.0 * d], panel_px: (80, 80), device: Device::Screen { pixel_ratio: 1.0 } };
        let v = Basemap::new(plain("#ff0000")).clip(inside).view(&plot).unwrap();
        assert_eq!(v.provenance.extent, Provenance::Context);
    }

    /// A white map with a black vertical line `width_css` CSS pixels wide at
    /// Web Mercator `x`.
    fn line_at(x: f64, width_css: f64) -> String {
        let lon = x / WORLD * 180.0;
        format!(
            r##"{{"version":8,"sources":{{"l":{{"type":"geojson","data":{{"type":"Feature","properties":{{}},"geometry":{{"type":"LineString","coordinates":[[{lon},-80],[{lon},80]]}}}}}}}},"layers":[{{"id":"bg","type":"background","paint":{{"background-color":"#ffffff"}}}},{{"id":"l","type":"line","source":"l","paint":{{"line-color":"#000000","line-width":{width_css}}}}}]}}"##
        )
    }

    /// 0.3.0 drew a size that is not whole CSS pixels (720 px at 168 dpi is
    /// 411.43) one pixel smaller and resampled it, blurring every edge.
    #[test]
    fn a_size_that_is_not_whole_css_pixels_is_not_resampled() {
        let b = [-12_469_979.0, 5_026_397.0, -12_466_414.0, 5_031_348.0];
        let probe = Basemap::new(plain("#ffffff")).bounds(b, Crs::WebMercator).size(720, 1000).dpi(168.0);
        let v = probe.view(&Context::Standalone).unwrap();
        let ratio = 168.0 / CSS_DPI;
        let mx = (v.bounds[2] - v.bounds[0]) / 720.0;
        // A line one device pixel wide on the centre of column 300.
        let style = line_at(v.bounds[0] + 300.5 * mx, 1.0 / ratio);
        let m = Basemap::new(style.clone()).bounds(b, Crs::WebMercator).size(720, 1000).dpi(168.0);
        let r = m.render(&Context::Standalone).unwrap();
        assert_eq!((r.width, r.height, r.bounds), (720, 1000, v.bounds));
        assert!((r.zoom - v.zoom).abs() < 1e-12);
        let lum = |x: usize, y: usize| r.pixels[(y * 720 + x) * 4] as i32;
        for y in [10, 500, 990] {
            assert!(lum(300, y) < 30, "the line's column is black, not smeared: {}", lum(300, y));
            assert!(lum(299, y) > 225 && lum(301, y) > 225, "its neighbours stay white: {} {}", lum(299, y), lum(301, y));
        }
        // The pixels are a raw render's: the top-left of a render of the
        // same grid at a whole number of CSS pixels (412 x 572, 721 x 1001).
        let (bbox, css) = covering(&v, ratio).unwrap();
        assert_eq!(css, (412, 572));
        let raw = Basemap::new(style).bounds(bbox, Crs::WebMercator).size(721, 1001).dpi(168.0).render(&Context::Standalone).unwrap();
        assert_eq!((raw.width, raw.height), (721, 1001));
        assert!((raw.zoom - v.zoom).abs() < 1e-9, "the same zoom: {} {}", raw.zoom, v.zoom);
        let top_left: Vec<u8> = raw.pixels.chunks_exact(721 * 4).take(1000).flat_map(|row| &row[..720 * 4]).copied().collect();
        assert!(top_left == r.pixels, "pixel-identical to the raw render");
    }

    /// The column, in pixel-centre coordinates, at the centre of darkness of
    /// row `y` between columns `from..to`.
    fn dark_centre(r: &Rendering, y: usize, from: usize, to: usize) -> f64 {
        let w = r.width as usize;
        let (mut sum, mut at) = (0.0, 0.0);
        for x in from..to {
            let d = 255.0 - r.pixels[(y * w + x) * 4] as f64;
            sum += d;
            at += d * x as f64;
        }
        at / sum
    }

    /// Plots: a Web Mercator panel is the plain render, pixel for pixel; a
    /// warped one (lon/lat, or any CRS through its mesh) samples pixel
    /// centres, so a line stays centred where it belongs.
    #[test]
    fn plot_panels_keep_pixels_where_they_belong() {
        use crate::plot::{Panel, PanelCrs};
        let screen = Device::Screen { pixel_ratio: 1.75 };
        let b = [-12_469_979.0, 5_026_397.0, -12_466_414.0, 5_031_348.0];
        let v = Basemap::new(plain("#fff")).bounds(b, Crs::WebMercator).size(720, 1000).dpi(168.0).view(&Context::Standalone).unwrap();
        let mx = (v.bounds[2] - v.bounds[0]) / 720.0;
        let style = line_at(v.bounds[0] + 300.5 * mx, 1.0 / 1.75);
        let alone = Basemap::new(style.clone()).bounds(b, Crs::WebMercator).size(720, 1000).dpi(168.0).render(&Context::Standalone).unwrap();
        let panel = Panel::new(v.bounds, PanelCrs::WebMercator, (720, 1000), screen).unwrap();
        let plotted = Basemap::new(style.clone()).render_panel(&panel).unwrap();
        assert!(plotted.pixels == alone.pixels, "a Web Mercator panel is the plain render");

        // Through a mesh (an "other" CRS whose units happen to be metres of
        // Web Mercator): the line's darkness is centred on column 300.
        let mesh = Mesh::from_fn(4, 4, 720, 1000, |x, y| (v.bounds[0] + x * mx, v.bounds[3] - y * mx)).unwrap();
        let other = Panel::new(v.bounds, PanelCrs::Other { name: "EPSG:32612".into(), mesh }, (720, 1000), screen).unwrap();
        let warped = Basemap::new(style).render_panel(&other).unwrap();
        for y in [10, 500, 990] {
            let c = dark_centre(&warped, y, 290, 311);
            assert!((c - 300.0).abs() < 0.05, "row {y}: centred at {c}");
        }

        // Lon/lat: a meridian at the centre of column 123 of a 400 px panel.
        let lon = -2.0 + 123.5 / 400.0 * 4.0;
        let style = line_at(lon / 180.0 * WORLD, 1.0 / 1.75);
        let p = Panel::new([-2.0, 50.0, 2.0, 52.0], PanelCrs::LonLat, (400, 300), screen).unwrap();
        let r = Basemap::new(style).render_panel(&p).unwrap();
        for y in [5, 150, 295] {
            let c = dark_centre(&r, y, 113, 134);
            assert!((c - 123.0).abs() < 0.05, "row {y}: centred at {c}");
        }
    }

    #[test]
    fn the_covering_render_wraps_at_the_date_line_and_gives_up_near_the_world() {
        let v = Basemap::new(plain("#fff")).bounds([WORLD - 5000.0, 0.0, -WORLD + 5000.0, 7000.0], Crs::WebMercator).size(720, 500).dpi(168.0);
        let v = v.view(&Context::Standalone).unwrap();
        let (bbox, _) = covering(&v, 168.0 / CSS_DPI).unwrap();
        assert!(bbox[0] > bbox[2] && bbox[2] > v.bounds[2] && bbox[2] < 0.0, "{bbox:?}");
        assert!((bbox[3] - v.bounds[3]).abs() < 1e-9 && bbox[1] < v.bounds[1]);
        // The whole world at 168 dpi: drawn and resampled instead.
        let w = Basemap::new(plain("#123456")).size(800, 600).dpi(168.0);
        let wv = w.view(&Context::Standalone).unwrap();
        assert!(covering(&wv, 168.0 / CSS_DPI).is_none());
        let r = w.render(&Context::Standalone).unwrap();
        assert_eq!((r.width, r.height, &r.pixels[(300 * 800 + 400) * 4..][..4]), (800, 600, &[0x12, 0x34, 0x56, 0xff][..]));
    }

    #[test]
    fn a_warp_resamples_into_the_maps_crs() {
        // An identity "CRS": the warped map equals the Web Mercator one.
        let m = Basemap::new(plain("#336699")).center(10.0, 20.0).zoom(5.0).size(120, 90);
        let v = m.view(&Context::Standalone).unwrap();
        let b = v.bounds;
        let mesh = Mesh::from_fn(4, 4, v.width, v.height, |x, y| (b[0] + x / 120.0 * (b[2] - b[0]), b[3] - y / 90.0 * (b[3] - b[1]))).unwrap();
        let w = Warp::new("EPSG:3857 (identity)", b, mesh).unwrap();
        let r = m.render_view(&v, Some(&w)).unwrap();
        assert_eq!((r.width, r.height, r.crs.as_str(), r.bounds), (120, 90, "EPSG:3857 (identity)", b));
        assert!(r.pixels.chunks_exact(4).all(|p| p == [0x33, 0x66, 0x99, 0xff]));
        // SVG of a warped map is an error, not a silent Web Mercator SVG.
        let e = m.render_svg_view(&v, Some(&w), &SvgOptions::default()).unwrap_err().to_string();
        assert!(e.contains("Web Mercator"), "{e}");
    }

    #[test]
    fn svg_carries_the_clip() {
        let d = 1_000_000.0;
        let m = Basemap::new(plain("#ff0000"))
            .center(0.0, 0.0)
            .zoom(3.0)
            .size(64, 64)
            .clip(Shape::new(vec![vec![vec![(-d, -d), (d, -d), (d, d), (-d, d)]]], Keep::Outside).unwrap());
        let svg = m.render_svg(&Context::Standalone, &SvgOptions::default()).unwrap();
        assert!(svg.contains(r#"<clipPath id="basemapper-clip"><path clip-rule="evenodd" d="M0 0H64V64H0Z"#), "{svg}");
        assert!(svg.contains(r#"<g clip-path="url(#basemapper-clip)">"#));
    }

    #[test]
    fn on_error_fail_turns_problems_into_errors() {
        let video = r##"{"version":8,"sources":{"g":{"type":"geojson","data":{"type":"FeatureCollection","features":[]}},"v":{"type":"video","urls":[]}},"layers":[{"id":"bg","type":"background","paint":{"background-color":"#fff"}},{"id":"d","type":"circle","source":"g"},{"id":"m","type":"raster","source":"v"}]}"##;
        let m = Basemap::new(video).center(0.0, 0.0).zoom(2.0).size(32, 32);
        assert!(m.render(&Context::Standalone).unwrap().notes_at(Level::Problem).count() > 0);
        let e = m.fetch(FetchOptions::new().on_error(OnError::Fail)).render(&Context::Standalone).unwrap_err().to_string();
        assert!(e.contains("on_error") && e.contains("video"), "{e}");
    }

    #[test]
    fn filters_belong_to_each_style_of_a_stack() {
        let small = |styles: Vec<Style>| Basemap::stack(styles).center(0.0, 0.0).zoom(2.0).size(64, 48).render(&Context::Standalone);
        let labels = || vec!["labels".to_string()];
        let r = small(vec![Style::new(inline("#ff0000", false)), Style::new(inline("#0000ff", true)).features(labels())]).unwrap();
        assert_eq!(&r.pixels[..4], &[0xff, 0, 0, 0xff]);
        let e = small(vec![Style::new(inline("#ff0000", false)).features(labels()), Style::new(inline("#0000ff", true))]).unwrap_err();
        assert!(e.to_string().contains("nothing of this style of the stack"), "{e}");
        let e = small(vec![Style::new(inline("#ff0000", false)).features(labels())]).unwrap_err();
        assert!(e.to_string().contains("nothing"), "{e}");
    }

    #[test]
    fn one_group_is_filtered_as_one_map() {
        let req = |json: String| RenderRequest {
            labels: crate::Labels::Only,
            ..RenderRequest::new([-1e6, -1e6, 1e6, 1e6], 64, 48, crate::StyleInput::detect(&json))
        };
        let r = crate::render_stack(vec![req(inline("#ff0000", false)), req(inline("#0000ff", true))]).unwrap();
        assert_eq!(r.pixels[3], 0, "labels only: no background drawn");
        let mut second = req(inline("#0000ff", true));
        second.group = 1;
        let e = crate::render_stack(vec![req(inline("#ff0000", false)), second]).unwrap_err();
        assert!(e.to_string().contains("style of the stack"), "{e}");
    }
}
