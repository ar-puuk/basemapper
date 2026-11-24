#' Create a basemap
#'
#' A basemap is what to draw (one style, or several stacked bottom to top),
#' what is given of its view, its CRS, a clip and fetch options. It renders on
#' its own ([render()]) or goes into a plot (`ggplot(nc) + m`,
#' `tm_basemap(m)`), which supplies what it leaves out. The same object exists
#' in Python (`basemapper.basemap()`) and Rust (`Basemap`).
#'
#' Each control has one meaning everywhere:
#'
#' * the extent (`bounds`, or `center` with `zoom`): where;
#' * `zoom`: the detail (which roads, places and labels the style draws);
#' * `symbol_scale`: the size of lines, labels and icons on the page;
#' * `dpi`: the sharpness (image pixels per inch), and nothing else.
#'
#' **Any three of extent, zoom, printed size and symbol scale give the
#' fourth**; the dpi turns the printed size into pixels. A `width` /
#' `height` in `"in"`, `"cm"` or `"mm"` is the printed size; in pixels it is a
#' printed size through the dpi (`px / dpi` inches), so with the pixels fixed,
#' a higher dpi prints the map smaller. What nothing gives is defaulted, in
#' this order: dpi (96), symbol scale (1), image size (800 x 600 px at 96 dpi,
#' scaled with the dpi), extent (the world); the zoom is never a default.
#' Over-determined requests are errors that say what to drop. In a plot, the
#' plot gives the extent and printed size and the device the dpi: give a
#' `zoom` or a `symbol_scale` (see [geom_basemap()]).
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
#' @param dpi Image pixels per inch: the sharpness (default 96). In a plot
#'   the device sets it, and a basemap's `dpi` is an error.
#' @param symbol_scale The size of lines, labels and icons on the page, as a
#'   multiple of the style's designed size. At 1 (the default) a CSS pixel is
#'   1/96 inch, so a label the style sizes at 12 px prints at 9 pt; 0.84
#'   prints it at 84% of that. It changes what is drawn, not the sharpness:
#'   with the extent and printed size fixed (always so in a plot), a smaller
#'   symbol scale leaves room for more detail (a higher zoom), a larger one
#'   for less. `zoom` and `symbol_scale` each set the other there, so give
#'   one. To keep the look of 0.3's `geom_basemap(dpi = d)` saved at
#'   `ggsave(dpi = D)`, use `symbol_scale = d / D`.
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
                    units = "px", dpi = NULL, symbol_scale = NULL, crs = NULL, clip = NULL, fetch = fetch_options()) {
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
      symbol_scale = num1(symbol_scale),
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
  if (!is.null(m$symbol_scale)) inputs$symbol_scale <- m$symbol_scale
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
#' @param center,zoom,width,height,units,dpi,symbol_scale As in [basemap()];
#'   left out, they keep their value.
#' @param bounds As in [basemap()].
#' @return A new `basemap`.
#' @export
#' @examples
#' \dontrun{
#' m <- basemap("OpenFreeMap.Liberty", center = c(-111.89, 40.76), zoom = 12)
#' set_view(m, zoom = 10)
#' set_view(m, dpi = 300, width = 6, height = 4, units = "in")
#' }
set_view <- function(m, ..., center, zoom, width, height, units, dpi, symbol_scale) {
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
  if (!missing(symbol_scale)) m$symbol_scale <- num1(symbol_scale)
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

#' The zoom for a map of a given printed width
#'
#' `zoom_for()` is the MapLibre zoom at which `bounds` drawn `width` wide
#' has lines and labels at `symbol_scale` times their designed size, by the
#' same rules as [basemap()] (in the map's CRS: `crs`, else the layer's when
#' projected, else Web Mercator). In a plot, give it as the `zoom`: the same
#' zoom draws the same picture at any panel size, so a page map and its
#' thumbnail match, while `symbol_scale` keeps the label size instead.
#' Python: `basemapper.zoom_for()`.
#'
#' @param bounds As in [basemap()]: `c(west, south, east, north)`,
#'   [around()], or a spatial layer.
#' @param width The printed width of the map, in `units`.
#' @param units `"in"` (default), `"cm"` or `"mm"`.
#' @param symbol_scale The size of lines and labels at that width (see
#'   [basemap()]).
#' @param crs The map's CRS, as in [basemap()].
#' @return The zoom, a number.
#' @export
#' @examples
#' \dontrun{
#' z <- zoom_for(tract, 3.6, symbol_scale = 0.84)
#' ggplot(tract) + geom_basemap("UGRC.Lite", zoom = z)      # the page map
#' # The same zoom in a 2.27 in thumbnail: the same picture, smaller
#' }
zoom_for <- function(bounds, width, units = "in", symbol_scale = 1, crs = NULL) {
  call <- rlang::current_env()
  units <- chr1(units)
  if (!units %in% c("in", "cm", "mm")) {
    abort_basemapper("`units` must be \"in\", \"cm\" or \"mm\": zoom_for() needs a printed width.", call = call)
  }
  if (!is.numeric(width) || length(width) != 1L || !is.finite(width) || width <= 0) {
    abort_basemapper("`width` must be a number > 0.", call = call)
  }
  extent <- extent_of(bounds, call = call)
  b <- box_in(extent, map_crs(crs, extent, call = call))
  span <- if (b[3] >= b[1]) b[3] - b[1] else b[3] - b[1] + 2 * 20037508.342789244
  # The height that keeps the map's shape, so the width decides the zoom.
  height <- width * (b[4] - b[2]) / span
  m <- basemap(.ZOOM_FOR_STYLE, bounds = bounds, crs = crs, width = width, height = height, units = units,
               symbol_scale = symbol_scale)
  m$.prepared$view$zoom
}

# A style that is never drawn: zoom_for() resolves only a view.
.ZOOM_FOR_STYLE <- '{"version":8,"sources":{},"layers":[{"id":"bg","type":"background"}]}'

#' @export
print.basemap <- function(x, ...) {
  v <- x$.prepared$view
  n <- length(x$styles)
  auto <- function(what) if (identical(v$provenance[[what]], "given")) "" else sprintf(" (%s)", v$provenance[[what]])
  c0 <- v$center
  cat(sprintf("<basemap> %d style%s, %s\n", n, if (n == 1L) "" else "s", crs_name(x$.prepared$crs)))
  cat(sprintf("  size   %d x %d px%s at %g dpi%s\n", v$width, v$height, auto("size"), v$dpi, auto("dpi")))
  cat(sprintf("  zoom   %.2f%s, 1:%s\n", v$zoom, auto("zoom"), format(round(v$scale), big.mark = ",")))
  if (!isTRUE(all.equal(v$symbol_scale, 1))) cat(sprintf("  symbols %.2fx their designed size\n", v$symbol_scale))
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
  r <- rendering_of(core_render(m$styles, p$inputs, m$fetch, p$clip, p$frame, mesh, call = call), p$crs)
  attr(r, "alt") <- basemap_alt(m)
  r
}

# A style in a few words: a catalogue name, a service's host or a file's
# name, and its features filter.
style_label <- function(s) {
  src <- if (is.character(s$source)) s$source else s$source$name %||% s$source$input %||% s$source$url
  src <- if (grepl("^\\s*\\{", src)) {
    name <- tryCatch(jsonlite::fromJSON(src, simplifyVector = FALSE)$name, error = function(e) NULL)
    if (is.character(name) && length(name) == 1L && nzchar(name)) name else "an inline style"
  } else if (grepl("^[a-z]+://", src)) {
    sub("^[a-z]+://([^/?#]+).*$", "\\1", src)
  } else {
    basename(src)
  }
  if (is.null(s$features)) src else sprintf("%s (features: %s)", src, paste(s$features, collapse = ", "))
}

# The default alt text of a basemap's figure in knitr (a chunk's `fig.alt`
# wins): what it shows, where, and at what detail.
basemap_alt <- function(m) {
  v <- m$.prepared$view
  c0 <- v$center
  crs <- crs_name(m$.prepared$crs)
  sprintf("Basemap of %s around %.2f° %s, %.2f° %s at zoom %.1f (1:%s)%s",
          paste(vapply(m$styles, style_label, character(1)), collapse = " under "),
          abs(c0[2]), if (c0[2] >= 0) "N" else "S", abs(c0[1]), if (c0[1] >= 0) "E" else "W",
          v$zoom, format(round(v$scale), big.mark = ","),
          if (identical(crs, "EPSG:3857")) "" else paste0(", in ", crs))
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
#' describes it instead. The image's alt text is the chunk's `fig.alt`, or
#' else a description of the map: its styles, where it is centred, its zoom
#' and scale, and its CRS when not Web Mercator.
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
