use crate::error::BasemapError;
use crate::style::StyleInput;

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
    pub zoom: Option<u8>,
    pub tile_timeout_ms: u32,
    pub max_tiles: u32,
    pub tile_concurrency: u32,
    pub layers: Option<Vec<String>>,
    /// Optional authentication token. Sent as `Authorization: Bearer <token>`
    /// for raster sources; appended as `?access_token=<token>` for Mapbox
    /// vector sources.
    pub auth_token: Option<String>,
    /// When `false`, individual tile fetch failures are logged and skipped
    /// rather than aborting the entire render.
    pub fail_on_tile_error: bool,
}

impl RenderRequest {
    pub fn validate(&self) -> Result<(), BasemapError> {
        let [xmin, ymin, xmax, ymax] = self.bbox;
        if xmin >= xmax || ymin >= ymax {
            return Err(BasemapError::InvalidBbox(format!(
                "[{xmin}, {ymin}, {xmax}, {ymax}]: xmin must be < xmax and ymin must be < ymax"
            )));
        }
        if !xmin.is_finite() || !ymin.is_finite() || !xmax.is_finite() || !ymax.is_finite() {
            return Err(BasemapError::InvalidBbox(
                "bbox contains non-finite values".into(),
            ));
        }
        if self.width == 0 || self.height == 0 || self.width > 16_384 || self.height > 16_384 {
            return Err(BasemapError::InvalidDimensions(self.width, self.height));
        }
        if let Some(z) = self.zoom {
            if z > 22 {
                return Err(BasemapError::InvalidBbox(format!(
                    "zoom {z} out of range 0–22"
                )));
            }
        }
        if let Some(ref layers) = self.layers {
            validate_layers(layers)?;
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
/// Formula: zoom = log2(EARTH_CIRCUMFERENCE / (256 * metres_per_pixel))
pub fn compute_zoom(bbox: [f64; 4], width_px: u32) -> u8 {
    const EARTH_CIRC: f64 = 40_075_016.686;
    let bbox_width_m = bbox[2] - bbox[0];
    let mpp = bbox_width_m / width_px as f64;
    let zoom = (EARTH_CIRC / (256.0 * mpp)).log2().floor() as i32;
    zoom.clamp(0, 22) as u8
}

/// Count how many tiles cover a bbox at a given zoom level.
pub fn tile_count(bbox: [f64; 4], zoom: u8) -> u32 {
    let (x0, y0) = mercator_to_tile(bbox[0], bbox[3], zoom);
    let (x1, y1) = mercator_to_tile(bbox[2], bbox[1], zoom);
    (x1 - x0 + 1) * (y1 - y0 + 1)
}

/// Convert Web Mercator metres to slippy-map tile coordinates.
pub fn mercator_to_tile(x: f64, y: f64, zoom: u8) -> (u32, u32) {
    let n = 2u32.pow(zoom as u32) as f64;
    let half_circ = 20_037_508.342789244;
    let tx = ((x + half_circ) / (2.0 * half_circ) * n).floor() as u32;
    let ty = ((half_circ - y) / (2.0 * half_circ) * n).floor() as u32;
    (tx.min(n as u32 - 1), ty.min(n as u32 - 1))
}

#[derive(Debug, Clone)]
pub struct RenderResult {
    pub pixels: Vec<u8>,
    pub bounds: SpatialBounds,
    pub width: u32,
    pub height: u32,
    pub zoom_used: u8,
}
