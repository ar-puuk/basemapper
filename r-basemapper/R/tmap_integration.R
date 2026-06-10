#' Add a styled basemap layer to a tmap v4 pipeline.
#'
#' Returns a deferred tmap element that renders tiles at draw time using the
#' bounding box resolved from the active `tm_shape()` — no explicit `bbox`
#' argument is needed. This mirrors how tmap's own `tm_basemap()` works.
#'
#' @param style_input Character: a MapLibre GL style URL or inline JSON string.
#' @param zoom Integer zoom level (0–22), or `NULL` for automatic.
#' @param alpha Numeric opacity (0–1).
#' @param layers Character vector of layer IDs to filter. See `render_basemap_raw()`.
#' @param width_px,height_px Output dimensions in pixels. Defaults to 800×600
#'   when `NULL`.
#' @param ... Reserved for future use.
#'
#' @return A composable tmap element (class `"tmap"`). Put it before `tm_shape()`
#'   so it renders behind subsequent layers.
#' @export
#' @examples
#' \dontrun{
#' library(tmap)
#' library(sf)
#' nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' style <- basemapper::raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
#' basemapper::tm_basemap(style) +
#'   tm_shape(nc) +
#'   tm_sf(fill = NA, col = "steelblue")
#' }
tm_basemap <- function(
    style_input,
    zoom      = NULL,
    alpha     = 1,
    layers    = NULL,
    width_px  = NULL,
    height_px = NULL,
    ...
) {
  if (!requireNamespace("tmap", quietly = TRUE)) stop("Package 'tmap' is required.")

  tmap::tm_element_list(
    tmap::tm_element(
      args = list(
        style_input = style_input,
        zoom        = zoom,
        alpha       = alpha,
        layers      = layers,
        width_px    = width_px,
        height_px   = height_px
      ),
      mapping.fun   = "tm_basemap_rust",
      zindex        = 0,
      group         = NA,
      group.control = "radio",
      # "tm_basemap" triggers tmap's basemap-detection logic, which forces the
      # display CRS to crs_basemap (EPSG:3857) — required to match our raster.
      # "tm_basemap_rust" is first so S3 dispatch reaches our own methods.
      subclass      = c("tm_basemap_rust", "tm_basemap", "tm_aux_layer")
    )
  )
}

# Per-render cache — populated by tmapGridAuxPrepare, consumed by tmapGridAuxPlot.
# Avoids accessing tmap:::.TMAP_GRID from outside that package.
.BASEMAPPER_TMAP <- new.env(parent = emptyenv())

