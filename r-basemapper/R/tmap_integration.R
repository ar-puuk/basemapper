#' Basemaps in tmap
#'
#' `tm_basemap(m)` puts a [basemap()] under a tmap v4 map; `tm_basemap(style,
#' ...)` builds the basemap from [basemap()]'s arguments (tmap only accepts
#' its own elements, so this is the way in). As in ggplot2, the map gives the
#' extent, the CRS (the basemap is warped into it), the image size and the
#' sharpness (the device's dpi); what the map shows is set by the basemap's
#' `zoom` or `symbol_scale`, and a basemap's `dpi` is an error. Put it before
#' `tm_shape()`, so it draws behind the layers.
#'
#' @param x A [basemap()], or a style (anything [basemap()] takes).
#' @param ... With a style: [basemap()]'s arguments that apply in a plot
#'   (`zoom` or `symbol_scale`, `clip`, `fetch`).
#' @param alpha The basemap's opacity (0--1).
#' @param attribution The providers' credit line, as a `tmap::tm_credits()`
#'   element in the bottom-right corner: `TRUE` (their own), `FALSE`, or your
#'   text.
#' @return A tmap element.
#' @export
#' @examples
#' \dontrun{
#' library(tmap)
#' nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' tm_basemap("OpenFreeMap.Positron") + tm_shape(nc) + tm_polygons(fill = NA)
#' }
tm_basemap <- function(x, ..., alpha = 1, attribution = TRUE) {
  rlang::check_installed("tmap", "to add a basemap to a tmap map.")
  m <- if (inherits(x, "basemap")) {
    if (length(list(...))) abort_basemapper("`x` is already a basemap: change it with set_view().")
    x
  } else {
    basemap(x, ...)
  }
  plot_parts(m)  # a basemap that cannot go in a plot is an error now
  el <- tmap::tm_element_list(
    tmap::tm_element(
      args = list(basemap = m, alpha = alpha),
      mapping.fun = "tm_basemap_rust",
      zindex = 0,
      group = NA,
      group.control = "radio",
      # tm_aux_layer: drawn by the hooks below, in the map's own CRS.
      subclass = c("tm_basemap_rust", "tm_aux_layer")
    )
  )
  credit <- attribution_text(attribution, m$styles)
  if (!nzchar(credit)) return(el)
  el + tmap::tm_credits(credit, size = 0.5, color = "grey20", bg = TRUE, bg.color = "white",
                        bg.alpha = 0.7, position = tmap::tm_pos_in("right", "bottom"))
}

# Per-render cache: filled by tmapGridAuxPrepare, read by tmapGridAuxPlot.
.BASEMAPPER_TMAP <- new.env(parent = emptyenv())

