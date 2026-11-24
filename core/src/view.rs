//! Resolving a view: the extent, zoom, image size and dpi of a map, from
//! whatever the user gave and the context it is drawn in
//! (docs/design/api-review.md §4.2).
//!
//! One relation ties the four:
//!
//! ```text
//! ground width (Web Mercator metres) = CSS px × metres per CSS px at the zoom
//! CSS px = image px × 96 / dpi        (or inches × 96, for a physical size)
//! ```
//!
//! so any three give the fourth. A value left out is derived if it can be,
//! else filled from a default (dpi, then image size, then extent); the zoom
//! is never a default. Over-determined requests are errors that say what to
//! drop. In a plot, the plot gives the extent and the image, and the device
//! the dpi. Every value is labelled given, derived, default or context, and
//! changes to what the user gave are reported as notes.
//!
//! Coordinates arrive in Web Mercator (`bounds`) or as longitude / latitude
//! (`center`): the host (R, Python) transforms other CRSs before calling.

use crate::notes::{Level, Note};
use crate::BasemapError;

/// Half the Web Mercator world's width, in metres.
pub const HALF: f64 = 20_037_508.342_789_244;
const CIRC: f64 = 2.0 * HALF;
/// A CSS pixel is 1/96 inch, as in MapLibre and every browser.
pub const CSS_DPI: f64 = 96.0;
/// The default dpi: one image pixel per CSS pixel.
pub const DEFAULT_DPI: f64 = 96.0;
/// The default image size, in CSS pixels (scaled with the dpi).
pub const DEFAULT_CSS_SIZE: (f64, f64) = (800.0, 600.0);
/// The dpi of a vector export (PDF, SVG), which has none of its own.
pub const VECTOR_EXPORT_DPI: f64 = 300.0;
const MAX_ZOOM: f64 = 22.0;
const MAX_LAT: f64 = 85.051_128_779_806_59;

/// The unit of `width` and `height`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Units {
    /// Image pixels.
    #[default]
    Px,
    In,
    Cm,
    Mm,
}

impl Units {
    /// `"px"`, `"in"`, `"cm"` or `"mm"`.
    pub fn parse(s: &str) -> Result<Units, BasemapError> {
        match s {
            "px" => Ok(Units::Px),
            "in" => Ok(Units::In),
            "cm" => Ok(Units::Cm),
            "mm" => Ok(Units::Mm),
            _ => Err(BasemapError::InvalidInput(format!("units must be \"px\", \"in\", \"cm\" or \"mm\", not {s:?}"))),
        }
    }

    /// Units per inch, for a physical unit.
    fn per_inch(self) -> Option<f64> {
        match self {
            Units::Px => None,
            Units::In => Some(1.0),
            Units::Cm => Some(2.54),
            Units::Mm => Some(25.4),
        }
    }
}

/// What the user gave for a view; `None` for what they left out.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ViewInputs {
    /// An area in Web Mercator metres, `[xmin, ymin, xmax, ymax]`
    /// (`xmin > xmax` across the date line).
    pub bounds: Option<[f64; 4]>,
    /// The camera's centre, longitude / latitude.
    pub center: Option<(f64, f64)>,
    /// The MapLibre zoom.
    pub zoom: Option<f64>,
    /// The image's `(width, height)` in `units`.
    pub size: Option<(f64, f64)>,
    pub units: Units,
    pub dpi: Option<f64>,
}

/// The kind of device a plot is drawn on, which decides its dpi.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Device {
    /// A screen: 96 dpi times its pixel ratio (2 on most high-density screens).
    Screen { pixel_ratio: f64 },
    /// A raster export (PNG file, `ggsave()`, `savefig()`), at its dpi.
    RasterExport { dpi: f64 },
    /// A vector export (PDF, SVG), which has no dpi.
    VectorExport,
}

impl ViewInputs {
    /// Nothing given: everything derived or defaulted.
    pub fn new() -> Self {
        ViewInputs::default()
    }

    /// An area in Web Mercator metres (`xmin > xmax` across the date line).
    pub fn bounds(mut self, bounds: [f64; 4]) -> Self {
        self.bounds = Some(bounds);
        self
    }

