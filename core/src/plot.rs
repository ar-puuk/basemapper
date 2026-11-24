//! Basemaps in plots (docs/design/api-review.md §4.5, F1, F8): one plan for
//! every plot library (ggplot2, tmap, matplotlib, plotnine).
//!
//! A plot library describes its panel ([`Panel`]): the limits in the plot's
//! CRS, the size in device pixels, and the device ([`Device`], whose dpi the
//! D16 rule gives). [`plan`] turns that and what the basemap gives (a zoom, or
//! a dpi) into the [`View`] to render and, for a plot not in Web Mercator,
//! the [`Warp`] into its CRS:
//!
//! - **Web Mercator:** the panel's limits are the extent; no warp.
//! - **Longitude / latitude:** the core builds the mesh itself (the
//!   projection is a formula), so R and Python need no projection library.
//! - **Any other CRS:** the host computes the mesh (`sf`, `pyproj`) of panel
//!   pixels to Web Mercator.
//!
//! In a warped plot the zoom follows the map's scale at the panel's centre,
//! where the reader looks: one panel pixel there is as many Web Mercator
//! metres as the warp maps it to.
//!
//! [`AttributionLayout`] is the credit line's layout, computed once here and
//! drawn by each library with its own text primitive.

use crate::view::{self, Context, Device, View, ViewInputs, HALF};
use crate::warp::{Mesh, Warp};
use crate::BasemapError;

/// The CRS of a plot panel.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PanelCrs {
    /// EPSG:3857: no warp.
    WebMercator,
    /// EPSG:4326 longitude / latitude: the core builds the mesh.
    LonLat,
    /// Any other CRS, by name (e.g. `"EPSG:32612"`), with the mesh of panel
    /// pixels to Web Mercator the host computed for the panel's size.
    Other { name: String, mesh: Mesh },
}

/// A plot panel, as the plot library describes it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Panel {
    /// The panel's extent in its CRS, `[xmin, ymin, xmax, ymax]`.
    pub limits: [f64; 4],
    pub crs: PanelCrs,
    /// The panel's size in device pixels.
    pub px: (u32, u32),
    pub device: Device,
}

impl Panel {
    pub fn new(limits: [f64; 4], crs: PanelCrs, px: (u32, u32), device: Device) -> Result<Panel, BasemapError> {
        if limits.iter().any(|v| !v.is_finite()) || limits[0] >= limits[2] || limits[1] >= limits[3] {
            return Err(BasemapError::InvalidInput("plot panel limits must be finite with xmin < xmax and ymin < ymax".into()));
        }
        if px.0 == 0 || px.1 == 0 {
            return Err(BasemapError::InvalidInput("a plot panel needs a size of at least 1 x 1 px".into()));
        }
        Ok(Panel { limits, crs, px, device })
    }
}

/// The view to render for `panel`, and the warp into its CRS (none for a
/// Web Mercator panel). `inputs` may give a zoom or a dpi; an extent, a
/// centre or a size is an error (the plot gives them).
pub fn plan(panel: &Panel, inputs: &ViewInputs) -> Result<(View, Option<Warp>), BasemapError> {
    let (w, h) = panel.px;
    let (extent, warp) = match &panel.crs {
        PanelCrs::WebMercator => (panel.limits, None),
        PanelCrs::LonLat => {
            let mesh = lonlat_mesh(panel.limits, w, h)?;
            (lonlat_extent(panel.limits, w, h), Some(Warp::new("EPSG:4326", panel.limits, mesh)?))
        }
        PanelCrs::Other { name, mesh } => (equivalent_extent(mesh, w, h)?, Some(Warp::new(name.clone(), panel.limits, mesh.clone())?)),
    };
    let view = view::resolve(inputs, &Context::Plot { extent, panel_px: panel.px, device: panel.device })?;
    Ok((view, warp))
}

/// The mesh of a lon/lat panel: x is linear in longitude, y follows
/// Mercator's latitude scale, so rows are dense enough that interpolating
/// between them is off by under a tenth of a pixel.
pub fn lonlat_mesh(limits: [f64; 4], width: u32, height: u32) -> Result<Mesh, BasemapError> {
    let [west, south, east, north] = limits;
    if south < -90.0 || north > 90.0 {
        return Err(BasemapError::InvalidInput("a lon/lat panel's latitudes must be within -90 and 90".into()));
    }
    let rows = (height as usize / 8 + 2).clamp(2, 513);
    // Latitudes past Web Mercator's limit have no position in it: NaN nodes
    // leave them transparent.
    let max_lat = 85.051_128_779_806_59;
    Mesh::from_fn(2, rows, width, height, |x, y| {
        let lon = west + x / width as f64 * (east - west);
        let lat = north - y / height as f64 * (north - south);
        if lat.abs() > max_lat {
            (f64::NAN, f64::NAN)
        } else {
            view::lonlat_to_mercator(lon, lat)
        }
    })
}

