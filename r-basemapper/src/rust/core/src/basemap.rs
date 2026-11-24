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
                // CSS pixels are whole numbers; a size that is not a whole
                // number of them is resampled to the exact pixels below.
                let css = css_px(view.width, view.height, ratio);
                let r = crate::rendering::render_collecting(|| self.requests(view.bounds, css, ratio), view.zoom, view.dpi, before)?;
                if (r.width, r.height) == (view.width, view.height) {
                    r
                } else {
                    let b = r.bounds;
                    let mesh = Mesh::from_fn(2, 2, view.width, view.height, |x, y| {
                        (b[0] + x / view.width as f64 * (b[2] - b[0]), b[3] - y / view.height as f64 * (b[3] - b[1]))
                    })?;
                    resampled(r, &mesh, view.width, view.height)?
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