    /// The camera's centre, longitude / latitude (with a zoom).
    pub fn center(mut self, lon: f64, lat: f64) -> Self {
        self.center = Some((lon, lat));
        self
    }

    pub fn zoom(mut self, zoom: f64) -> Self {
        self.zoom = Some(zoom);
        self
    }

    /// The image's width and height, in `units`.
    pub fn size(mut self, width: f64, height: f64, units: Units) -> Self {
        self.size = Some((width, height));
        self.units = units;
        self
    }

    pub fn dpi(mut self, dpi: f64) -> Self {
        self.dpi = Some(dpi);
        self
    }
}

/// Where a view is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum Context {
    /// On its own.
    #[default]
    Standalone,
    /// In a plot: its extent (Web Mercator), its panel in pixels, its device.
    Plot { extent: [f64; 4], panel_px: (u32, u32), device: Device },
}

/// Where a value of a resolved view came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    Given,
    Derived,
    Default,
    Context,
}

impl Provenance {
    /// `"given"`, `"derived"`, `"default"` or `"context"`.
    pub fn name(self) -> &'static str {
        match self {
            Provenance::Given => "given",
            Provenance::Derived => "derived",
            Provenance::Default => "default",
            Provenance::Context => "context",
        }
    }
}

/// Where each of a view's four values came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Provenances {
    pub extent: Provenance,
    pub size: Provenance,
    pub dpi: Provenance,
    pub zoom: Provenance,
}

/// A resolved view: everything a render needs, always consistent.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct View {
    /// The extent shown, Web Mercator metres (`xmin > xmax` across the date line).
    pub bounds: [f64; 4],
    /// The image, in pixels.
    pub width: u32,
    pub height: u32,
    pub dpi: f64,
    pub zoom: f64,
    pub provenance: Provenances,
    pub notes: Vec<Note>,
}

impl View {
    /// The centre of the view, longitude / latitude (across the date line
    /// when `xmin > xmax`).
    pub fn center(&self) -> (f64, f64) {
        let (x, y) = centre(self.bounds);
        mercator_to_lonlat(x, y)
    }

    /// The true map scale at the centre latitude, as N of 1:N (see
    /// [`scale_at`]).
    pub fn scale(&self) -> f64 {
        scale_at(self.zoom, (self.bounds[1] + self.bounds[3]) / 2.0)
    }

    /// The values basemapper chose because nothing gave them: *decided*
    /// notes (§4.8), shown on request, never interrupting.
    pub fn decided(&self) -> Vec<Note> {
        let mut notes = Vec::new();
        let p = &self.provenance;
        if p.zoom == Provenance::Derived {
            notes.push(Note::new(Level::Decided, "zoom_auto", format!("zoom {:.2} (auto)", self.zoom)));
        }
        if p.dpi == Provenance::Default {
            notes.push(Note::new(Level::Decided, "dpi_default", format!("{} dpi (default)", self.dpi)));
        }
        if p.size == Provenance::Default {
            notes.push(Note::new(Level::Decided, "size_default", format!("{} x {} px (default)", self.width, self.height)));
        }
        if p.extent == Provenance::Default {
            notes.push(Note::new(Level::Decided, "extent_default", "the whole world (no bounds, center or clip given)"));
        }
        notes
    }

    /// MapLibre's device pixels per CSS pixel.
    pub fn pixel_ratio(&self) -> f64 {
        self.dpi / CSS_DPI
    }

    /// The view's size in CSS pixels (MapLibre's container size).
    pub fn css_size(&self) -> (f64, f64) {
        (self.width as f64 / self.pixel_ratio(), self.height as f64 / self.pixel_ratio())
    }
}

