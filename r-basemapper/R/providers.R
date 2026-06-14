#' Generate a MapLibre GL Style JSON string for an XYZ raster tile source.
#'
#' @param url_template Character: tile URL with `{z}`, `{x}`, `{y}` placeholders.
#' @param tile_size Integer tile size in pixels (default 256L).
#' @return A JSON character string ready to pass to `render_basemap_raw()`.
#' @export
#' @examples
#' style <- raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
raster_provider <- function(url_template, tile_size = 256L) {
  style <- list(
    version = 8L,
    sources = list(
      `raster-source` = list(
        type     = "raster",
        tiles    = list(url_template),
        tileSize = as.integer(tile_size)
      )
    ),
    layers = list(
      list(id = "raster-layer", type = "raster", source = "raster-source")
    )
  )
  jsonlite::toJSON(style, auto_unbox = TRUE)
}

#' Return the root.json style endpoint URL for an ArcGIS VectorTileServer.
#'
#' The Rust core fetches this URL to obtain the full, ESRI-published MapLibre
#' GL style JSON. Returns a plain URL string, not inline JSON.
#'
#' @param base_url Character: base ArcGIS VectorTileServer URL.
#' @return A URL character string.
#' @export
#' @examples
#' url <- esri_vector_provider("https://basemaps.arcgis.com/arcgis/rest/services/World_Basemap_v2/VectorTileServer")
esri_vector_provider <- function(base_url) {
  paste0(sub("/*$", "", base_url), "/resources/styles/root.json")
}

#' Generate a MapLibre GL Style JSON string for an ArcGIS MapServer raster source.
#'
#' @param url_template Character: ArcGIS tile URL (e.g., `.../MapServer/tile/{z}/{y}/{x}`).
#' @param tile_size Integer tile size in pixels (default 256L).
#' @return A JSON character string ready to pass to `render_basemap_raw()`.
#' @export
#' @examples
#' style <- esri_raster_provider("https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}")
esri_raster_provider <- function(url_template, tile_size = 256L) {
  style <- list(
    version = 8L,
    sources = list(
      `esri-raster-source` = list(
        type     = "raster",
        tiles    = list(url_template),
        tileSize = as.integer(tile_size)
      )
    ),
    layers = list(
      list(id = "esri-raster-layer", type = "raster", source = "esri-raster-source")
    )
  )
  jsonlite::toJSON(style, auto_unbox = TRUE)
}

#' Generate a MapLibre GL Style JSON string for an MVT vector tile source.
#'
#' Recognised paint keys: `fill-color`, `fill-opacity`, `fill-outline-color`,
#' `line-color`, `line-width`, `line-opacity`. Unrecognised keys are dropped
#' with a `warning()`. An empty paint list applies a light-grey default style.
#'
#' @param url_template Character: MVT tile URL with `{z}`, `{x}`, `{y}` placeholders.
#' @param source_layer Character: the name of the layer within each MVT tile to
#'   render. This is tile-server specific (e.g. `"water"`, `"roads"`); it maps
#'   to the MapLibre GL `"source-layer"` property and is required for vector
#'   sources.
#' @param paint Named list of MapLibre GL paint properties.
#' @return A JSON character string ready to pass to `render_basemap_raw()`.
#' @export
#' @examples
#' style <- vector_provider(
#'   "https://example.com/tiles/{z}/{x}/{y}.mvt",
#'   source_layer = "land",
#'   paint = list("fill-color" = "#e8e0d8", "line-color" = "#aaa")
#' )
vector_provider <- function(url_template, source_layer, paint = list()) {
  fill_keys <- c("fill-color", "fill-opacity", "fill-outline-color")
  line_keys <- c("line-color", "line-width", "line-opacity")
  known_keys <- c(fill_keys, line_keys)

  unknown <- setdiff(names(paint), known_keys)
  if (length(unknown) > 0) {
    warning(paste0("vector_provider: unrecognised paint key(s) dropped: ",
                   paste(unknown, collapse = ", ")))
    paint[unknown] <- NULL
  }

  fill_paint <- paint[intersect(names(paint), fill_keys)]
  line_paint <- paint[intersect(names(paint), line_keys)]

  if (is.null(fill_paint[["fill-color"]]))   fill_paint[["fill-color"]]   <- "#e8e0d8"
  if (is.null(fill_paint[["fill-opacity"]])) fill_paint[["fill-opacity"]] <- 1
  if (is.null(line_paint[["line-color"]]))   line_paint[["line-color"]]   <- "#aaaaaa"
  if (is.null(line_paint[["line-width"]]))   line_paint[["line-width"]]   <- 1

  style <- list(
    version = 8L,
    sources = list(
      `vector-source` = list(
        type  = "vector",
        tiles = list(url_template)
      )
    ),
    layers = list(
      list(id = "vector-fill", type = "fill",
           source = "vector-source", `source-layer` = source_layer,
           paint = fill_paint),
      list(id = "vector-line", type = "line",
           source = "vector-source", `source-layer` = source_layer,
           paint = line_paint)
    )
  )
  jsonlite::toJSON(style, auto_unbox = TRUE)
}
