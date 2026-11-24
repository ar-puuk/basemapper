#' Create a basemap
#'
#' A basemap is what to draw (one style, or several stacked bottom to top),
#' what is given of its view, its CRS, a clip and fetch options. It renders on
#' its own ([render()]) or goes into a plot (`ggplot(nc) + m`,
#' `tm_basemap(m)`), which supplies what it leaves out. The same object exists
#' in Python (`basemapper.basemap()`) and Rust (`Basemap`).
#'
#' Three controls, each with one meaning everywhere: the extent (`bounds`, or
#' `center` with `zoom`), `zoom` (the feature detail) and `dpi` (the
#' sharpness); `width` / `height` and [around()] express them in other terms.
#' **Any three of extent, zoom, image size and dpi give the fourth.** What
#' nothing gives is defaulted, in this order: dpi (96), image size (800 x 600
#' at 96 dpi, scaled with the dpi), extent (the world); the zoom is never a
#' default. Over-determined requests are errors that say what to drop.
#'
#' @param style A style, or a list (or character vector) of styles stacked
#'   bottom to top: a catalogue name (`"OpenFreeMap.Liberty"`), a style URL
#'   or JSON, tile data, or a [style()].
#' @param ... Not used: the arguments after `style` must be named.
#' @param bounds The area to show: `c(west, south, east, north)` in longitude
#'   / latitude (west > east across the date line), [around()] (a ground
#'   distance around a point), or a spatial layer (`sf`, `sfc`, `bbox`; its
#'   CRS is read).
#' @param center `c(longitude, latitude)` of the camera; needs `zoom`.
#' @param zoom The MapLibre zoom (0--22, fractional): which roads, places and
#'   labels the style draws.
#' @param width,height The image size, in `units`.
#' @param units `"px"` (default), `"in"`, `"cm"` or `"mm"`; with a physical
#'   unit, `dpi` sets only the pixel count.
#' @param dpi Image pixels per inch: the sharpness (default 96).
#' @param crs The map's CRS (anything [sf::st_crs()] takes, or an object with
#'   a CRS). Default: the `bounds` layer's CRS when it is projected, else Web
#'   Mercator. In another CRS the map is drawn in Web Mercator and warped
#'   into it (sf needed).
#' @param clip Draw only inside a layer's polygons (an `sf` or `sfc` object),
#'   or outside them with [outside()]. An inside clip also gives the default
#'   extent (and CRS, when projected).
#' @param fetch How the data is downloaded: a [fetch_options()] object.
#'
#' @return A `basemap` object.
#' @seealso [render()], [set_view()], [get_bounds()].
#' @export
#' @examples
#' \dontrun{
#' m <- basemap("OpenFreeMap.Liberty", center = c(-111.8887, 40.7556), zoom = 12.4, dpi = 192)
#' img <- render(m)              # 1600 x 1200 px, sharp
#' write_png(img, "slc.png")
#' render_svg(m, "slc.svg")      # vector
#'
#' # This area at this detail: the size follows
#' nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' basemap("OpenFreeMap.Positron", bounds = nc, zoom = 7)
#'
#' # 20 x 15 km around a point
#' basemap("OpenFreeMap.Liberty", bounds = around(c(-111.9, 40.76), c(20, 15), units = "km"))
#' }
basemap <- function(style, ..., bounds = NULL, center = NULL, zoom = NULL, width = NULL, height = NULL,
                    units = "px", dpi = NULL, crs = NULL, clip = NULL, fetch = fetch_options()) {
  call <- rlang::current_env()
  check_basemap_dots(list(...), call = call)
  if (inherits(style, "basemap")) {
    abort_basemapper("`style` is already a basemap: change it with set_view() or set_bounds().", call = call)
  }
  m <- structure(
    list(
      styles = styles_arg(style, call = call),
      bounds = bounds,
      center = if (is.null(center)) NULL else as.numeric(center),
      zoom = num1(zoom),
      width = num1(width),
      height = num1(height),
      units = chr1(units),
      dpi = num1(dpi),
      crs = crs,
      clip = clip,
      fetch = check_fetch(fetch, call)
    ),
    class = "basemap"
  )
  prepare(m, call = call)
}