/// Web Mercator metres of a longitude / latitude point (latitude clamped to
/// the projection's extent).
pub fn lonlat_to_mercator(lon: f64, lat: f64) -> (f64, f64) {
    let lat = lat.clamp(-MAX_LAT, MAX_LAT);
    let r = HALF / std::f64::consts::PI;
    (lon.to_radians() * r, (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln() * r)
}

/// The true map scale, as N of 1:N, at `zoom` and Web Mercator northing `y`:
/// the zoom fixes the ground size of a CSS pixel (1/96 inch), so the scale
/// is the same at any dpi.
pub fn scale_at(zoom: f64, y: f64) -> f64 {
    let r = HALF / std::f64::consts::PI;
    let lat = 2.0 * (y / r).exp().atan() - std::f64::consts::FRAC_PI_2;
    metres_per_px(zoom) * lat.cos() * CSS_DPI / 0.0254
}

/// Longitude / latitude of a Web Mercator point.
pub fn mercator_to_lonlat(x: f64, y: f64) -> (f64, f64) {
    let r = HALF / std::f64::consts::PI;
    ((x / r).to_degrees(), (2.0 * (y / r).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees())
}

/// Web Mercator metres per CSS pixel at `zoom` (512 px tiles).
pub fn metres_per_px(zoom: f64) -> f64 {
    CIRC / (512.0 * 2f64.powf(zoom))
}

fn zoom_for(metres_per_px: f64) -> f64 {
    (CIRC / (512.0 * metres_per_px)).log2()
}

/// The lowest zoom at which the world fills a CSS-pixel view.
fn min_zoom(css: (f64, f64)) -> f64 {
    (css.0.max(css.1) / 512.0).log2()
}

/// A box's width in metres, across the date line when `xmin > xmax`.
fn width_m(b: [f64; 4]) -> f64 {
    if b[2] >= b[0] {
        b[2] - b[0]
    } else {
        (HALF - b[0]) + (b[2] + HALF)
    }
}

fn wrap_x(x: f64) -> f64 {
    (x + HALF).rem_euclid(CIRC) - HALF
}

fn centre(b: [f64; 4]) -> (f64, f64) {
    (wrap_x(b[0] + width_m(b) / 2.0), (b[1] + b[3]) / 2.0)
}

/// The box of a `css` view at `zoom` around `(cx, cy)`, the centre moved so
/// the view stops at the poles; and whether it moved.
fn frame(cx: f64, cy: f64, css: (f64, f64), zoom: f64) -> ([f64; 4], bool) {
    let m = metres_per_px(zoom);
    let (hw, hh) = (css.0 * m / 2.0, css.1 * m / 2.0);
    let cy2 = if 2.0 * hh >= CIRC { 0.0 } else { cy.clamp(-HALF + hh, HALF - hh) };
    let moved = (cy2 - cy).abs() > 1e-6;
    let (x0, x1) = if 2.0 * hw >= CIRC - 1e-6 { (-HALF, HALF) } else { (wrap_x(cx - hw), wrap_x(cx + hw)) };
    let snap = |y: f64| if (y.abs() - HALF).abs() < 1e-3 { HALF.copysign(y) } else { y };
    ([x0, snap(cy2 - hh), x1, snap(cy2 + hh)], moved)
}

/// The image's pixels and CSS size for a size in `units` at `dpi`.
fn image(size: (f64, f64), units: Units, dpi: f64) -> ((u32, u32), (f64, f64)) {
    match units.per_inch() {
        None => ((size.0.round() as u32, size.1.round() as u32), (size.0 * CSS_DPI / dpi, size.1 * CSS_DPI / dpi)),
        Some(per_inch) => {
            let (w, h) = (size.0 / per_inch, size.1 / per_inch);
            (((w * dpi).round() as u32, (h * dpi).round() as u32), (w * CSS_DPI, h * CSS_DPI))
        }
    }
}

fn invalid(msg: impl Into<String>) -> BasemapError {
    BasemapError::InvalidInput(msg.into())
}

fn check(inputs: &ViewInputs) -> Result<(), BasemapError> {
    if let Some(b) = inputs.bounds {
        if b.iter().any(|v| !v.is_finite()) {
            return Err(invalid("bounds must be finite"));
        }
        if b[1] >= b[3] {
            return Err(invalid("bounds must have ymin < ymax"));
        }
        if b[0] == b[2] {
            return Err(invalid("bounds must have a width (xmin != xmax)"));
        }
    }
    if let Some((lon, lat)) = inputs.center {
        if !lon.is_finite() || !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
            return Err(invalid("center must be a finite longitude and a latitude between -90 and 90"));
        }
    }
    if let Some(z) = inputs.zoom {
        if !z.is_finite() || !(0.0..=MAX_ZOOM).contains(&z) {
            return Err(invalid(format!("zoom must be between 0 and {MAX_ZOOM}, not {z}")));
        }
    }
    if let Some(d) = inputs.dpi {
        if !d.is_finite() || d <= 0.0 {
            return Err(invalid(format!("dpi must be a number > 0, not {d}")));
        }
    }
    if let Some((w, h)) = inputs.size {
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            return Err(invalid("width and height must be numbers > 0"));
        }
    }
    Ok(())
}

