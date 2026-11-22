#' Styles for common tile services
#'
#' Each function returns a style for [basemap()] and the other renderers'
#' `style` argument: a style URL (named GL styles, ArcGIS vector tile
#' servers) or inline MapLibre GL style JSON. They are implemented once in
#' the Rust core, so R and Python (`basemapper.raster_provider()`, ...) return
#' the same style for the same call.
#'
#' * `basemap_provider()`: a named basemap; [list_providers()] lists them.
#' * `raster_provider()`: an XYZ raster tile URL (`{z}`, `{x}`, `{y}`).
#' * `vector_provider()`: an MVT tile URL drawn as a fill and a line layer
#'   of `source_layer`.
#' * `esri_raster_provider()` / `esri_vector_provider()`: ArcGIS MapServer
#'   raster tiles / the published style of a VectorTileServer.
#' * `wms_provider()` / `wmts_provider()`: OGC WMS GetMap / WMTS GetTile
#'   endpoints as 256 px tiles in EPSG:3857.
#' * `pmtiles_provider()`: a raster PMTiles archive served over HTTP.
#'
#' @param name Provider name, e.g. `"OpenFreeMap.liberty"` or
#'   `"Carto.Positron"`; see [list_providers()].
#' @param api_key API key, required by MapTiler; optional for Stadia (lifts
#'   rate limits); ignored by the others.
#' @param url_template Tile URL with `{z}`, `{x}`, `{y}` placeholders (ArcGIS:
#'   `.../MapServer/tile/{z}/{y}/{x}`).
#' @param tile_size Tile size in pixels.
#' @param source_layer Layer within each vector tile to draw (MapLibre's
#'   `source-layer`), e.g. `"water"`.
#' @param paint Named list of paint properties: `fill-color`,
#'   `fill-opacity`, `fill-outline-color`, `line-color`, `line-width`,
#'   `line-opacity`. Others are dropped with a warning; unset ones default
#'   to a light grey style.
#' @param base_url ArcGIS VectorTileServer URL.
#' @param url Base service URL without query parameters (WMS / WMTS), or the
#'   `.pmtiles` archive URL.
#' @param layers WMS `LAYERS` parameter.
#' @param styles WMS `STYLES` parameter.
#' @param version Service version.
#' @param format Image MIME type.
#' @param transparent Request transparent WMS images.
#' @param extra_params Additional query parameters appended verbatim, or
#'   `NULL`.
#' @param layer WMTS `LAYER` parameter.
#' @param tilematrixset WMTS tile matrix set.
#' @param style WMTS `STYLE` parameter.
#' @return A style string (URL or JSON).
#' @name providers
#' @examples
#' basemap_provider("OpenFreeMap.liberty")
#' style <- raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
#' style <- vector_provider("https://example.com/tiles/{z}/{x}/{y}.mvt", "land",
#'                          paint = list("fill-color" = "#e8e0d8"))
#' style <- wms_provider("https://example.com/wms", layers = "relief")
NULL

#' @rdname providers
#' @export
basemap_provider <- function(name, api_key = NULL) {
  .Call("wrap__basemap_provider_raw", as.character(name)[1L],
        if (is.null(api_key)) NULL else as.character(api_key), PACKAGE = "basemapper")
}

#' Names of the built-in basemap providers
#'
#' The names [basemap_provider()] accepts. Python:
#' `basemapper.list_providers()`.
#'
#' @return A character vector.
#' @export
#' @examples
#' list_providers()
list_providers <- function() {
  .Call("wrap__list_providers_raw", PACKAGE = "basemapper")
}

#' @rdname providers
#' @export
raster_provider <- function(url_template, tile_size = 256L) {
  .Call("wrap__raster_provider_raw", as.character(url_template), as.integer(tile_size),
        PACKAGE = "basemapper")
}

#' @rdname providers
#' @export
vector_provider <- function(url_template, source_layer, paint = list()) {
  paint_json <- if (length(paint) == 0L) "{}" else as.character(jsonlite::toJSON(paint, auto_unbox = TRUE))
  s <- .Call("wrap__vector_provider_raw", as.character(url_template), as.character(source_layer),
             paint_json, PACKAGE = "basemapper")
  dropped <- attr(s, "dropped")
  if (length(dropped) > 0L) {
    warning("vector_provider: unrecognised paint key(s) dropped: ", paste(dropped, collapse = ", "),
            call. = FALSE)
  }
  attributes(s) <- NULL
  s
}

#' @rdname providers
#' @export
esri_raster_provider <- function(url_template, tile_size = 256L) {
  .Call("wrap__esri_raster_provider_raw", as.character(url_template), as.integer(tile_size),
        PACKAGE = "basemapper")
}

#' @rdname providers
#' @export
esri_vector_provider <- function(base_url) {
  .Call("wrap__esri_vector_provider_raw", as.character(base_url), PACKAGE = "basemapper")
}

#' @rdname providers
#' @export
wms_provider <- function(url, layers, styles = "", version = "1.3.0", format = "image/png",
                         transparent = FALSE, extra_params = NULL) {
  .Call("wrap__wms_provider_raw", as.character(url), as.character(layers), as.character(styles),
        as.character(version), as.character(format), isTRUE(transparent),
        if (is.null(extra_params)) NULL else as.character(extra_params), PACKAGE = "basemapper")
}

#' @rdname providers
#' @export
wmts_provider <- function(url, layer, tilematrixset = "GoogleMapsCompatible", format = "image/png",
                          style = "default", version = "1.0.0", extra_params = NULL) {
  .Call("wrap__wmts_provider_raw", as.character(url), as.character(layer), as.character(tilematrixset),
        as.character(format), as.character(style), as.character(version),
        if (is.null(extra_params)) NULL else as.character(extra_params), PACKAGE = "basemapper")
}

#' @rdname providers
#' @export
pmtiles_provider <- function(url) {
  .Call("wrap__pmtiles_provider_raw", as.character(url), PACKAGE = "basemapper")
}
