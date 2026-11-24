// PyO3's #[pyfunction] macro generates From<PyErr>→PyErr conversions internally.
#![allow(clippy::useless_conversion)]
//! The compiled core of the Python package (`basemapper._basemapper`).
//!
//! The Python package is an adapter (docs/design/api-review.md §4.1): its
//! objects are plain values (styles, view inputs, fetch options, clips as
//! dicts and tuples), and every function here turns them into the core's
//! objects, so validation, view resolution, rendering and notes are the
//! core's, the same as in R and Rust. Python adds only what needs Python:
//! CRSs (pyproj), geometries (GeoPandas) and the plot libraries.
use basemapper_core::view::{self, CrsFrame};
use basemapper_core::{
    providers, Basemap, BasemapError, Context, Device, FetchOptions, FontEmbed, Keep, Mesh, Note, OnError, Panel,
    PanelCrs, Rendering, Shape, Source, Style, SvgOptions, SvgText, TileKind, TileStyleOptions, Units, View,
    ViewInputs, Warp,
};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

// ── Errors ───────────────────────────────────────────────────────────────────

fn exception(py: Python<'_>, class: &str, msg: &str) -> PyErr {
    py.import("basemapper.exceptions")
        .and_then(|m| m.getattr(class))
        .and_then(|cls| cls.call1((msg,)))
        .map(PyErr::from_value)
        .unwrap_or_else(|_| PyRuntimeError::new_err(msg.to_owned()))
}

fn validation_error(py: Python<'_>, msg: &str) -> PyErr {
    exception(py, "ValidationError", msg)
}

/// A `BasemapError` as the typed Python exception of its `kind()`.
fn core_err(py: Python<'_>, e: BasemapError) -> PyErr {
    let class = match e.kind() {
        "validation" => "ValidationError",
        "style" => "StyleError",
        "network" => "NetworkError",
        _ => "BasemapError",
    };
    exception(py, class, &e.to_string())
}

// ── Reading Python values ────────────────────────────────────────────────────

/// `d[key]`, or None when missing or None.
fn item<'py>(d: &Bound<'py, PyDict>, key: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    Ok(d.get_item(key)?.filter(|v| !v.is_none()))
}

