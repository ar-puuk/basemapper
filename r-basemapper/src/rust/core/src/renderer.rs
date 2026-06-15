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

// ─────────────────────────── Vector tile rendering ───────────────────────────

use std::{borrow::Cow, cell::RefCell, rc::Rc, sync::Arc};
use maplibre::{
    background::BackgroundPlugin,
    context::MapContext,
    coords::{WorldCoords, WorldTileCoords, Zoom, ZoomLevel},
    headless::{
        create_headless_renderer, environment::HeadlessEnvironment, map::HeadlessMap,
    },
    kernel::Kernel,
    plugin::Plugin,
    raster::{DefaultRasterTransferables, RasterPlugin},
    render::{
        graph::{Node, NodeRunError, RenderContext, RenderGraphContext, SlotInfo},
        resource::{BufferedTextureHead, Head},
        tile_view_pattern::ViewTileSources,
        view_state::ViewState,
        RenderPlugin, RenderResources, RenderStageLabel,
    },
    schedule::Schedule,
    style::{layer::StyleLayer, Style as MaplibreStyle},
    tcs::{
        system::{System, SystemContainer, SystemError},
        world::World,
    },
    vector::{DefaultVectorTransferables, VectorPlugin},
    window::PhysicalSize,
};

/// Copies the headless surface texture to the readback buffer after the main render pass.
/// Mirrors `CopySurfaceBufferNode` from `maplibre::headless::graph_node` (private module).
struct CopySurfaceNode;

impl Node for CopySurfaceNode {
    fn input(&self) -> Vec<SlotInfo> {
        vec![]
    }

    fn update(&mut self, _state: &mut RenderResources) {}

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_ctx: &mut RenderContext,
        state: &RenderResources,
        _world: &World,
    ) -> Result<(), NodeRunError> {
        if let Head::Headless(bt) = state.surface().head() {
            let size = state.surface().size();
            render_ctx.command_encoder.copy_texture_to_buffer(
                bt.copy_texture(),
                wgpu::ImageCopyBuffer {
                    buffer: bt.buffer(),
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(bt.bytes_per_row()),
                        rows_per_image: None,
                    },
                },
                wgpu::Extent3d {
                    width: size.width(),
                    height: size.height(),
                    depth_or_array_layers: 1,
                },
            );
        }
        Ok(())
    }
}

/// Reads the readback buffer into an in-memory `Vec<u8>` during the Cleanup stage.
/// Mirrors `WriteSurfaceBufferSystem` but stores pixels in memory instead of to disk.
struct InMemoryCaptureSystem {
    pixels: Rc<RefCell<Option<Vec<u8>>>>,
}

impl System for InMemoryCaptureSystem {
    fn name(&self) -> Cow<'static, str> {
        "basemapper_capture".into()
    }

    fn run(&mut self, context: &mut MapContext) -> Result<(), SystemError> {
        // Capture surface dimensions and the buffer Arc before entering the match.
        // Using a scope so the `surface` borrow is released before we access `device`.
        let bt: Arc<BufferedTextureHead>;
        let width: u32;
        let height: u32;
        {
            let surface = context.renderer.resources.surface();
            let size = surface.size();
            width = size.width();
            height = size.height();
            match surface.head() {
                Head::Headed(_) => return Err(SystemError::Setup),
                Head::Headless(b) => {
                    bt = b.clone();
                }
            }
        } // surface borrow dropped here

        let padded_bpr = bt.bytes_per_row() as usize;
        let unpadded_bpr = (width * 4) as usize;

        let buffer_slice = bt.map_async(&context.renderer.device);
        let padded_buf = buffer_slice.get_mapped_range();

        let mut pix = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height as usize {
            let s = row * padded_bpr;
            pix.extend_from_slice(&padded_buf[s..s + unpadded_bpr]);
        }
        drop(padded_buf);
        bt.unmap();

        *self.pixels.borrow_mut() = Some(pix);
        Ok(())
    }
}

/// Plugin that wires `CopySurfaceNode` + `InMemoryCaptureSystem` into the render pipeline.
struct CapturePlugin {
    pixels: Rc<RefCell<Option<Vec<u8>>>>,
}

impl Plugin<HeadlessEnvironment> for CapturePlugin {
    fn build(
        &self,
        schedule: &mut Schedule,
        _kernel: Rc<Kernel<HeadlessEnvironment>>,
        world: &mut World,
        graph: &mut maplibre::render::graph::RenderGraph,
    ) {
        let draw_graph = graph
            .get_sub_graph_mut("draw")
            .expect("RenderPlugin must run before CapturePlugin");
        draw_graph.add_node("copy_pass", CopySurfaceNode);
        draw_graph
            .add_node_edge("main_pass", "copy_pass")
            .expect("main_pass node must exist");

        schedule.add_system_to_stage(
            RenderStageLabel::Cleanup,
            SystemContainer::new(InMemoryCaptureSystem {
                pixels: self.pixels.clone(),
            }),
        );

        // Remove the Extract stage — in headless mode we push tile data directly
        // rather than letting the scheduler pull it from a live source.
        schedule.remove_stage(RenderStageLabel::Extract);

        world
            .resources
            .get_mut::<ViewTileSources>()
            .expect("ViewTileSources must exist")
            .clear();
    }
}