/// [`equivalent_extent`] of a lon/lat panel, exactly: at latitude φ a degree
/// of longitude is C / 360 Web Mercator metres and a degree of latitude
/// C / 360 / cos φ, at the centre's latitude (kept within Web Mercator's).
fn lonlat_extent(limits: [f64; 4], width: u32, height: u32) -> [f64; 4] {
    let [west, south, east, north] = limits;
    let max_lat = 85.051_128_779_806_59;
    let lat = ((south + north) / 2.0).clamp(-max_lat, max_lat);
    let per_degree = 2.0 * HALF / 360.0;
    let sx = (east - west) / width as f64 * per_degree;
    let sy = (north - south) / height as f64 * per_degree / lat.to_radians().cos();
    let m = (sx * sy).sqrt();
    let (cx, cy) = view::lonlat_to_mercator((west + east) / 2.0, lat);
    let (hw, hh) = (width as f64 * m / 2.0, height as f64 * m / 2.0);
    [cx - hw, (cy - hh).max(-HALF), cx + hw, (cy + hh).min(HALF)]
}

/// A Web Mercator box with the panel's pixel size and the mesh's scale at
/// the panel's centre (the geometric mean of its x and y scales): the extent
/// a plot in Web Mercator at that scale would have, which gives the zoom.
fn equivalent_extent(mesh: &Mesh, width: u32, height: u32) -> Result<[f64; 4], BasemapError> {
    let (w, h) = (width as f64, height as f64);
    let at = |x: f64, y: f64| mesh.position(x, y, w, h);
    let none = || BasemapError::InvalidInput("the plot's centre has no position in Web Mercator".into());
    let (cx, cy) = at(w / 2.0, h / 2.0).ok_or_else(none)?;
    // Scales over the middle fifth of the panel each way, or a narrower step
    // where that reaches past Web Mercator's area (a panel up to a pole).
    let dist = |a: (f64, f64), b: (f64, f64)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let scale = |ux: f64, uy: f64, d: f64| {
        let mut d = d;
        for _ in 0..8 {
            if let (Some(a), Some(b)) = (at(w / 2.0 - ux * d, h / 2.0 - uy * d), at(w / 2.0 + ux * d, h / 2.0 + uy * d)) {
                return Some(dist(a, b) / (2.0 * d));
            }
            d /= 4.0;
        }
        None
    };
    let sx = scale(1.0, 0.0, w / 10.0).ok_or_else(none)?;
    let sy = scale(0.0, 1.0, h / 10.0).ok_or_else(none)?;
    let m = (sx * sy).sqrt();
    if !(m.is_finite() && m > 0.0) {
        return Err(none());
    }
    let (hw, hh) = (w * m / 2.0, h * m / 2.0);
    Ok([cx - hw, (cy - hh).max(-HALF), cx + hw, (cy + hh).min(HALF)])
}

// ── Attribution ──────────────────────────────────────────────────────────────

/// The credit line's layout in a plot: small text in the bottom-right
/// corner on a translucent white box, wrapped to the panel's width. Sizes
/// are in points, so every library draws it the same at any dpi.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AttributionLayout {
    /// The wrapped lines, top to bottom.
    pub lines: Vec<String>,
    pub font_size_pt: f64,
    /// The text colour, `#rrggbb`.
    pub color: String,
    /// The box's fill, `#rrggbb`, and its opacity (0-1).
    pub background: String,
    pub background_alpha: f64,
    /// The gap between the text and the box's edge, and between the box and
    /// the panel's corner.
    pub padding_pt: f64,
}

/// The font size of the credit line.
pub const ATTRIBUTION_PT: f64 = 6.0;

/// The layout of `text` for a panel `width_pt` points wide (none for empty
/// text). Lines are wrapped at word boundaries to the characters that fit
/// (half a font size each, at least 20), never splitting a word.
pub fn attribution_layout(text: &str, width_pt: f64) -> Option<AttributionLayout> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    let chars = ((width_pt / (ATTRIBUTION_PT * 0.5)).floor() as usize).max(20);
    Some(AttributionLayout {
        lines: wrap(&text, chars),
        font_size_pt: ATTRIBUTION_PT,
        color: "#333333".into(),
        background: "#ffffff".into(),
        background_alpha: 0.7,
        padding_pt: 1.5,
    })
}

