#' Create a basemap view
#'
#' A map view to render headlessly: one style, or several stacked. The view
#' is a centre and MapLibre zoom (512 px tiles; fractional zooms are rendered
#' natively, not resampled), or bounds to fit. `width` and `height` are CSS
#' pixels; images are `pixel_ratio` times larger with the same view and label
#' layout (MapLibre's `devicePixelRatio`): `pixel_ratio = 2` gives a sharp 2x
#' image, `pixel_ratio = dpi / 96` matches a print resolution.
#'
#' Render it with [render_png()], [render_svg()], [render_raster()] or
#' [render_array()]. The same function exists in Python
#' (`basemapper.basemap()`) and Rust (`Basemap::new()`), with the same
#' arguments.
#'
#' @param style A MapLibre GL style URL or JSON string, a catalogue name
#'   (see [basemap_provider()]), or a character vector / list of those
#'   stacked bottom to top (e.g. a basemap published as separate hillshade,
#'   base and label services; `"UGRC.Lite"` names such a stack).
#' @param width,height View size in CSS pixels.
#' @param ... Not used: the arguments after `height` must be named. (The
#'   deprecated `tile_timeout_ms`, `max_tiles`, `tile_concurrency` and
#'   `fail_on_tile_error` are still accepted, with a warning: use `fetch`.)
#' @param center `c(longitude, latitude)` of the view centre.
#' @param zoom MapLibre zoom (0--22, fractional allowed).
#' @param bounds Bounds to fit instead of `center`/`zoom`: `c(xmin, ymin,
#'   xmax, ymax)` in `crs` (default EPSG:4326), an [sf::st_bbox()] result, or
#'   an `sf` / `sfc` object.
#' @param crs CRS of `bounds` (anything [sf::st_crs()] accepts).
#' @param pixel_ratio Device pixels per CSS pixel.
#' @param layers Optional layer filter applied to every style: plain IDs keep
#'   only those layers, minus-prefixed IDs drop them; `*` and `?` are
#'   wildcards (`"-label_*"`). See [list_layers()]. Names that match no
#'   layer of any style warn.
#' @param labels `"all"` (or `TRUE`) keeps every layer, `"none"` (or `FALSE`) drops the labels (text and
#'   icons), `"only"` keeps only the labels on a transparent background, to
#'   stack over another map. Catalogue raster basemaps switch to their
#'   label-free or label-only tiles.
#' @param features Optional feature filter applied to every style: plain
#'   names keep those features, minus-prefixed names drop them, e.g.
#'   `c("water", "roads")` or `"-buildings"`. Features: background, water,
#'   landcover, parks, buildings, roads, rail, boundaries, places, poi,
#'   terrain, other (the `feature` column of [list_layers()]). Features no
#'   layer of any style belongs to warn.
#' @param auth_token Optional access token for the tile service (sent only to the style's own server).
#' @param fetch How tiles are downloaded: a [fetch_options()] object
#'   (timeout, tile limit, parallel downloads, what a failed tile does).
#'
#' @return A `basemap` object.
#' @export
#' @examples
#' \dontrun{
#' # UGRC's Lite basemap: its hillshade, base and labels services, stacked
#' m <- basemap("UGRC.Lite", 800, 600, center = c(-111.8887, 40.7556), zoom = 12.4,
#'              pixel_ratio = 2)
#' render_png(m, "slc.png")   # 1600 x 1200
#' render_svg(m, "slc.svg")   # vector
#' r <- render_raster(m)      # terra SpatRaster in EPSG:3857
#'
#' # Fit a layer's extent
#' nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' m <- basemap("OpenFreeMap.Positron", bounds = nc)
#' }
basemap <- function(
    style,
    width       = 800L,
    height      = 600L,
    ...,
    center      = NULL,
    zoom        = NULL,
    bounds      = NULL,
    crs         = NULL,
    pixel_ratio = 1,
    layers      = NULL,
    labels      = TRUE,
    features    = NULL,
    auth_token  = NULL,
    fetch       = fetch_options()
) {
  fetch  <- fetch_with_deprecated(fetch, list(...), "basemap")$fetch
  styles <- style_strings(style)
  width  <- as.integer(width)
  height <- as.integer(height)
  if (length(width) != 1L || length(height) != 1L || is.na(width) || is.na(height) ||
      width < 1L || height < 1L) {
    abort_basemapper("`width` and `height` must be positive integers.")
  }
  check_pixel_ratio(pixel_ratio)
  m <- structure(
    list(
      styles = styles, width = width, height = height, pixel_ratio = as.numeric(pixel_ratio),
      center = c(0, 0), zoom = 0, fetch = fetch,
      options = c(
        list(layers = layers, labels = labels_arg(labels), features = features, auth_token = auth_token),
        fetch_core(fetch)
      )
    ),
    class = "basemap"
  )
  if (!is.null(bounds)) {
    if (!is.null(center) || !is.null(zoom)) {
      abort_basemapper("Give either `bounds` or `center`/`zoom`, not both.")
    }
    return(set_bounds(m, bounds, crs = crs))
  }
  set_view(m, center = center %||% c(0, 0), zoom = zoom %||% 0)
}

