#' Detect the EPSG code from a ggplot2 coord object.
#'
#' @param coord A ggplot2 coord object (typically from coord_sf()).
#' @return Integer EPSG code, or 4326L with a message if not detectable.
detect_crs_from_coord <- function(coord) {
  crs <- tryCatch(
    {
      crs_obj <- coord$crs
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
  c(xmin = coords[1, 1], ymin = coords[1, 2],
    xmax = coords[2, 1], ymax = coords[2, 2])
}