/// Resolve a view from what the user gave, in its context.
pub fn resolve(inputs: &ViewInputs, context: &Context) -> Result<View, BasemapError> {
    check(inputs)?;
    if inputs.bounds.is_some() && inputs.center.is_some() {
        return Err(invalid("give either bounds (an area) or center with zoom (the camera), not both"));
    }
    match context {
        Context::Plot { extent, panel_px, device } => resolve_in_plot(inputs, *extent, *panel_px, *device),
        Context::Standalone => resolve_standalone(inputs),
    }
}

fn resolve_in_plot(inputs: &ViewInputs, extent: [f64; 4], panel: (u32, u32), device: Device) -> Result<View, BasemapError> {
    if inputs.bounds.is_some() || inputs.center.is_some() {
        return Err(invalid(
            "a basemap in a plot takes its extent from the plot: remove bounds / center, or draw it on its own",
        ));
    }
    if inputs.size.is_some() {
        return Err(invalid("a basemap in a plot takes its size from the plot panel: remove width / height"));
    }
    if inputs.zoom.is_some() && inputs.dpi.is_some() {
        return Err(invalid(
            "in a plot, the extent and the image are the plot's: give zoom or dpi, not both (zoom gives an effective dpi; dpi gives the zoom)",
        ));
    }
    let (pw, ph) = (panel.0.max(1) as f64, panel.1.max(1) as f64);
    let mut notes = Vec::new();
    let (dpi, dpi_from, zoom, zoom_from) = match inputs.zoom {
        Some(z) => {
            // The plot fixes the extent and the pixels: the dpi follows.
            let needed = width_m(extent) / metres_per_px(z);
            let dpi = CSS_DPI * pw / needed;
            let device_dpi = device_dpi(device);
            notes.push(Note::new(
                Level::Adjusted,
                "effective_dpi",
                format!(
                    "zoom {z} drawn at an effective {dpi:.0} dpi on a {:.0} dpi device: labels at {:.2}x their size",
                    device_dpi.0,
                    dpi / device_dpi.0
                ),
            ));
            (dpi, Provenance::Derived, z, Provenance::Given)
        }
        None => {
            let (dpi, from) = match inputs.dpi {
                Some(d) => (d, Provenance::Given),
                None => {
                    let (d, from) = device_dpi(device);
                    if device == Device::VectorExport {
                        notes.push(Note::new(
                            Level::Adjusted,
                            "default_dpi",
                            format!("a vector device has no dpi: the basemap is rendered at {d:.0} dpi"),
                        ));
                    }
                    (d, from)
                }
            };
            let css = (pw * CSS_DPI / dpi, ph * CSS_DPI / dpi);
            let z = zoom_for((width_m(extent) / css.0).max((extent[3] - extent[1]) / css.1));
            (dpi, from, z, Provenance::Derived)
        }
    };
    let css = (pw * CSS_DPI / dpi, ph * CSS_DPI / dpi);
    let (cx, cy) = centre(extent);
    let (bounds, _) = frame(cx, cy, css, zoom);
    Ok(View {
        bounds,
        width: panel.0,
        height: panel.1,
        dpi,
        zoom,
        provenance: Provenances { extent: Provenance::Context, size: Provenance::Context, dpi: dpi_from, zoom: zoom_from },
        notes,
    })
}

