#' List a style's layers
#'
#' The layers of a style in drawing order, with their types: the IDs are
#' what the `layers` filter of [basemap()] and the renderers accepts (plain
#' IDs keep, minus-prefixed IDs drop). Python: `basemapper.list_layers()`.
#'
#' @param style A MapLibre GL style URL or JSON string.
#' @param auth_token Optional access token for the style URL.
#' @return A data frame with columns `id` and `type`.
#' @export
#' @examples
#' \dontrun{
#' list_layers("OpenFreeMap.Positron")
#' }
list_layers <- function(style, auth_token = NULL) {
  ids <- .Call("wrap__list_layers_raw", as.character(style)[1L],
               if (is.null(auth_token)) NULL else as.character(auth_token), PACKAGE = "basemapper")
  data.frame(id = as.character(ids), type = attr(ids, "type"), stringsAsFactors = FALSE)
}