fn f64_of(d: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<f64>> {
    item(d, key)?.map(|v| v.extract::<f64>()).transpose()
}

fn u32_of(d: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<u32>> {
    item(d, key)?.map(|v| v.extract::<u32>()).transpose()
}

fn u8_of(d: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<u8>> {
    item(d, key)?.map(|v| v.extract::<u8>()).transpose()
}

fn str_of(d: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    item(d, key)?.map(|v| v.extract::<String>()).transpose()
}

fn strs_of(d: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<Vec<String>>> {
    item(d, key)?.map(|v| v.extract::<Vec<String>>()).transpose()
}

/// Two numbers, from any sequence (a tuple or a list).
fn pair_of(d: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<(f64, f64)>> {
    match item(d, key)? {
        None => Ok(None),
        Some(v) => match v.extract::<Vec<f64>>()?.as_slice() {
            [a, b] => Ok(Some((*a, *b))),
            _ => Err(validation_error(d.py(), &format!("{key} must be two numbers"))),
        },
    }
}

fn box_of(d: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<[f64; 4]>> {
    item(d, key)?.map(|v| v.extract::<[f64; 4]>()).transpose()
}

fn required<T>(py: Python<'_>, v: Option<T>, what: &str) -> PyResult<T> {
    v.ok_or_else(|| validation_error(py, &format!("{what} is required")))
}

fn kind_of(py: Python<'_>, s: Option<String>) -> PyResult<Option<TileKind>> {
    s.map(|k| TileKind::parse(&k)).transpose().map_err(|e| core_err(py, e))
}

/// A style spec (`{"source": {...}, "layers", "features", "key"}`) as the
/// core's `Style`, validated.
fn style_of(py: Python<'_>, spec: &Bound<'_, PyDict>) -> PyResult<Style> {
    let src = required(py, item(spec, "source")?, "a style's source")?;
    let src = src.cast::<PyDict>().map_err(|_| validation_error(py, "a style's source must be a dict"))?;
    let kind = str_of(src, "kind")?.unwrap_or_else(|| "auto".into());
    let source = match kind.as_str() {
        "auto" => Source::from(required(py, str_of(src, "value")?, "the style")?),
        "provider" => Source::provider(
            required(py, str_of(src, "name")?, "provider(name)")?,
            kind_of(py, str_of(src, "form")?)?,
            str_of(src, "tiles")?,
        ),
        "tiles" => {
            let paint: serde_json::Map<String, serde_json::Value> = match str_of(src, "paint")? {
                Some(json) => serde_json::from_str(&json).map_err(|e| validation_error(py, &format!("paint must be a mapping: {e}")))?,
                None => serde_json::Map::new(),
            };
            Source::tiles(
                required(py, str_of(src, "input")?, "tiles(input)")?,
                TileStyleOptions {
                    kind: kind_of(py, str_of(src, "type")?)?,
                    tile_size: u32_of(src, "tile_size")?,
                    scheme: str_of(src, "scheme")?,
                    minzoom: u8_of(src, "minzoom")?,
                    maxzoom: u8_of(src, "maxzoom")?,
                    attribution: str_of(src, "attribution")?,
                    source_layers: strs_of(src, "source_layers")?,
                    paint,
                },
            )
        }
        "wms" => {
            let mut s = Source::wms(required(py, str_of(src, "url")?, "wms(url)")?, required(py, str_of(src, "layers")?, "wms(layers)")?);
            if let Source::Wms { styles, version, format, transparent, extra, .. } = &mut s {
                if let Some(v) = str_of(src, "styles")? {
                    *styles = v;
                }
                if let Some(v) = str_of(src, "version")? {
                    *version = v;
                }
                if let Some(v) = str_of(src, "format")? {
                    *format = v;
                }
                if let Some(v) = item(src, "transparent")? {
                    *transparent = v.extract()?;
                }
                *extra = str_of(src, "extra_params")?;
            }
            s
        }
        "wmts" => {
            let mut s = Source::wmts(required(py, str_of(src, "url")?, "wmts(url)")?, required(py, str_of(src, "layer")?, "wmts(layer)")?);
            if let Source::Wmts { tilematrixset, format, style, version, extra, .. } = &mut s {
                if let Some(v) = str_of(src, "tilematrixset")? {
                    *tilematrixset = v;
                }
                if let Some(v) = str_of(src, "format")? {
                    *format = v;
                }
                if let Some(v) = str_of(src, "style")? {
                    *style = v;
                }
                if let Some(v) = str_of(src, "version")? {
                    *version = v;
                }
                *extra = str_of(src, "extra_params")?;
            }
            s
        }
        other => return Err(validation_error(py, &format!("unknown style source kind {other:?}"))),
    };
    let mut style = Style::new(source);
    if let Some(l) = strs_of(spec, "layers")? {
        style = style.layers(l);
    }
    if let Some(f) = strs_of(spec, "features")? {
        style = style.features(f);
    }
    if let Some(k) = str_of(spec, "key")? {
        style = style.key(k);
    }
    style.validate().map_err(|e| core_err(py, e))?;
    Ok(style)
}

fn styles_of(py: Python<'_>, specs: &[Bound<'_, PyDict>]) -> PyResult<Vec<Style>> {
    if specs.is_empty() {
        return Err(validation_error(py, "a basemap needs at least one style"));
    }
    specs.iter().map(|s| style_of(py, s)).collect()
}

/// View inputs (`bounds` in Web Mercator or the frame's CRS, `center`
/// lon/lat, `zoom`, `width`, `height`, `units`, `dpi`).
fn inputs_of(py: Python<'_>, d: &Bound<'_, PyDict>) -> PyResult<ViewInputs> {
    let mut v = ViewInputs::new();
    if let Some(b) = box_of(d, "bounds")? {
        v = v.bounds(b);
    }
    if let Some((lon, lat)) = pair_of(d, "center")? {
        v = v.center(lon, lat);
    }
    if let Some(z) = f64_of(d, "zoom")? {
        v = v.zoom(z);
    }
    match (f64_of(d, "width")?, f64_of(d, "height")?) {
        (Some(w), Some(h)) => {
            let units = Units::parse(&str_of(d, "units")?.unwrap_or_else(|| "px".into())).map_err(|e| core_err(py, e))?;
            v = v.size(w, h, units);
        }
        (None, None) => {}
        _ => return Err(validation_error(py, "give both width and height, or neither")),
    }
    if let Some(dpi) = f64_of(d, "dpi")? {
        v = v.dpi(dpi);
    }
    if let Some(scale) = f64_of(d, "symbol_scale")? {
        v = v.symbol_scale(scale);
    }
    Ok(v)
}

fn fetch_of(py: Python<'_>, d: &Bound<'_, PyDict>) -> PyResult<FetchOptions> {
    let mut f = FetchOptions::new();
    if let Some(t) = f64_of(d, "timeout")? {
        if !(t > 0.0 && t.is_finite()) {
            return Err(validation_error(py, "timeout must be a number of seconds > 0"));
        }
        f = f.timeout(std::time::Duration::from_secs_f64(t));
    }
    if let Some(n) = u32_of(d, "max_tiles")? {
        f = f.max_tiles(n);
    }
    if let Some(n) = u32_of(d, "concurrency")? {
        f = f.concurrency(n);
    }
    if let Some(e) = str_of(d, "on_error")? {
        f = f.on_error(OnError::parse(&e).map_err(|e| core_err(py, e))?);
    }
    f.validate().map_err(|e| core_err(py, e))?;
    Ok(f)
}

/// A clip: `(polygons, keep)`, each polygon a list of rings of `(x, y)`.
type ClipArg = (Vec<Vec<Vec<(f64, f64)>>>, String);

fn clip_of(py: Python<'_>, clip: ClipArg) -> PyResult<Shape> {
    let keep = match clip.1.as_str() {
        "inside" => Keep::Inside,
        "outside" => Keep::Outside,
        other => return Err(validation_error(py, &format!("clip keep must be \"inside\" or \"outside\", not {other:?}"))),
    };
    Shape::new(clip.0, keep).map_err(|e| core_err(py, e))
}

/// A mesh: `(nx, ny, nodes)`, nodes row-major from the top-left, NaN where
/// a node has no Web Mercator position.
type MeshArg = (usize, usize, Vec<(f64, f64)>);

fn mesh_of(py: Python<'_>, m: MeshArg) -> PyResult<Mesh> {
    Mesh::new(m.0, m.1, m.2).map_err(|e| core_err(py, e))
}

fn frame_of(py: Python<'_>, d: &Bound<'_, PyDict>) -> PyResult<CrsFrame> {
    CrsFrame::new(
        required(py, str_of(d, "name")?, "the frame's CRS name")?,
        required(py, pair_of(d, "origin")?, "the frame's origin")?,
        required(py, pair_of(d, "origin_3857")?, "the frame's Web Mercator origin")?,
        required(py, f64_of(d, "scale")?, "the frame's scale")?,
    )
    .map_err(|e| core_err(py, e))
}

fn device_of(py: Python<'_>, d: &Bound<'_, PyDict>) -> PyResult<Device> {
    match str_of(d, "kind")?.as_deref() {
        Some("screen") => Ok(Device::Screen { pixel_ratio: f64_of(d, "pixel_ratio")?.unwrap_or(1.0) }),
        Some("raster_export") => Ok(Device::RasterExport { dpi: required(py, f64_of(d, "dpi")?, "a raster export's dpi")? }),
        Some("vector_export") => Ok(Device::VectorExport),
        other => Err(validation_error(py, &format!("unknown device kind {other:?}"))),
    }
}

fn panel_of(py: Python<'_>, d: &Bound<'_, PyDict>, mesh: Option<MeshArg>) -> PyResult<Panel> {
    let crs_name = required(py, str_of(d, "crs")?, "the panel's CRS")?;
    let crs = match crs_name.as_str() {
        "EPSG:3857" => PanelCrs::WebMercator,
        "EPSG:4326" => PanelCrs::LonLat,
        _ => PanelCrs::Other {
            name: crs_name.clone(),
            mesh: mesh_of(py, mesh.ok_or_else(|| validation_error(py, &format!("a panel in {crs_name} needs a mesh")))?)?,
        },
    };
    let device = device_of(py, &required(py, item(d, "device")?, "the panel's device")?.cast_into::<PyDict>()?)?;
    Panel::new(
        required(py, box_of(d, "limits")?, "the panel's limits")?,
        crs,
        required(py, item(d, "px")?, "the panel's size")?.extract::<(u32, u32)>()?,
        device,
    )
    .map_err(|e| core_err(py, e))
}

// ── Converting results ───────────────────────────────────────────────────────

/// A note as `(level, kind, message, details, count)`.
type NoteTuple = (&'static str, &'static str, String, Vec<String>, u32);

fn note_tuple(n: &Note) -> NoteTuple {
    (n.level.name(), n.kind, n.message.clone(), n.details.clone(), n.count)
}

fn view_dict<'py>(py: Python<'py>, v: &View) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("bounds", v.bounds.to_vec())?;
    d.set_item("width", v.width)?;
    d.set_item("height", v.height)?;
    d.set_item("dpi", v.dpi)?;
    d.set_item("zoom", v.zoom)?;
    d.set_item("symbol_scale", v.symbol_scale)?;
    d.set_item("scale", v.scale())?;
    d.set_item("center", v.center())?;
    let p = PyDict::new(py);
    p.set_item("extent", v.provenance.extent.name())?;
    p.set_item("size", v.provenance.size.name())?;
    p.set_item("dpi", v.provenance.dpi.name())?;
    p.set_item("zoom", v.provenance.zoom.name())?;
    d.set_item("provenance", p)?;
    d.set_item("notes", v.notes.iter().map(note_tuple).collect::<Vec<_>>())?;
    d.set_item("decided", v.decided().iter().map(note_tuple).collect::<Vec<_>>())?;
    Ok(d)
}

/// A rendering as a dict; its pixels one `bytes` object (a single copy out
/// of the core, which numpy then views without copying).
fn rendering_dict<'py>(py: Python<'py>, r: Rendering) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("symbol_scale", r.symbol_scale)?;
    d.set_item("scale", r.scale())?;
    d.set_item("pixels", PyBytes::new(py, &r.pixels))?;
    d.set_item("width", r.width)?;
    d.set_item("height", r.height)?;
    d.set_item("bounds", r.bounds.to_vec())?;
    d.set_item("crs", r.crs.clone())?;
    d.set_item("zoom", r.zoom)?;
    d.set_item("tile_zoom", r.tile_zoom)?;
    d.set_item("dpi", r.dpi)?;
    d.set_item("notes", r.notes.iter().map(note_tuple).collect::<Vec<_>>())?;
    Ok(d)
}

fn basemap_of(py: Python<'_>, styles: &[Bound<'_, PyDict>], inputs: &Bound<'_, PyDict>, fetch: &Bound<'_, PyDict>, clip: Option<ClipArg>) -> PyResult<Basemap> {
    let mut m = Basemap::stack(styles_of(py, styles)?).with_inputs(inputs_of(py, inputs)?).fetch(fetch_of(py, fetch)?);
    if let Some(c) = clip {
        m = m.clip(clip_of(py, c)?);
    }
    Ok(m)
}

