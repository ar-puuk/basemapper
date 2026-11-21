use crate::error::BasemapError;
use crate::tile_fetcher::{TileCoord, TileData};
use image::RgbaImage;
use std::collections::HashMap;

/// Composite raster tiles onto a single RGBA canvas.
///
/// Each tile occupies an exact (tile_px × tile_px) region on the canvas.
/// The canvas extends from bbox[0..3] at the given zoom; pixel offsets
/// are computed from the tile index relative to the top-left tile.
pub fn composite_raster_tiles(
    tiles: &[(TileCoord, TileData)],
    bbox: [f64; 4],
    zoom: u8,
    width: u32,
    height: u32,
    transparent_background: bool,
) -> Result<RgbaImage, BasemapError> {
    let mut canvas = if transparent_background {
        RgbaImage::new(width, height)
    } else {
        let mut img = RgbaImage::new(width, height);
        for px in img.pixels_mut() {
            *px = image::Rgba([255, 255, 255, 255]);
        }
        img
    };

    let half_circ = 20_037_508.342789244_f64;
    let tile_size_m = (2.0 * half_circ) / 2u64.pow(zoom as u32) as f64;
    let scale_x = width as f64 / (bbox[2] - bbox[0]);
    let scale_y = height as f64 / (bbox[3] - bbox[1]);
    let tile_px_w = (tile_size_m * scale_x).round() as u32;
    let tile_px_h = (tile_size_m * scale_y).round() as u32;

    let tile_map: HashMap<(u32, u32), &[u8]> = tiles
        .iter()
        .map(|(c, d)| ((c.x, c.y), d.as_slice()))
        .collect();

    for (&(tx, ty), data) in &tile_map {
        let img = image::load_from_memory(data)
            .map_err(|e| BasemapError::TileDecodeError(e.to_string()))?
            .to_rgba8();
        let resized = image::imageops::resize(
            &img,
            tile_px_w,
            tile_px_h,
            image::imageops::FilterType::Triangle,
        );
        let tile_xmin = tx as f64 * tile_size_m - half_circ;
        let tile_ymax = half_circ - ty as f64 * tile_size_m;
        let px_x = ((tile_xmin - bbox[0]) * scale_x).round() as i64;
        let px_y = ((bbox[3] - tile_ymax) * scale_y).round() as i64;
        image::imageops::overlay(&mut canvas, &resized, px_x, px_y);
    }

    Ok(canvas)
}