/// Render vector tiles for the given bbox using maplibre-rs.
///
/// Creates a headless GPU canvas sized to fit the entire tile grid at 512 px/tile,
/// renders all tiles in a single pass (no per-tile compositing), then crops and
/// resizes the captured output to `width × height`.  Single-pass rendering means
/// tile-boundary geometry bleeds across tile edges — no seams.
///
/// Parameters mirror `RenderRequest` so the same timeout, concurrency, auth, and
/// fail-soft semantics apply to both the raster and vector paths (SC-006 / #2).
#[allow(clippy::too_many_arguments)]
pub async fn render_vector_tiles(
    client: &reqwest::Client,
    style_json: &str,
    tile_url_template: &str,
    auth_token: Option<&str>,
    bbox: [f64; 4],
    zoom: u8,
    width: u32,
    height: u32,
    tile_timeout_ms: u32,
    tile_concurrency: u32,
    fail_on_tile_error: bool,
    transparent: bool,
) -> Result<Vec<u8>, BasemapError> {
    const TILE_PX: u32 = 512;

    let ml_style: MaplibreStyle = serde_json::from_str(style_json)
        .map_err(|e| BasemapError::StyleParseError(format!("vector style parse: {e}")))?;

    let tile_coords = crate::tile_fetcher::build_tile_coords(bbox, zoom);
    if tile_coords.is_empty() {
        // Transparent or white blank canvas depending on the layers flag.
        let fill = if transparent { 0u8 } else { 255u8 };
        let mut buf = vec![fill; (width * height * 4) as usize];
        if !transparent {
            // Set alpha channel to 255 for opaque white.
            for chunk in buf.chunks_exact_mut(4) {
                chunk[3] = 255;
            }
        }
        return Ok(buf);
    }

    // Tile grid bounds.
    let min_tx = tile_coords.iter().map(|c| c.x as i32).min().unwrap();
    let max_tx = tile_coords.iter().map(|c| c.x as i32).max().unwrap();
    let min_ty = tile_coords.iter().map(|c| c.y as i32).min().unwrap();
    let max_ty = tile_coords.iter().map(|c| c.y as i32).max().unwrap();
    let n_tiles_x = (max_tx - min_tx + 1) as u32;
    let n_tiles_y = (max_ty - min_ty + 1) as u32;

    // Canvas = entire tile grid at native 512 px/tile resolution.
    let canvas_w = n_tiles_x * TILE_PX;
    let canvas_h = n_tiles_y * TILE_PX;

    let (kernel, renderer) = create_headless_renderer(canvas_w, canvas_h, None).await;

    let pixels_cell: Rc<RefCell<Option<Vec<u8>>>> = Rc::new(RefCell::new(None));
    let capture = CapturePlugin {
        pixels: pixels_cell.clone(),
    };

    let plugins: Vec<Box<dyn Plugin<HeadlessEnvironment>>> = vec![
        Box::new(RenderPlugin::default()),
        Box::new(BackgroundPlugin::default()),
        Box::new(VectorPlugin::<DefaultVectorTransferables>::default()),
        Box::new(RasterPlugin::<DefaultRasterTransferables>::default()),
        Box::new(capture),
    ];

    let mut map = HeadlessMap::new(ml_style.clone(), renderer, kernel, plugins)
        .map_err(|e| BasemapError::RenderError(format!("HeadlessMap init: {e:?}")))?;

    // Override the default ViewState so the camera looks at our tile grid.
    // At Zoom(z), one tile = TILE_PX world pixels → 1 world pixel = 1 canvas pixel.
    // Center = centre of the tile grid in world-pixel coords.
    let center_world_x = (min_tx as f64 + n_tiles_x as f64 / 2.0) * TILE_PX as f64;
    let center_world_y = (min_ty as f64 + n_tiles_y as f64 / 2.0) * TILE_PX as f64;
    {
        let ctx = map.map_context_mut();
        ctx.view_state = ViewState::new(
            PhysicalSize::new(canvas_w, canvas_h).expect("canvas size must be non-zero"),
            WorldCoords { x: center_world_x, y: center_world_y },
            Zoom::new(zoom as f64),
            cgmath::Deg(0.0_f64),
            cgmath::Rad(std::f64::consts::PI / 4.0_f64),
        );
    }

    let vector_layers: Vec<StyleLayer> = ml_style
        .layers
        .into_iter()
        .filter(|l| l.source_layer.is_some())
        .collect();

    // Use the shared client (already configured with a global timeout) and
    // honour per-tile timeout, concurrency, auth_token, and fail_on_tile_error
    // so the vector path has the same contract as the raster path (SC-006 / #2).
    let timeout = std::time::Duration::from_millis(tile_timeout_ms as u64);
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(tile_concurrency.max(1) as usize));

    // Fetch all tiles concurrently (mirrors fetch_all_tiles in tile_fetcher.rs).
    type TileFetchResult = Result<(crate::tile_fetcher::TileCoord, Box<[u8]>), BasemapError>;
    let mut join_set: tokio::task::JoinSet<TileFetchResult> = tokio::task::JoinSet::new();

    for &coord in &tile_coords {
        let client = client.clone();
        let sem = semaphore.clone();
        // Append Mapbox access token to the URL when an auth_token is provided.
        let url = {
            let base = crate::tile_fetcher::expand_url(tile_url_template, coord);
            if let Some(token) = auth_token {
                format!("{base}?access_token={token}")
            } else {
                base
            }
        };

        join_set.spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let resp = client
                .get(&url)
                .timeout(timeout)
                .send()
                .await
                .map_err(|_| BasemapError::TileFetchFailed { url: url.clone(), status: 0 })?;
            if !resp.status().is_success() {
                return Err(BasemapError::TileFetchFailed {
                    url: url.clone(),
                    status: resp.status().as_u16(),
                });
            }
            let bytes = resp
                .bytes()
                .await
                .map_err(|_| BasemapError::TileFetchFailed { url: url.clone(), status: 0 })?;
            Ok((coord, bytes.to_vec().into_boxed_slice()))
        });
    }

    let mut fetched: Vec<(crate::tile_fetcher::TileCoord, Box<[u8]>)> = Vec::new();
    while let Some(res) = join_set.join_next().await {
        let tile_result = res.map_err(|e| BasemapError::RenderError(e.to_string()))?;
        match tile_result {
            Ok(tile) => fetched.push(tile),
            Err(e) => {
                if fail_on_tile_error {
                    return Err(e);
                }
                log::warn!("vector tile fetch failed (skipped): {e}");
            }
        }
    }

    // Tessellate and register every successfully fetched tile.
    for (coord, tile_bytes) in fetched {
        let world_coord = WorldTileCoords {
            x: coord.x as i32,
            y: coord.y as i32,
            z: ZoomLevel::new(zoom),
        };

        let mut all_layers = Vec::new();
        for layer in &vector_layers {
            let tessellated = map
                .process_tile_at(tile_bytes.clone(), layer, world_coord)
                .await;
            all_layers.extend(tessellated);
        }

        map.spawn_tile_at(world_coord, all_layers);
    }

    // Single full-scene render — all tiles drawn in one GPU pass.
    map.run_once();

    let canvas_pixels = pixels_cell
        .borrow_mut()
        .take()
        .ok_or_else(|| BasemapError::RenderError("no pixels captured from full-scene render".into()))?;

    // Crop canvas to the bbox sub-region, then resize to the requested output.
    let half_circ = 20_037_508.342789244_f64;
    let world_size = TILE_PX as f64 * 2_f64.powi(zoom as i32);

    let bbox_wx0 = (bbox[0] + half_circ) / (2.0 * half_circ) * world_size;
    let bbox_wy0 = (half_circ - bbox[3]) / (2.0 * half_circ) * world_size;
    let bbox_wx1 = (bbox[2] + half_circ) / (2.0 * half_circ) * world_size;
    let bbox_wy1 = (half_circ - bbox[1]) / (2.0 * half_circ) * world_size;

    let canvas_ox = min_tx as f64 * TILE_PX as f64;
    let canvas_oy = min_ty as f64 * TILE_PX as f64;

    let crop_left = ((bbox_wx0 - canvas_ox).floor() as i64).clamp(0, canvas_w as i64) as u32;
    let crop_top = ((bbox_wy0 - canvas_oy).floor() as i64).clamp(0, canvas_h as i64) as u32;
    let crop_right = ((bbox_wx1 - canvas_ox).ceil() as i64).clamp(0, canvas_w as i64) as u32;
    let crop_bot = ((bbox_wy1 - canvas_oy).ceil() as i64).clamp(0, canvas_h as i64) as u32;
    let crop_w = crop_right.saturating_sub(crop_left).max(1);
    let crop_h = crop_bot.saturating_sub(crop_top).max(1);

    let full_canvas = RgbaImage::from_raw(canvas_w, canvas_h, canvas_pixels)
        .ok_or_else(|| BasemapError::RenderError("canvas size mismatch".into()))?;

    let cropped = image::imageops::crop_imm(&full_canvas, crop_left, crop_top, crop_w, crop_h)
        .to_image();

    let output = if cropped.width() == width && cropped.height() == height {
        cropped
    } else {
        image::imageops::resize(&cropped, width, height, image::imageops::FilterType::Triangle)
    };

    Ok(output.into_raw())
}

