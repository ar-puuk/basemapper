//! `Rendering`: one render's image with everything needed to use it (where
//! it is, at what detail and resolution) and what happened making it
//! (docs/design/api-review.md D4, §4.8). One render, many conversions: a
//! PNG, an array, a georeferenced raster in R and Python.

use crate::notes::{Collector, Level, Note};
use crate::{BasemapError, RenderRequest, RenderResult};

/// A rendered map: straight-alpha RGBA pixels and where they are.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Rendering {
    /// Row-major RGBA, `width * height * 4` bytes.
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// The extent the pixels cover, `[xmin, ymin, xmax, ymax]` in `crs`
    /// units (`xmin > xmax` across the date line).
    pub bounds: [f64; 4],
    /// The pixels' CRS, `"EPSG:3857"`.
    pub crs: String,
    /// The MapLibre zoom the map was drawn at.
    pub zoom: f64,
    /// The zoom of the most detailed tiles fetched.
    pub tile_zoom: u8,
    /// Image pixels per inch (96 is one pixel per CSS pixel).
    pub dpi: f64,
    /// What basemapper decided, adjusted, or could not do.
    pub notes: Vec<Note>,
}

impl Rendering {
    /// The true map scale at the centre latitude, as N of 1:N: the zoom
    /// fixes the ground size of a CSS pixel (1/96 inch), so the scale is the
    /// same at any dpi.
    pub fn scale(&self) -> f64 {
        let cy = (self.bounds[1] + self.bounds[3]) / 2.0;
        let r = 20_037_508.342_789_244 / std::f64::consts::PI;
        let lat = 2.0 * (cy / r).exp().atan() - std::f64::consts::FRAC_PI_2;
        crate::view::metres_per_px(self.zoom) * lat.cos() * crate::view::CSS_DPI / 0.0254
    }

    /// The notes at `level`.
    pub fn notes_at(&self, level: Level) -> impl Iterator<Item = &Note> {
        self.notes.iter().filter(move |n| n.level == level)
    }

    /// The image as a PNG file's bytes.
    pub fn to_png(&self) -> Result<Vec<u8>, BasemapError> {
        crate::encode_png(&self.pixels, self.width, self.height)
    }
}

/// Render `requests` (built inside the call, so building them can note
/// things too) as a stack at `zoom` and `dpi`, collecting what happened:
/// the notes `before` (e.g. the view's), then the render's, then filter
/// names that matched nothing.
pub(crate) fn render_collecting(
    requests: impl FnOnce() -> Result<Vec<RenderRequest>, BasemapError>,
    zoom: f64,
    dpi: f64,
    before: Vec<Note>,
) -> Result<Rendering, BasemapError> {
    let collector = Collector::new();
    for n in before {
        collector.add(n);
    }
    let result: RenderResult = collector.enter(|| requests().and_then(crate::render_stack))?;
    for w in &result.warnings {
        collector.add(Note::new(Level::Problem, "filter_unmatched", w.clone()));
    }
    let b = &result.bounds;
    Ok(Rendering {
        pixels: result.pixels,
        width: result.width,
        height: result.height,
        bounds: [b.xmin, b.ymin, b.xmax, b.ymax],
        crs: format!("EPSG:{}", b.crs_epsg),
        zoom,
        tile_zoom: result.zoom_used,
        dpi,
        notes: collector.notes(),
    })
}
