# Coordinates and CRSs, the R side (docs/design/api-review.md §4.2, D18,
# D19): what needs sf, and nothing else. The view's arithmetic is the core's.
#
# - Plain numbers are longitude / latitude; spatial objects (sf, sfc, bbox,
#   around()) bring their CRS.
# - The map's CRS is `crs` if given, else a spatial extent layer's when that
#   CRS is projected, else Web Mercator.
# - A map in another CRS: the core needs a frame (a point, its Web Mercator
#   position, the local scale) and a mesh of the image's pixels to Web
#   Mercator; both are computed here with sf. Web Mercator and lon/lat need
#   no sf.

#' An area around a point
#'
#' An area of a true ground distance around a point, for [basemap()]'s
#' `bounds`. It is computed on the sphere (half the span each way), so it
#' needs neither PROJ nor sf, and is accurate to well under 1% for spans up
#' to a few hundred kilometres. Python: `basemapper.around()`.
#'
#' @param center `c(longitude, latitude)`.
#' @param span `c(width, height)` on the ground, or one number for a square.
#' @param units The unit of `span`: `"m"` (default), `"km"`, `"mi"` or `"ft"`.
#' @return A `basemapper_around` object: an area in longitude / latitude.
#' @export
#' @examples
#' around(c(-111.9, 40.76), c(20, 15), units = "km")
around <- function(center, span, units = "m") {
  center <- as.numeric(center)
  span <- as.numeric(span)
  if (length(span) == 1L) span <- c(span, span)
  if (length(center) != 2L || length(span) != 2L) {
    abort_basemapper("`center` must be c(longitude, latitude) and `span` one or two distances.")
  }
  b <- call_core("wrap__around_raw", center, span, as.character(units)[1L], call = rlang::current_env())
  structure(list(center = center, span = span, units = units, bounds = b), class = "basemapper_around")
}

#' @export
print.basemapper_around <- function(x, ...) {
  cat(sprintf("<around> %g x %g %s around (%g, %g)\n", x$span[1], x$span[2], x$units, x$center[1], x$center[2]))
  invisible(x)
}

#' The area outside a layer
#'
#' The area outside a layer's polygons, for [basemap()]'s `clip`: the
#' basemap everywhere except inside them. `outside()` only marks the layer;
#' the polygon-only rule applies as for `clip = layer`. Unlike an inside
#' clip, it sets no default extent or CRS. Python: `basemapper.outside()`.
#'
#' @param layer Polygons: an `sf` or `sfc` object.
#' @return A `basemapper_outside` object.
#' @export
#' @examples
#' \dontrun{
#' basemap("OpenFreeMap.Liberty", bounds = county, clip = outside(lakes))
#' }
outside <- function(layer) {
  if (inherits(layer, "basemapper_outside")) return(layer)
  spatial_crs(layer, "outside()")
  structure(list(layer = layer), class = "basemapper_outside")
}

#' @export
print.basemapper_outside <- function(x, ...) {
  cat("<outside> the area outside a layer's polygons\n")
  invisible(x)
}

# ── CRS handles ──────────────────────────────────────────────────────────────

# A CRS: Web Mercator and WGS 84 lon/lat as light handles (no sf needed),
# anything else as an sf crs.
.CRS_3857 <- structure(list(epsg = 3857L, name = "EPSG:3857"), class = "basemapper_crs")
.CRS_4326 <- structure(list(epsg = 4326L, name = "EPSG:4326"), class = "basemapper_crs")

crs_of <- function(x, call = rlang::caller_env()) {
  if (is.null(x)) return(NULL)
  if (inherits(x, "basemapper_crs")) return(x)
  if (is.numeric(x) && length(x) == 1L && !is.na(x) && x %in% c(3857, 4326)) {
    return(if (x == 3857) .CRS_3857 else .CRS_4326)
  }
  if (is.character(x) && length(x) == 1L && !is.na(x)) {
    u <- toupper(trimws(x))
    if (u %in% c("EPSG:3857", "EPSG:900913")) return(.CRS_3857)
    if (u %in% c("EPSG:4326", "WGS84", "OGC:CRS84", "CRS84")) return(.CRS_4326)
  }
  rlang::check_installed("sf", "for a CRS other than Web Mercator or WGS 84 longitude / latitude.")
  crs <- tryCatch(sf::st_crs(x), error = function(e) NULL)
  if (is.null(crs) || is.na(crs)) abort_basemapper(sprintf("Not a CRS: %s.", format(x)[1L]), call = call)
  code <- crs$epsg
  if (!is.null(code) && !is.na(code) && code == 3857) return(.CRS_3857)
  if (isTRUE(crs == sf::st_crs(4326)) || identical(code, 4326L)) return(.CRS_4326)
  crs
}

is_web_mercator <- function(crs) inherits(crs, "basemapper_crs") && crs$epsg == 3857L
is_lonlat <- function(crs) inherits(crs, "basemapper_crs") && crs$epsg == 4326L

