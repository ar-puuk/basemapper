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
#' @param bbox An explicit bounding box (sf bbox, numeric vector
#'   `c(xmin, ymin, xmax, ymax)` in any CRS, or an sf/stars/sfc object).
#'   When `NULL`, derived from `tmap::bb()` of the current pipeline's primary
#'   shape.
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
#' tm_shape(nc) +
#'   tm_basemap("https://demotiles.maplibre.org/style.json") +
#'   tm_sf()
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
  if (!requireNamespace("stars", quietly = TRUE)) stop("Package 'stars' is required.")

  # Resolve bounding box.
  if (is.null(bbox)) {
    bbox_obj <- tryCatch(tmap::bb(), error = function(e) NULL)
    if (is.null(bbox_obj)) {
      stop(paste0(
        "tm_basemap: no bounding box could be inferred. ",
        "Supply an explicit `bbox` argument or call tm_shape() first."
      ))
    }
    # tmap::bb() returns a named vector c(xmin, ymin, xmax, ymax) in the
    # primary shape's CRS; reproject to EPSG:3857.
    bbox_wgs <- sf::st_bbox(bbox_obj, crs = sf::st_crs(bbox_obj))
    bbox_3857_vec <- reproject_bbox_to_3857(
      bbox_wgs["xmin"], bbox_wgs["ymin"],
      bbox_wgs["xmax"], bbox_wgs["ymax"],
      sf::st_crs(bbox_obj)$epsg %||% 4326L
    )
  } else if (inherits(bbox, c("sf", "sfc", "stars", "bbox"))) {
    bb <- sf::st_bbox(bbox)
    bbox_3857_vec <- reproject_bbox_to_3857(
      bb["xmin"], bb["ymin"], bb["xmax"], bb["ymax"],
      sf::st_crs(bbox)$epsg %||% 4326L
    )
  } else {
    # Assume c(xmin, ymin, xmax, ymax) in WGS-84.
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

  # Wrap the RGBA array as a georeferenced stars object.
  # stars expects dimensions: [band, x, y] or [x, y, band].
  rgba <- aperm(m, c(3L, 2L, 1L))  # [channels, width, height]

  bbox_sf <- sf::st_bbox(
    c(xmin = bbox_3857_vec["xmin"], ymin = bbox_3857_vec["ymin"],
      xmax = bbox_3857_vec["xmax"], ymax = bbox_3857_vec["ymax"]),
    crs = sf::st_crs(3857)
  )

  stars_obj <- stars::st_as_stars(rgba)
  stars_obj <- sf::st_set_crs(stars_obj, 3857)
  attr(stars_obj, "bbox") <- bbox_sf

  # H1: use tm_rgb() for multi-band RGBA data; fall back to tm_raster() if
  # colours render incorrectly (see plan.md Implementation Hazards).
  tmap::tm_shape(stars_obj) + tmap::tm_rgb(alpha = alpha)
}