fn svg_options(py: Python<'_>, text: &str, embed_fonts: &str, font_dirs: Option<Vec<String>>) -> PyResult<SvgOptions> {
    let mut opts = SvgOptions::default();
    opts.text = match text {
        "editable" => SvgText::Editable,
        "bitmap" => SvgText::Bitmap,
        "paths" => SvgText::Paths,
        _ => return Err(validation_error(py, "text must be \"editable\", \"bitmap\" or \"paths\"")),
    };
    opts.embed_fonts = match embed_fonts {
        "subset" => FontEmbed::Subset,
        "full" => FontEmbed::Full,
        "none" => FontEmbed::None,
        _ => return Err(validation_error(py, "embed_fonts must be \"subset\", \"full\" or \"none\"")),
    };
    if let Some(dirs) = font_dirs {
        let mut d: Vec<std::path::PathBuf> = dirs.into_iter().map(Into::into).collect();
        d.extend(opts.font_dirs);
        opts.font_dirs = d;
    }
    Ok(opts)
}

// ── Functions ────────────────────────────────────────────────────────────────

/// Check a style spec (its source and filters) without fetching anything.
#[pyfunction]
fn check_style(py: Python<'_>, spec: Bound<'_, PyDict>) -> PyResult<()> {
    style_of(py, &spec).map(|_| ())
}

