#' Render a basemap and return raw RGBA bytes.
#'
#' Low-level wrapper around the Rust FFI. The returned raw vector must be
#' reshaped before use; see the example below.
#'
#' @param bbox_3857 Numeric vector of length 4: `c(xmin, ymin, xmax, ymax)`
#'   in EPSG:3857 metres.
#' @param width Integer output pixel width.
#' @param height Integer output pixel height.
#' @param style_input Character: a MapLibre GL style URL or inline JSON string.
#' @param zoom Integer zoom level (0–22), or `NULL` for automatic selection.
#' @param tile_timeout_ms Integer per-tile HTTP timeout in milliseconds.
#' @param max_tiles Integer maximum tile downloads per render.
#' @param layers Character vector of layer IDs to filter. Plain IDs keep only
#'   those layers; minus-prefixed IDs (e.g. `"-roads"`) exclude those layers.
#'   Mixed positive/negative lists raise an error. `NULL` renders all layers.
#'
#' @return A `raw` vector of length `width * height * 4` (RGBA, row-major).
#' @export
#'
#' @examples
#' \dontrun{
#' raw <- render_basemap_raw(
#'   bbox_3857   = c(-13700000, 4500000, -13600000, 4600000),
#'   width       = 400L,
#'   height      = 300L,
#'   style_input = "https://demotiles.maplibre.org/style.json"
#' )
#' m <- array(as.integer(raw), dim = c(4L, 400L, 300L))
#' m <- aperm(m, c(3, 2, 1))  # [height, width, channels]
#' }
render_basemap_raw <- function(
    bbox_3857,
    width,
    height,
    style_input,
    zoom            = NULL,
    tile_timeout_ms = 10000L,
    max_tiles       = 256L,
    layers          = NULL
) {
  raw <- .Call(
    basemapper_render_basemap_raw,
    as.numeric(bbox_3857),
    as.integer(width),
    as.integer(height),
    as.character(style_input),
    if (is.null(zoom)) NULL else as.integer(zoom),
    as.integer(tile_timeout_ms),
    as.integer(max_tiles),
    if (is.null(layers)) NULL else as.character(layers)
  )

  # Reshape to [height, width, channels] array.
  n_channels <- 4L
  m <- array(as.integer(raw), dim = c(n_channels, as.integer(width), as.integer(height)))
  m <- aperm(m, c(3L, 2L, 1L))  # reorder to [height, width, channels]

  attr(m, "xmin")     <- bbox_3857[[1]]
  attr(m, "ymin")     <- bbox_3857[[2]]
  attr(m, "xmax")     <- bbox_3857[[3]]
  attr(m, "ymax")     <- bbox_3857[[4]]
  attr(m, "crs_epsg") <- 3857L
  m
}
