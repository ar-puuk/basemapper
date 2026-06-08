#' List all layer IDs from a MapLibre GL style.
#'
#' Fetches (if URL) or parses (if inline JSON) a MapLibre GL style and returns
#' all layer `id` values. Also prints a two-column data.frame of id and type to
#' the console for interactive exploration.
#'
#' @param style_input Character: a MapLibre GL style URL or inline JSON string.
#' @return A character vector of all layer `id` values.
#' @export
#' @examples
#' \dontrun{
#' ids <- list_layers("https://demotiles.maplibre.org/style.json")
#' }
list_layers <- function(style_input) {
  style_json <- if (startsWith(trimws(style_input), "{")) {
    style_input
  } else {
    resp <- httr2::request(style_input) |>
      httr2::req_timeout(10) |>
      httr2::req_perform()
    httr2::resp_body_string(resp)
  }

  style <- jsonlite::fromJSON(style_json, simplifyVector = FALSE)
  layers <- style[["layers"]]

  if (is.null(layers) || length(layers) == 0) {
    message("No layers found in style.")
    return(character(0))
  }

  ids   <- vapply(layers, function(l) l[["id"]]   %||% "", character(1))
  types <- vapply(layers, function(l) l[["type"]] %||% "", character(1))

  df <- data.frame(id = ids, type = types, stringsAsFactors = FALSE)
  print(df)

  invisible(sort(ids))
}