/// Resolve a standalone view: Web Mercator, or with `frame` a map in that
/// frame's CRS (then also `crs_bounds`, the extent shown in it).
#[pyfunction]
#[pyo3(signature = (inputs, frame=None))]
fn resolve<'py>(py: Python<'py>, inputs: Bound<'py, PyDict>, frame: Option<Bound<'py, PyDict>>) -> PyResult<Bound<'py, PyDict>> {
    let inputs = inputs_of(py, &inputs)?;
    match frame {
        None => view_dict(py, &view::resolve(&inputs, &Context::Standalone).map_err(|e| core_err(py, e))?),
        Some(f) => {
            let (v, crs_bounds) = view::resolve_in_crs(&inputs, &frame_of(py, &f)?).map_err(|e| core_err(py, e))?;
            let d = view_dict(py, &v)?;
            d.set_item("crs_bounds", crs_bounds.to_vec())?;
            Ok(d)
        }
    }
}

/// The view for a plot `panel` (its `mesh` for a CRS other than Web
/// Mercator and lon/lat), and the warp's CRS and bounds (None in Web
/// Mercator).
#[pyfunction]
#[pyo3(signature = (inputs, panel, mesh=None))]
fn plan<'py>(py: Python<'py>, inputs: Bound<'py, PyDict>, panel: Bound<'py, PyDict>, mesh: Option<MeshArg>) -> PyResult<Bound<'py, PyDict>> {
    let p = panel_of(py, &panel, mesh)?;
    let (v, warp) = basemapper_core::plot::plan(&p, &inputs_of(py, &inputs)?).map_err(|e| core_err(py, e))?;
    let d = view_dict(py, &v)?;
    d.set_item("warp", warp.map(|w| (w.crs.clone(), w.bounds.to_vec())))?;
    Ok(d)
}

