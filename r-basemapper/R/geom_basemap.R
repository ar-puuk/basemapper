# Basemaps in ggplot2 (docs/design/api-review.md §4.5, D14, D15, D16):
# `ggplot(nc) + m`, or the alias geom_basemap(). The basemap renders at draw
# time, for the panel's limits, pixels and device, through the core's plot
# plan (the same plan as tmap and Python's plots), warped into the plot's
# CRS so it lines up with the data (F1).

#' @keywords internal
GeomBasemap <- ggplot2::ggproto(
  "GeomBasemap",
  ggplot2::Geom,
  required_aes = character(0),
  default_aes = ggplot2::aes(),
  draw_panel = function(data, panel_params, coord, basemap, alpha, attribution) {
    # coord_sf(): x.range is the panel's extent in its CRS (x_range is
    # normalised); other coords have only one of them.
    x <- panel_params$x.range %||% panel_params$x_range
    y <- panel_params$y.range %||% panel_params$y_range
    grid::gTree(
      limits = c(min(x), min(y), max(x), max(y)),
      crs = ggplot_crs(coord, panel_params),
      basemap = basemap,
      alpha = alpha,
      attribution = attribution,
      cl = "BasemapGrob"
    )
  }
)

# The plot's CRS: coord_sf()'s, else longitude / latitude (said once).
ggplot_crs <- function(coord, panel_params) {
  crs <- panel_params$crs %||% coord$crs
  if (is.null(crs)) {
    rlang::inform(
      "The plot has no CRS (use coord_sf() for spatial data): the basemap assumes longitude / latitude.",
      .frequency = "once", .frequency_id = "basemapper_no_plot_crs"
    )
    return(.CRS_4326)
  }
  crs_of(crs)
}

# The device a plot is drawn on (D16), and its pixels per inch.
# - A vector export (PDF, SVG, PostScript) has no pixels: the image is the
#   panel's size at 300 dpi.
# - A screen: 96 x its pixel ratio (1: R's screen devices do not report it).
# - A raster export (png(), ragg, ggsave()), or any other device: its dpi.
plot_device <- function() {
  dev <- tolower(names(grDevices::dev.cur()))
  ppi <- tryCatch({
    d <- grDevices::dev.size("px")[1] / grDevices::dev.size("in")[1]
    if (is.finite(d) && d > 0) d else 96
  }, error = function(e) 96)
  vector <- c("pdf", "svg", "cairo_pdf", "cairo_ps", "postscript", "devsvg", "svglite", "xfig", "pictex")
  if (any(startsWith(dev, vector))) return(list(device = list(kind = "vector_export"), ppi = 300))
  screen <- c("windows", "quartz", "x11", "x11cairo", "rstudiogd", "rstudio", "unigd", "httpgd")
  if (dev %in% screen) return(list(device = list(kind = "screen", pixel_ratio = 1), ppi = ppi))
  list(device = list(kind = "raster_export", dpi = ppi), ppi = ppi)
}

# The style specs and view inputs a plot renders with (the plot supplies the
# extent, size and dpi; the core rejects the rest, D15).
plot_parts <- function(m, call = rlang::caller_env()) {
  if (!is.null(m$crs)) {
    abort_basemapper("A basemap in a plot takes its CRS from the plot: remove `crs`.", call = call)
  }
  inputs <- base_inputs(m, call = call)
  if (!is.null(m$bounds)) inputs$bounds <- c(0, 0, 1, 1)
  if (length(inputs) == 0L) inputs <- structure(list(), names = character())
  # The core's plot rules, on a stand-in panel: a conflict is an error now,
  # not when the plot is drawn.
  probe <- list(limits = c(0, 0, 1, 1), crs = "EPSG:3857", px = c(100, 100),
                device = list(kind = "raster_export", dpi = 96))
  call_core("wrap__plan_raw", to_json(inputs), to_json(probe), 0L, 0L, numeric(), numeric(), call = call)
  list(styles = lapply(m$styles, style_spec), inputs = inputs)
}

