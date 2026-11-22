use crate::error::BasemapError;
use crate::style::StyleInput;

const EARTH_HALF: f64 = 20_037_508.342_789_244;
const EARTH_CIRC: f64 = EARTH_HALF * 2.0;

#[derive(Debug, Clone)]
pub struct SpatialBounds {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
    pub crs_epsg: u32,
}

#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub bbox: [f64; 4],
    pub width: u32,
    pub height: u32,
    pub style_input: StyleInput,
    /// Tile detail override: the tile zoom on the 256 px grid (vector
    /// sources use one less, their 512 px tiles), instead of the zoom the
    /// view implies. `None` (the default) picks tiles as MapLibre does.
    pub tile_zoom: Option<u8>,
    pub tile_timeout_ms: u32,
    pub max_tiles: u32,
    pub tile_concurrency: u32,
    pub layers: Option<Vec<String>>,
    /// Keep every layer, drop the labels, or keep only the labels (see
    /// [`crate::features`]).
    pub labels: crate::Labels,
    /// Feature filter: plain names keep those features, `-`-prefixed names
    /// drop them (see [`crate::features::FEATURES`]).
    pub features: Option<Vec<String>>,
    /// Optional authentication token. Sent as `Authorization: Bearer <token>`
    /// for raster sources; appended as `?access_token=<token>` for Mapbox
    /// vector sources.
    pub auth_token: Option<String>,
    /// When `false`, individual tile fetch failures are logged and skipped
    /// rather than aborting the entire render.
    pub fail_on_tile_error: bool,
    /// Device pixels per CSS pixel (MapLibre's devicePixelRatio). `width` and
    /// `height` are CSS pixels and set the view and zoom; the image is
    /// `pixel_ratio` times larger, with lines, labels and icons drawn at the
    /// same size relative to the map (e.g. 2 for a sharp 2x image, or
    /// dpi / 96 for print). Vector styles use the `@2x` sprite above 1.
    pub pixel_ratio: f64,
}

impl RenderRequest {
    /// A request with default settings: automatic zoom, pixel ratio 1, all
    /// layers, 10 s tile timeout, at most 256 tiles, 16 parallel fetches,
    /// failed tiles left empty.
    pub fn new(bbox: [f64; 4], width: u32, height: u32, style_input: crate::StyleInput) -> Self {
        RenderRequest {
            bbox,
            width,
            height,
            style_input,
            tile_zoom: None,
            tile_timeout_ms: 10_000,
            max_tiles: 256,
            tile_concurrency: 16,
            layers: None,
            labels: crate::Labels::All,
            features: None,
            auth_token: None,
            fail_on_tile_error: false,
            pixel_ratio: 1.0,
        }
    }

    pub fn validate(&self) -> Result<(), BasemapError> {
        let [xmin, ymin, xmax, ymax] = self.bbox;
        if !xmin.is_finite() || !ymin.is_finite() || !xmax.is_finite() || !ymax.is_finite() {
            return Err(BasemapError::InvalidBbox(
                "bbox contains non-finite values".into(),
            ));
        }
        // Zero-width bbox is always invalid (normal or anti-meridian).
        if xmin == xmax {
            return Err(BasemapError::InvalidBbox(format!(
                "[{xmin}, {ymin}, {xmax}, {ymax}]: bbox has zero width (xmin == xmax)"
            )));
        }
        if ymin >= ymax {
            return Err(BasemapError::InvalidBbox(format!(
                "[{xmin}, {ymin}, {xmax}, {ymax}]: ymin ({ymin}) must be < ymax ({ymax})"
            )));
        }
        // y must be within the Web Mercator extent. x may extend up to one
        // world width beyond ±180° (a view showing a wrapped world copy, as
        // MapLibre draws it), as long as the bbox spans at most one world.
        if ymin < -EARTH_HALF || ymax > EARTH_HALF {
            return Err(BasemapError::InvalidBbox(format!(
                "[{xmin}, {ymin}, {xmax}, {ymax}]: y must be within \
                 ±{EARTH_HALF:.0} (Web Mercator bounds)"
            )));
        }
        let x_limit = EARTH_HALF + 2.0 * EARTH_HALF;
        if xmin < -x_limit || xmax > x_limit || (xmin < xmax && xmax - xmin > 2.0 * EARTH_HALF) {
            return Err(BasemapError::InvalidBbox(format!(
                "[{xmin}, {ymin}, {xmax}, {ymax}]: x must be within ±{x_limit:.0} \
                 and span at most one world width"
            )));
        }
        // xmin > xmax is the anti-meridian case — accepted and handled by render().
        if self.width == 0 || self.height == 0 || self.width > 16_384 || self.height > 16_384 {
            return Err(BasemapError::InvalidDimensions(self.width, self.height));
        }
        if !self.pixel_ratio.is_finite() || self.pixel_ratio <= 0.0 || self.pixel_ratio > 16.0 {
            return Err(BasemapError::InvalidBbox(format!(
                "pixel_ratio {} must be > 0 and at most 16",
                self.pixel_ratio
            )));
        }
        let (dw, dh) = (self.width as f64 * self.pixel_ratio, self.height as f64 * self.pixel_ratio);
        if dw.round() > 16_384.0 || dh.round() > 16_384.0 {
            return Err(BasemapError::InvalidDimensions(dw.round() as u32, dh.round() as u32));
        }
        if let Some(z) = self.tile_zoom {
            if z > 22 {
                return Err(BasemapError::InvalidBbox(format!(
                    "zoom {z} out of range 0–22"
                )));
            }
        }
        if let Some(ref layers) = self.layers {
            validate_layers(layers)?;
        }
        if let Some(ref features) = self.features {
            crate::features::validate_features(features)?;
        }
        Ok(())
    }
}