/// Render: standalone in Web Mercator; with `frame` and `warp_mesh`, in the
/// frame's CRS (the mesh built over the `crs_bounds` that `resolve` gave,
/// for its pixel size); with `panel` (and its mesh, for a CRS other than
/// Web Mercator and lon/lat), for a plot panel.
#[pyfunction]
#[pyo3(signature = (styles, inputs, fetch, clip=None, frame=None, warp_mesh=None, panel=None))]
#[allow(clippy::too_many_arguments)]
fn render<'py>(
    py: Python<'py>,
    styles: Vec<Bound<'py, PyDict>>,
    inputs: Bound<'py, PyDict>,
    fetch: Bound<'py, PyDict>,
    clip: Option<ClipArg>,
    frame: Option<Bound<'py, PyDict>>,
    warp_mesh: Option<MeshArg>,
    panel: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let m = basemap_of(py, &styles, &inputs, &fetch, clip)?;
    let result = if let Some(p) = panel {
        let p = panel_of(py, &p, warp_mesh)?;
        py.detach(|| m.render_panel(&p))
    } else if let Some(f) = frame {
        let frame = frame_of(py, &f)?;
        let (view, crs_bounds) = view::resolve_in_crs(m.inputs(), &frame).map_err(|e| core_err(py, e))?;
        let mesh = mesh_of(py, required(py, warp_mesh, "a warp mesh for a map not in Web Mercator")?)?;
        let warp = Warp::new(frame.name.clone(), crs_bounds, mesh).map_err(|e| core_err(py, e))?;
        py.detach(|| m.render_view(&view, Some(&warp)))
    } else {
        py.detach(|| m.render(&Context::Standalone))
    };
    rendering_dict(py, result.map_err(|e| core_err(py, e))?)
}

