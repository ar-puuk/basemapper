#' List a style's layers
#'
#' The layers of a style in drawing order, with their types and features:
#' the IDs are what the `layers` filter of [basemap()] and the renderers
#' accepts (plain IDs keep, minus-prefixed IDs drop), the features what
#' `features` accepts; `symbol` layers are the labels `labels` keeps or
#' drops. Python: `basemapper.list_layers()`.
#'
#' A layer's feature comes from its vector tile source layer, with tables
#' for the OpenMapTiles (OpenFreeMap, Stadia, MapTiler), Protomaps (CARTO)
#' and Shortbread (VersaTiles) schemas and keywords for others (Esri's
#' descriptive names, layer ids). The features are background, water,
#' landcover, parks, buildings, roads, rail, boundaries, places, poi,
#' terrain and other.
#'
#' @param style A MapLibre GL style URL or JSON string.
#' @param auth_token Optional access token for the style URL.
#' @return A data frame with columns `id`, `type` and `feature`.
#' @export
#' @examples
#' \dontrun{
#' list_layers("OpenFreeMap.Positron")
#' }
list_layers <- function(style, auth_token = NULL) {
  ids <- .Call("wrap__list_layers_raw", as.character(style)[1L],
               if (is.null(auth_token)) NULL else as.character(auth_token), PACKAGE = "basemapper")
  data.frame(id = as.character(ids), type = attr(ids, "type"), feature = attr(ids, "feature"),
             stringsAsFactors = FALSE)
}