/// A device's dpi, and where it comes from.
fn device_dpi(device: Device) -> (f64, Provenance) {
    match device {
        Device::Screen { pixel_ratio } => (CSS_DPI * pixel_ratio.max(f64::MIN_POSITIVE), Provenance::Context),
        Device::RasterExport { dpi } => (dpi, Provenance::Context),
        Device::VectorExport => (VECTOR_EXPORT_DPI, Provenance::Default),
    }
}

fn resolve_standalone(inputs: &ViewInputs) -> Result<View, BasemapError> {
    let (dpi, dpi_from) = match inputs.dpi {
        Some(d) => (d, Provenance::Given),
        None => (DEFAULT_DPI, Provenance::Default),
    };
    let mut notes = Vec::new();

    if let Some((lon, lat)) = inputs.center {
        // The camera: centre + zoom (+ size, dpi) give the area shown.
        let Some(zoom) = inputs.zoom else {
            return Err(invalid(
                "a center needs a zoom; for an area around a point, use bounds = around(center, span)",
            ));
        };
        let (px, css, size_from) = size_or_default(inputs, dpi);
        let z = zoom.max(min_zoom(css));
        let (cx, cy) = lonlat_to_mercator(lon, lat);
        let (bounds, moved) = frame(cx, cy, css, z);
        if z > zoom + 1e-12 || moved || lat.abs() > MAX_LAT {
            notes.push(Note::new(
                Level::Adjusted,
                "view_constrained",
                "the view was kept inside the map's world: the zoom raised or the centre moved so it stops at the poles",
            ));
        }
        return Ok(View {
            bounds,
            width: px.0,
            height: px.1,
            dpi,
            zoom: z,
            provenance: Provenances { extent: Provenance::Derived, size: size_from, dpi: dpi_from, zoom: Provenance::Given },
            notes,
        });
    }

    let (b, extent_from) = match inputs.bounds {
        Some(b) => (b, Provenance::Given),
        None => ([-HALF, -HALF, HALF, HALF], Provenance::Default),
    };
    let (bw, bh) = (width_m(b), b[3] - b[1]);
    let (cx, cy) = centre(b);

    match (inputs.zoom, inputs.size) {
        (Some(zoom), Some(size)) => {
            if inputs.units.per_inch().is_some() {
                return Err(invalid(
                    "bounds, zoom and a width / height in physical units fix the map completely: drop one of bounds, zoom or width / height",
                ));
            }
            if inputs.dpi.is_some() {
                return Err(invalid(
                    "bounds, zoom, width / height and dpi over-determine the map: give any three (without zoom it is derived; without dpi, dpi is; without width / height, the size is)",
                ));
            }
            // Extent + zoom + pixels: the dpi follows.
            let m = metres_per_px(zoom);
            let dpi = CSS_DPI * (size.0 / (bw / m)).min(size.1 / (bh / m));
            let css = (size.0 * CSS_DPI / dpi, size.1 * CSS_DPI / dpi);
            let (bounds, _) = frame(cx, cy, css, zoom);
            Ok(View {
                bounds,
                width: size.0.round() as u32,
                height: size.1.round() as u32,
                dpi,
                zoom,
                provenance: Provenances { extent: extent_from, size: Provenance::Given, dpi: Provenance::Derived, zoom: Provenance::Given },
                notes,
            })
        }
        (Some(zoom), None) => {
            // Extent + zoom + dpi: the image size follows (this area at this detail).
            let m = metres_per_px(zoom);
            let css = (bw / m, bh / m);
            Ok(View {
                bounds: b,
                width: (css.0 * dpi / CSS_DPI).round().max(1.0) as u32,
                height: (css.1 * dpi / CSS_DPI).round().max(1.0) as u32,
                dpi,
                zoom,
                provenance: Provenances { extent: extent_from, size: Provenance::Derived, dpi: dpi_from, zoom: Provenance::Given },
                notes,
            })
        }
        (None, _) => {
            // Extent + image size + dpi: the zoom follows (automatic zoom).
            let (px, css, size_from) = size_or_default(inputs, dpi);
            let fitted = zoom_for((bw / css.0).max(bh / css.1));
            let z = fitted.max(min_zoom(css));
            let (bounds, moved) = frame(cx, cy, css, z);
            if extent_from == Provenance::Given && (z > fitted + 1e-12 || moved) {
                notes.push(Note::new(
                    Level::Adjusted,
                    "view_constrained",
                    "the bounds reach past the map's world: the view shows the part inside it",
                ));
            }
            Ok(View {
                bounds,
                width: px.0,
                height: px.1,
                dpi,
                zoom: z,
                provenance: Provenances { extent: extent_from, size: size_from, dpi: dpi_from, zoom: Provenance::Derived },
                notes,
            })
        }
    }
}

