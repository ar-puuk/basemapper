# basemapper API and design review

**Status:** accepted (all decisions D1–D21, as recommended) · **Plan:** [implementation-plan.md](implementation-plan.md) · **Date:** 2026-11-23 · **Baseline:** `dev` @ `76fdf91` (v0.2.0 plus fixes)

**Scope:** the public interface of the R and Python packages (primary) and the
Rust crate (secondary but supported), and the architecture behind it. The
goal is one coherent design that is simple for end users, easy to extend, and
safe to keep changing. Nothing here has been implemented. Breaking changes
are acceptable: it is better to break now than later.

---

## 1. The principle: no decoherence

Kristen Maevyn's essay [*The Decoherence Trap*](https://maevyn.codes/decoherence)
defines **decoherence** as

> the accumulation of locally valid but mutually incompatible or redundant
> models within a system, such that the cost of change is dominated by
> translating among those models.

Its lessons, applied here:

1. **One model per concept.** Each idea (where the map is, how detailed it is,
   how big the image is, what is drawn) has exactly one representation, owned
   by one component. Every other place uses that one.
2. **Finish transitions.** A half-old, half-new system is worse than either.
   So: a clean break, no layers of shims. Deprecation shims are a short bridge,
   not a destination. This applies to the four `fetch` shims added in 0.2.0.
3. **Test an abstraction across contexts.** A good abstraction holds up in
   every use case (§2) without code bending to fit each one.
4. **Solve the right problem first.** "Figure out what problems we're trying to
   solve in the first place", then find "a single solution that solves each
   one without exploding into a labyrinth".
5. **Restore coherence fully.** Not "just enough to go back to our old patterns".
6. **References are evidence, not authority.** Other products show which
   problems are real and how solutions fare; a choice is made on its merits
   for basemapper's users. Where the right design is new, basemapper leads.

basemapper grew over months, under several plans. Most of its parts are
locally sensible. The findings below are almost all places where those
locally sensible parts disagree.

---

## 2. What users do (the contexts the design must hold up in)

| # | Use case | Who decides the extent | Who decides the size | What they get |
|---|---|---|---|---|
| U1 | A map on its own (report figure, web image) | the user: a place and a zoom, or an area to fit | the user (pixels) | PNG, SVG |
| U2 | A basemap under a plot (ggplot2, tmap, matplotlib, plotnine) | the plot's data / coordinate limits | the plot device (inches × dpi) | a layer in the plot |
| U3 | A georeferenced raster for GIS work | the user: an area, often in a projected CRS | the user, or "this area at this level of detail" | array with extent and CRS; `terra` / `xarray` |
| U4 | Editable vector output | the user | the user | SVG |
| U5 | Batch / server rendering (many views, one style) | code | code | any of the above, fast, with low memory |
| U6 | Exploring a style before using it | — | — | its layers, features, attribution |

Each use case needs the same four ingredients: **what to draw** (styles),
**where** (extent), **how detailed** (scale), and **how big** (output size).
The design should express these four once and reuse them in every context.

---

## 3. Findings

Each finding has its evidence and the use cases it affects. Severity:
**bug** (wrong output, or documented behaviour that fails), **decoherence**
(redundant or conflicting models), **friction** (harder than needed).

### F1 (bug) Plot layers misplace the basemap outside Web Mercator

The guides say: "The basemap is rendered in Web Mercator and **warped** to the
plot's coordinate system, so data in any CRS works"
(`r-basemapper/vignettes/articles/plotting.Rmd`, `py-basemapper/user_guide/03-plotting.qmd`).
The landing page says "in any CRS".

Only `tm_basemap()` warps (`terra::project`). `geom_basemap()`
(`R/geom_basemap.R`) and `add_basemap()` / plotnine's `geom_basemap()`
(`matplotlib_integration.py`) reproject only the *bounding box* to EPSG:3857,
render, and stretch the Web Mercator image over the panel. On a
longitude/latitude panel (the default `coord_sf()` for lon/lat data), the
mid-latitude offset is:

| Extent | Offset at mid-latitude (600 px panel) |
|---|---|
| North Carolina | 0.4% (2.6 px) |
| Contiguous US | 4.2% (25 px) |
| Europe | 11.1% (66 px) |

On a projected panel (state plane, UTM, Albers) the image is also not rotated
or curved to match. City-scale plots look right, which is why it went
unnoticed. Affects U2.

### F2 (decoherence) Three implementations of the map view

The "view" (styles, size, centre, zoom, pixel ratio, filters, fetch settings)
is implemented three times:

- **Rust:** `Basemap` (`core/src/basemap.rs`), a builder with getters and
  render methods.
- **R:** an S3 list built in `basemap()`, with its own `set_view`,
  `set_bounds`, `get_*`, validation and bbox handling (`R/basemap.R`,
  `R/bbox_utils.R`).
- **Python:** the `Basemap` class, with its own copies of the same
  (`_basemap.py`, `bbox_utils.py`).

The R and Python packages **do not use** the Rust `Basemap`. They compute a
bbox themselves and call a lower-level entry point (`render_raw`, which builds
`RenderRequest`s). So each binding re-implements the view math by calling
helper functions (`bbox_from_center`, `fit_bounds`, now `constrain_view`)
in its own order. Every view rule has to be added three times; the
default-view bug fixed in `76fdf91` needed edits in all three.

### F3 (decoherence) Three overlapping knobs for detail

| Knob | Where | Meaning |
|---|---|---|
| `zoom` | `basemap()` | MapLibre camera zoom: 512 px tiles, fractional, sets the scale |
| `tile_zoom` | `render_basemap_raw()`, plot layers | an *integer* override of which tiles are fetched, on the **256 px** grid (vector sources use one less), while the style is still evaluated at the scale zoom |
| `pixel_ratio` | everywhere | device pixels per CSS pixel: same view, denser image |

Problems:

- Two different "zoom" grids (512 px and 256 px) leak into the API.
- `tile_zoom` creates a state MapLibre never has: style rules evaluated at one
  zoom, tile data from another. Labels, line widths and feature selection
  stop matching.
