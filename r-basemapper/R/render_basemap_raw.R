#' Render a basemap for bounds in one call
#'
#' A one-shot render: fit `bounds` into a `width` x `height` CSS-pixel view
#' and return the pixels. [basemap()] with [render_array()] gives the same
#' result with a reusable view; plot integrations use this function. The
#' same function exists in Python (`basemapper.render_basemap_raw()`).
#'
#' @param bounds Bounding box: `c(xmin, ymin, xmax, ymax)` in `crs` (default
#'   EPSG:4326, with a message when the values look projected), an
#'   [sf::st_bbox()] result, or an `sf` / `sfc` / `sfg` object (CRS read from
#'   the object).
#' @param width,height View size in CSS pixels.
#' @inheritParams basemap
#' @param tile_zoom Tile detail override: the tile zoom on the 256 px grid
#'   (vector sources use one less, their tiles being 512 px), or `NULL` to
#'   pick tiles as MapLibre does for the view.
#'
#' @return An integer array `[height * pixel_ratio, width * pixel_ratio, 4]`
#'   (RGBA, 0--255) with the input extent as attributes `xmin`, `ymin`,
#'   `xmax`, `ymax` and `crs_epsg` (`NA` for a non-EPSG CRS).
#' @export
#'
#' @examples
#' \dontrun{
#' # WGS-84 bounds (default CRS)
#' a <- render_basemap_raw(c(-122.5, 37.7, -122.4, 37.8), 400, 300,
#'                         "OpenFreeMap.Positron")
#'
#' # sf object: CRS read from it; two styles stacked; 2x pixels
#' nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' a <- render_basemap_raw(nc, 400, 300,
#'                         c("Esri.WorldShadedRelief",
#'                           "OpenFreeMap.Positron"),
#'                         pixel_ratio = 2)
#' }
render_basemap_raw <- function(
    bounds,
    width,
    height,
    style,
    crs                = NULL,
    pixel_ratio        = 1,
    tile_zoom          = NULL,
    layers             = NULL,
    auth_token         = NULL,
    tile_timeout_ms    = 10000L,
    max_tiles          = 256L,
    tile_concurrency   = 16L,
    fail_on_tile_error = FALSE
) {
  check_pixel_ratio(pixel_ratio)
  norm       <- normalize_bbox(bounds, crs)
  input_vals <- norm$vals
  crs_obj    <- norm$crs_obj
  bbox_3857 <- if (!is.na(crs_obj$epsg) && crs_obj$epsg == 3857L) {
    input_vals
  } else {
    reproject_bbox_to_3857(input_vals[["xmin"]], input_vals[["ymin"]],
                           input_vals[["xmax"]], input_vals[["ymax"]], from_crs = crs_obj)
  }
  px <- render_styles_3857(
    unname(bbox_3857), width, height, style, pixel_ratio, tile_zoom = tile_zoom,
    options = list(layers = layers, auth_token = auth_token, tile_timeout_ms = tile_timeout_ms,
                   max_tiles = max_tiles, tile_concurrency = tile_concurrency,
                   fail_on_tile_error = fail_on_tile_error)
  )
  with_extent(
    pixels_to_array(px),
    unname(input_vals[c("xmin", "ymin", "xmax", "ymax")]),
    if (!is.na(crs_obj$epsg)) as.integer(crs_obj$epsg) else NA_integer_
  )
}