#' Change a basemap view
#'
#' `set_view()` sets the centre and/or zoom; `set_bounds()` fits bounds
#' (keeping the view's size). Python: `m.set_view()`, `m.set_bounds()`.
#'
#' @param m A [basemap()] object.
#' @param center `c(longitude, latitude)`, or `NULL` to keep it.
#' @param zoom MapLibre zoom, or `NULL` to keep it.
#' @inheritParams basemap
#' @return The modified `basemap`.
#' @export
set_view <- function(m, center = NULL, zoom = NULL) {
  check_basemap(m)
  if (!is.null(center)) {
    if (!is.numeric(center) || length(center) != 2L) {
      abort_basemapper("center must be c(longitude, latitude).")
    }
    m$center <- as.numeric(center)
  }
  if (!is.null(zoom)) {
    if (!is.numeric(zoom) || length(zoom) != 1L || zoom < 0 || zoom > 22) {
      abort_basemapper("zoom must be a single number between 0 and 22.")
    }
    m$zoom <- as.numeric(zoom)
  }
  m
}

#' @rdname set_view
#' @export
set_bounds <- function(m, bounds, crs = NULL) {
  check_basemap(m)
  b <- bounds_3857(bounds, crs)
  v <- call_core("wrap__fit_bounds_raw", as.numeric(b), m$width, m$height)
  m$center <- v[1:2]
  m$zoom   <- min(max(v[3], 0), 22)
  m
}

#' Read a basemap view
#'
#' * `get_bounds()`: the view's extent, `c(xmin, ymin, xmax, ymax)` in
#'   EPSG:3857 metres, or degrees with `lonlat = TRUE`.
#' * `get_center()`: `c(longitude, latitude)` of the centre.
#' * `get_zoom()`: the MapLibre zoom.
#' * `get_size()`: `c(width, height)` of rendered images in device pixels.
#'
#' Python: `m.get_bounds()`, `m.get_center()`, `m.get_zoom()`,
#' `m.get_size()`.
#'
#' @param m A [basemap()] object.
#' @param lonlat Return degrees (EPSG:4326) instead of EPSG:3857 metres.
#' @return A named numeric vector.
#' @export
get_bounds <- function(m, lonlat = FALSE) {
  check_basemap(m)
  b <- call_core("wrap__bbox_from_center_raw", m$center[1], m$center[2], m$zoom,
             m$width, m$height)
  names(b) <- c("xmin", "ymin", "xmax", "ymax")
  if (!lonlat) return(b)
  lo <- mercator_to_lonlat(b[["xmin"]], b[["ymin"]])
  hi <- mercator_to_lonlat(b[["xmax"]], b[["ymax"]])
  c(xmin = lo[[1]], ymin = lo[[2]], xmax = hi[[1]], ymax = hi[[2]])
}

