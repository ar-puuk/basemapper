#' Render a basemap and return raw RGBA bytes.
#'
#' Low-level wrapper around the Rust FFI.  Reprojects \code{bbox} to
#' EPSG:3857 internally; all output attributes reflect the \emph{input} CRS.
#'
#' @param bbox Bounding box — one of:
#'   \itemize{
#'     \item Numeric vector of length 4: \code{c(xmin, ymin, xmax, ymax)} in
#'       the CRS given by \code{crs} (defaults to EPSG:4326 / WGS-84).
#'     \item An \code{\link[sf]{st_bbox}} result; CRS is read from the object.
#'     \item An \code{sfc}, \code{sfg}, or \code{sf} data frame; bounding box
#'       and CRS are extracted automatically.
#'   }
#' @param width Integer output pixel width.
#' @param height Integer output pixel height.
#' @param style_input Character: a MapLibre GL style URL or inline JSON string.
#' @param crs CRS of \code{bbox} — anything accepted by \code{sf::st_crs()},
#'   e.g.\ an integer EPSG code, WKT string, or \code{crs} object.  Ignored
#'   when \code{bbox} already carries a CRS (e.g.\ an \code{st_bbox} result).
#'   Defaults to EPSG:4326 (WGS-84) with a message when coordinates appear to
#'   be in a projected system.
#' @param zoom Integer zoom level (0--22), or \code{NULL} for automatic
#'   selection.
#' @param tile_timeout_ms Integer per-tile HTTP timeout in milliseconds.
#' @param max_tiles Integer maximum tile downloads per render.
#' @param layers Character vector of layer IDs to filter. Plain IDs keep only
#'   those layers; minus-prefixed IDs (e.g. \code{"-roads"}) exclude those
#'   layers.  Mixed positive/negative lists raise an error.  \code{NULL}
#'   renders all layers.
#' @param auth_token Character authentication token, or \code{NULL}. Sent as
#'   \code{Authorization: Bearer <token>} for raster sources; appended as
#'   \code{?access_token=<token>} for Mapbox vector sources.
#' @param fail_on_tile_error Logical. When \code{FALSE}, individual tile fetch
#'   failures are skipped (with a warning) rather than aborting the render.
#'   Defaults to \code{TRUE}.
#' @param tile_concurrency Integer maximum number of tiles fetched in parallel.
#'   Defaults to \code{16L}.
#'
#' @return A 3-D integer array \code{[height, width, 4]} (RGBA, row-major) with
#'   spatial-extent attributes \code{xmin}, \code{ymin}, \code{xmax},
#'   \code{ymax} in the \emph{input} CRS, and \code{crs_epsg} set to the input
#'   EPSG code (\code{NA_integer_} for non-EPSG CRS specifications).
#' @export
#'
#' @examples
#' \dontrun{
#' # WGS-84 bounding box (default CRS)
#' m <- render_basemap_raw(
#'   bbox        = c(-122.5, 37.7, -122.4, 37.8),
#'   width       = 400L,
#'   height      = 300L,
#'   style_input = "https://demotiles.maplibre.org/style.json"
#' )
#'
#' # sf::st_bbox() result — CRS inferred automatically
#' library(sf)
#' nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' m <- render_basemap_raw(
#'   bbox        = sf::st_bbox(nc),
#'   width       = 400L,
#'   height      = 300L,
#'   style_input = "https://demotiles.maplibre.org/style.json"
#' )
#'
#' # Explicit EPSG:3857 bbox
#' m <- render_basemap_raw(
#'   bbox        = c(-13700000, 4500000, -13600000, 4600000),
#'   crs         = 3857L,
#'   width       = 400L,
#'   height      = 300L,
#'   style_input = "https://demotiles.maplibre.org/style.json"
#' )
#'
#' # Authenticated tile source
#' m <- render_basemap_raw(
#'   bbox        = c(-122.5, 37.7, -122.4, 37.8),
#'   width       = 400L,
#'   height      = 300L,
#'   style_input = "https://api.mapbox.com/styles/v1/mapbox/streets-v12",
#'   auth_token  = Sys.getenv("MAPBOX_TOKEN")
#' )
#' }
render_basemap_raw <- function(
    bbox,
    width,
    height,
    style_input,
    crs               = NULL,
    zoom              = NULL,
    tile_timeout_ms   = 10000L,
    max_tiles         = 256L,
    layers            = NULL,
    auth_token        = NULL,
    fail_on_tile_error = TRUE,
    tile_concurrency  = 16L
) {
  norm       <- normalize_bbox(bbox, crs)
  input_vals <- norm$vals
  crs_obj    <- norm$crs_obj

  if (!is.na(crs_obj$epsg) && crs_obj$epsg == 3857L) {
    bbox_3857 <- input_vals
  } else {
    bbox_3857 <- reproject_bbox_to_3857(
      input_vals[["xmin"]], input_vals[["ymin"]],
      input_vals[["xmax"]], input_vals[["ymax"]],
      from_crs = crs_obj
    )
  }

  raw <- .Call(
    "wrap__render_basemap_raw",
    as.numeric(bbox_3857),
    as.integer(width),
    as.integer(height),
    as.character(style_input),
    if (is.null(zoom))       NULL else as.integer(zoom),
    as.integer(tile_timeout_ms),
    as.integer(max_tiles),
    if (is.null(layers))     NULL else as.character(layers),
    if (is.null(auth_token)) NULL else as.character(auth_token),
    isTRUE(fail_on_tile_error),
    as.integer(tile_concurrency),
    PACKAGE = "basemapper"
  )

  n_channels <- 4L
  m <- array(as.integer(raw), dim = c(n_channels, as.integer(width), as.integer(height)))
  m <- aperm(m, c(3L, 2L, 1L))  # reorder to [height, width, channels]

  attr(m, "xmin")     <- unname(input_vals[["xmin"]])
  attr(m, "ymin")     <- unname(input_vals[["ymin"]])
  attr(m, "xmax")     <- unname(input_vals[["xmax"]])
  attr(m, "ymax")     <- unname(input_vals[["ymax"]])
  attr(m, "crs_epsg") <- if (!is.na(crs_obj$epsg)) as.integer(crs_obj$epsg) else NA_integer_
  m
}