- In plots, the extent and the device size are fixed by the plot, so scale and
  symbol size trade off. The only "more detail" control is `tile_zoom`, or
  lowering `pixel_ratio` (which is the coherent one, but nobody would guess it).
- `zoom` exists for `basemap()` but not for plot layers or
  `render_basemap_raw()`; `tile_zoom` exists for those but not for `basemap()`.

Affects U1, U2, U3.

### F4 (decoherence) Several extent models, none complete

- `basemap()`: `center` + `zoom` (camera), or `bounds` + `crs` (fit to size).
- `render_basemap_raw()`: `bounds` + `crs` + size (fit), returns the extent of
  the **input** box in the **input** CRS, though the image covers the fitted
  Web Mercator view.
- Plot layers: implicit, from the plot.
- **Missing:** "this area at this level of detail" (`bounds` + `zoom`, size
  derived), the natural request for U3.
- `center` is longitude/latitude only; `bounds` takes any CRS.
- **Bug:** date-line-crossing boxes (`xmin > xmax`) are documented in the
  README and supported by the core, but R's and Python's bbox validation reject
  them ("bbox xmin (1.9e+07) must be less than xmax"). Verified in both.
- A view can be at most one world wide (core limit), so very wide views zoom
  in (see `constrain_view`), unlike MapLibre's side-by-side world copies.

### F5 (decoherence) Duplicate entry points

- `render_basemap_raw()` and `basemap()` + `render_array()` do the same job
  with different argument models (F4) and different results.
- Rust has `render`, `render_stack`, `render_svg`, `render_svg_with`,
  `render_svg_stack`, `render_svg_stack_with` (every combination of
  single/stacked × image/SVG × options), plus the `Basemap` methods, plus
  `RenderRequest` as a public struct.
- Styles have six producers returning the same kind of thing (a string):
  bare names (`"CartoDB.Positron"`), `basemap_provider()`, the `providers`
  object, `tile_style()`, `wms_style()`, `wmts_style()`.

### F6 (decoherence) Styles are strings; their options live in the wrong place

- A style is a URL or JSON *string*; a stack is a vector of strings.
- `layers`, `labels`, `features` and `auth_token` are **view** options that
  apply to every style of a stack. So a stack cannot drop the labels of one
  style only, and cannot use two different keys for two services. The stack
  code has special rules to make the filters "apply to the stack as one
  basemap" (`render_stack` in `lib.rs`), a sign the concept is misplaced.
- `basemap_provider(api_key=)` puts a key into a URL; `auth_token` sends a
  header. Both are credentials, configured in different places.
- Three filters overlap: `layers` (layer ids, wildcards, `-` to drop),
  `labels` (`all` / `none` / `only`) and `features` (semantic names, `-` to
  drop). `labels` is a special case of `features` (labels are a feature
  class), with extra behaviour for raster providers (switching to their
  label-free tiles).

### F7 (decoherence) Outputs differ by language and by function

- R `render_array()` attaches the extent and EPSG code as attributes; Python
  `render_array()` returns a bare `ndarray` with no location.
- R has `render_raster()` (`terra`); Python has no georeferenced equivalent,
  except that `add_basemap()` *returns* an `xarray.DataArray` of its first
  render (a plot function returning data is itself surprising).
- Each `render_*` call renders again: PNG plus array means two renders. The
  caches added in 0.2.0 make the second cheaper, but it is still a full
  rasterisation.
- R arrays are integer arrays: 4 bytes per channel, 4× the image (a
  4000×3000 render is 192 MB as `[h, w, 4]` integers).

### F8 (decoherence) Logic duplicated across the bindings

| Logic | Copies |
|---|---|
| Physical size → CSS px + pixel ratio (`dpi / 96`) for plots | `geom_basemap` (R), `tm_basemap` (R), `BasemapImage` (Python, used by matplotlib and plotnine) |
| Bounds normalisation (vector, `bbox`, `sf`, dict, GeoDataFrame) and reprojection | `R/bbox_utils.R`, `bbox_utils.py` |
| Validation (sizes, zoom, pixel ratio, labels) | R, Python and the core, with different messages and limits (F4's date-line bug is one result) |
| Attribution drawing and wrapping | R grob, tmap credits, matplotlib artist |
| View math (F2) | Rust, R, Python |

### F9 (decoherence) The Rust API

- All 13 modules are `pub`, including renderer internals (`vector::gl`,
  `vector::symbol`). Every internal change is a potential breaking change.
- `Basemap` still has the four fetch setters that R and Python replaced with
  `fetch`; there is no Rust `FetchOptions`.
- `RenderRequest` is public with public fields; the bindings construct it with
  struct literals, which is why it cannot be made `#[non_exhaustive]`.
- The README's claim that "the R, Python and Rust APIs are the same: the same
  functions, the same argument names and defaults" is not true (F2, F5, this
  finding).

### F10 (decoherence) Unfinished transitions

- `tile_timeout_ms`, `max_tiles`, `tile_concurrency` and `fail_on_tile_error`
  survive as deprecated arguments next to `fetch` (0.2.0). That was a
  reasonable bridge for one release; kept, it is two models of the same thing.
- `render_basemap_raw` survives next to `basemap()` (F5).

### F11 (friction) Smaller inconsistencies

- Python `render_basemap_raw()` accepts optional arguments by position; R
  requires names after `style`.
- R values are immutable (`set_view()` returns a copy); Python's `Basemap` is
  mutable (`set_view()` changes it and returns it). A view reused for two plots
  behaves differently in each language.
- `get_attribution()`: R takes a view or styles; Python takes styles, and the
  view has a method.
- `tm_basemap()` has `width` / `height`; the other plot layers do not.
- `pixel_ratio` defaults to 1 for `basemap()` and to `dpi / 96` for plot
  layers. Both are right for their context, but the rule is not stated as one
  model.
- `tile_cache()` (global) is the only process-wide setting; everything else is
  per view.

### F12 (decoherence) Diagnostics that never reach the user