#' @rdname get_bounds
#' @export
get_center <- function(m) {
  check_basemap(m)
  c(longitude = m$center[1], latitude = m$center[2])
}

#' @rdname get_bounds
#' @export
get_zoom <- function(m) {
  check_basemap(m)
  m$zoom
}

#' @rdname get_bounds
#' @export
get_size <- function(m) {
  check_basemap(m)
  c(width = device_px(m$width, m$pixel_ratio), height = device_px(m$height, m$pixel_ratio))
}

#' @export
print.basemap <- function(x, ...) {
  n <- length(x$styles)
  s <- get_size(x)
  cat(sprintf(
    "<basemap> %d style%s, %d x %d @ %gx (%d x %d px), center (%.5f, %.5f), zoom %.3f\n",
    n, if (n == 1L) "" else "s", x$width, x$height, x$pixel_ratio, s[["width"]], s[["height"]],
    x$center[1], x$center[2], x$zoom
  ))
  invisible(x)
}

#' Show a basemap view
#'
#' `plot()` renders the view and draws it on the current graphics device.
#' In R Markdown and Quarto documents, a view printed by a chunk (`m` on its
#' own line) renders and shows as an image at its CSS pixel size, sharp at
#' any `pixel_ratio`; `print(m)` describes the view instead.
#'
#' @param x A view from [basemap()].
#' @param ... Unused.
#' @return `x`, invisibly (`plot()`); a knitr image (`knit_print()`).
#' @examples
#' \dontrun{
#' m <- basemap("OpenFreeMap.Positron", 800, 450, center = c(-111.8887, 40.7556), zoom = 12.4)
#' plot(m)
#' }
#' @name plot.basemap
#' @export
plot.basemap <- function(x, ...) {
  img <- pixels_to_native(render_pixels(x))
  grid::grid.newpage()
  grid::grid.raster(img, interpolate = TRUE)
  invisible(x)
}

#' @rdname plot.basemap
#' @param options knitr chunk options.
#' @exportS3Method knitr::knit_print
knit_print.basemap <- function(x, options = list(), ...) {
  .BASEMAPPER_KNIT$n <- .BASEMAPPER_KNIT$n + 1L
  path <- knitr::fig_path("-basemap.png", options, number = .BASEMAPPER_KNIT$n)
  dir.create(dirname(path), recursive = TRUE, showWarnings = FALSE)
  render_png(x, path = path)
  knitr::include_graphics(path, dpi = 96 * x$pixel_ratio)
}

.BASEMAPPER_KNIT <- new.env(parent = emptyenv())
.BASEMAPPER_KNIT$n <- 0L

#' Render a basemap view
#'
#' * `render_png()` writes or returns a PNG.
#' * `render_svg()` writes or returns an SVG document (`width` x `height`
#'   user units; `pixel_ratio` does not apply): vector layers as paths and
#'   text, raster tiles (raster basemaps, hillshade) as embedded images.
#' * `render_array()` returns an integer array `[height, width, 4]` (RGBA,
#'   0--255) with the extent as attributes, like [render_basemap_raw()].
#' * `render_raster()` (R only) returns a [terra::rast()] (EPSG:3857, layers
#'   red, green, blue, alpha).
#'
#' Image sizes are `width * pixel_ratio` by `height * pixel_ratio` device
#' pixels. Python: `m.render_png()`, `m.render_svg()`, `m.render_array()`.
#'
#' @param m A [basemap()] object.
#' @param path Output file, or `NULL` to return the result.
#' @param text For `render_svg()`: `"editable"` writes labels as `<text>`
#'   elements (editable, selectable) at MapLibre's glyph positions, halos as
#'   strokes, with the style's fonts embedded, and falls back to `"bitmap"`
#'   for labels whose font file is not available; `"bitmap"` draws every
#'   label from the style's own glyph bitmaps, thresholded by SVG filters as
#'   MapLibre's shader does (MapLibre's look at any zoom, not editable);
#'   `"paths"` writes glyph outlines traced from those bitmaps (plain paths).
#' @param embed_fonts For `render_svg()`: `"subset"` (only the glyphs used),
#'   `"full"` (whole fonts, for retyping labels) or `"none"` (fonts by name
#'   only). Fonts come from `font_dirs`, the `BASEMAPPER_FONT_DIRS` folders, a
#'   local cache, then Google Fonts for open fonts; others are referenced by
#'   name.
#' @param font_dirs For `render_svg()`: folders of .ttf / .otf files to use
#'   first.
#' @return `render_png()`: the PNG as a raw vector (invisibly when written).
#'   `render_svg()`: the SVG as a string (invisibly when written).
#' @export
render_png <- function(m, path = NULL) {
  px  <- render_pixels(m)
  png <- call_core("wrap__encode_png_raw", px$pixels, px$width, px$height)
  if (is.null(path)) return(png)
  writeBin(png, path)
  invisible(png)
}

