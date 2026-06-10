#' Add a styled basemap layer to a tmap v4 pipeline.
#'
#' Derives the map extent from the tmap pipeline's active primary shape via
#' `tmap::bb()`, renders the basemap via the Rust core, wraps the pixel array
#' as a georeferenced `stars` object, and returns a composable tmap element
#' (`tmap::tm_shape() + tmap::tm_rgb()`).
#'
#' @section tmap v4 note:
#' H1: If `tm_rgb()` applies an unwanted colour palette (rare with RGBA stars
#' objects), fall back to `tm_raster()` with explicit band selection. Prefer
#' `tm_rgb()` first.
#'
#' @param style_input Character: a MapLibre GL style URL or inline JSON string.
#' @param bbox Required map extent: an sf/sfc/stars object, an `sf::st_bbox()`
#'   result, or a numeric `c(xmin, ymin, xmax, ymax)` in WGS-84. Pass the same
#'   object you gave to `tm_shape()`. Cannot be inferred automatically because
#'   tmap evaluates each element before the `+` pipeline is assembled.
#' @param zoom Integer zoom level (0–22), or `NULL` for automatic.
#' @param alpha Numeric opacity (0–1).
#' @param layers Character vector of layer IDs to filter. See `render_basemap_raw()`.
#' @param width_px,height_px Output dimensions in pixels. When `NULL`, 800×600
#'   is used as a sensible default for static maps.
#' @param ... Additional arguments (reserved for future use).
#'
#' @return A composable tmap element: `tmap::tm_shape(stars_obj) + tmap::tm_rgb()`.
#' @export
#' @examples
#' \dontrun{
#' library(tmap)
#' library(sf)
#' nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' style <- basemapper::raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
#' # In tmap v4 each tm_shape() call sets the active shape for subsequent layer
#' # functions. Put tm_basemap() FIRST so it renders as background, then
#' # tm_shape(nc) re-establishes nc as the active shape for tm_sf().
#' basemapper::tm_basemap(style, bbox = nc) +
#'   tm_shape(nc) +
#'   tm_sf(fill = NA, col = "steelblue")
#' }
tm_basemap <- function(
    style_input,
    bbox      = NULL,
    zoom      = NULL,
    alpha     = 1,
    layers    = NULL,
    width_px  = NULL,
    height_px = NULL,
    ...
) {
  if (!requireNamespace("tmap",  quietly = TRUE)) stop("Package 'tmap' is required.")
  if (!requireNamespace("terra", quietly = TRUE)) stop("Package 'terra' is required.")

  # Resolve bounding box and capture the input CRS for final warp.
  # NOTE: unlike ggplot2, tmap evaluates each element independently before
  # combining them with +.  tm_basemap() is therefore called before it can
  # see the tm_shape() shape, so the bbox must be supplied explicitly.
  if (is.null(bbox)) {
    stop(
      "tm_basemap() requires an explicit `bbox` argument when used inside a ",
      "tmap pipeline.\n",
      "Pass the same sf object you gave to tm_shape(), e.g.:\n",
      "  basemapper::tm_basemap(style, bbox = nc) + tm_shape(nc) + tm_sf()"
    )
  } else if (inherits(bbox, c("sf", "sfc", "stars", "bbox"))) {
    bb         <- sf::st_bbox(bbox)
    input_crs  <- sf::st_crs(bbox)
    bbox_3857_vec <- reproject_bbox_to_3857(
      bb["xmin"], bb["ymin"], bb["xmax"], bb["ymax"],
      input_crs$epsg %||% 4326L
    )
  } else {
    # Assume c(xmin, ymin, xmax, ymax) in WGS-84.
    input_crs     <- sf::st_crs(4326L)
    bbox_3857_vec <- reproject_bbox_to_3857(
      bbox[1], bbox[2], bbox[3], bbox[4], 4326L
    )
  }

  w <- as.integer(width_px  %||% 800L)
  h <- as.integer(height_px %||% 600L)

  m <- render_basemap_raw(
    bbox_3857       = bbox_3857_vec,
    width           = w,
    height          = h,
    style_input     = style_input,
    zoom            = zoom,
    layers          = layers
  )

  # Wrap pixel array as a terra SpatRaster.
  # terra is used instead of stars because tmap v4 reprojects SpatRaster
  # objects via GDAL natively, avoiding the curvilinear-output / mixed-
  # dimension failure that stars::st_transform causes for in-memory rasters.
  # terra values are row-major (top-to-bottom); m is [height, width, channels]
  # so t(m[,,band]) yields the correct row-major order.
  xmin_v <- unname(bbox_3857_vec["xmin"])
  xmax_v <- unname(bbox_3857_vec["xmax"])
  ymin_v <- unname(bbox_3857_vec["ymin"])
  ymax_v <- unname(bbox_3857_vec["ymax"])

  r <- terra::rast(
    nrows = h, ncols = w,
    xmin  = xmin_v, xmax = xmax_v,
    ymin  = ymin_v, ymax = ymax_v,
    nlyr  = 3L, crs = "EPSG:3857"
  )
  terra::values(r) <- cbind(
    as.vector(t(m[, , 1L])),
    as.vector(t(m[, , 2L])),
    as.vector(t(m[, , 3L]))
  )
  names(r) <- c("R", "G", "B")
  terra::RGB(r) <- c(1L, 2L, 3L)

  tmap::tm_shape(r) + tmap::tm_rgb(col_alpha = alpha)
}