/// The image's pixels and CSS size: the given size, or the default (800 x
/// 600 CSS px, scaled with the dpi).
fn size_or_default(inputs: &ViewInputs, dpi: f64) -> ((u32, u32), (f64, f64), Provenance) {
    match inputs.size {
        Some(size) => {
            let (px, css) = image(size, inputs.units, dpi);
            (px, css, Provenance::Given)
        }
        None => {
            let css = DEFAULT_CSS_SIZE;
            let px = ((css.0 * dpi / CSS_DPI).round() as u32, (css.1 * dpi / CSS_DPI).round() as u32);
            (px, css, Provenance::Default)
        }
    }
}

/// A map CRS other than Web Mercator, as the view needs it: a point in it
/// (`origin`, in the CRS's units), the same point in Web Mercator, and the
/// Web Mercator metres per CRS unit there. The host computes these with its
/// projection library (`sf`, `pyproj`); the view's arithmetic stays here.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CrsFrame {
    pub name: String,
    pub origin: (f64, f64),
    pub origin_3857: (f64, f64),
    pub scale: f64,
}

impl CrsFrame {
    pub fn new(name: impl Into<String>, origin: (f64, f64), origin_3857: (f64, f64), scale: f64) -> Result<CrsFrame, BasemapError> {
        let finite = [origin.0, origin.1, origin_3857.0, origin_3857.1, scale].iter().all(|v| v.is_finite());
        if !finite || scale <= 0.0 {
            return Err(invalid("a CRS frame needs a finite origin and a scale > 0"));
        }
        Ok(CrsFrame { name: name.into(), origin, origin_3857, scale })
    }

    fn to_3857(&self, b: [f64; 4]) -> [f64; 4] {
        let (o, m, k) = (self.origin, self.origin_3857, self.scale);
        [m.0 + (b[0] - o.0) * k, m.1 + (b[1] - o.1) * k, m.0 + (b[2] - o.0) * k, m.1 + (b[3] - o.1) * k]
    }

    fn from_3857(&self, b: [f64; 4]) -> [f64; 4] {
        let (o, m, k) = (self.origin, self.origin_3857, self.scale);
        [o.0 + (b[0] - m.0) / k, o.1 + (b[1] - m.1) / k, o.0 + (b[2] - m.0) / k, o.1 + (b[3] - m.1) / k]
    }
}

/// Resolve a standalone view of a map in `frame`'s CRS. `inputs.bounds` is
/// in that CRS (`center` stays longitude / latitude). The extent, zoom,
/// size and dpi are related at the frame's scale, as for a Web Mercator map
/// at that point. Returns the view (its bounds an equivalent Web Mercator
/// box) and the extent shown, in the CRS: the area to build the warp mesh
/// over, for the view's pixel size.
pub fn resolve_in_crs(inputs: &ViewInputs, frame: &CrsFrame) -> Result<(View, [f64; 4]), BasemapError> {
    if inputs.bounds.is_none() && inputs.center.is_none() {
        return Err(invalid(format!(
            "a map in {} needs an extent: bounds (or a layer), or center with zoom",
            frame.name
        )));
    }
    if let Some(b) = inputs.bounds {
        if b[0] >= b[2] {
            return Err(invalid(format!("bounds in {} must have xmin < xmax", frame.name)));
        }
    }
    let mut equivalent = inputs.clone();
    equivalent.bounds = inputs.bounds.map(|b| frame.to_3857(b));
    let view = resolve(&equivalent, &Context::Standalone)?;
    let crs_bounds = frame.from_3857(view.bounds);
    Ok((view, crs_bounds))
}

