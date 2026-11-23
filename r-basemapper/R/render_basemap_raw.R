#' Render a basemap for bounds in one call
#'
#' The low-level one-shot render: fit `bounds` into a `width` x `height`
#' CSS-pixel view and return the pixels, with the extent in the input's own
#' CRS. For most uses, [basemap()] with [render_array()] or
#' [render_raster()] is simpler and gives a view to reuse. The same function
#' exists in Python (`basemapper.render_basemap_raw()`).
#'
#' @param bounds Bounding box: `c(xmin, ymin, xmax, ymax)` in `crs` (default
#'   EPSG:4326, with a message when the values look projected), an
#'   [sf::st_bbox()] result, or an `sf` / `sfc` / `sfg` object (CRS read from
#'   the object).
#' @param width,height View size in CSS pixels.
#' @param style A style, as [basemap()] takes it.
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
    ...,
    crs         = NULL,
    pixel_ratio = 1,
    tile_zoom   = NULL,
    layers      = NULL,
    labels      = TRUE,
    features    = NULL,
    auth_token  = NULL,
    fetch       = fetch_options()
) {
  fetch <- fetch_with_deprecated(fetch, list(...), "render_basemap_raw", after = "style")$fetch
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
    options = c(list(layers = layers, labels = labels_arg(labels), features = features,
                     auth_token = auth_token), fetch_core(fetch))
  )
  with_extent(
    pixels_to_array(px),
    unname(input_vals[c("xmin", "ymin", "xmax", "ymax")]),
    if (!is.na(crs_obj$epsg)) as.integer(crs_obj$epsg) else NA_integer_
  )
}
