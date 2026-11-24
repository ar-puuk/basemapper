//! `Basemap`: a map view to render, the Rust counterpart of R's
//! `basemap()` and Python's `basemapper.basemap()` / `Basemap`.
//!
//! The same verbs exist in all three languages: R `verb(m, ...)`, Python
//! `m.verb(...)`, Rust `m.verb(...)`:
//!
//! | Task | Rust |
//! |---|---|
//! | create | `Basemap::new(style)` + `.size()`, `.center()`, `.zoom()`, `.bounds()`, `.pixel_ratio()`, ... |
//! | change the view | `set_view(center, zoom)`, `set_bounds(bounds, crs)` |
//! | read the view | `get_bounds(lonlat)`, `get_center()`, `get_zoom()`, `get_size()` |
//! | render | `render_array()`, `render_png(path)`, `render_svg(path, options)` |

use crate::bbox::{bbox_from_center, constrain_view, fit_bounds, RenderRequest, RenderResult};
use crate::{encode_png, render_stack, render_svg_stack_with, BasemapError, Style, SvgOptions};
use std::path::Path;

const EARTH_HALF: f64 = 20_037_508.342_789_244;

/// A map view: one style, or several stacked bottom to top (each with its
/// own filters and key, see [`Style`]), a size in CSS pixels, a centre and MapLibre zoom (512 px tiles, fractional allowed),
/// and a device pixel ratio (images are `pixel_ratio` times larger with the
/// same view and label layout).
#[derive(Clone, Debug)]
pub struct Basemap {
    styles: Vec<Style>,
    width: u32,
    height: u32,
    center: (f64, f64),
    zoom: f64,
    pixel_ratio: f64,
    tile_timeout_ms: u32,
    max_tiles: u32,
    tile_concurrency: u32,
    fail_on_tile_error: bool,
}

/// Coordinate reference system of bounds given to [`Basemap::set_bounds`]:
/// the core reads longitude/latitude and Web Mercator (R and Python
/// reproject other CRSs before calling it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crs {
    /// EPSG:4326 longitude/latitude degrees.
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

