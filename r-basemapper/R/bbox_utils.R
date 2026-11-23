# Null-coalescing operator used across the package.
`%||%` <- function(x, y) if (!is.null(x)) x else y

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
#'   ymax)}) and \code{crs_obj} (an \code{sf::crs} object, or for EPSG:4326 and
#'   EPSG:3857 a list with its \code{epsg}, so those need no \code{sf}).
#' @keywords internal
normalize_bbox <- function(bbox, crs = NULL) {
  if (inherits(bbox, "bbox")) {
    crs_obj <- if (!is.null(crs)) sf::st_crs(crs) else sf::st_crs(bbox)
    if (is.na(crs_obj)) abort_basemapper("The bbox has no CRS; pass `crs` explicitly.")
    vals <- c(
      xmin = unname(bbox["xmin"]), ymin = unname(bbox["ymin"]),
      xmax = unname(bbox["xmax"]), ymax = unname(bbox["ymax"])
    )
  } else if (inherits(bbox, c("sfc", "sfg", "sf"))) {
    return(normalize_bbox(sf::st_bbox(bbox), crs))
  } else {
    bbox <- as.numeric(bbox)
    if (length(bbox) != 4L) {
      abort_basemapper("`bbox` must be a numeric vector of length 4: c(xmin, ymin, xmax, ymax).")
    }
    if (is.null(crs)) .maybe_message_projected(bbox[1L], bbox[2L], bbox[3L], bbox[4L])
    crs_obj <- crs_object(crs)
    vals <- c(xmin = bbox[1L], ymin = bbox[2L], xmax = bbox[3L], ymax = bbox[4L])
  }

  if (any(!is.finite(vals))) {
    abort_basemapper(paste0("bbox contains non-finite values: ", paste(vals, collapse = ", ")))
  }
  if (vals[["xmin"]] >= vals[["xmax"]]) {
    abort_basemapper(paste0(
      "bbox xmin (", vals[["xmin"]], ") must be less than xmax (", vals[["xmax"]], ")."
    ))
  }
  if (vals[["ymin"]] >= vals[["ymax"]]) {
    abort_basemapper(paste0(
      "bbox ymin (", vals[["ymin"]], ") must be less than ymax (", vals[["ymax"]], ")."
    ))
  }

  list(vals = vals, crs_obj = crs_obj)
}

#' Reproject a bounding box to EPSG:3857 (Web Mercator).
#'
#' Projects all four sides of the bbox (not just two corners) by converting to
#' a polygon first, ensuring the reprojected envelope is correct even at high
#' latitudes or for large geographic extents.
#'
#' @param xmin,ymin,xmax,ymax Bounding-box corners in \code{from_crs} units.
#' @param from_crs Source CRS — anything accepted by \code{sf::st_crs()},
#'   including integer EPSG codes, WKT strings, and \code{crs} objects.
#' @return Named numeric vector c(xmin, ymin, xmax, ymax) in EPSG:3857.
reproject_bbox_to_3857 <- function(xmin, ymin, xmax, ymax, from_crs) {
  epsg <- simple_epsg(from_crs)
  if (identical(epsg, 3857L)) {
    return(c(xmin = xmin, ymin = ymin, xmax = xmax, ymax = ymax))
  }
  if (identical(epsg, 4326L)) {
    # Web Mercator is monotonic in each axis: the box's corners bound it
    # (as PROJ transforms the bbox polygon's corners).
    lo <- lonlat_to_3857(xmin, ymin)
    hi <- lonlat_to_3857(xmax, ymax)
    return(c(xmin = lo[[1]], ymin = lo[[2]], xmax = hi[[1]], ymax = hi[[2]]))
  }
  rlang::check_installed("sf", "to reproject bounds from this CRS.")
  bbox_poly <- sf::st_as_sfc(
    sf::st_bbox(
      c(xmin = xmin, ymin = ymin, xmax = xmax, ymax = ymax),
      crs = from_crs
    )
  )
  b <- sf::st_bbox(sf::st_transform(bbox_poly, 3857L))
  c(
    xmin = unname(b["xmin"]), ymin = unname(b["ymin"]),
    xmax = unname(b["xmax"]), ymax = unname(b["ymax"])
  )
}

# EPSG:4326 and EPSG:3857 are handled without sf: these are the CRS values
# naming them. Returns 4326L, 3857L or NA (another CRS, which needs sf).
simple_epsg <- function(crs) {
  if (is.null(crs)) return(4326L)
  if (is.list(crs) && isTRUE(crs$basemapper_simple)) return(crs$epsg)
  if (is.numeric(crs) && length(crs) == 1L && !is.na(crs) && crs %in% c(4326, 3857)) {
    return(as.integer(crs))
  }
  if (is.character(crs) && length(crs) == 1L && !is.na(crs)) {
    u <- toupper(trimws(crs))
    if (u %in% c("EPSG:4326", "WGS84", "OGC:CRS84", "CRS84")) return(4326L)
    if (u %in% c("EPSG:3857", "EPSG:900913")) return(3857L)
  }
  if (inherits(crs, "crs")) {
    e <- crs$epsg
    if (!is.null(e) && !is.na(e) && e %in% c(4326, 3857)) return(as.integer(e))
  }
  NA_integer_
}

# A CRS as normalize_bbox() returns it: a light stand-in for 4326 / 3857
# (with `$epsg`, as an sf crs has), else sf::st_crs().
crs_object <- function(crs) {
  e <- simple_epsg(crs)
  if (!is.na(e)) return(list(epsg = e, basemapper_simple = TRUE))
  rlang::check_installed("sf", "to use bounds in this CRS.")
  sf::st_crs(crs)
}

# EPSG:4326 degrees to EPSG:3857 metres (latitude clamped to Web Mercator's
# extent), as the core computes them.
lonlat_to_3857 <- function(lon, lat) {
  r <- 6378137
  lat <- pmin(pmax(lat, -85.0511287798066), 85.0511287798066)
  c(lon * pi / 180 * r, log(tan(pi / 4 + lat * pi / 360)) * r)
}

# EPSG:3857 metres to EPSG:4326 degrees.
mercator_to_lonlat <- function(x, y) {
  r <- 6378137
  c(x / r * 180 / pi, (2 * atan(exp(y / r)) - pi / 2) * 180 / pi)
}