# Resolve the view now (an impossible basemap is an error here, not later)
# and keep what the core and the renders need.
prepare <- function(m, call = rlang::caller_env()) {
  extent <- if (!is.null(m$bounds)) {
    extent_of(m$bounds, call = call)
  } else if (!is.null(m$clip) && !inherits(m$clip, "basemapper_outside") && is.null(m$center)) {
    extent_of(m$clip, call = call)  # an inside clip frames the map
  }
  crs <- map_crs(m$crs, extent, call = call)
  inputs <- base_inputs(m, call = call)
  frame <- NULL
  if (is_web_mercator(crs)) {
    if (!is.null(extent)) inputs$bounds <- box_in(extent, crs)
  } else if (!is.null(extent)) {
    b <- box_in(extent, crs)
    inputs$bounds <- b
    frame <- crs_frame(crs, c((b[1] + b[3]) / 2, (b[2] + b[4]) / 2), call = call)
  } else if (!is.null(m$center)) {
    o <- project(m$center[1], m$center[2], .CRS_4326, crs)
    frame <- crs_frame(crs, c(o$x, o$y), call = call)
  } else {
    abort_basemapper(sprintf("A map in %s needs an extent: bounds (or a layer), or center with zoom.",
                             crs_name(crs)), call = call)
  }
  clip <- clip_spec(m$clip, crs, call = call)
  if (length(inputs) == 0L) inputs <- structure(list(), names = character())  # a JSON object, not []
  view <- from_json(call_core("wrap__resolve_raw", to_json(inputs), to_json(frame), call = call))
  m$.prepared <- list(crs = crs, inputs = inputs, frame = frame, clip = clip, view = view)
  m
}

base_inputs <- function(m, call = rlang::caller_env()) {
  inputs <- list()
  if (!is.null(m$center)) {
    if (length(m$center) != 2L || any(!is.finite(m$center))) {
      abort_basemapper("`center` must be c(longitude, latitude).", call = call)
    }
    inputs$center <- m$center
  }
  if (!is.null(m$zoom)) inputs$zoom <- m$zoom
  if (!is.null(m$width) || !is.null(m$height)) {
    if (is.null(m$width) || is.null(m$height)) abort_basemapper("Give both `width` and `height`, or neither.", call = call)
    inputs$width <- m$width
    inputs$height <- m$height
    inputs$units <- m$units
  }
  if (!is.null(m$dpi)) inputs$dpi <- m$dpi
  inputs
}

check_basemap_dots <- function(dots, call) {
  if (length(dots) == 0L) return(invisible())
  nms <- names(dots) %||% rep("", length(dots))
  if (any(!nzchar(nms))) {
    abort_basemapper(
      "Arguments after `style` in basemap() must be named, e.g. `width = 800, height = 600`.",
      call = call
    )
  }
  removed_arguments("basemap", nms, call = call)
}

check_basemap <- function(m, call = rlang::caller_env()) {
  if (!inherits(m, "basemap")) abort_basemapper("`m` must be a basemap() object.", call = call)
}