# Render basemap `m` for a panel of `limits` (in `crs`) `size_in` inches
# big, on the current device.
render_panel <- function(m, limits, crs, size_in) {
  parts <- plot_parts(m)
  dev <- plot_device()
  px <- pmax(1, round(size_in * dev$ppi))
  spec <- panel_spec(limits, crs, px, dev$device)
  raw <- core_render(m$styles, parts$inputs, m$fetch, clip_spec(m$clip, crs), NULL, spec$mesh, spec$panel)
  r <- rendering_of(raw, crs)
  .BASEMAPPER_LAST$rendering <- r
  r
}

# The last rendering a plot drew (for tests and debugging: a plot's grob has
# no handle to it).
.BASEMAPPER_LAST <- new.env(parent = emptyenv())

# A rendering with its alpha channel scaled by `alpha` (0-1).
fade <- function(r, alpha) {
  a <- seq.int(4L, length(r$pixels), by = 4L)
  r$pixels[a] <- as.raw(round(as.integer(r$pixels[a]) * max(0, min(1, alpha))))
  r
}

#' @exportS3Method grid::makeContent
makeContent.BasemapGrob <- function(x) {
  w_in <- grid::convertWidth(grid::unit(1, "npc"), "in", valueOnly = TRUE)
  h_in <- grid::convertHeight(grid::unit(1, "npc"), "in", valueOnly = TRUE)
  r <- render_panel(x$basemap, x$limits, x$crs, c(w_in, h_in))
  # The opacity goes into the image itself: graphics devices need not honour
  # an alpha on rasters (Windows' png() ignores it).
  if (!is.null(x$alpha) && x$alpha < 1) r <- fade(r, x$alpha)
  img <- grid::rasterGrob(
    as_native(r), x = 0.5, y = 0.5, width = grid::unit(1, "npc"), height = grid::unit(1, "npc"),
    default.units = "npc", interpolate = FALSE
  )
  credit <- attribution_grob(attribution_text(x$attribution, x$basemap$styles), w_in)
  grid::setChildren(x, if (is.null(credit)) grid::gList(img) else grid::gList(img, credit))
}

basemap_layer <- function(m, alpha = 1, attribution = TRUE) {
  plot_parts(m)  # a basemap that cannot go in a plot is an error now
  ggplot2::layer(
    geom = GeomBasemap, mapping = NULL, data = NULL, stat = "identity", position = "identity",
    show.legend = FALSE, inherit.aes = FALSE,
    params = list(basemap = m, alpha = alpha, attribution = attribution)
  )
}

#' The last basemap a plot drew
#'
#' A plot renders its basemap when it is drawn (printed, saved, knitted), for
#' the panel's size and the device's dpi. `last_rendering()` returns that
#' rendering, to read what it was drawn at: `$zoom`, `$symbol_scale`, `$dpi`,
#' `$width`, `$height`, `$scale` (the printed 1:N) and its notes. With several
#' panels or basemaps, it is the last one drawn. Python:
#' `basemapper.last_rendering()`.
#'
#' @return A `basemapper_rendering`, or `NULL` before any plot drew a
#'   basemap in this session.
#' @seealso [zoom_for()] to choose a zoom before drawing.
#' @export
#' @examples
#' \dontrun{
#' library(ggplot2)
#' nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' p <- ggplot(nc) + geom_basemap("OpenFreeMap.Positron", symbol_scale = 0.84) + geom_sf(fill = NA)
#' ggsave("nc.png", p, width = 6, height = 3, dpi = 200)
#' r <- last_rendering()
#' r$zoom
#' r$symbol_scale
#' }
last_rendering <- function() {
  .BASEMAPPER_LAST$rendering
}

