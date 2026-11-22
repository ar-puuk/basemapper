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

#' The attribution a style asks maps to show
#'
#' Every source the styles' layers draw from contributes its attribution
#' (from the style, the source's TileJSON or PMTiles metadata), as
#' MapLibre's attribution control collects it: plain text, without
#' duplicates, joined with `" | "`. Use it for a caption or credit line,
#' e.g. `labs(caption = get_attribution(style))`. Python:
#' `basemapper.get_attribution()` and `m.get_attribution()`.
#'
#' @param x A view from [basemap()], or a style (anything [basemap()]
#'   takes) or vector of styles stacked bottom to top.
#' @param auth_token Optional access token for the tile service (for a view,
#'   its own is used).
#' @return A character string; `""` when the styles give no attribution.
#' @export
#' @examples
#' get_attribution("Esri.WorldImagery")
#' \dontrun{
#' m <- basemap("OpenFreeMap.Liberty", center = c(-111.89, 40.76), zoom = 12)
#' get_attribution(m)
#' }
get_attribution <- function(x, auth_token = NULL) {
  if (inherits(x, "basemap")) {
    styles <- x$styles
    auth_token <- auth_token %||% x$options$auth_token
  } else {
    styles <- style_strings(x)
  }
  .Call("wrap__attribution_raw", styles, if (is.null(auth_token)) NULL else as.character(auth_token)[1L],
        PACKAGE = "basemapper")
}