#' @rdname render_png
#' @export
render_svg <- function(m, path = NULL, text = c("editable", "bitmap", "paths"),
                       embed_fonts = c("subset", "full", "none"), font_dirs = NULL) {
  text <- match.arg(text)
  embed_fonts <- match.arg(embed_fonts)
  check_basemap(m)
  o <- m$options
  svg <- call_core(
    "wrap__render_svg_raw",
    as.numeric(get_bounds(m)), m$width, m$height, m$styles,
    NULL, o$tile_timeout_ms, o$max_tiles,
    if (is.null(o$layers)) NULL else as.character(o$layers),
    o$labels %||% "all",
    if (is.null(o$features)) NULL else as.character(o$features),
    if (is.null(o$auth_token)) NULL else as.character(o$auth_token),
    o$fail_on_tile_error, o$tile_concurrency,
    text, embed_fonts,
    if (is.null(font_dirs)) NULL else normalizePath(as.character(font_dirs), mustWork = FALSE)
  )
  if (is.null(path)) return(svg)
  writeLines(svg, path, useBytes = TRUE, sep = "")
  invisible(svg)
}

#' @rdname render_png
#' @export
render_array <- function(m) {
  px <- render_pixels(m)
  with_extent(pixels_to_array(px), get_bounds(m), 3857L)
}

#' @rdname render_png
#' @export
render_raster <- function(m) {
  rlang::check_installed("terra", "to return a SpatRaster.")
  px <- render_pixels(m)
  b  <- get_bounds(m)
  r  <- terra::rast(
    nrows = px$height, ncols = px$width, nlyrs = 4L,
    xmin = unname(b["xmin"]), xmax = unname(b["xmax"]),
    ymin = unname(b["ymin"]), ymax = unname(b["ymax"]),
    crs = "EPSG:3857"
  )
  # Pixels are row-major RGBA from the top-left: one column per band.
  v <- matrix(as.integer(px$pixels), ncol = 4L, byrow = TRUE)
  terra::values(r) <- v
  names(r) <- c("red", "green", "blue", "alpha")
  terra::RGB(r) <- 1:3
  r
}

# ── Internals ─────────────────────────────────────────────────────────────────

check_basemap <- function(m) {
  if (!inherits(m, "basemap")) abort_basemapper("`m` must be a basemap() object.")
}

check_pixel_ratio <- function(pixel_ratio) {
  if (!is.numeric(pixel_ratio) || length(pixel_ratio) != 1L || !(pixel_ratio > 0)) {
    abort_basemapper("pixel_ratio must be a single number > 0.")
  }
}

# Device pixels for a CSS size, as the core rounds them (halves away from
# zero; R's round() would round them to even).
device_px <- function(css, pixel_ratio) {
  max(1L, as.integer(floor(css * pixel_ratio + 0.5)))
}