crs_name <- function(crs) {
  if (inherits(crs, "basemapper_crs")) return(crs$name)
  if (!is.null(crs$epsg) && !is.na(crs$epsg)) return(paste0("EPSG:", crs$epsg))
  crs$input %||% "custom CRS"
}

# The CRS as sf takes it (sf is needed by the caller anyway).
sf_crs <- function(crs) if (inherits(crs, "basemapper_crs")) sf::st_crs(crs$epsg) else crs

is_projected <- function(crs) {
  if (inherits(crs, "basemapper_crs")) return(crs$epsg == 3857L)
  !isTRUE(sf::st_is_longlat(crs))
}

# Points (x, y vectors) from one CRS to another.
project <- function(x, y, from, to) {
  if (identical(from$name, to$name) && inherits(from, "basemapper_crs") && inherits(to, "basemapper_crs")) {
    return(list(x = x, y = y))
  }
  if (is_lonlat(from) && is_web_mercator(to)) {
    r <- call_core("wrap__lonlat_to_3857_raw", as.numeric(x), as.numeric(y))
    n <- length(x)
    return(list(x = r[seq_len(n)], y = r[n + seq_len(n)]))
  }
  if (is_web_mercator(from) && is_lonlat(to)) {
    r <- call_core("wrap__mercator_to_lonlat_raw", as.numeric(x), as.numeric(y))
    n <- length(x)
    return(list(x = r[seq_len(n)], y = r[n + seq_len(n)]))
  }
  rlang::check_installed("sf", "to transform coordinates between these CRSs.")
  p <- suppressWarnings(sf::sf_project(sf_crs(from), sf_crs(to), cbind(x, y), keep = TRUE, warn = FALSE))
  list(x = p[, 1], y = p[, 2])
}

# ── Extents ──────────────────────────────────────────────────────────────────

spatial_crs <- function(x, what, call = rlang::caller_env()) {
  if (!inherits(x, c("sf", "sfc", "sfg", "bbox"))) {
    abort_basemapper(sprintf("%s: give a spatial layer (an sf or sfc object).", what), call = call)
  }
  crs <- sf::st_crs(x)
  if (is.na(crs)) abort_basemapper(sprintf("%s: the layer has no CRS; set one with sf::st_set_crs().", what), call = call)
  crs_of(crs, call = call)
}

# `bounds` as an extent: list(box, crs, from_layer). Numbers are lon/lat
# (west > east across the date line); spatial objects and around() bring
# their CRS.
extent_of <- function(x, call = rlang::caller_env()) {
  if (inherits(x, "basemapper_around")) return(list(box = x$bounds, crs = .CRS_4326, from_layer = FALSE))
  if (inherits(x, c("sf", "sfc", "sfg", "bbox"))) {
    crs <- spatial_crs(x, "bounds", call = call)
    b <- sf::st_bbox(x)
    return(list(box = unname(as.numeric(b[c("xmin", "ymin", "xmax", "ymax")])), crs = crs, from_layer = TRUE))
  }
  if (!is.numeric(x) || length(x) != 4L) {
    abort_basemapper(
      "`bounds` must be c(west, south, east, north) in longitude / latitude, around(...), or a spatial layer.",
      call = call
    )
  }
  b <- unname(as.numeric(x))
  if (any(!is.finite(b))) abort_basemapper("`bounds` must be finite.", call = call)
  if (!(-90 <= b[2] && b[2] < b[4] && b[4] <= 90)) {
    abort_basemapper(paste0(
      "`bounds` numbers are longitude / latitude: south < north, within -90 and 90. ",
      "For projected coordinates, give a spatial layer (it carries its CRS)."
    ), call = call)
  }
  list(box = b, crs = .CRS_4326, from_layer = FALSE)
}

# The map's CRS (D18, D19): `crs` if given; else the extent layer's when
# projected; else Web Mercator.
map_crs <- function(crs, extent, call = rlang::caller_env()) {
  if (!is.null(crs)) return(crs_of(crs, call = call))
  if (!is.null(extent) && isTRUE(extent$from_layer) && is_projected(extent$crs)) return(extent$crs)
  .CRS_3857
}

# The extent's box in `crs`: corner by corner for lon/lat into Web Mercator
# (keeping west > east across the date line), else the envelope of the
# densified edges.
box_in <- function(extent, crs) {
  b <- extent$box
  if (identical(crs_name(extent$crs), crs_name(crs)) && inherits(crs, "basemapper_crs")) return(b)
  if (is_lonlat(extent$crs) && is_web_mercator(crs)) {
    p <- project(b[c(1, 3)], b[c(2, 4)], extent$crs, crs)
    return(c(p$x[1], p$y[1], p$x[2], p$y[2]))
  }
  rlang::check_installed("sf", "to transform bounds between these CRSs.")
  poly <- sf::st_as_sfc(sf::st_bbox(c(xmin = b[1], ymin = b[2], xmax = b[3], ymax = b[4]), crs = sf_crs(extent$crs)))
  poly <- sf::st_segmentize(poly, dfMaxLength = max(b[3] - b[1], b[4] - b[2]) / 20)
  out <- sf::st_bbox(sf::st_transform(poly, sf_crs(crs)))
  unname(as.numeric(out[c("xmin", "ymin", "xmax", "ymax")]))
}