#' @exportS3Method tmap::tmapGridAuxPrepare
tmapGridAuxPrepare.tm_basemap_rust <- function(a, bs, id, o) {
  rlang::check_installed("terra", "to draw a basemap in tmap.")
  crs <- crs_of(sf::st_crs(bs[[1]]))
  xs <- lapply(bs, function(b) {
    limits <- unname(as.numeric(b[c("xmin", "ymin", "xmax", "ymax")]))
    r <- tryCatch(render_panel(a$basemap, limits, crs, tmap_panel_inches(o, limits)), error = function(e) {
      rlang::warn(paste("basemapper: the basemap could not be drawn:", conditionMessage(e)), class = "basemapper_problem")
      NULL
    })
    if (is.null(r)) return(NULL)
    # The rendering is already in the map's CRS, over the panel's limits.
    ras <- terra::rast(nrows = r$height, ncols = r$width, nlyrs = 3L, xmin = limits[1], xmax = limits[3],
                       ymin = limits[2], ymax = limits[4], crs = if (inherits(crs, "basemapper_crs")) crs$name else crs$wkt)
    v <- matrix(as.integer(r$pixels), ncol = 4L, byrow = TRUE)
    rgb <- v[, 1:3]
    rgb[v[, 4] == 0L, ] <- NA_integer_  # outside the CRS's area: transparent
    terra::values(ras) <- rgb
    names(ras) <- c("red", "green", "blue")
    ras
  })

  ss <- lapply(xs, function(x) {
    if (is.null(x)) return(NULL)
    do.call(tmap::tmapShape, list(
      shp = x, is.main = FALSE, crs = sf::st_crs(bs[[1]]), bbox = NULL, unit = NULL, filter = NULL, layer = NULL,
      shp_name = "x", smeta = list(), o = o, tmf = NULL
    ))
  })

  srgb <- tmap::tm_scale_rgb(max_color_value = 255, value.na = "#FFFFFF")
  # srgb$FUN is a character string resolved in tmap's namespace, not ours.
  srgb_fn <- if (is.character(srgb$FUN)) get(srgb$FUN, envir = asNamespace("tmap"), inherits = FALSE) else srgb$FUN
  ds <- lapply(ss, function(s) {
    if (is.null(s)) return(NULL)
    d <- s$dt
    rgb_vals <- do.call(srgb_fn, list(
      x1 = d$red, x2 = d$green, x3 = d$blue, scale = srgb, legend = list(), o = o, aes = "col",
      layer = "raster", layer_args = tmap::opt_tm_rgb(interpolate = FALSE)$mapping.args, sortRev = NA,
      bypass_ord = TRUE
    ))
    d[["col"]] <- rgb_vals[[1]]
    d[["legnr"]] <- rgb_vals[[2]]
    d[["crtnr"]] <- rgb_vals[[3]]
    d[["col_alpha"]] <- a$alpha
    d
  })

  key <- as.character(id)
  if (!exists("shp", envir = .BASEMAPPER_TMAP, inherits = FALSE)) {
    .BASEMAPPER_TMAP$shp <- list()
    .BASEMAPPER_TMAP$dts <- list()
  }
  .BASEMAPPER_TMAP$shp[[key]] <- lapply(ss, function(s) if (is.null(s)) NULL else s$shpTM)
  .BASEMAPPER_TMAP$dts[[key]] <- ds
  "basemap"
}

#' @exportS3Method tmap::tmapGridAuxPlot
tmapGridAuxPlot.tm_basemap_rust <- function(a, bi, bbx, facet_row, facet_col, facet_page, id, pane, group, o) {
  key <- as.character(id)
  dt <- .BASEMAPPER_TMAP$dts[[key]][[bi]]
  shpTM <- .BASEMAPPER_TMAP$shp[[key]][[bi]]
  if (!is.null(dt)) {
    # Drawn pixel for pixel: the image is rendered at the panel's size, and
    # smoothing it would only blur it.
    a2 <- structure(list(interpolate = FALSE), class = "tm_data_raster")
    tmap::tmapGridDataPlot(a2, shpTM, dt, list(), bbx, facet_row, facet_col, facet_page, id, pane, group,
                           glid = 0L, o)
  }
}

#' @exportS3Method tmap::tmapLeafletAuxPrepare
tmapLeafletAuxPrepare.tm_basemap_rust <- function(a, bs, id, o) {
  warning("basemapper::tm_basemap() does not support tmap's view mode; use tmap_mode(\"plot\").", call. = FALSE)
  ""
}

#' @exportS3Method tmap::tmapLeafletAuxPlot
tmapLeafletAuxPlot.tm_basemap_rust <- function(a, bi, bbx, facet_row, facet_col, facet_page, id, pane, group, o) {
  invisible(NULL)
}

# Estimated size (inches) of one map panel: the device size minus tmap's
# outer margins, split over the facet grid, fitted to the limits' aspect.
tmap_panel_inches <- function(o, limits) {
  dev <- o$devsize %||% tryCatch(grDevices::dev.size("in"), error = function(e) c(7, 7))
  om <- o$outer.margins %||% rep(0.02, 4) # bottom, left, top, right
  if (length(om) != 4L || anyNA(om)) om <- rep(0.02, 4)
  w <- dev[1] * (1 - om[2] - om[4]) / (o$ncols %||% 1)
  h <- dev[2] * (1 - om[1] - om[3]) / (o$nrows %||% 1)
  asp <- (limits[3] - limits[1]) / (limits[4] - limits[2])
  if (is.finite(asp) && asp > 0) {
    if (w / h > asp) w <- h * asp else h <- w / asp
  }
  c(max(w, 0.1), max(h, 0.1))
}
