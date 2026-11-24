#' The layers a style draws
#'
#' A style's layers in drawing order, with its filters applied: their `id`
#' (what a style's `layers` matches), `type` (`symbol` layers are the
#' labels), `source_layer` (the vector tile layer drawn; `NA` for background
#' and raster layers) and `feature` (what `features` matches). A style with
#' filters lists the layers it keeps, so this previews a filter:
#' `list_layers(style(s, features = "roads"))`. A catalogue stack lists every
#' part's layers, bottom to top. Python: `basemapper.list_layers()`.
#'
#' A layer's feature comes from its source layer, with tables for the
#' OpenMapTiles, Protomaps and Shortbread schemas and keywords for others: a
#' convenience to check here, with `layers` for exact control.
#'
#' @param style A style (anything [style()] takes).
#' @return A data frame with columns `id`, `type`, `source_layer` and
#'   `feature`.
#' @export
#' @examples
#' \dontrun{
#' list_layers("OpenFreeMap.Positron")
#' list_layers(style("OpenFreeMap.Positron", features = "labels"))   # what would be drawn
#' }
list_layers <- function(style) {
  s <- style_arg(style)
  x <- from_json(call_core("wrap__list_layers_raw", to_json(style_spec(s)), call = rlang::current_env()))
  data.frame(id = json_chr(x$id), type = json_chr(x$type), source_layer = json_chr(x$source_layer),
             feature = json_chr(x$feature), stringsAsFactors = FALSE)
}

#' The credit line a basemap's providers ask for
#'
#' Every source the styles draw from contributes its attribution (from the
#' style, its TileJSON or PMTiles metadata), as plain text without
#' duplicates, joined with `" | "`. The plot layers draw it by default; use
#' it for a caption, e.g. `labs(caption = get_attribution(m))`. Python:
#' `basemapper.get_attribution()`.
#'
#' @param x A [basemap()], a style, or a list of styles.
#' @return A character string; `""` when the styles give none.
#' @export
#' @examples
#' get_attribution("Esri.WorldImagery")
get_attribution <- function(x) {
  styles <- if (inherits(x, "basemap")) x$styles else styles_arg(x)
  call_core("wrap__attribution_raw", to_json(lapply(styles, style_spec)), call = rlang::current_env())
}

# A JSON array of strings (null as NA) as a character vector.
json_chr <- function(v) {
  vapply(as.list(v %||% list()), function(e) if (is.null(e) || length(e) == 0L) NA_character_ else as.character(e), "")
}

# Attribution per style set, so plots redrawn (resized, re-saved) do not
# fetch the styles again. Keys are looked up with match(): a style set can
# hold inline style JSON, longer than R allows a variable name to be.
.BASEMAPPER_ATTRIBUTION <- new.env(parent = emptyenv())
.BASEMAPPER_ATTRIBUTION$keys <- character()
.BASEMAPPER_ATTRIBUTION$text <- character()
.BASEMAPPER_ATTRIBUTION$failed <- list() # key -> time of the last failed lookup

# The text a plot's `attribution` argument asks for: TRUE the styles' own
# (cached; "" when it cannot be read), FALSE / NULL / NA none, a string as
# given.
attribution_text <- function(attribution, styles) {
  if (is.null(attribution) || isFALSE(attribution) || identical(attribution, NA)) return("")
  if (is.character(attribution)) return(paste(attribution, collapse = " "))
  if (!isTRUE(attribution)) abort_basemapper("`attribution` must be TRUE, FALSE or a character string.")
  key <- paste(vapply(styles, function(s) paste(unlist(style_spec(s)), collapse = "\t"), ""), collapse = "\r")
  cache <- .BASEMAPPER_ATTRIBUTION
  hit <- match(key, cache$keys)
  if (!is.na(hit)) return(cache$text[[hit]])
  # A failed lookup (a network blip) is remembered for a minute only, then
  # retried, rather than leaving every later plot of the session uncredited.
  failed_at <- cache$failed[[key]]
  if (!is.null(failed_at) && difftime(Sys.time(), failed_at, units = "secs") < 60) return("")
  text <- tryCatch(get_attribution(styles), error = function(e) NULL)
  if (is.null(text)) {
    cache$failed[[key]] <- Sys.time()
    return("")
  }
  cache$failed[[key]] <- NULL
  cache$keys <- c(cache$keys, key)
  cache$text <- c(cache$text, text)
  text
}

# The credit line in the bottom-right corner of the current viewport, laid
# out by the core (the same layout as Python's) for its width.
attribution_grob <- function(text, width_in) {
  if (!nzchar(text)) return(NULL)
  a <- from_json(.Call("wrap__attribution_layout_raw", text, width_in * 72, PACKAGE = "basemapper"))
  if (is.null(a)) return(NULL)
  pad <- grid::unit(a$padding_pt, "pt")
  txt <- grid::textGrob(
    paste(a$lines, collapse = "\n"), x = grid::unit(1, "npc") - pad, y = pad, just = c("right", "bottom"),
    gp = grid::gpar(fontsize = a$font_size_pt, col = a$color, lineheight = 1)
  )
  box <- grid::rectGrob(
    x = grid::unit(1, "npc"), y = grid::unit(0, "npc"), just = c("right", "bottom"),
    width = grid::grobWidth(txt) + 2 * pad, height = grid::grobHeight(txt) + 2 * pad,
    gp = grid::gpar(fill = grDevices::adjustcolor(a$background, a$background_alpha), col = NA)
  )
  grid::gTree(children = grid::gList(box, txt), name = "basemap_attribution")
}