The core records 19 conditions with `log::warn!`, for example a tile that
failed and was skipped, missing glyphs or sprite, a style feature it does not
draw (terrain, GeoJSON clustering, a source type), an undecodable or
non-square DEM tile, a font that could not be subset. Neither binding forwards
the Rust log, so R and Python users never see any of them: a map with holes
in it renders without a word. Only the filter warnings (names matching
nothing) travel back, through a separate `warnings` field.

At the same time, the choices basemapper makes for the user (automatic zoom,
default dpi, a constrained view) are visible only by calling getters.

### What is already coherent (keep)

- The Rust core owns fetching, styles, rendering, caching, the catalogue and
  attribution. R and Python share it exactly.
- The MapLibre conventions: CSS pixels, fractional zoom, `devicePixelRatio`,
  512 px tiles. They are well defined and verified pixel by pixel.
- One error taxonomy across languages (validation / style / network / render).
- Download settings as one object (`fetch_options()` / `FetchOptions`).
- Plot layers that render at draw time, for the panel and device resolution.

---

## 4. The proposed model

### 4.1 Four concepts, one owner each

| Concept | What it is | Owner |
|---|---|---|
| **Style** | what to draw: a source (catalogue name, style URL/JSON, tile data, OGC service) plus its own options: filters, credentials, opacity | core |
| **View** | the three primary controls (extent, zoom, dpi) and the image size, each given, derived or defaulted by one relation and one default order (§4.2), always constrained as MapLibre constrains it | core |
| **Basemap** | styles (bottom to top) + a view + fetch settings: renders on its own, or goes into a plot, which fills in what the view leaves out | core; R and Python wrap it |
| **Rendering** | the pixels or SVG of a basemap, with its extent and CRS, convertible to PNG, arrays, rasters | core; R and Python convert |

R and Python become thin: they convert their native types (vectors, `sf`,
GeoDataFrames, devices) into these concepts and back, and add plot
integration. They hold no view math, validation or bbox logic of their own.

### 4.2 Size, resolution and detail

#### What others do

| Product | "Size" means | High density | Detail |
|---|---|---|---|
| MapLibre GL JS | the container's CSS size | `pixelRatio` (defaults to the screen's `devicePixelRatio`) | `zoom`; `bounds` "overrides center and zoom" |
| Google Static Maps | `size`, pixels | `scale` = 1 or 2: "twice as many pixels … the contents of the map don't change" | `zoom`, or fitted to markers |
| Mapbox / MapTiler Static | `width` × `height`, pixels | `@2x` | `zoom`; `bbox`: "the most detailed zoom level that fits" |
| ggplot2 `ggsave()` | `width`, `height` in `units` (`"in"`, `"cm"`, `"mm"`, `"px"`) | `dpi` | — |
| contextily `add_basemap()` | (the axes) | — | `zoom = "auto"` or a number; `zoom_adjust` nudges the automatic one |
| ggspatial `annotation_map_tile()` | (the plot) | — | `zoom`, or `zoomin`, a "delta on default zoom" |

What this shows: users of every product need the same three controls (how
big the image is, how sharp it is, how detailed the map is), and the browser's
"device pixels per CSS pixel" is too low-level for them (even the static map
services reduce it to a 1-or-2 switch). It does not settle the design; the
reasons below do.

- **Legibility is physical.** A label is readable or not at its printed or
  displayed size: a style's 12 px label must stay 12/96 of an inch whatever
  the resolution. A size plus a `dpi` states exactly that; a pixel ratio
  states it indirectly.
- **One quantity for every context.** A plot device already has a size and a
  dpi. If standalone maps use the same two, plot layers and standalone maps
  share one model instead of translating between two.
- **Detail is zoom.** Styles are written in zoom: which roads, places and
  labels appear is decided per zoom level. Zoom is therefore the detail
  variable; a map scale (1:25,000) is derived from it.

So:

#### The model: three primary controls

A map answers three questions. Each has one argument, and each means the same
thing everywhere:

| Question | Primary control | Meaning |
|---|---|---|
| **What to show** | the extent: `bounds` (an area), or `center` (+ `zoom`, the camera) | the part of the Earth on the map |
| **How much feature detail** | `zoom` | the MapLibre zoom (fractional): which roads, places and labels the style draws, and at what size |
| **How sharp** | `dpi` | the output's resolution: how many pixels make an inch |

Two **helpers** describe the same things in other terms, so users can say
what they know:

| Helper | Meaning | Unit |
|---|---|---|
| `width`, `height` | the image's size | `units`: `"px"` (default), `"in"`, `"cm"`, `"mm"`. `units` applies to `width` and `height` only |
| `around(center, span, units)` | an area for `bounds`: `span` (width, height) on the ground around `center` | `center` in longitude / latitude (or the CRS given to `around()`); `span` in **ground distance**, in `units`: `"m"` (default), `"km"`, `"mi"`, `"ft"`. This `units` belongs to `around()` and can only mean ground distance |

`around()` computes the box on the sphere around `center` (half the span to
each side, measured on the ground), so it needs neither PROJ nor `sf`, and is
accurate to well under 1% for spans up to a few hundred kilometres. A scalar
`span` gives a square.

Pixels or inches are one concept, the image's size, in different units:
`width = 6, height = 4, units = "in", dpi = 300` is an 1800 × 1200 image.
Label and symbol sizes are not a control: the style sets them per zoom, and
`dpi` turns them into pixels.

#### Coordinates carry their CRS; `crs` is the map's

Two different things used to share one argument: the CRS of the numbers a
user types, and the CRS of the map they get. They are now separate.

**Coordinates carry their own CRS.** No argument says how to read them:

| Input | Read as |
|---|---|
| plain numbers (`center = c(-111.9, 40.76)`, `bounds = c(xmin, ymin, xmax, ymax)`) | always longitude / latitude: what people know and what every web map takes |
| a spatial object for `bounds` (`sf`, `sfc`, `bbox`, GeoDataFrame, GeoSeries) | its own CRS |
| `around(...)` | an area with its CRS attached (longitude / latitude), built from true ground distances |