#' @exportS3Method tmap::tmapGridAuxPrepare
tmapGridAuxPrepare.tm_basemap_rust <- function(a, bs, id, o) {
  if (!requireNamespace("terra",      quietly = TRUE)) stop("Package 'terra' is required.")
  if (!requireNamespace("data.table", quietly = TRUE)) stop("Package 'data.table' is required.")

  crs         <- sf::st_crs(bs[[1]])
  isproj      <- !sf::st_is_longlat(crs)
  crs_is_3857 <- isTRUE(crs == sf::st_crs("EPSG:3857"))

  w <- as.integer(a$width_px  %||% 800L)
  h <- as.integer(a$height_px %||% 600L)

  xs <- lapply(bs, function(b) {
    b_3857 <- sf::st_bbox(sf::st_transform(sf::st_as_sfc(b), crs = "EPSG:3857"))

    m <- tryCatch(
      render_basemap_raw(
        bbox        = c(b_3857["xmin"], b_3857["ymin"],
                        b_3857["xmax"], b_3857["ymax"]),
        crs         = 3857L,
        width       = w,
        height      = h,
        style_input = a$style_input,
        zoom        = a$zoom,
        layers      = a$layers
      ),
      error = function(e) {
        warning("basemapper: render_basemap_raw failed: ", conditionMessage(e))
        NULL
      }
    )
    if (is.null(m)) return(NULL)

    r <- terra::rast(
      nrows = h, ncols = w,
      xmin  = unname(b_3857["xmin"]), xmax = unname(b_3857["xmax"]),
      ymin  = unname(b_3857["ymin"]), ymax = unname(b_3857["ymax"]),
      nlyr  = 3L, crs = "EPSG:3857"
    )
    terra::values(r) <- cbind(
      as.vector(t(m[, , 1L])),
      as.vector(t(m[, , 2L])),
      as.vector(t(m[, , 3L]))
    )
    names(r) <- c("red", "green", "blue")

    # Mirror tmap's tiles logic: reproject to map CRS only when the map uses
    # a non-3857 projected CRS. Geographic CRS (e.g. WGS-84) is handled by
    # tmap's own rendering pipeline after tmapShape().
    if (isproj && !crs_is_3857) {
      b_ext <- terra::ext(unname(b[c("xmin", "xmax", "ymin", "ymax")]))
      asp   <- as.numeric((b["xmax"] - b["xmin"]) / (b["ymax"] - b["ymin"]))
      tot   <- terra::ncell(r) * 2L
      nc    <- as.integer(round(sqrt(tot * asp)))
      nr    <- as.integer(round(tot / nc))
      r_tgt <- terra::rast(b_ext, nrows = nr, ncols = nc, crs = crs$wkt)
      r     <- terra::project(r, r_tgt, method = "near")
    }
    r
  })

  ss <- lapply(xs, function(x) {
    if (is.null(x)) return(NULL)
    do.call(tmap::tmapShape, list(
      shp = x, is.main = FALSE, crs = crs, bbox = NULL,
      unit = NULL, filter = NULL, layer = NULL, shp_name = "x",
      smeta = list(), o = o, tmf = NULL
    ))
  })

  srgb <- tmap::tm_scale_rgb(max_color_value = 255, value.na = "#FFFFFF")
  # srgb$FUN is a character string resolved in tmap's namespace, not ours.
  srgb_fn <- if (is.character(srgb$FUN)) {
    get(srgb$FUN, envir = asNamespace("tmap"), inherits = FALSE)
  } else {
    srgb$FUN
  }
  ds <- lapply(ss, function(s) {
    if (is.null(s)) return(NULL)
    d <- s$dt
    rgb_vals <- do.call(srgb_fn, list(
      x1         = d$red,
      x2         = d$green,
      x3         = d$blue,
      scale      = srgb,
      legend     = list(),
      o          = o,
      aes        = "col",
      layer      = "raster",
      layer_args = tmap::opt_tm_rgb(interpolate = TRUE)$mapping.args,
      sortRev    = NA,
      bypass_ord = TRUE
    ))
    data.table::set(d, j = "col",       value = rgb_vals[[1]])
    data.table::set(d, j = "legnr",     value = rgb_vals[[2]])
    data.table::set(d, j = "crtnr",     value = rgb_vals[[3]])
    data.table::set(d, j = "col_alpha", value = a$alpha)
    d
  })

  shpTMs <- lapply(ss, function(s) if (is.null(s)) NULL else s$shpTM)

  if (!exists("bmaps_shpTHs", envir = .BASEMAPPER_TMAP, inherits = FALSE)) {
    .BASEMAPPER_TMAP$bmaps_shpTHs <- list()
    .BASEMAPPER_TMAP$bmaps_dts    <- list()
  }
  key <- as.character(id)
  .BASEMAPPER_TMAP$bmaps_shpTHs[[key]] <- shpTMs
  .BASEMAPPER_TMAP$bmaps_dts[[key]]    <- ds

  a$style_input
}

#' @exportS3Method tmap::tmapGridAuxPlot
tmapGridAuxPlot.tm_basemap_rust <- function(a, bi, bbx, facet_row, facet_col,
                                            facet_page, id, pane, group, o) {
  key   <- as.character(id)
  dt    <- .BASEMAPPER_TMAP$bmaps_dts[[key]][[bi]]
  shpTM <- .BASEMAPPER_TMAP$bmaps_shpTHs[[key]][[bi]]
  if (!is.null(dt)) {
    a2 <- structure(list(interpolate = TRUE), class = "tm_data_raster")
    tmap::tmapGridDataPlot(
      a2, shpTM, dt, list(), bbx,
      facet_row, facet_col, facet_page, id, pane, group,
      glid = 0L, o
    )
  }
}

#' @exportS3Method tmap::tmapLeafletAuxPrepare
tmapLeafletAuxPrepare.tm_basemap_rust <- function(a, bs, id, o) {
  warning(
    "basemapper::tm_basemap() does not support tmap interactive (view) mode. ",
    "Switch to plot mode with tmap_mode('plot').",
    call. = FALSE
  )
  ""
}

#' @exportS3Method tmap::tmapLeafletAuxPlot
tmapLeafletAuxPlot.tm_basemap_rust <- function(a, bi, bbx, facet_row, facet_col,
                                               facet_page, id, pane, group, o) {
  invisible(NULL)
}
