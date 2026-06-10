#' Detect the EPSG code from a ggplot2 coord/panel_params object.
#'
#' @param coord A ggplot2 coord object (typically from coord_sf()).
#' @param panel_params Panel parameters from draw_panel (may contain resolved CRS).
#' @return Integer EPSG code, or 4326L with a message if not detectable.
detect_crs_from_coord <- function(coord, panel_params = NULL) {
  crs <- tryCatch(
    {
      # panel_params$crs is set by CoordSf$setup_panel_params to the
      # CRS resolved from the sf layer data.  coord$crs is only non-NULL
      # when the user passes crs= explicitly to coord_sf().
      crs_obj <- (panel_params$crs %||% coord$crs)
      if (is.null(crs_obj)) stop("null crs")
      sf::st_crs(crs_obj)$epsg
    },
    error = function(e) {
      message("Could not detect CRS from coord; assuming EPSG:4326 (WGS-84).")
      4326L
    }
  )
  as.integer(crs)
}

#' Reproject a bounding box to EPSG:3857 (Web Mercator).
#'
#' @param xmin,ymin,xmax,ymax Bounding-box corners in `from_epsg` units.
#' @param from_epsg Source EPSG code (integer).
#' @return Named numeric vector c(xmin, ymin, xmax, ymax) in EPSG:3857.
reproject_bbox_to_3857 <- function(xmin, ymin, xmax, ymax, from_epsg) {
  pts <- sf::st_sfc(
    sf::st_point(c(xmin, ymin)),
    sf::st_point(c(xmax, ymax)),
    crs = from_epsg
  )
  pts_3857 <- sf::st_transform(pts, 3857)
  coords <- sf::st_coordinates(pts_3857)
  # as.numeric() strips the inner matrix-column names ("X"/"Y") that R appends
  # when subsetting a named matrix, so downstream named access works correctly.
  c(xmin = as.numeric(coords[1, "X"]), ymin = as.numeric(coords[1, "Y"]),
    xmax = as.numeric(coords[2, "X"]), ymax = as.numeric(coords[2, "Y"]))
}