# ── A map in another CRS ─────────────────────────────────────────────────────

# The core's frame for `crs` at `origin` (in `crs`): its Web Mercator
# position and the Web Mercator metres per CRS unit there (the geometric
# mean of the x and y scales).
crs_frame <- function(crs, origin, call = rlang::caller_env()) {
  d <- if (is_projected(crs)) 100 else 0.01
  pts <- project(origin[1] + c(0, -d, d, 0, 0), origin[2] + c(0, 0, 0, -d, d), crs, .CRS_3857)
  sx <- sqrt((pts$x[3] - pts$x[2])^2 + (pts$y[3] - pts$y[2])^2) / (2 * d)
  sy <- sqrt((pts$x[5] - pts$x[4])^2 + (pts$y[5] - pts$y[4])^2) / (2 * d)
  scale <- sqrt(sx * sy)
  if (!all(is.finite(c(pts$x[1], pts$y[1], scale))) || scale <= 0) {
    abort_basemapper(sprintf("The map's centre has no position in Web Mercator in %s.", crs_name(crs)), call = call)
  }
  list(name = crs_name(crs), origin = as.numeric(origin), origin_3857 = c(pts$x[1], pts$y[1]), scale = scale)
}

# The mesh of a `width x height` image of `bounds` (in `crs`) to Web
# Mercator: a node every 32 px (at least 2, at most 129 a side), NA where a
# point has no Web Mercator position.
crs_mesh <- function(crs, bounds, width, height) {
  nx <- min(max(width %/% 32 + 2, 2), 129)
  ny <- min(max(height %/% 32 + 2, 2), 129)
  px <- seq(0, width, length.out = nx)
  py <- seq(0, height, length.out = ny)
  gx <- rep(bounds[1] + px / width * (bounds[3] - bounds[1]), times = ny)
  gy <- rep(bounds[4] - py / height * (bounds[4] - bounds[2]), each = nx)
  p <- project(gx, gy, crs, .CRS_3857)
  bad <- !(is.finite(p$x) & is.finite(p$y))
  p$x[bad] <- NA_real_
  p$y[bad] <- NA_real_
  list(nx = nx, ny = ny, x = p$x, y = p$y)
}

# ── Clips ────────────────────────────────────────────────────────────────────

# A polygon layer's polygons in `crs`, each a list of rings (n x 2
# matrices: the exterior, then the holes). Points and lines are an error:
# buffer them (the distance is yours).
clip_rings <- function(layer, crs, call = rlang::caller_env()) {
  if (inherits(layer, "basemapper_outside")) layer <- layer$layer
  src <- spatial_crs(layer, "clip", call = call)
  geom <- sf::st_geometry(layer)
  if (!identical(crs_name(src), crs_name(crs)) || !inherits(crs, "basemapper_crs")) {
    geom <- sf::st_transform(geom, sf_crs(crs))
  }
  geom <- geom[!sf::st_is_empty(geom)]
  types <- unique(as.character(sf::st_geometry_type(geom)))
  bad <- setdiff(types, c("POLYGON", "MULTIPOLYGON"))
  if (length(bad)) {
    abort_basemapper(sprintf(
      "`clip` needs polygons, not %s: buffer points or lines first (sf::st_buffer(x, distance), in a projected CRS).",
      paste(bad, collapse = ", ")
    ), call = call)
  }
  polys <- sf::st_cast(geom, "POLYGON")
  if (length(polys) == 0L) abort_basemapper("`clip`: the layer has no polygons.", call = call)
  lapply(polys, function(p) lapply(unclass(p), function(r) unname(r[, 1:2, drop = FALSE])))
}

clip_spec <- function(clip, crs, call = rlang::caller_env()) {
  if (is.null(clip)) return(NULL)
  keep <- if (inherits(clip, "basemapper_outside")) "outside" else "inside"
  list(polygons = clip_rings(clip, crs, call = call), keep = keep)
}

# ── Plot panels ──────────────────────────────────────────────────────────────

# The core's panel for a plot, and its mesh when the core cannot build it
# (a CRS other than Web Mercator and lon/lat).
panel_spec <- function(limits, crs, px, device) {
  mesh <- NULL
  name <- if (is_web_mercator(crs)) "EPSG:3857" else if (is_lonlat(crs)) "EPSG:4326" else {
    mesh <- crs_mesh(crs, limits, px[1], px[2])
    crs_name(crs)
  }
  list(panel = list(limits = as.numeric(limits), crs = name, px = as.numeric(px), device = device), mesh = mesh)
}