#' Change a basemap
#'
#' A basemap never changes: these return a new one (value semantics, as
#' everything in R). `set_view()` changes what is given of the view (`NULL`
#' unsets a value; a `center` replaces the bounds); `set_bounds()` shows an
#' area (the centre is unset). Python: `m.set_view()`, `m.set_bounds()`.
#'
#' @param m A [basemap()] object.
#' @param ... Not used: the arguments after `m` must be named.
#' @param center,zoom,width,height,units,dpi As in [basemap()]; left out,
#'   they keep their value.
#' @param bounds As in [basemap()].
#' @return A new `basemap`.
#' @export
#' @examples
#' \dontrun{
#' m <- basemap("OpenFreeMap.Liberty", center = c(-111.89, 40.76), zoom = 12)
#' set_view(m, zoom = 10)
#' set_view(m, dpi = 300, width = 6, height = 4, units = "in")
#' }
set_view <- function(m, ..., center, zoom, width, height, units, dpi) {
  call <- rlang::current_env()
  check_basemap(m, call)
  check_named_dots(list(...), "set_view", "m", call = call)
  if (!missing(center)) {
    m$center <- if (is.null(center)) NULL else as.numeric(center)
    if (!is.null(center)) m$bounds <- NULL
  }
  if (!missing(zoom)) m$zoom <- num1(zoom)
  if (!missing(width)) m$width <- num1(width)
  if (!missing(height)) m$height <- num1(height)
  if (!missing(units)) m$units <- chr1(units) %||% "px"
  if (!missing(dpi)) m$dpi <- num1(dpi)
  prepare(m, call = call)
}

#' @rdname set_view
#' @export
set_bounds <- function(m, bounds) {
  call <- rlang::current_env()
  check_basemap(m, call)
  m$bounds <- bounds
  m$center <- NULL
  prepare(m, call = call)
}

#' Read a basemap's view
#'
#' The view's values, each given, derived or defaulted by the core's one
#' rule. The view never reaches past the poles. Python: the properties
#' `m.bounds`, `m.center`, `m.zoom`, `m.size`.
#'
#' * `get_bounds()`: `c(xmin, ymin, xmax, ymax)` of the map in its CRS: at
#'   least the extent given, grown to the image's shape.
#' * `get_center()`: `c(longitude, latitude)` of the centre.
#' * `get_zoom()`: the MapLibre zoom.
#' * `get_size()`: `c(width, height)` of the image in pixels.
#'
#' `print(m)` shows these with the dpi, CRS, scale and where each value came
#' from.
#'
#' @param m A [basemap()] object.
#' @return A named numeric vector (one number for `get_zoom()`).
#' @export
get_bounds <- function(m) {
  check_basemap(m)
  v <- m$.prepared$view
  stats::setNames(v$crs_bounds %||% v$bounds, c("xmin", "ymin", "xmax", "ymax"))
}

#' @rdname get_bounds
#' @export
get_center <- function(m) {
  check_basemap(m)
  stats::setNames(m$.prepared$view$center, c("longitude", "latitude"))
}

#' @rdname get_bounds
#' @export
get_zoom <- function(m) {
  check_basemap(m)
  m$.prepared$view$zoom
}

#' @rdname get_bounds
#' @export
get_size <- function(m) {
  check_basemap(m)
  v <- m$.prepared$view
  c(width = v$width, height = v$height)
}

#' @export
print.basemap <- function(x, ...) {
  v <- x$.prepared$view
  n <- length(x$styles)
  auto <- function(what) if (identical(v$provenance[[what]], "given")) "" else sprintf(" (%s)", v$provenance[[what]])
  c0 <- v$center
  cat(sprintf("<basemap> %d style%s, %s\n", n, if (n == 1L) "" else "s", crs_name(x$.prepared$crs)))
  cat(sprintf("  size   %d x %d px%s at %g dpi%s\n", v$width, v$height, auto("size"), v$dpi, auto("dpi")))
  cat(sprintf("  zoom   %.2f%s, 1:%s\n", v$zoom, auto("zoom"), format(round(v$scale), big.mark = ",")))
  # A given centre is given; else it follows from the extent (bounds, a
  # clip) or, with none, from the default world.
  centre_from <- if (!is.null(x$center)) "" else if (identical(v$provenance$extent, "default")) " (default)" else " (derived)"
  cat(sprintf("  center (%.4f, %.4f)%s\n", c0[1], c0[2], centre_from))
  invisible(x)
}