#' Basemaps in ggplot2
#'
#' Two ways to put a basemap in a ggplot, drawing the same layer:
#'
#' * `ggplot(nc) + basemap(...)`: the usual way. [basemap()] describes the
#'   map (its style, `zoom` or `symbol_scale`, `clip`, `fetch`); added to a
#'   plot, it is drawn at full opacity with the providers' credit line in the
#'   corner.
#' * `ggplot(nc) + geom_basemap(...)`: when you need a setting of the layer
#'   itself, which a basemap does not have: `alpha` (fade it) or
#'   `attribution` (turn the credit line off, or write your own). It takes
#'   [basemap()]'s arguments, or a basemap you already made:
#'   `geom_basemap(m, alpha = 0.5)`.
#'
#' So `+ basemap("OpenFreeMap.Liberty", zoom = 8)` and
#' `+ geom_basemap("OpenFreeMap.Liberty", zoom = 8)` are the same plot; only
#' `alpha` and `attribution` need `geom_basemap()`. In {tmap},
#' [tm_basemap()] takes all of these.
#'
#' The plot gives the extent (the panel's limits), the CRS
#' (`coord_sf()`'s; the basemap is warped into it), the image size (the
#' panel) and the sharpness: on screen 96 dpi, in a raster export
#' (`ggsave()`, `png()`, ragg) the device's dpi, in PDF / SVG 300 dpi.
#'
#' What the map shows is set in one place, in [basemap()] (or
#' `geom_basemap()`): `zoom` (the detail; the size of lines and labels follows) or
#' `symbol_scale` (the size of lines and labels; the zoom follows). Give one.
#' The plot fixes the extent, so a larger symbol scale also means a lower
#' zoom (fewer roads and labels) and a smaller one more detail. With
#' neither, the symbol scale is 1 and the map is the same at any dpi. A
#' basemap's `dpi`, extent, CRS or size is an error in a plot. A clip is a
#' mask, in the plot's CRS. [last_rendering()] reads what a drawn plot used;
#' [zoom_for()] chooses a zoom in advance. Python (plotnine):
#' `ggplot(df) + m`, `geom_basemap()`.
#'
#' @param style A style, a list of styles, or a [basemap()].
#' @param ... [basemap()]'s arguments that apply in a plot: `zoom` or
#'   `symbol_scale`, `clip`, `fetch`.
#' @param alpha The basemap's opacity (0--1).
#' @param attribution The providers' credit line in the bottom-right corner
#'   of each panel: `TRUE` (their own, see [get_attribution()]), `FALSE`
#'   (none), or your text.
#' @return A ggplot2 layer.
#' @export
#' @examples
#' \dontrun{
#' library(ggplot2)
#' nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' # The usual way: a basemap added to the plot
#' ggplot(nc) + basemap("OpenFreeMap.Positron") + geom_sf(fill = NA)
#' ggplot(nc) + basemap("OpenFreeMap.Positron", zoom = 8) + geom_sf(fill = NA)
#' # Smaller lines and labels, more detail; ggsave() sets the sharpness
#' p <- ggplot(nc) + basemap("OpenFreeMap.Positron", symbol_scale = 0.8) + geom_sf(fill = NA)
#' ggsave("nc.png", p, width = 8, height = 3, dpi = 300)
#' # geom_basemap() for the layer's own settings: faded, credit in a caption
#' m <- basemap("OpenFreeMap.Positron")
#' ggplot(nc) +
#'   geom_basemap(m, alpha = 0.5, attribution = FALSE) +
#'   geom_sf(fill = NA) +
#'   labs(caption = get_attribution(m))
#' }
geom_basemap <- function(style, ..., alpha = 1, attribution = TRUE) {
  m <- if (inherits(style, "basemap")) {
    if (length(list(...))) abort_basemapper("`style` is already a basemap: change it with set_view().")
    style
  } else {
    basemap(style, ...)
  }
  basemap_layer(m, alpha = alpha, attribution = attribution)
}

#' @exportS3Method ggplot2::ggplot_add
ggplot_add.basemap <- function(object, plot, ...) {
  plot + basemap_layer(object)
}