/// Render an SVG document (Web Mercator only: with `frame`, the core's
/// error for a map in another CRS).
#[pyfunction]
#[pyo3(signature = (styles, inputs, fetch, clip=None, frame=None, text="editable", embed_fonts="subset", font_dirs=None))]
#[allow(clippy::too_many_arguments)]
fn render_svg<'py>(
    py: Python<'py>,
    styles: Vec<Bound<'py, PyDict>>,
    inputs: Bound<'py, PyDict>,
    fetch: Bound<'py, PyDict>,
    clip: Option<ClipArg>,
    frame: Option<Bound<'py, PyDict>>,
    text: &str,
    embed_fonts: &str,
    font_dirs: Option<Vec<String>>,
) -> PyResult<String> {
    let m = basemap_of(py, &styles, &inputs, &fetch, clip)?;
    let opts = svg_options(py, text, embed_fonts, font_dirs)?;
    let result = match frame {
        None => py.detach(|| m.render_svg(&Context::Standalone, &opts)),
        Some(f) => {
            let frame = frame_of(py, &f)?;
            let (view, crs_bounds) = view::resolve_in_crs(m.inputs(), &frame).map_err(|e| core_err(py, e))?;
            // The extent's corners: enough for the core to know the CRS.
            let b = view.bounds;
            let mesh = Mesh::new(2, 2, vec![(b[0], b[3]), (b[2], b[3]), (b[0], b[1]), (b[2], b[1])]).map_err(|e| core_err(py, e))?;
            let warp = Warp::new(frame.name.clone(), crs_bounds, mesh).map_err(|e| core_err(py, e))?;
            m.render_svg_view(&view, Some(&warp), &opts)
        }
    };
    result.map_err(|e| core_err(py, e))
}

/// The layers a style spec draws, with its filters, as (id, type, source
/// layer, feature).
#[pyfunction]
#[allow(clippy::type_complexity)]
fn list_layers(py: Python<'_>, spec: Bound<'_, PyDict>) -> PyResult<Vec<(String, String, Option<String>, String)>> {
    let style = style_of(py, &spec)?;
    let rows = py.detach(|| basemapper_core::list_style_layers(&style)).map_err(|e| core_err(py, e))?;
    Ok(rows.into_iter().map(|l| (l.id, l.kind, l.source_layer, l.feature)).collect())
}

/// The attribution text of style specs (stacked), joined with " | ".
#[pyfunction]
fn attribution(py: Python<'_>, styles: Vec<Bound<'_, PyDict>>) -> PyResult<String> {
    let m = Basemap::stack(styles_of(py, &styles)?);
    py.detach(|| m.attribution()).map_err(|e| core_err(py, e))
}

