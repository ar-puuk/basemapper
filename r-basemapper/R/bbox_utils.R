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

#' Emit a message when bbox coordinates fall outside WGS-84 bounds.
#' @keywords internal
.maybe_message_projected <- function(xmin, ymin, xmax, ymax) {
  if (xmin < -180 || xmax > 180 || ymin < -90 || ymax > 90) {
    message(
      "bbox coordinates appear to be outside WGS-84 bounds; ",
      "they may be in a projected CRS (e.g. EPSG:3857). ",
      "Pass crs= explicitly to suppress this message."
    )
  }
}

#' Normalise a bbox input to coordinates plus an sf CRS object.
#'
#' Dispatches on the type of \code{bbox}:
#' \itemize{
#'   \item \code{sf::st_bbox()} result (class \code{"bbox"}) — CRS read from the
#'     object.
#'   \item \code{sfc}, \code{sfg}, or \code{sf} data frame — \code{sf::st_bbox()}
#'     called first, then dispatch as above.
#'   \item Numeric vector of length 4 — CRS taken from \code{crs}; defaults to
#'     EPSG:4326 (WGS-84) with a message when coordinates appear projected.
#' }
#'
#' @param bbox Bounding box input (see above).
#' @param crs CRS override — anything accepted by \code{sf::st_crs()}, e.g. an
#'   integer EPSG code, WKT string, or \code{crs} object.  Overrides a CRS
#'   baked into a \code{"bbox"} object when supplied.
#' @return A list with \code{vals} (named numeric \code{c(xmin, ymin, xmax,
#'   ymax)}) and \code{crs_obj} (an \code{sf::crs} object).
#' @keywords internal
normalize_bbox <- function(bbox, crs = NULL) {
  if (inherits(bbox, "bbox")) {
    crs_obj <- if (!is.null(crs)) sf::st_crs(crs) else sf::st_crs(bbox)
    if (is.na(crs_obj)) stop("bbox has no CRS; pass crs= explicitly.")
    vals <- c(
      xmin = unname(bbox["xmin"]), ymin = unname(bbox["ymin"]),
      xmax = unname(bbox["xmax"]), ymax = unname(bbox["ymax"])
    )
  } else if (inherits(bbox, c("sfc", "sfg", "sf"))) {
    return(normalize_bbox(sf::st_bbox(bbox), crs))
  } else {
    bbox <- as.numeric(bbox)
    if (length(bbox) != 4L) {
      stop("`bbox` must be a numeric vector of length 4: c(xmin, ymin, xmax, ymax).")
    }
    if (is.null(crs)) {
      crs_obj <- sf::st_crs(4326L)
      .maybe_message_projected(bbox[1L], bbox[2L], bbox[3L], bbox[4L])
    } else {
      crs_obj <- sf::st_crs(crs)
    }
    vals <- c(xmin = bbox[1L], ymin = bbox[2L], xmax = bbox[3L], ymax = bbox[4L])
  }
  list(vals = vals, crs_obj = crs_obj)
}

#' Reproject a bounding box to EPSG:3857 (Web Mercator).
#'
#' @param xmin,ymin,xmax,ymax Bounding-box corners in \code{from_crs} units.
#' @param from_crs Source CRS — anything accepted by \code{sf::st_crs()},
#'   including integer EPSG codes, WKT strings, and \code{crs} objects.
#' @return Named numeric vector c(xmin, ymin, xmax, ymax) in EPSG:3857.
reproject_bbox_to_3857 <- function(xmin, ymin, xmax, ymax, from_crs) {
  pts <- sf::st_sfc(
    sf::st_point(c(xmin, ymin)),
    sf::st_point(c(xmax, ymax)),
    crs = from_crs
  )
  pts_3857 <- sf::st_transform(pts, 3857)
  coords <- sf::st_coordinates(pts_3857)
  # as.numeric() strips the inner matrix-column names ("X"/"Y") that R appends
  # when subsetting a named matrix, so downstream named access works correctly.
  c(xmin = as.numeric(coords[1, "X"]), ymin = as.numeric(coords[1, "Y"]),
    xmax = as.numeric(coords[2, "X"]), ymax = as.numeric(coords[2, "Y"]))
}
