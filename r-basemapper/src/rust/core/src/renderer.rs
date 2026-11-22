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
    pixel_ratio: f64,
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
    let tile_map: HashMap<(u32, u32), &[u8]> = tiles
        .iter()
        .map(|(c, d)| ((c.x, c.y), d.as_slice()))
        .collect();

    for (&(tx, ty), data) in &tile_map {
        let img = image::load_from_memory(data)
            .map_err(|e| BasemapError::TileDecodeError(e.to_string()))?
            .to_rgba8();
        // Tiles snap to the CSS pixel grid (MapLibre's alignedProjMatrix
        // for raster tiles), then scale to device pixels. Each tile spans
        // from its own snapped edge to its neighbour's, so tiles meet with
        // no gap or overlap at any pixel ratio.
        let snap = |v: f64| ((v / pixel_ratio).round() * pixel_ratio).round() as i64;
        let edge_x = |i: f64| snap((i * tile_size_m - half_circ - bbox[0]) * scale_x);
        let edge_y = |j: f64| snap((half_circ - j * tile_size_m - bbox[3]) * -scale_y);
        let (px_x, px_y) = (edge_x(tx as f64), edge_y(ty as f64));
        let (w, h) = (edge_x(tx as f64 + 1.0) - px_x, edge_y(ty as f64 + 1.0) - px_y);
        if w <= 0 || h <= 0 {
            continue;
        }
        let resized = image::imageops::resize(&img, w as u32, h as u32, image::imageops::FilterType::Triangle);
        image::imageops::overlay(&mut canvas, &resized, px_x, px_y);
    }

    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_png() -> TileData {
        let img = RgbaImage::from_pixel(256, 256, image::Rgba([20, 40, 60, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    /// Fractional pixel ratios (from a figure's dpi) once left a white row
    /// or column between tiles.
    #[test]
    fn tiles_meet_without_seams_at_fractional_ratios() {
        let half = 20_037_508.342789244_f64;
        let tile_m = 2.0 * half / 4.0; // zoom 2
        let png = solid_png();
        let tiles: Vec<(TileCoord, TileData)> = [(1, 1), (2, 1), (1, 2), (2, 2)]
            .iter()
            .map(|&(x, y)| (TileCoord { z: 2, x, y }, png.clone()))
            .collect();
        // A view straddling the four tiles' shared corner, off-grid.
        let (cx, cy) = (0.0137 * tile_m, -0.0291 * tile_m);
        let bbox = [cx - 0.4 * tile_m, cy - 0.3 * tile_m, cx + 0.4 * tile_m, cy + 0.3 * tile_m];
        for ratio in [1.0, 1.04, 1.146, 1.5, 2.0, 2.37] {
            let w = (300.0 * ratio) as u32;
            let h = (225.0 * ratio) as u32;
            let img = composite_raster_tiles(&tiles, bbox, 2, w, h, false, ratio).unwrap();
            let white = img.pixels().filter(|p| p.0 == [255, 255, 255, 255]).count();
            assert_eq!(white, 0, "ratio {ratio}: {white} canvas pixels show between tiles");
        }
    }
}
