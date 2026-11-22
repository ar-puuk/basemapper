//! In-house CPU vector-tile renderer: MVT decode, MapLibre style evaluation,
//! and tiny-skia rasterisation. Targets pixel parity with MapLibre GL JS
//! (measured by tests/golden.rs).

pub mod cluster;
pub mod expr;
pub mod fonts;
pub mod geojson;
pub mod heatmap;
pub mod hillshade;
pub mod gl;
pub mod mvt;
pub mod render;
pub mod rtl;
pub mod svg;
pub mod vertical;
pub mod symbol;
pub mod symbol_line;