fn lonlat_to_3857(lon: f64, lat: f64) -> (f64, f64) {
    let lat = lat.clamp(-85.051_128_779_806_59, 85.051_128_779_806_59);
    let r = EARTH_HALF / std::f64::consts::PI;
    (lon.to_radians() * r, (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln() * r)
}

fn mercator_to_lonlat(x: f64, y: f64) -> (f64, f64) {
    let r = EARTH_HALF / std::f64::consts::PI;
    ((x / r).to_degrees(), (2.0 * (y / r).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees())
}

impl Basemap {
    /// A view of one style (a [`Style`], or a string: a catalogue name, a
    /// style URL or JSON, tile data): 800 x 600 CSS pixels at zoom 0.
    pub fn new(style: impl Into<Style>) -> Self {
        Self::stack(vec![style.into()])
    }

    /// A view of several styles drawn bottom to top (e.g. imagery under a
    /// vector basemap's labels).
    pub fn stack(styles: Vec<Style>) -> Self {
        Basemap {
            styles,
            width: 800,
            height: 600,
            center: (0.0, 0.0),
            zoom: 0.0,
            pixel_ratio: 1.0,
            tile_timeout_ms: 10_000,
            max_tiles: 256,
            tile_concurrency: 16,
            fail_on_tile_error: false,
        }
    }

    // ── Builder (the constructor arguments of R / Python) ───────────────────

    /// View size in CSS pixels.
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// View centre as (longitude, latitude).
    pub fn center(mut self, longitude: f64, latitude: f64) -> Self {
        self.center = (longitude, latitude);
        self
    }

    /// MapLibre zoom (0-22, fractional allowed).
    pub fn zoom(mut self, zoom: f64) -> Self {
        self.zoom = zoom;
        self
    }

    /// Fit `bounds` (`[xmin, ymin, xmax, ymax]` in `crs`) instead of a
    /// centre and zoom. Apply after [`size`](Self::size).
    pub fn bounds(mut self, bounds: [f64; 4], crs: Crs) -> Result<Self, BasemapError> {
        self.set_bounds(bounds, crs)?;
        Ok(self)
    }

    /// Device pixels per CSS pixel (MapLibre's `devicePixelRatio`).
    pub fn pixel_ratio(mut self, pixel_ratio: f64) -> Self {
        self.pixel_ratio = pixel_ratio;
        self
    }

    /// Per-tile HTTP timeout in milliseconds.
    pub fn tile_timeout_ms(mut self, ms: u32) -> Self {
        self.tile_timeout_ms = ms;
        self
    }

    /// Maximum tiles fetched per style.
    pub fn max_tiles(mut self, n: u32) -> Self {
        self.max_tiles = n;
        self
    }

    /// Parallel tile downloads.
    pub fn tile_concurrency(mut self, n: u32) -> Self {
        self.tile_concurrency = n;
        self
    }

    /// Abort on a failed tile instead of leaving it empty.
    pub fn fail_on_tile_error(mut self, fail: bool) -> Self {
        self.fail_on_tile_error = fail;
        self
    }

    // ── View ────────────────────────────────────────────────────────────────

    /// Set the centre and/or zoom (`None` keeps the current value).
    pub fn set_view(&mut self, center: Option<(f64, f64)>, zoom: Option<f64>) -> Result<&mut Self, BasemapError> {
        if let Some(z) = zoom {
            if !(0.0..=22.0).contains(&z) {
                return Err(BasemapError::InvalidInput("zoom must be between 0 and 22".into()));
            }
            self.zoom = z;
        }
        if let Some((lon, lat)) = center {
            if !lon.is_finite() || !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
                return Err(BasemapError::InvalidInput("center must be a finite longitude and a latitude between -90 and 90".into()));
            }
            self.center = (lon, lat);
        }
        Ok(self)
    }

    /// Fit the view to `bounds` in `crs`, keeping its size.
    pub fn set_bounds(&mut self, bounds: [f64; 4], crs: Crs) -> Result<&mut Self, BasemapError> {
        let b = match crs {
            Crs::WebMercator => bounds,
            Crs::LonLat => {
                let (x0, y0) = lonlat_to_3857(bounds[0], bounds[1]);
                let (x1, y1) = lonlat_to_3857(bounds[2], bounds[3]);
                [x0, y0, x1, y1]
            }
        };
        if !(b[2] > b[0] && b[3] > b[1]) {
            return Err(BasemapError::InvalidBbox("bounds must have xmin < xmax and ymin < ymax".into()));
        }
        let (lon, lat, zoom) = fit_bounds(b, self.width, self.height);
        self.center = (lon, lat);
        self.zoom = zoom.clamp(0.0, 22.0);
        Ok(self)
    }

    /// The view shown: the requested centre and zoom, constrained as MapLibre
    /// constrains them (see [`constrain_view`]). The view keeps what was asked
    /// for, so a later [`size`](Self::size) constrains it afresh.
    fn view(&self) -> (f64, f64, f64) {
        constrain_view(self.center.0, self.center.1, self.zoom, self.width, self.height)
    }

    /// The view's `[xmin, ymin, xmax, ymax]`: EPSG:3857 metres, or degrees
    /// with `lonlat`.
    pub fn get_bounds(&self, lonlat: bool) -> [f64; 4] {
        let (lon, lat, zoom) = self.view();
        let b = bbox_from_center(lon, lat, zoom, self.width, self.height);
        if !lonlat {
            return b;
        }
        let (w, s) = mercator_to_lonlat(b[0], b[1]);
        let (e, n) = mercator_to_lonlat(b[2], b[3]);
        [w, s, e, n]
    }

    /// The view centre as (longitude, latitude), as shown: a latitude whose
    /// view would reach past a pole is moved so it stops at the pole.
    pub fn get_center(&self) -> (f64, f64) {
        let (lon, lat, _) = self.view();
        (lon, lat)
    }

    /// The view's MapLibre zoom, as shown: at least the zoom at which the
    /// world fills the view.
    pub fn get_zoom(&self) -> f64 {
        self.view().2
    }

    /// Size of rendered images in device pixels (`width`, `height` times
    /// `pixel_ratio`).
    pub fn get_size(&self) -> (u32, u32) {
        (crate::vector::render::device_px(self.width, self.pixel_ratio), crate::vector::render::device_px(self.height, self.pixel_ratio))
    }

    /// The attribution text the view's styles ask maps to show (see
    /// [`crate::attribution()`]).
    pub fn get_attribution(&self) -> Result<String, BasemapError> {
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

    // ── Rendering ───────────────────────────────────────────────────────────

    /// The view, checked: the builder setters take any value, so a bad
    /// zoom or centre is reported when rendering.
    fn check_view(&self) -> Result<(), BasemapError> {
        let (lon, lat) = self.center;
        if !self.zoom.is_finite() || !(0.0..=22.0).contains(&self.zoom) {
            return Err(BasemapError::InvalidInput(format!("zoom {} must be between 0 and 22", self.zoom)));
        }
        if !lon.is_finite() || !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
            return Err(BasemapError::InvalidInput(format!(
                "center ({lon}, {lat}) must be a finite longitude and a latitude between -90 and 90"
            )));
        }
        Ok(())
    }

    fn requests(&self) -> Result<Vec<RenderRequest>, BasemapError> {
        self.check_view()?;
        let bbox = self.get_bounds(false);
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
                    tile_timeout_ms: self.tile_timeout_ms,
                    max_tiles: self.max_tiles,
                    tile_concurrency: self.tile_concurrency,
                    fail_on_tile_error: self.fail_on_tile_error,
                    pixel_ratio: self.pixel_ratio,
                    ..RenderRequest::new(bbox, self.width, self.height, part.input)
                });
            }
        }
        Ok(requests)
    }

    /// Render the view: the image, where it is, its zoom and dpi, and notes
    /// on what was decided, adjusted or could not be done.
    pub fn render(&self) -> Result<crate::Rendering, BasemapError> {
        self.check_view()?;
        let dpi = self.pixel_ratio * crate::view::CSS_DPI;
        crate::rendering::render_collecting(|| self.requests(), self.get_zoom(), dpi, Vec::new())
    }

    /// Render to straight-alpha RGBA pixels (device pixels).
    pub fn render_array(&self) -> Result<RenderResult, BasemapError> {
        render_stack(self.requests()?)
    }

    /// Render a PNG, written to `path` when given; returns its bytes.
    pub fn render_png(&self, path: Option<&Path>) -> Result<Vec<u8>, BasemapError> {
        let r = self.render_array()?;
        let png = encode_png(&r.pixels, r.width, r.height)?;
        if let Some(p) = path {
            std::fs::write(p, &png).map_err(|e| BasemapError::RenderError(format!("writing {}: {e}", p.display())))?;
        }
        Ok(png)
    }

    /// Render a vector SVG (`width` x `height` user units; `pixel_ratio`
    /// does not apply), written to `path` when given; returns the document.
    pub fn render_svg(&self, path: Option<&Path>, options: &SvgOptions) -> Result<String, BasemapError> {
        let svg = render_svg_stack_with(self.requests()?, options)?;
        if let Some(p) = path {
            std::fs::write(p, &svg).map_err(|e| BasemapError::RenderError(format!("writing {}: {e}", p.display())))?;
        }
        Ok(svg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_round_trips() {
        let mut m = Basemap::new("https://example.com/style.json").size(800, 600).pixel_ratio(2.0);
        m.set_view(Some((-111.89, 40.76)), Some(12.5)).unwrap();
        assert_eq!(m.get_center(), (-111.89, 40.76));
        assert_eq!(m.get_zoom(), 12.5);
        assert_eq!(m.get_size(), (1600, 1200));
        let b = m.get_bounds(true);
        assert!(b[0] < -111.89 && b[2] > -111.89 && b[1] < 40.76 && b[3] > 40.76);
        // Fitting the view's own bounds gives the same view back.
        let mut n = Basemap::new("x").size(800, 600);
        n.set_bounds(m.get_bounds(false), Crs::WebMercator).unwrap();
        assert!((n.get_zoom() - 12.5).abs() < 1e-9);
        assert!((n.get_center().0 + 111.89).abs() < 1e-9 && (n.get_center().1 - 40.76).abs() < 1e-9);
        let mut l = Basemap::new("x").size(800, 600);
        l.set_bounds(m.get_bounds(true), Crs::LonLat).unwrap();
        assert!((l.get_zoom() - 12.5).abs() < 1e-6);
        assert!(m.set_view(None, Some(23.0)).is_err());
        assert_eq!(Crs::parse("epsg:3857").unwrap(), Crs::WebMercator);
        assert!(Crs::parse("EPSG:2163").is_err());
    }

    const WORLD: f64 = EARTH_HALF;

    fn within_world(b: [f64; 4]) -> bool {
        b[1] >= -WORLD && b[3] <= WORLD
    }

    #[test]
    fn default_view_fits_the_world() {
        // 800 x 600 at zoom 0: the world (512 px) is smaller than the view,
        // so the zoom rises until it fills it (here its width).
        let m = Basemap::new("x");
        assert!((m.get_zoom() - (800f64 / 512.0).log2()).abs() < 1e-12, "{}", m.get_zoom());
        assert_eq!(m.get_center(), (0.0, 0.0));
        let b = m.get_bounds(false);
        assert!(within_world(b), "{b:?}");
        assert!((b[2] - b[0] - 2.0 * WORLD).abs() < 1e-3, "the view's width is the world's: {b:?}");
        // A tall view fills the height; a view inside the world is not zoomed.
        assert!((Basemap::new("x").size(300, 1024).get_zoom() - 1.0).abs() < 1e-12);
        assert_eq!(Basemap::new("x").size(500, 400).get_zoom(), 0.0);
        // And it renders (it used to be an invalid bbox).
        let style = r##"{"version":8,"sources":{},"layers":[{"id":"bg","type":"background","paint":{"background-color":"#abcdef"}}]}"##;
        let r = Basemap::new(style).render_array().expect("the default view renders");
        assert_eq!((r.width, r.height, &r.pixels[..4]), (800, 600, &[0xab, 0xcd, 0xef, 0xff][..]));
    }

    #[test]
    fn views_stop_at_the_poles() {
        let mut m = Basemap::new("x").size(800, 600);
        m.set_view(Some((10.0, 84.0)), Some(3.0)).unwrap();
        let b = m.get_bounds(false);
        assert!(within_world(b) && (b[3] - WORLD).abs() < 1e-3, "touches the north edge: {b:?}");
        assert!(m.get_center().1 < 84.0 && m.get_center().0 == 10.0);
        m.set_view(Some((0.0, -89.0)), None).unwrap();
        let b = m.get_bounds(false);
        assert!(within_world(b) && (b[1] + WORLD).abs() < 1e-3, "touches the south edge: {b:?}");
        // The request is kept: a smaller view is constrained afresh.
        let n = Basemap::new("x").size(800, 600).center(0.0, 40.0).zoom(0.0).size(200, 100);
        assert_eq!((n.get_center(), n.get_zoom()), ((0.0, 40.0), 0.0));
    }

    /// A style with a background and, with `label`, an (empty) label layer.
    fn inline(color: &str, label: bool) -> String {
        let symbol = if label {
            r#",{"id":"names","type":"symbol","source":"g","layout":{"text-field":"x"}}"#
        } else {
            ""
        };
        format!(
            r#"{{"version":8,"sources":{{"g":{{"type":"geojson","data":{{"type":"FeatureCollection","features":[]}}}}}},"layers":[{{"id":"bg","type":"background","paint":{{"background-color":"{color}"}}}}{symbol}]}}"#
        )
    }

    #[test]
    fn filters_belong_to_each_style_of_a_stack() {
        let small = |styles: Vec<Style>| Basemap::stack(styles).size(64, 48).render_array();
        let labels = || vec!["labels".to_string()];
        // The base drawn whole, only the labels of the top style: the top has
        // no background, so the base shows through.
        let r = small(vec![Style::new(inline("#ff0000", false)), Style::new(inline("#0000ff", true)).features(labels())]).unwrap();
        assert_eq!(&r.pixels[..4], &[0xff, 0, 0, 0xff]);
        // A filter on one style is that style's: here it leaves nothing of
        // it, which is an error, though the other style would draw.
        let e = small(vec![Style::new(inline("#ff0000", false)).features(labels()), Style::new(inline("#0000ff", true))]).unwrap_err();
        assert!(e.to_string().contains("nothing of this style of the stack"), "{e}");
        // One style on its own: the same error.
        let e = small(vec![Style::new(inline("#ff0000", false)).features(labels())]).unwrap_err();
        assert!(e.to_string().contains("nothing"), "{e}");
    }

    #[test]
    fn one_group_is_filtered_as_one_map() {
        // The parts of one style (a catalogue stack's services) share their
        // filter: a part it leaves nothing of is skipped.
        let req = |json: String| RenderRequest {
            labels: crate::Labels::Only,
            ..RenderRequest::new([-1e6, -1e6, 1e6, 1e6], 64, 48, crate::StyleInput::detect(&json))
        };
        let r = render_stack(vec![req(inline("#ff0000", false)), req(inline("#0000ff", true))]).unwrap();
        assert_eq!(r.pixels[3], 0, "labels only: no background drawn");
        let mut second = req(inline("#0000ff", true));
        second.group = 1;
        let e = render_stack(vec![req(inline("#ff0000", false)), second]).unwrap_err();
        assert!(e.to_string().contains("style of the stack"), "{e}");
    }
}