**`crs` is only the map's CRS**: the CRS of the output (the rendering, its
bounds, `as_raster()` / `to_xarray()`, the array's georeferencing). The core
renders in Web Mercator (the styles' own projection) and warps into `crs`,
with the same warp as the plot layers (§4.5).

| | Map CRS |
|---|---|
| on its own | `crs` if given; else the CRS of a spatial layer giving the extent (`bounds`, or `clip` when it sets the extent: an `sf` object, GeoDataFrame, ...) **when that CRS is projected**; else Web Mercator (EPSG:3857), as every web map shows it. Plain numbers, `around()` and layers in longitude / latitude therefore give Web Mercator |
| in a plot | the plot's CRS (the basemap is warped into it) |

Only projected CRSs are inferred (decision D19). A layer in a geographic CRS
(longitude / latitude, such as EPSG:4326 or NAD27) is almost always data as
published, not a chosen display projection, and a basemap drawn in plain
longitude / latitude looks stretched; so it falls back to Web Mercator, like
plain numbers. `crs = 4326` still gives one when wanted; in a plot, the
plot's CRS applies as always.

So a map in Web Mercator around a lat/long point, or in UTM from an `sf`
object, are both one line:

```r
basemap(s, bounds = around(c(-111.9, 40.76), c(20, 15), units = "km"))   # 20 × 15 km, Web Mercator
basemap(s, bounds = nc_utm)                                      # the data's area, in its CRS (inferred)
basemap(s, center = c(-111.9, 40.76), zoom = 12, crs = 32612)     # the camera, drawn in UTM 12N
```

`crs` accepts anything that names a CRS: a code (`3857`), a string
(`"EPSG:32612"`), or an object that has one (`sf::st_crs(x)`, an `sf`
object, a `pyproj.CRS`, a GeoDataFrame).

Caveats, for the docs:

- **Labels in another CRS** are warped with the image: over large areas in a
  strongly different CRS they are slightly rotated or skewed. At city or
  state scale this is invisible. Labels kept upright in any CRS would need
  vector re-rendering, out of scope for now.
- **SVG in another CRS:** paths and text positions are transformed through the
  same coordinate mesh and embedded raster tiles are warped. Until that
  exists, `render_svg()` of a map not in Web Mercator is an error, not a
  silent Web Mercator SVG.

#### One relation: any three determine the fourth

Extent, zoom, image size and dpi are tied by one relation:

```
ground width = image pixels × 96 / dpi × metres per style pixel at the zoom
```

For example, 800 px at 96 dpi and zoom 12 at Salt Lake City cover about
11.6 km; at 192 dpi the same 800 px cover 5.8 km. So **any three of extent,
zoom, image size and dpi determine the fourth**:

| You give | Derived | Use case |
|---|---|---|
| extent + zoom + dpi | the image size: "this area at this detail" | U3 |
| extent + image size + dpi | the zoom (**automatic zoom**) | U1, U3 |
| extent + zoom + image size | the dpi: "this area at zoom 12 in 800 px" is drawn at 150 dpi | U1 |
| `center` + zoom + image size + dpi | the area shown: the MapLibre camera | U1 |

`dpi` changes only sharpness (the same map, with more or fewer pixels)
unless you fix the image size in pixels and leave the zoom out: then `dpi`
decides how much detail fits in those pixels, as on paper. That is the one
case where dpi changes content, and only because you fixed the pixels.

#### Defaults: only for what cannot be derived

A value you give is used. A value you leave out is derived if it can be;
otherwise it is filled from a default, in this order, until exactly one
value is left to derive:

| Order | Value | On its own | In a plot |
|---|---|---|---|
| 1 | `dpi` | 96 | from where the plot is drawn (below) |
| 2 | image size | 800 × 600 px at 96 dpi, scaled with the dpi (2500 × 1875 at 300): a fixed physical size, so dpi stays a sharpness control | the plot panel |
| — | map CRS | `crs`, else a spatial extent layer's if projected (D19), else Web Mercator | the plot's CRS |
| 3 | extent | the whole world | the plot's axes |
| — | `zoom` | never a default: always derived when not given | always derived when not given |

So:

| You write | Result |
|---|---|
| `basemap(s)` | the world, 800 × 600, 96 dpi, zoom derived |
| `render(basemap(s, bounds = nc), dpi = 300)` | 2500 × 1875, the same map as at 96 dpi, sharper |
| `basemap(s, bounds = nc)` | 800 × 600, 96 dpi, zoom derived |
| `basemap(s, bounds = nc, zoom = 9)` | 96 dpi, image size derived |
| `basemap(s, bounds = nc, zoom = 9, width = 800, height = 600)` | dpi derived |
| `basemap(s, center = c(-111.9, 40.76), zoom = 12)` | 800 × 600, 96 dpi, the area shown derived |
| `basemap(s, bounds = around(c(-111.9, 40.76), c(20, 15), units = "km"))` | 20 × 15 km on the ground, Web Mercator; 800 × 600, 96 dpi, zoom derived |
| `basemap(s, bounds = nc_utm)` | the data's area, rendered in UTM (inferred from the layer) |
| `ggplot(nc) + basemap(s)` | everything from the plot, zoom derived |

#### Conflicts

| Case | Response |
|---|---|
| extent, zoom, image size and dpi all given | an error that shows what each combination of three would give |
| `bounds` together with `center` | an error: two extents |
| `center` without `zoom` | an error: "give `zoom`, or `bounds = around(center, span)`" |
| a basemap with its own extent or `crs` placed in a plot | an error: the plot's axes and CRS are the map's (decision D15) |
| in a plot, a `zoom` you chose | the plot fixes the extent and the image, so the dpi is derived (an effective dpi), and reported once: "zoom 12 drawn at an effective 150 dpi on a 300 dpi device: labels at half size" |
| the shape of `bounds` differs from the image's | the map shows at least `bounds`: the extent grows on one side. Shown on request, not warned |

Every value is labelled **given**, **derived** or **default**, which is what
the notes report (§4.8).

#### In a plot

`ggplot(nc) + basemap(s)` needs nothing else. The axes give the extent, the
plot's coordinate system gives the CRS (the basemap is warped into it), and
the panel gives the image size. `crs`, `bounds` and `center` are not
needed. The dpi depends on where the plot is drawn:

| Drawn on | `dpi` used |
|---|---|
| a screen (RStudio / Positron pane, `windows()`, `quartz()`, `X11()`, Jupyter, a matplotlib window) | 96 × the screen's pixel ratio (2 on most high-density screens, where it can be detected) |
| a raster export (`ggsave()`, `png()`, `ragg`, `savefig()`) | the device's dpi |
| a vector export (PDF, SVG) | 300: such devices have no dpi (reported as a default) |
| any, with `basemap(s, dpi = ...)` | yours |

The zoom is derived from the extent, the panel's pixels and that dpi.
Pixels divided by dpi is the panel's physical size, so a plot previewed on
screen and saved with `ggsave(dpi = 300)` derive **the same zoom**: the
export shows the same map as the preview, only sharper. When the device type
is unknown, it is treated as an export: a device's own dpi always matches its
pixels.

#### One basemap object, used anywhere

Because unset values come from the context, one object serves every
context. It renders on its own, or goes into a plot, which supplies what it
leaves out:

```r
m <- basemap("OpenFreeMap.Liberty")                 # nothing set
render(m)                                          # on its own: the world, 800 × 600
ggplot(nc) + m + geom_sf()                         # in a plot: extent, size and dpi from the plot
ggplot(nc) + basemap("OpenFreeMap.Liberty", zoom = 9)  # more detail (an effective dpi)
```

```python
m = basemapper.basemap("OpenFreeMap.Liberty")
m.render()                                         # on its own
m.add_to(ax)                                       # matplotlib
ggplot(df) + m + geom_point()                      # plotnine
```

`geom_basemap()`, `tm_basemap()` and `add_basemap()` remain as **aliases**:
they build the same object with the same arguments and place it, with one
implementation behind them (decision D14).

Whether to also offer a relative nudge for the derived zoom, as contextily
(`zoom_adjust`) and ggspatial (`zoomin`) do, is decision D11. The derived
zoom is shown on the rendering and in verbose mode (§4.8), so a user can see
where to start.

#### Consequences

- **Three primary controls**, each with one meaning everywhere: extent,
  `zoom`, `dpi`. Helpers (`width` / `height`, `around()`) only express them in
  other terms.
- **One relation and one default order** decide every case, on its own and
  in a plot; there are no context-specific rules.
- **`zoom` is always the MapLibre zoom** (512 px tiles, fractional). The
  second grid (`tile_zoom`, 256 px) and `pixel_ratio` leave the API
  (internally the pixel ratio is `dpi / 96`).
- **A plot preview and its export show the same map.**
- **Conflicts are errors with the alternatives spelled out;** nothing is
  silently dropped.
- The view is **always constrained** (poles, minimum zoom), in the core.

### 4.3 Styles as values

```r
# R
s <- style("OpenFreeMap.Liberty", labels = FALSE)          # catalogue name, options
k <- style("MapTiler.Streets", key = Sys.getenv("MAPTILER_API_KEY"))
t <- style("data/parcels.pmtiles", paint = list(`fill-color` = "#e8e0d8"))
basemap(list(style("Esri.WorldImagery"), style("OpenFreeMap.Liberty", features = "labels")), ...)
```

```python
# Python
s = basemapper.style("OpenFreeMap.Liberty", labels=False)
basemapper.basemap([basemapper.style("Esri.WorldImagery"),
                    basemapper.style("OpenFreeMap.Liberty", features=["labels"])], ...)
```

- **One constructor, `style()`**, for every source. It replaces
  `basemap_provider()`, `tile_style()`, `wms_style()` and `wmts_style()`
  (OGC services by URL plus parameters, or by explicit arguments).
- **Filters and credentials belong to the style.** Each style of a stack has
  its own; the "stack as one basemap" special cases disappear.
- **One credential argument, `key`,** used however the source needs it: put in
  the URL for catalogue providers, sent as a Bearer token to private servers
  (scoped to their origin, as now).
- **Two filters instead of three:** `features` (semantic classes, now
  including `"labels"`; `-` drops) and `layers` (layer-id patterns, for the
  rest). `labels = FALSE` becomes `features = "-labels"`. (Whether to keep
  `labels` as a shorthand is decision D3.)
- A plain string still works wherever a style is expected; it means
  `style(string)`. Simple uses stay simple.
- `list_layers(style)` and `get_attribution(x)` take styles or basemaps.

### 4.4 Renderings as values

```r
img <- render(m)          # one render
write_png(img, "map.png")
as_array(img)             # [h, w, 4], with extent and CRS
as_raster(img)            # terra SpatRaster (terra suggested)
plot(img)
```

```python
img = m.render()           # one render
img.save("map.png")
img.to_numpy()             # (h, w, 4) uint8
img.bounds, img.crs        # georeferencing, the same in both languages
img.to_xarray()            # xarray extra
```

- One render, many conversions (F7). Georeferencing is the same in both
  languages.
- `render(m, dpi = 300)` is the same as `render(set_view(m, dpi = 300))`: dpi
  is a property of the basemap like the others, and this spelling exists
  because exporting one map at several resolutions is common.
- SVG stays its own call (`render_svg()`), since it is a different rendering
  rather than a format of the same pixels.
- `render_png()` and `render_array()` can remain as one-line conveniences
  over `render()`, or go (decision D4).
- R keeps pixels as raw bytes and converts lazily (nativeRaster for plots,
  arrays on request), cutting the 4× integer-array cost.

### 4.5 Plot layers

A plot layer is a basemap placed in a plot (§4.2): the plot supplies the
extent (its axes and CRS) and the output (the panel's size, the device's
dpi), the core resolves the rest with the same rule as on its own. How it is
placed is each library's idiom:

| Library | Placing a basemap |
|---|---|
| ggplot2 | `ggplot(nc) + basemap(s) + geom_sf()` (via `ggplot_add()`), or the alias `geom_basemap(s)` |
| tmap | `tm_basemap(basemap(s))`, or the alias `tm_basemap(s)` (tmap only accepts its own elements) |
| matplotlib | `basemap(s).add_to(ax)`, or the alias `add_basemap(ax, s)` |
| plotnine | `ggplot(df) + basemap(s) + geom_point()` |

Then:

- **Warp, don't stretch (F1).** Render in Web Mercator, then resample into the
  panel's CRS. The host computes a coarse grid of panel-pixel → Web Mercator
  coordinates with `sf` (R) or `pyproj` (Python). The core resamples the
  image through that mesh. No GDAL is needed, and every plot layer behaves the
  same; tmap's separate `terra::project` path goes away.
- No plot-specific arguments: what can be set is what a basemap has (styles,
  `zoom`, `fetch`, ...), plus the drawing options `alpha` and `attribution`.
  Size and dpi always come from the device; `tm_basemap()`'s `width` /
  `height` go.
- `add_to(ax)` returns the artist, not a data array.
- Attribution layout (wrapping, placement) computed once, drawn by each
  library's own primitive.

### 4.6 One basemap object, the same semantics in every language

- Values are **immutable** in all three languages: `set_view()` /
  `m.set_view()` return a new basemap (as R does, and as pandas' `set_*` do).
  One view can safely feed several plots.
- The R and Python objects **wrap the core's**: the core validates and
  computes, the bindings convert. Validation messages and limits are
  identical across languages.
- The same verbs in each language, spelled idiomatically:

| Task | R | Python | Rust |
|---|---|---|---|
| create | `basemap(styles, ...)` | `basemapper.basemap(styles, ...)` | `Basemap::new(styles).view(...)` |
| change the view | `set_view(m, center =, zoom =)`, `set_bounds(m, bounds)` | `m.set_view(...)`, `m.set_bounds(...)` | `m.with_view(...)`, `m.with_bounds(...)` |
| read the view | `get_bounds(m)`, `get_center(m)`, `get_zoom(m)`, `get_size(m)` | `m.bounds`, `m.center`, `m.zoom`, `m.size` | `m.bounds()`, ... |
| render | `render(m)`, `render_svg(m)` | `m.render()`, `m.render_svg()` | `m.render()`, `m.render_svg(...)` |
| use in a plot | `ggplot(...) + m`, `tm_basemap(m)`; aliases `geom_basemap()`, `tm_basemap(s)` | `m.add_to(ax)`, `ggplot(...) + m`; aliases `add_basemap()`, `geom_basemap()` | — |
| inspect a style | `list_layers()`, `get_attribution()` | same | same |

### 4.7 Before and after

| Task | Today (R) | Proposed (R) |
|---|---|---|
| City map | `basemap("OpenFreeMap.Liberty", 800, 600, center = c(-111.9, 40.76), zoom = 12)` | unchanged |
| Area at a given detail (GIS) | not possible: fit to a pixel size, then compute the zoom | `basemap(s, bounds = nc, zoom = 9)` |
| Basemap under a ggplot | `ggplot(nc) + geom_basemap(s) + geom_sf()` | `ggplot(nc) + basemap(s) + geom_sf()` |
| More detail under a ggplot | `geom_basemap(s, tile_zoom = 11)` (mixed zoom state) | `ggplot(nc) + basemap(s, zoom = 11)` (an effective dpi, reported) |
| Same basemap, plot and image | two calls with the same arguments | `m <- basemap(s)`; `ggplot(nc) + m`; `render(m)` |
| A sharp image for a web page | `basemap(s, 800, 600, ..., pixel_ratio = 2)` (a 1600 × 1200 image) | `basemap(s, 1600, 1200, ..., dpi = 192)` |
| A map for a 6 × 4 inch print | `basemap(s, 576, 384, ..., pixel_ratio = 300 / 96)` | `basemap(s, 1800, 1200, dpi = 300, ...)` |
| The same map at screen and print resolution | two views with different sizes and ratios | `render(m)`, `render(m, dpi = 300)` (same content, sharper) |
| 20 km around a point | compute a zoom from the width by hand | `basemap(s, bounds = around(c(-111.9, 40.76), c(20, 15), units = "km"))` |
| A basemap raster in the data's CRS | render in 3857, then `terra::project()` | `as_raster(render(basemap(s, bounds = nc_utm)))`: in UTM |
| Imagery under one style's labels | `basemap(c("Esri.WorldImagery", basemap_provider("OpenFreeMap.Liberty", labels = "only")))` | `basemap(list("Esri.WorldImagery", style("OpenFreeMap.Liberty", features = "labels")))` |
| Two keyed services in one stack | not possible (one `auth_token`) | `style(a, key = k1)`, `style(b, key = k2)` |
| PNG and array of one view | two renders | `img <- render(m)`, then `write_png(img, ...)`, `as_array(img)` |
| Pacific-centred box | documented, fails | `basemap(s, bounds = c(150, -40, -120, 10))` |
| Basemap under a lon/lat ggplot | misplaced by up to tens of pixels | correct (warped) |

### 4.8 Telling users what was decided

Every rendering carries **notes**: a list of structured diagnostics (kind,
message, details) collected by the core during the render. They replace both
the scattered `log::warn!` calls and the separate filter `warnings` field.
Each note has one of three levels, and the level decides how the bindings
surface it:

| Level | When | How it is surfaced |
|---|---|---|
| **Decided** | basemapper chose a value the user left open: the automatic zoom, the default dpi (96 standalone, the device's in plots) | never interrupts: shown in `print()` / `repr()` (`zoom 12.31 (auto)`), on the `Rendering` (`img$zoom`, `img$dpi`, `img$scale`), and in a verbose mode (`options(basemapper.verbose = TRUE)`, the `basemapper` logger at INFO in Python) |
| **Adjusted** | basemapper changed what the user asked for, or settled for less: zoom raised or a centre moved by the view constraints; a `zoom` chosen in a plot, so the dpi is derived (an effective dpi, with labels at a different size than on the device's dpi); coarser raster tiles to stay within `max_tiles`; `zoom` beyond the source's most detailed tiles (stretched, blurry); 300 dpi assumed for a PDF / SVG device | a message, **once per kind per session**: R `cli::cli_inform(.frequency = "once")`, Python `warnings.warn` (shown once per call site by default) |
| **Problem** | the output is incomplete or likely not what was meant: tiles skipped, glyphs / sprite / DEM missing, unsupported style features, filter names matching nothing, a token not sent to a server outside its scope | an R `warning()` / Python `UserWarning`, every time; and an error instead when `fetch$on_error = "fail"` |

Why this split:

- A warning means something may be wrong. Routine choices are not, and
  warning about them on every redraw or batch job teaches users to ignore
  warnings, including the real ones.
- An adjustment is worth knowing once, not on every plot redraw.
- Problems must never be silent, which today they are (F12).
- The information lives on the result, so it is available to code (batch
  checks, tests), not only to people reading a console.

### 4.9 Where basemapper should lead

Designs no reference offers, each justified by a use case:

- **`bounds` + `zoom` → size** (U3): "this area at this level of detail" is the
  natural GIS request; every other tool makes the user reverse-engineer a
  pixel size.
- **One relation for extent, zoom, size and dpi** (U1, U2, U3): any three
  give the fourth, the same on its own and in a plot, instead of
  tile-count heuristics; a plot's preview and its export show the same map.
- **The map scale on every rendering** (U1, U3): each `Rendering` reports its
  true scale (1:N at its centre latitude, from zoom, dpi and latitude), for
  print and cartographic use, without a second detail argument.
- **Warping without GDAL** (U2, U3): basemaps in any plot CRS, aligned, with
  only `sf` / `pyproj` for the coordinates.
- **One core, three languages, one written contract** checked in CI (§7).


### 4.10 Clipping to a layer

`clip = layer` draws the basemap only inside the layer's polygons (a study
area, a county); outside is transparent. `clip = outside(layer)` does the
reverse: the basemap everywhere except inside the polygons.

```r
basemap(s, clip = nc)                                                 # nc's area, drawn only inside nc
basemap(s, bounds = around(c(-79, 35.5), 600, units = "km"), clip = nc)  # a wider frame, clipped to nc
ggplot(nc) + basemap(s, alpha = 0.3) + basemap(s, clip = nc)          # faded surroundings, full colour inside
ggplot(nc) + basemap(s, clip = outside(lakes)) + geom_sf()           # a hole where the lakes are
```

```python
basemapper.basemap(s, clip=gdf)
basemapper.basemap(s, bounds=frame, clip=basemapper.outside(lakes))
```

Rules:

- **A clip is a mask on the finished image**, in output pixels. It never
  changes the extent, zoom or dpi of a map that has them, so it is
  independent of the three primary controls.
- **Its one link to them is the default extent:** a clip sets the default
  extent to the bounding box of the area it keeps.
  - `clip = nc` keeps the inside, so without `bounds` (and outside a plot)
    the extent is nc's bounding box, as with `bounds = nc`.
  - `clip = outside(nc)` keeps everything outside, an area with no bounding
    box, so it sets no extent: `bounds` or a plot gives the frame, else the
    default (the whole world) applies.
  - With `bounds`, `bounds` is the frame and `clip` the mask, either way.
  - In a plot, the plot always gives the extent, so a clip there is only a
    mask (in either direction): it is not a second extent, and not a D15
    conflict.
  - The map's CRS is inferred from the layer that gives the extent (D18,
    D19): a clip that sets the extent also sets the CRS; `outside()` sets
    neither.
- **The geometry carries its own CRS**, like every coordinate input; R and
  Python transform it to the map's CRS (`sf::st_transform`, `to_crs`).
- **Polygons only.** A point or line layer is an error that suggests
  `sf::st_buffer()` / `GeoSeries.buffer()`: the buffer distance is the
  user's choice.
- **Labels are cut at the edge**, as with any clip. Keeping whole labels for
  features inside would change symbol placement, far more complex.
- **Detailed geometries are simplified** to the output's pixel resolution
  before rasterising (nothing visible is lost).
- **SVG:** an exact `<clipPath>` of the polygons; for `outside()`, the frame
  with the polygons as holes (even-odd).
- **`outside()` is a helper, not a sign.** `-nc` cannot express it: on an
  `sf` geometry unary minus already negates the coordinates (a mirrored layer
  would arrive), and on a GeoDataFrame it is arithmetic. `outside(layer)`
  only marks the layer; the same polygon-only rule applies.

Implementation: R and Python pass the polygon rings (in the map's CRS) to
the core, which maps them to output pixels with the rendering's bounds,
rasterises an alpha mask with the renderer's polygon rasteriser
(`vector/gl.rs`, at 4× for smooth edges; even-odd for holes and
multipolygons), inverts it for `outside()`, and multiplies the image's alpha
by it. The mask is applied
after the warp, so it works in any map CRS. About a day or two with tests,
cleanest after the View / warp refactor.

---

## 5. Architecture

1. **The core owns every rule.** View math and constraints, validation, bounds
   handling (including date-line boxes), the plot plan, warping, attribution
   layout, rendering and conversions all live in Rust, once.
2. **Bindings are adapters.** R and Python convert types (vectors, `sf`,
   GeoDataFrames, devices, arrays) and integrate with plotting libraries.
   CRS transforms stay in the host (`sf` / `pyproj`), as now; the core stays
   free of PROJ and GDAL.
3. **One entry point per job in the core:** `Basemap::render()`,
   `Basemap::render_svg()`, `style::resolve()`, `list_layers()`,
   `attribution()`. `RenderRequest` and the six render functions become
   internal.
4. **A curated Rust facade.** Only `Basemap`, `View`, `Style`, `Rendering`,
   `FetchOptions`, `SvgOptions`, the error type and the catalogue are public;
   modules become `pub(crate)`. Public structs are `#[non_exhaustive]` with
   constructors, so they can grow without breaking users.
5. **Binding objects wrap core objects.** Python: `#[pyclass]` types. R: an S3
   object whose fields are produced and validated by the core, keeping R's
   value semantics.

---

## 6. Performance and memory

| Item | Today | Proposed |
|---|---|---|
| Several outputs of one view | one render each | one render (`Rendering`), many conversions |
| R pixel arrays | integer arrays (4 B per channel) | raw bytes kept; `nativeRaster` for plots, arrays on request |
| Python arrays | one extra copy | the `Rendering` owns the buffer; `to_numpy()` views it |
| Plot redraws | the same render repeated on each draw | keyed by the view plan (matplotlib already does this; extend it to R) |
| Warping | n/a (stretching) | one mesh resample in Rust, parallel |
| Duplicated view/validation code | three copies | one |

The caches and parallel decoding from 0.2.0 stay as they are.

---

## 7. Future-proofing

- **Struct growth without breakage:** `#[non_exhaustive]` and constructors in
  Rust; keyword-only options in Python; named options in R.
- **New sources** (MapLibre Tiles, new OGC services) fit `style()` without new
  top-level functions.
- **World copies:** lifting the one-world limit later is a change inside
  `View`, invisible to callers.
- **New outputs** (GeoTIFF, WebP) are conversions of a `Rendering`.
- **Per-style options** (opacity, filters, keys) have a home.
- **CRS support** grows by improving the warp, not by adding parameters.
- **A versioned contract:** the R, Python and Rust surfaces are checked
  against one written spec (`docs/api.md`) in CI, so they cannot drift apart
  again.

---

## 8. Migration plan

One clean break, **0.3.0**, with a migration guide. No deprecation shims
(principle 2): the 0.2.0 `fetch` shims and `render_basemap_raw` go in the same
release.

| Phase | Work | User-visible |
|---|---|---|
| 1. Core | `View` (all specification modes, constraints, date line), `Style` values, `Rendering`, the plot plan, mesh warping, the Rust facade | none yet; tests only |
| 2. Bindings | R and Python rebuilt as adapters over the core objects; plot layers on the shared plan with warping; immutable basemaps | the new API (§4) |
| 3. Remove | old entry points, shims, duplicated R/Python logic | 0.3.0 |
| 4. Docs | guides rewritten around the four concepts; migration guide; README claims checked by tests | 0.3.0 |

Tests to add first, so the refactor is safe: plot alignment in lon/lat and
projected CRSs (F1), date-line boxes from R and Python (F4), cross-language
parity of views and validation messages (F2, F8), and one-render-many-outputs
(F7). The golden images stay the ground truth for pixels.

---

## 9. Decisions

All accepted as recommended on 2026-11-23.

| # | Decision | Accepted |
|---|---|---|
| D1 | Clean 0.3.0 break with no shims, or one release of shims? | clean break (finish the transition) |
| D2 | `style()` objects, replacing `basemap_provider` / `tile_style` / `wms_style` / `wmts_style` | yes |
| D3 | Fold `labels` into `features` (`"labels"`, `"-labels"`), or keep `labels` as a shorthand? | fold it in: one filter model |
| D4 | `render()` returning a `Rendering`; keep `render_png()` / `render_array()` as conveniences? | `render()` plus `render_svg()` only |
| D5 | Immutable basemaps in Python (`set_view()` returns a copy) | yes, matching R |
| D6 | Replace `tile_zoom` with `zoom` in plot layers | yes |
| D7 | Warp in the core (host computes the coordinate mesh) | yes |
| D8 | Python view getters as properties (`m.zoom`) or methods (`m.get_zoom()`) | properties (Pythonic), with R's `get_*()` |
| D9 | Narrow the Rust public API to a facade | yes |
| D10 | Three primary controls (extent, `zoom`, `dpi`) with helpers (`width` / `height` with `units`, `around()`), "any three determine the fourth", and the default order dpi → image size → extent; `pixel_ratio` leaves the API | yes (§4.2) |
| D11 | Besides `zoom`, a relative nudge for the automatic zoom in plots (`zoom_adjust`, as contextily)? | no at first: one detail argument; add it only if absolute zooms prove awkward in practice |
| D12 | `dpi` shortcuts (`"retina"`, `"print"`)? ggsave's mean 320 / 300 / 72 (screen), but a map's screen is 96 | numbers only, to avoid a second, conflicting meaning of "screen" |
| D13 | Notes on every rendering, in three levels: decided (shown on request), adjusted (a message once per session), problem (a warning every time); core diagnostics forwarded to R and Python (F12) | yes |
| D14 | One basemap object for every context, placed with each library's idiom (`ggplot() + m`, `m.add_to(ax)`, `tm_basemap(m)`); `geom_basemap()`, `tm_basemap()` and `add_basemap()` kept as aliases over the one implementation | yes |
| D15 | A basemap with its own extent or `crs` added to a plot: an error, or the plot wins? | an error: two extents (or CRSs) for one map is a contradiction, not a preference |
| D16 | Plot dpi: on screens 96 × the screen's pixel ratio; in raster exports the device's dpi; in PDF / SVG 300; a given `dpi` overrides | yes: preview and export derive the same zoom |
| D17 | `around(center, span, units = "m")` for "an area around a point": `center` in lon/lat, `span` in true ground distance in its own `units` (`"m"`, `"km"`, `"mi"`, `"ft"`); `basemap()`'s `units` (`"px"`, `"in"`, `"cm"`, `"mm"`) for `width` / `height` only | yes |
| D18 | Coordinates carry their CRS (plain numbers are lon/lat; spatial objects and `around()` bring theirs); `crs` is only the map's (output) CRS: given, else inferred from a spatial layer giving the extent, else Web Mercator; in a plot, the plot's | yes |
| D19 | A spatial layer in a geographic (lon/lat) CRS: infer only projected CRSs; geographic layers fall back to Web Mercator (`crs = 4326` when wanted) | adopted |
| D20 | `clip = layer`: polygons only (points / lines are an error suggesting a buffer), labels cut at the edge, geometry simplified to the output resolution, default extent = the bounding box of the kept area (§4.10) | yes |
| D21 | Inverse clipping as `clip = outside(layer)` (a helper, not `-layer`): sets no default extent or CRS; supported in the first version | adopted |

F1 (plot misalignment) and the date-line bug (F4) are fixed by the
redesign: their tests are written first (phase 0 of the plan) and turn green
with the Python and R adapters (phases 3 and 4).