/// The credit line's layout in a plot `width_pt` points wide: (lines, font
/// size pt, colour, background, background alpha, padding pt), or None.
#[pyfunction]
#[allow(clippy::type_complexity)]
fn attribution_layout(text: &str, width_pt: f64) -> Option<(Vec<String>, f64, String, String, f64, f64)> {
    basemapper_core::plot::attribution_layout(text, width_pt)
        .map(|a| (a.lines, a.font_size_pt, a.color, a.background, a.background_alpha, a.padding_pt))
}

/// `[west, south, east, north]` (lon/lat) of `span` (width, height in
/// `units`) on the ground around `center`.
#[pyfunction]
fn around(py: Python<'_>, center: (f64, f64), span: (f64, f64), units: &str) -> PyResult<[f64; 4]> {
    let per = view::distance_unit(units).map_err(|e| core_err(py, e))?;
    view::around(center, (span.0 * per, span.1 * per)).map_err(|e| core_err(py, e))
}

/// Encode straight-alpha RGBA pixels as PNG bytes.
#[pyfunction]
fn encode_png(py: Python<'_>, pixels: &[u8], width: u32, height: u32) -> PyResult<Py<PyBytes>> {
    if pixels.len() != width as usize * height as usize * 4 {
        return Err(validation_error(py, "pixel buffer size does not match width * height * 4"));
    }
    basemapper_core::internal::encode_png(pixels, width, height).map(|b| PyBytes::new(py, &b).unbind()).map_err(|e| core_err(py, e))
}

/// The catalogue as (name, vector, raster, API key variable, attribution) rows.
#[pyfunction]
#[pyo3(signature = (kind=None))]
#[allow(clippy::type_complexity)]
fn list_providers(py: Python<'_>, kind: Option<String>) -> PyResult<Vec<(&'static str, bool, bool, Option<&'static str>, &'static str)>> {
    Ok(providers::list_providers(kind_of(py, kind)?)
        .into_iter()
        .map(|p| (p.name, p.kinds().contains(&TileKind::Vector), p.raster.is_some(), p.key.map(|k| k.env), p.attribution))
        .collect())
}

/// The tile cache directory (None when off); `dir` moves it ("" turns it
/// off), `clear` deletes the cached tiles.
#[pyfunction]
#[pyo3(signature = (dir=None, clear=false, max_size_mb=None))]
fn tile_cache(py: Python<'_>, dir: Option<String>, clear: bool, max_size_mb: Option<f64>) -> PyResult<Option<String>> {
    if let Some(d) = dir {
        basemapper_core::cache::set_dir((!d.is_empty()).then(|| d.into()));
    }
    if let Some(mb) = max_size_mb {
        if !(mb > 0.0) {
            return Err(core_err(py, BasemapError::InvalidInput("max_size must be a positive number of megabytes".into())));
        }
        basemapper_core::cache::set_max_size((mb * 1024.0 * 1024.0) as u64);
    }
    if clear {
        basemapper_core::cache::clear()
            .map_err(|e| core_err(py, BasemapError::RenderError(format!("could not clear the tile cache: {e}"))))?;
    }
    Ok(basemapper_core::cache::dir().map(|d| d.to_string_lossy().into_owned()))
}

#[pymodule]
fn _basemapper(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(check_style, m)?)?;
    m.add_function(wrap_pyfunction!(resolve, m)?)?;
    m.add_function(wrap_pyfunction!(plan, m)?)?;
    m.add_function(wrap_pyfunction!(render, m)?)?;
    m.add_function(wrap_pyfunction!(render_svg, m)?)?;
    m.add_function(wrap_pyfunction!(list_layers, m)?)?;
    m.add_function(wrap_pyfunction!(attribution, m)?)?;
    m.add_function(wrap_pyfunction!(attribution_layout, m)?)?;
    m.add_function(wrap_pyfunction!(around, m)?)?;
    m.add_function(wrap_pyfunction!(encode_png, m)?)?;
    m.add_function(wrap_pyfunction!(list_providers, m)?)?;
    m.add_function(wrap_pyfunction!(tile_cache, m)?)?;
    Ok(())
}