/// `text` wrapped greedily at spaces to lines of at most `width` characters
/// (a longer word gets a line of its own).
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::Level;
    use crate::view::Provenance;

    const SCREEN: Device = Device::Screen { pixel_ratio: 1.0 };

    #[test]
    fn a_web_mercator_panel_is_its_own_extent() {
        let p = Panel::new([-1e6, -5e5, 1e6, 5e5], PanelCrs::WebMercator, (800, 400), SCREEN).unwrap();
        let (v, warp) = plan(&p, &ViewInputs::new()).unwrap();
        assert!(warp.is_none());
        assert_eq!((v.width, v.height, v.dpi), (800, 400, 96.0));
        assert!(v.bounds.iter().zip([-1e6, -5e5, 1e6, 5e5]).all(|(a, b)| (a - b).abs() < 1e-6), "{:?}", v.bounds);
        assert_eq!(v.provenance.zoom, Provenance::Derived);
        assert!((view::metres_per_px(v.zoom) - 2500.0).abs() < 1e-6, "2e6 m over 800 px");
    }

    #[test]
    fn a_lonlat_mesh_is_exact_to_a_tenth_of_a_pixel() {
        // A tall panel at high latitude, where Mercator's y scale changes
        // most: interpolated rows against the formula, at every pixel centre.
        let limits = [10.0, 50.0, 30.0, 80.0];
        let (w, h) = (300, 900);
        let mesh = lonlat_mesh(limits, w, h).unwrap();
        let (_, wy0) = view::lonlat_to_mercator(0.0, 50.0);
        let (_, wy1) = view::lonlat_to_mercator(0.0, 80.0);
        let metres_per_px_y = (wy1 - wy0) / h as f64;
        let mut worst: f64 = 0.0;
        for py in 0..h {
            let lat = 80.0 - (py as f64 + 0.5) / h as f64 * 30.0;
            let (_, want) = view::lonlat_to_mercator(20.0, lat);
            let (_, got) = mesh.position(150.5, py as f64 + 0.5, w as f64, h as f64).unwrap();
            worst = worst.max((got - want).abs() / metres_per_px_y);
        }
        assert!(worst < 0.1, "worst error {worst} px");
    }

    #[test]
    fn a_lonlat_panel_warps_at_its_centre_scale() {
        // 1 degree per 100 px at the equator: 1113 m per px.
        let p = Panel::new([-2.0, -1.0, 2.0, 1.0], PanelCrs::LonLat, (400, 200), SCREEN).unwrap();
        let (v, warp) = plan(&p, &ViewInputs::new()).unwrap();
        let warp = warp.expect("a lon/lat panel is warped");
        assert_eq!((warp.crs.as_str(), warp.bounds), ("EPSG:4326", [-2.0, -1.0, 2.0, 1.0]));
        let m = 2.0 * HALF / 360.0 / 100.0;
        assert!((view::metres_per_px(v.zoom) / m - 1.0).abs() < 1e-3, "{} vs {m}", view::metres_per_px(v.zoom));
        // Past Web Mercator's latitudes the mesh has no position.
        let polar = Panel::new([0.0, 80.0, 10.0, 90.0], PanelCrs::LonLat, (100, 100), SCREEN).unwrap();
        let (_, w) = plan(&polar, &ViewInputs::new()).unwrap();
        assert!(w.unwrap().mesh.nodes.iter().any(|n| n.0.is_nan()));
    }

    #[test]
    fn a_host_mesh_gives_the_same_zoom_as_the_formula() {
        // The lon/lat mesh handed in as a host's: the same scale, so the same
        // zoom, as the exact lon/lat path.
        let limits = [-120.0, 30.0, -100.0, 45.0];
        let exact = Panel::new(limits, PanelCrs::LonLat, (600, 450), SCREEN).unwrap();
        let mesh = lonlat_mesh(limits, 600, 450).unwrap();
        let hosted = Panel::new(limits, PanelCrs::Other { name: "EPSG:4326 (host)".into(), mesh }, (600, 450), SCREEN).unwrap();
        let (a, _) = plan(&exact, &ViewInputs::new()).unwrap();
        let (b, wb) = plan(&hosted, &ViewInputs::new()).unwrap();
        assert!((a.zoom - b.zoom).abs() < 1e-3, "{} vs {}", a.zoom, b.zoom);
        assert_eq!(wb.unwrap().crs, "EPSG:4326 (host)");
    }

    #[test]
    fn a_zoom_or_scale_from_the_basemap_and_conflicts() {
        let p = Panel::new([-2.0, -1.0, 2.0, 1.0], PanelCrs::LonLat, (400, 200), Device::RasterExport { dpi: 300.0 }).unwrap();
        // A zoom: the device keeps its dpi; the zoom sets the scale of lines
        // and labels, reported as a decided note without "dpi".
        let (v, _) = plan(&p, &ViewInputs::new().zoom(6.0)).unwrap();
        assert_eq!((v.zoom, v.dpi, v.provenance.dpi), (6.0, 300.0, Provenance::Context));
        let n = &v.notes[0];
        assert_eq!((n.kind, n.level), ("symbol_scale", Level::Decided));
        assert!(n.message.contains("symbol_scale") && !n.message.contains("dpi"), "{}", n.message);
        assert!(plan(&p, &ViewInputs::new().bounds([0.0, 0.0, 1.0, 1.0])).is_err());
        assert!(Panel::new([1.0, 0.0, 0.0, 1.0], PanelCrs::LonLat, (10, 10), SCREEN).is_err());
    }

    /// In a plot the device sets the dpi, and only that: `dpi` is an error
    /// naming `scale`, and the same plot shows the same map at any dpi.
    #[test]
    fn the_device_sets_the_resolution_and_the_symbol_scale_what_is_drawn() {
        let limits = [-1.2e7, 5.0e6, -1.19e7, 5.1e6];
        let at = |dpi: f64| Device::RasterExport { dpi };
        // A 3.6 x 5 in panel.
        let panel = |dpi: f64| Panel::new(limits, PanelCrs::WebMercator, ((3.6 * dpi) as u32, (5.0 * dpi) as u32), at(dpi)).unwrap();
        let e = plan(&panel(200.0), &ViewInputs::new().dpi(168.0)).unwrap_err().to_string();
        assert!(e.contains("device sets the dpi") && e.contains("symbol_scale"), "{e}");
        let (v200, _) = plan(&panel(200.0), &ViewInputs::new().symbol_scale(0.84)).unwrap();
        let (v300, _) = plan(&panel(300.0), &ViewInputs::new().symbol_scale(0.84)).unwrap();
        assert_eq!((v200.dpi, v300.dpi), (200.0, 300.0), "the dpi is the device's");
        assert!((v200.zoom - v300.zoom).abs() < 1e-9, "the same map at any dpi: {} vs {}", v200.zoom, v300.zoom);
        assert!((v200.pixel_ratio() - 1.75).abs() < 1e-12, "0.84 at 200 dpi was dpi = 168");
        // Scale 1 by default: lines and labels at their designed size.
        let (v, _) = plan(&panel(200.0), &ViewInputs::new()).unwrap();
        assert_eq!(v.symbol_scale, 1.0);
        // A smaller scale shows more detail (a higher zoom).
        assert!((v200.zoom - v.zoom - (1.0f64 / 0.84).log2()).abs() < 1e-9);
        // The printed 1:N scale: the zoom's, over the symbol scale.
        assert!((v200.scale() - view::scale_at(v200.zoom, (v200.bounds[1] + v200.bounds[3]) / 2.0) / 0.84).abs() < 1e-6);
        assert!(plan(&panel(200.0), &ViewInputs::new().zoom(12.0).symbol_scale(0.84)).is_err());
    }

    #[test]
    fn the_credit_line_wraps_at_words() {
        let a = attribution_layout("  © OpenStreetMap contributors  © CARTO   ", 300.0).unwrap();
        assert_eq!(a.lines, ["© OpenStreetMap contributors © CARTO"], "100 characters fit");
        let a = attribution_layout("© OpenStreetMap contributors © CARTO", 60.0).unwrap();
        assert_eq!(a.lines, ["© OpenStreetMap", "contributors © CARTO"], "at least 20 characters a line");
        assert_eq!(wrap("a verylongwordthatdoesnotfit b", 5), ["a", "verylongwordthatdoesnotfit", "b"]);
        assert!(attribution_layout("   ", 300.0).is_none());
        assert_eq!((a.font_size_pt, a.background_alpha), (6.0, 0.7));
    }
}