/// Rejects lists that mix plain IDs and minus-prefixed IDs (H3 spec requirement).
fn validate_layers(layers: &[String]) -> Result<(), BasemapError> {
    if layers.is_empty() {
        return Ok(());
    }
    let has_positive = layers.iter().any(|l| !l.starts_with('-'));
    let has_negative = layers.iter().any(|l| l.starts_with('-'));
    if has_positive && has_negative {
        return Err(BasemapError::InvalidLayerFilter(
            "layer filter mixes positive and negation (-) entries; use one mode only".into(),
        ));
    }
    Ok(())
}

/// Compute tile zoom from bbox width and target pixel width.
///
/// Handles anti-meridian bboxes (xmin > xmax) by computing the wrapped width:
/// `(EARTH_HALF − xmin) + (xmax + EARTH_HALF)`.
/// Raster tile zoom as MapLibre picks it for a raster source with tiles of
/// `tile_size` px: `round(zoom + log2(512 / tile_size))` (its `roundZoom`),
/// i.e. the rounded zoom at which one tile spans `tile_size` canvas px.
pub fn raster_tile_zoom(bbox: [f64; 4], width_px: u32, tile_size: f64) -> u8 {
    let bbox_width_m = if bbox[0] <= bbox[2] {
        bbox[2] - bbox[0]
    } else {
        (EARTH_HALF - bbox[0]) + (bbox[2] + EARTH_HALF)
    };
    let mpp = bbox_width_m / width_px as f64;
    let zoom = (EARTH_CIRC / (tile_size * mpp)).log2().round() as i32;
    zoom.clamp(0, 22) as u8
}

pub fn compute_zoom(bbox: [f64; 4], width_px: u32) -> u8 {
    let bbox_width_m = if bbox[0] <= bbox[2] {
        bbox[2] - bbox[0]
    } else {
        // Anti-meridian: west side + east side
        (EARTH_HALF - bbox[0]) + (bbox[2] + EARTH_HALF)
    };
    let mpp = bbox_width_m / width_px as f64;
    let zoom = (EARTH_CIRC / (256.0 * mpp)).log2().floor() as i32;
    zoom.clamp(0, 22) as u8
}

/// Count how many tiles cover a bbox at a given zoom level.
///
/// Handles anti-meridian bboxes (xmin > xmax) correctly: tiles from
/// `[x0..max_tile]` plus `[0..x1]` across the dateline.
pub fn tile_count(bbox: [f64; 4], zoom: u8) -> u32 {
    let (x0, y0) = mercator_to_tile(bbox[0], bbox[3], zoom);
    let (x1, y1) = mercator_to_tile(bbox[2], bbox[1], zoom);
    if y1 < y0 {
        return 0;
    }
    let y_count = y1 - y0 + 1;
    let x_count = if x0 <= x1 {
        x1 - x0 + 1
    } else {
        // Anti-meridian: [x0..max_tile] + [0..x1]
        let max_tile = (1u32 << zoom).saturating_sub(1);
        (max_tile - x0 + 1).saturating_add(x1 + 1)
    };
    x_count.saturating_mul(y_count)
}

/// Convert Web Mercator metres to slippy-map tile coordinates.
pub fn mercator_to_tile(x: f64, y: f64, zoom: u8) -> (u32, u32) {
    let n = 2u32.pow(zoom as u32) as f64;
    let tx = ((x + EARTH_HALF) / EARTH_CIRC * n).floor() as u32;
    let ty = ((EARTH_HALF - y) / EARTH_CIRC * n).floor() as u32;
    (tx.min(n as u32 - 1), ty.min(n as u32 - 1))
}

#[derive(Debug, Clone)]
pub struct RenderResult {
    pub pixels: Vec<u8>,
    pub bounds: SpatialBounds,
    pub width: u32,
    pub height: u32,
    pub zoom_used: u8,
    /// Filter names (`layers`, `features`) that matched no layer: for a
    /// stack, those that matched nothing in any of its styles.
    pub warnings: Vec<String>,
}

/// EPSG:3857 bbox of a `width x height` CSS-pixel view centred on
/// (`lon`, `lat`) at MapLibre zoom `zoom` (512 px tiles; fractional zooms
/// allowed), as MapLibre's camera shows it.
pub fn bbox_from_center(lon: f64, lat: f64, zoom: f64, width: u32, height: u32) -> [f64; 4] {
    let r = EARTH_HALF / std::f64::consts::PI;
    let cx = lon.to_radians() * r;
    let lat = lat.clamp(-85.051_128_779_806_59, 85.051_128_779_806_59);
    let cy = (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln() * r;
    let mpp = EARTH_CIRC / (512.0 * 2f64.powf(zoom));
    let (hw, hh) = (width as f64 * mpp / 2.0, height as f64 * mpp / 2.0);
    [cx - hw, cy - hh, cx + hw, cy + hh]
}

/// MapLibre zoom at which a `width x height` CSS-pixel view fits the
/// EPSG:3857 `bounds` (fitBounds without padding), and the bounds' centre
/// as (lon, lat).
pub fn fit_bounds(bounds: [f64; 4], width: u32, height: u32) -> (f64, f64, f64) {
    let r = EARTH_HALF / std::f64::consts::PI;
    let mpp = ((bounds[2] - bounds[0]) / width as f64).max((bounds[3] - bounds[1]) / height as f64);
    let zoom = (EARTH_CIRC / (512.0 * mpp)).log2();
    let (cx, cy) = ((bounds[0] + bounds[2]) / 2.0, (bounds[1] + bounds[3]) / 2.0);
    let lon = (cx / r).to_degrees();
    let lat = (2.0 * (cy / r).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees();
    (lon, lat, zoom)
}
