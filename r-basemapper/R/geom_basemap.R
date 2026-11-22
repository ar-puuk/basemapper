#' ggproto class for basemap rendering inside a ggplot2 panel.
#'
#' Tile fetching is deferred to actual draw time via a custom grob so that
#' the viewport pixel dimensions are available when `render_basemap_raw` is
#' called.  This also means `coord_sf()` has already established the panel's
#' spatial extent before any fetching begins.
#'
#' @keywords internal
GeomBasemap <- ggplot2::ggproto(
  "GeomBasemap",
  ggplot2::Geom,
  required_aes  = character(0),
  default_aes   = ggplot2::aes(),

  draw_panel = function(data, panel_params, coord,
                        style_url, zoom, tile_timeout, max_tiles, alpha, layers,
                        ratio = NULL) {
    # Extract panel extent and reproject to EPSG:3857.
    x_range <- panel_params$x.range %||% panel_params$x_range
    y_range <- panel_params$y.range %||% panel_params$y_range
    from_epsg <- detect_crs_from_coord(coord, panel_params)
    bbox_3857 <- reproject_bbox_to_3857(
      x_range[1], y_range[1], x_range[2], y_range[2], from_epsg
    )

    # Return a lazy gTree.  makeContent.BasemapGrob is called at actual draw
    # time (inside an active viewport), where npc→px conversion is valid.
    grid::gTree(
      bbox_3857    = bbox_3857,
      style_url    = style_url,
      zoom         = zoom,
      tile_timeout = tile_timeout,
      max_tiles    = max_tiles,
      alpha        = alpha,
      layers       = layers,
      ratio        = ratio,
      cl           = "BasemapGrob"
    )
  }
)

#' Output resolution of the current graphics device, in dpi.
#'
#' Raster devices report their resolution (e.g. `ggsave(dpi = 300)`); vector
#' devices (PDF, SVG, PostScript) have none, so their embedded basemap is
#' rendered at 300 dpi.
#' @keywords internal
device_dpi <- function() {
  dev <- tolower(names(grDevices::dev.cur()))
  if (any(startsWith(dev, c("pdf", "svg", "cairo_pdf", "cairo_ps", "postscript", "devsvg", "svglite")))) {
    return(300)
  }
  tryCatch({
    d <- grDevices::dev.size("px")[1] / grDevices::dev.size("in")[1]
    if (is.finite(d) && d > 0) d else 96
  }, error = function(e) 96)
}

#' makeContent method for BasemapGrob.
#'
#' Called by grid at draw time (inside an active viewport). Renders the
#' basemap for the panel's size at the device's resolution and returns a
#' rasterGrob child.
#'
#' @param x A `BasemapGrob` created by [geom_basemap()].
#' @return A gTree with the rendered basemap as its rasterGrob child.
#' @keywords internal
#' @exportS3Method grid::makeContent
makeContent.BasemapGrob <- function(x) {
  dpi   <- device_dpi()
  # CSS px are 1/96 in: by default render at the device resolution with map
  # symbols at their physical size (ratio = dpi / 96).
  ratio <- x$ratio %||% (dpi / 96)
  w_in  <- grid::convertWidth(grid::unit(1, "npc"), "in", valueOnly = TRUE)
  h_in  <- grid::convertHeight(grid::unit(1, "npc"), "in", valueOnly = TRUE)
  css_w <- max(1L, as.integer(round(w_in * dpi / ratio)))
  css_h <- max(1L, as.integer(round(h_in * dpi / ratio)))

  px <- render_styles_3857(
    x$bbox_3857, css_w, css_h, x$style_url, ratio,
    zoom    = x$zoom,
    options = list(layers = x$layers, tile_timeout_ms = x$tile_timeout,
                   max_tiles = x$max_tiles)
  )

  img <- grid::rasterGrob(
    grDevices::as.raster(pixels_to_array(px) / 255),
    x             = 0.5,
    y             = 0.5,
    width         = grid::unit(1, "npc"),
    height        = grid::unit(1, "npc"),
    default.units = "npc",
    interpolate   = FALSE,
    gp            = grid::gpar(alpha = x$alpha)
  )

  grid::setChildren(x, grid::gList(img))
}

#' Add a styled basemap layer to a ggplot2 spatial plot.
#'
#' Wraps a lazy `GeomBasemap` ggproto layer that defers tile fetching until
#' `coord_sf()` has established the panel's geographic extent.
#'
#' The basemap is rendered when the plot is drawn, for the panel's size at
#' the device's resolution: `ggsave(dpi = 300)` gets a basemap rendered
#' natively at 300 dpi (and PDF / SVG output at 300 dpi), not an upscaled
#' screen image.
#'
#' @param style_url Character: a MapLibre GL style URL or inline JSON string,
#'   or a vector / list of styles stacked bottom to top.
#' @param zoom Integer zoom level (0–22), or `NULL` for automatic selection.
#' @param tile_timeout Integer per-tile HTTP timeout in milliseconds.
#' @param max_tiles Integer maximum tile downloads per render.
#' @param alpha Numeric opacity of the basemap layer (0–1).
#' @param layers Character vector of layer IDs to filter. Plain IDs keep only
#'   those layers; minus-prefixed IDs exclude those layers. `NULL` renders all.
#' @param ratio Device pixels per CSS pixel (MapLibre's devicePixelRatio).
#'   Default: device dpi / 96, so lines and labels keep their physical size at
#'   any resolution. Larger values draw map symbols smaller.
#' @param ... Additional arguments passed to `ggplot2::layer()`.
#'
#' @return A ggplot2 layer object.
#' @export
#'
#' @examples
#' \dontrun{
#' library(ggplot2)
#' library(sf)
#' nc <- st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' ggplot(nc) +
#'   geom_basemap(style_url = "https://demotiles.maplibre.org/style.json") +
#'   geom_sf(fill = NA, color = "steelblue") +
#'   coord_sf()
#' }
geom_basemap <- function(
    style_url,
    zoom        = NULL,
    tile_timeout = 10000L,
    max_tiles   = 256L,
    alpha       = 1,
    layers      = NULL,
    ratio       = NULL,
    ...
) {
  ggplot2::layer(
    geom        = GeomBasemap,
    mapping     = NULL,
    data        = NULL,
    stat        = "identity",
    position    = "identity",
    show.legend = FALSE,
    inherit.aes = FALSE,
    params      = list(
      style_url    = style_url,
      zoom         = zoom,
      tile_timeout = tile_timeout,
      max_tiles    = max_tiles,
      alpha        = alpha,
      layers       = layers,
      ratio        = ratio,
      ...
    )
  )
}