/// `[west, south, east, north]` in longitude / latitude of the area `span`
/// (`(width, height)`, in metres on the ground) around `center`, computed on
/// a sphere of the Earth's mean radius.
pub fn around(center: (f64, f64), span: (f64, f64)) -> Result<[f64; 4], BasemapError> {
    const SPHERE: f64 = 6_371_008.8;
    let (lon, lat) = center;
    if !lon.is_finite() || !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
        return Err(invalid("around(): center must be a longitude and a latitude between -90 and 90"));
    }
    if !(span.0 > 0.0 && span.1 > 0.0 && span.0.is_finite() && span.1.is_finite()) {
        return Err(invalid("around(): span must be positive distances"));
    }
    let dlat = (span.1 / 2.0 / SPHERE).to_degrees();
    let dlon = (span.0 / 2.0 / (SPHERE * lat.to_radians().cos())).to_degrees();
    Ok([lon - dlon, lat - dlat, lon + dlon, lat + dlat])
}

/// Metres per one of around()'s `units`: `"m"`, `"km"`, `"mi"` or `"ft"`.
pub fn distance_unit(units: &str) -> Result<f64, BasemapError> {
    match units {
        "m" => Ok(1.0),
        "km" => Ok(1000.0),
        "mi" => Ok(1609.344),
        "ft" => Ok(0.3048),
        _ => Err(invalid(format!("around(): units must be \"m\", \"km\", \"mi\" or \"ft\", not {units:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crs_frame_relates_the_view_at_its_scale() {
        // 1 CRS unit = 2 Web Mercator metres at the origin.
        let frame = CrsFrame::new("TEST:1", (500_000.0, 4_000_000.0), (-12_000_000.0, 5_000_000.0), 2.0).unwrap();
        let inputs = ViewInputs::new().bounds([499_500.0, 3_999_700.0, 500_500.0, 4_000_300.0]).size(800.0, 600.0, Units::Px);
        let (v, crs) = resolve_in_crs(&inputs, &frame).unwrap();
        // The same view as a 2000 x 1200 m Web Mercator box there.
        let direct = resolve(
            &ViewInputs::new().bounds([-12_001_000.0, 4_999_400.0, -11_999_000.0, 5_000_600.0]).size(800.0, 600.0, Units::Px),
            &Context::Standalone,
        )
        .unwrap();
        assert!((v.zoom - direct.zoom).abs() < 1e-12, "{} vs {}", v.zoom, direct.zoom);
        assert_eq!((v.width, v.height), (800, 600));
        // The extent shown, in the CRS: 1000 m wide, grown to 4:3 (750 m tall).
        assert!((crs[2] - crs[0] - 1000.0).abs() < 1e-6 && (crs[3] - crs[1] - 750.0).abs() < 1e-6, "{crs:?}");
        assert!(((crs[0] + crs[2]) / 2.0 - 500_000.0).abs() < 1e-6);
        // An extent is required, and the frame is checked.
        assert!(resolve_in_crs(&ViewInputs::new(), &frame).unwrap_err().to_string().contains("TEST:1"));
        assert!(CrsFrame::new("x", (0.0, 0.0), (0.0, 0.0), 0.0).is_err());
    }
}

#[cfg(test)]
mod center_tests {
    use super::*;

    #[test]
    fn the_centre_crosses_the_date_line() {
        let (x0, y0) = lonlat_to_mercator(170.0, -10.0);
        let (x1, y1) = lonlat_to_mercator(-170.0, 10.0);
        let v = resolve(&ViewInputs::new().bounds([x0, y0, x1, y1]), &Context::Standalone).unwrap();
        let (lon, lat) = v.center();
        assert!((lon.abs() - 180.0).abs() < 1e-9 && lat.abs() < 1e-9, "({lon}, {lat})");
        let (x, y) = lonlat_to_mercator(-111.9, 40.7);
        let (lon, lat) = mercator_to_lonlat(x, y);
        assert!((lon + 111.9).abs() < 1e-12 && (lat - 40.7).abs() < 1e-12);
    }
}
