use crate::error::BasemapError;
use crate::tile_fetcher::{TileCoord, TileData};
use image::RgbaImage;
use std::collections::HashMap;

pub struct WgpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// Initialize a headless wgpu device (no surface required).
pub async fn initialize_wgpu_headless() -> Result<WgpuContext, BasemapError> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| BasemapError::RenderError("no suitable GPU adapter found".into()))?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("basemapper"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .map_err(|e| BasemapError::RenderError(e.to_string()))?;

    Ok(WgpuContext { device, queue })
}

/// Create an off-screen render texture of the requested dimensions.
pub fn create_render_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("basemapper_output"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// Read the pixels from a wgpu texture back to CPU memory.
///
/// H2: map_async is asynchronous. We use device.poll(Maintain::Wait) to block
/// synchronously without spawning a new tokio task, avoiding deadlock.
pub fn read_texture_to_vec(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, BasemapError> {
    let bytes_per_row = align_to_256(width * 4);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    // H2: poll synchronously — do NOT await or spawn a tokio task here.
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);

    let data = slice.get_mapped_range();
    // Strip padding bytes added by wgpu's 256-byte row alignment requirement.
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let start = (row * bytes_per_row) as usize;
        let end = start + (width * 4) as usize;
        pixels.extend_from_slice(&data[start..end]);
    }
    drop(data);
    buffer.unmap();
    Ok(pixels)
}

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
    use crate::bbox::mercator_to_tile;

    let mut canvas = if transparent_background {
        RgbaImage::new(width, height)
    } else {
        let mut img = RgbaImage::new(width, height);
        for px in img.pixels_mut() {
            *px = image::Rgba([255, 255, 255, 255]);
        }
        img
    };

    let (tx0, ty0) = mercator_to_tile(bbox[0], bbox[3], zoom);
    let half_circ = 20_037_508.342789244_f64;
    let tile_size_m = (2.0 * half_circ) / 2u64.pow(zoom as u32) as f64;
    let _canvas_xmin = tx0 as f64 * tile_size_m - half_circ;
    let _canvas_ymax = half_circ - ty0 as f64 * tile_size_m;
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
        let _tile_orig_w = img.width();
        let _tile_orig_h = img.height();
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

/// wgpu requires buffer rows to be aligned to 256 bytes.
fn align_to_256(n: u32) -> u32 {
    (n + 255) & !255
}