# One style or a vector / list of styles as character strings.
style_strings <- function(style) {
  styles <- if (is.list(style)) style else as.list(style)
  styles <- vapply(styles, function(s) as.character(s)[1L], character(1))
  if (length(styles) == 0L || anyNA(styles)) abort_basemapper("At least one style is required.")
  unname(styles)
}

bounds_3857 <- function(bounds, crs = NULL) {
  norm <- normalize_bbox(bounds, crs)
  v <- norm$vals
  if (!is.na(norm$crs_obj$epsg) && norm$crs_obj$epsg == 3857L) {
    return(unname(v[c("xmin", "ymin", "xmax", "ymax")]))
  }
  unname(reproject_bbox_to_3857(v[["xmin"]], v[["ymin"]], v[["xmax"]], v[["ymax"]],
                                from_crs = norm$crs_obj))
}

render_pixels <- function(m) {
  check_basemap(m)
  render_styles_3857(get_bounds(m), m$width, m$height, m$styles, m$pixel_ratio,
                     tile_zoom = NULL, options = m$options)
}

# `labels` (TRUE, FALSE or "only") as the core's "all" / "none" / "only".
labels_arg <- function(labels) {
  if (is.character(labels) && length(labels) == 1L && labels %in% c("all", "none", "only")) return(labels)
  if (isTRUE(labels)) return("all")
  if (isFALSE(labels)) return("none")
  abort_basemapper('labels must be TRUE, FALSE or "only".')
}

# Render styles over an EPSG:3857 bbox; width/height in CSS px.
render_styles_3857 <- function(bbox_3857, width, height, styles, pixel_ratio,
                               tile_zoom = NULL, options = list()) {
  o <- utils::modifyList(
    list(layers = NULL, labels = "all", features = NULL, auth_token = NULL, tile_timeout_ms = 10000L,
         max_tiles = 256L, tile_concurrency = 16L, fail_on_tile_error = FALSE),
    options
  )
  raw <- call_core(
    "wrap__render_raw",
    as.numeric(bbox_3857), as.integer(width), as.integer(height), style_strings(styles),
    as.numeric(pixel_ratio),
    if (is.null(tile_zoom)) NULL else as.integer(tile_zoom),
    as.integer(o$tile_timeout_ms), as.integer(o$max_tiles),
    if (is.null(o$layers)) NULL else as.character(o$layers),
    labels_arg(o$labels),
    if (is.null(o$features)) NULL else as.character(o$features),
    if (is.null(o$auth_token)) NULL else as.character(o$auth_token),
    isTRUE(o$fail_on_tile_error), as.integer(o$tile_concurrency)
  )
  w <- attr(raw, "width")
  h <- attr(raw, "height")
  for (note in attr(raw, "warnings")) warning(note, call. = FALSE)
  attributes(raw) <- NULL
  list(pixels = raw, width = w, height = h)
}

# Straight-alpha RGBA bytes as a nativeRaster: R's own packed colour
# format (one integer per pixel, 0xAABBGGRR), which is what RGBA bytes read
# as little-endian integers are. Drawn without any conversion, at a quarter
# of the memory of an integer array and an eighth of a double one.
pixels_to_native <- function(px) {
  n <- px$width * px$height
  v <- readBin(px$pixels, what = "integer", n = n, size = 4L, endian = "little")
  # nativeRaster data are row-major, as the pixels are.
  structure(v, dim = c(px$height, px$width), class = "nativeRaster", channels = 4L)
}

pixels_to_array <- function(px) {
  a <- array(as.integer(px$pixels), dim = c(4L, px$width, px$height))
  aperm(a, c(3L, 2L, 1L))  # [height, width, channel]
}

# An RGBA array with its extent and CRS as attributes.
with_extent <- function(a, b, epsg) {
  attr(a, "xmin") <- unname(b[[1]])
  attr(a, "ymin") <- unname(b[[2]])
  attr(a, "xmax") <- unname(b[[3]])
  attr(a, "ymax") <- unname(b[[4]])
  attr(a, "crs_epsg") <- epsg
  a
}