#' Render a basemap
#'
#' `render()` renders the map: a `basemapper_rendering` with its pixels and
#' where they are, converted with [write_png()], [as_array()],
#' [as_raster()] or `plot()`. `render(m, dpi = 300)` is
#' `render(set_view(m, dpi = 300))`: the same map, sharper (when the image
#' size is not fixed in pixels). `render_svg()` draws an SVG document; SVG is
#' Web Mercator for now, so a map in another CRS is an error. Python:
#' `m.render()`, `m.render_svg()`.
#'
#' A rendering carries notes on what basemapper decided, adjusted or could
#' not do, shown by their level (see [basemapper-errors]).
#'
#' @param m A [basemap()] object.
#' @param ... Not used: the arguments after `m` must be named.
#' @param dpi Render at this dpi instead of the basemap's.
#' @return `render()`: a `basemapper_rendering`. `render_svg()`: the SVG as a
#'   string (invisibly when written to `path`).
#' @export
#' @examples
#' \dontrun{
#' m <- basemap("OpenFreeMap.Liberty", center = c(-111.89, 40.76), zoom = 12)
#' img <- render(m)
#' img
#' write_png(img, "map.png")
#' a <- as_array(img)                  # [height, width, 4], with bounds and CRS
#' r <- as_raster(img)                 # terra SpatRaster
#' write_png(render(m, dpi = 300), "map-300.png")
#' }
render <- function(m, ..., dpi = NULL) {
  call <- rlang::current_env()
  check_basemap(m, call)
  check_named_dots(list(...), "render", "m", call = call)
  if (!is.null(dpi)) m <- set_view(m, dpi = dpi)
  p <- m$.prepared
  mesh <- if (is.null(p$frame)) NULL else crs_mesh(p$crs, p$view$crs_bounds, p$view$width, p$view$height)
  rendering_of(core_render(m$styles, p$inputs, m$fetch, p$clip, p$frame, mesh, call = call), p$crs)
}

#' @rdname render
#' @param path A file to write, or `NULL` to return the SVG.
#' @param text `"editable"`: labels as `<text>` with the style's fonts
#'   embedded (labels without a font file fall back to `"bitmap"`);
#'   `"bitmap"`: labels from the style's glyph bitmaps (MapLibre's look, not
#'   editable); `"paths"`: glyph outlines as paths.
#' @param embed_fonts `"subset"` (only the glyphs used), `"full"` or `"none"`
#'   (fonts by name only).
#' @param font_dirs Folders of .ttf / .otf files to use first.
#' @export
render_svg <- function(m, path = NULL, ..., text = c("editable", "bitmap", "paths"),
                       embed_fonts = c("subset", "full", "none"), font_dirs = NULL) {
  call <- rlang::current_env()
  check_basemap(m, call)
  check_named_dots(list(...), "render_svg", "path", call = call)
  text <- match.arg(text)
  embed_fonts <- match.arg(embed_fonts)
  p <- m$.prepared
  args <- c(
    list("wrap__render_svg_raw", to_json(lapply(m$styles, style_spec)), to_json(p$inputs),
         to_json(fetch_spec(m$fetch))),
    clip_args(p$clip),
    list(to_json(p$frame), text, embed_fonts,
         if (is.null(font_dirs)) character() else normalizePath(as.character(font_dirs), mustWork = FALSE))
  )
  svg <- do.call(call_core, c(args, list(call = call)))
  if (is.null(path)) return(svg)
  writeLines(svg, path, useBytes = TRUE, sep = "")
  invisible(svg)
}

#' Show a basemap
#'
#' `plot()` renders the basemap and draws it on the current graphics device.
#' In R Markdown and Quarto documents, a basemap printed by a chunk renders
#' and shows as an image at its CSS size, sharp at any dpi; `print(m)`
#' describes it instead.
#'
#' @param x A [basemap()] object.
#' @param ... Unused.
#' @param options knitr chunk options.
#' @return `x`, invisibly (`plot()`); a knitr image (`knit_print()`).
#' @name plot.basemap
#' @export
plot.basemap <- function(x, ...) {
  plot(render(x))
  invisible(x)
}

#' @rdname plot.basemap
#' @exportS3Method knitr::knit_print
knit_print.basemap <- function(x, options = list(), ...) {
  knit_print.basemapper_rendering(render(x), options = options, ...)
}
