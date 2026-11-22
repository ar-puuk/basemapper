#' List a style's layers, or preview a filter
#'
#' The layers of a style in drawing order: their `id` (what `layers`
#' matches), `type` (`symbol` layers are the labels `labels` keeps or
#' drops), `source_layer` (the vector tile layer drawn; `NA` for background
#' and raster layers) and `feature` (what `features` matches). With
#' `layers`, `labels` or `features`, the layers a render with those filters
#' draws; no rows when they leave nothing. Python:
#' `basemapper.list_layers()`.
#'
#' A layer's feature comes from its source layer, with tables for the
#' OpenMapTiles (OpenFreeMap, Stadia, MapTiler), Protomaps (CARTO) and
#' Shortbread (VersaTiles) schemas and keywords for others (Esri's
#' descriptive names, layer ids). The features are background, water,
#' landcover, parks, buildings, roads, rail, boundaries, places, poi,
#' terrain and other. They are a convenience: check them here, and use
#' `layers` for exact control.
#'
#' @param style A style (anything [basemap()] takes).
#' @param auth_token Optional access token for the style URL.
#' @param layers,labels,features Filters to preview, as the renderers take
#'   them (see [basemap()]).
#' @return A data frame with columns `id`, `type`, `source_layer` and
#'   `feature`.
#' @export
#' @examples
#' \dontrun{
#' list_layers("OpenFreeMap.Positron")
#' list_layers("OpenFreeMap.Positron", labels = "only")      # what would be drawn
#' list_layers("OpenFreeMap.Positron", layers = "highway_*")
#' }
list_layers <- function(style, auth_token = NULL, layers = NULL, labels = TRUE, features = NULL) {
  ids <- .Call("wrap__list_layers_raw", as.character(style)[1L],
               if (is.null(auth_token)) NULL else as.character(auth_token),
               if (is.null(layers)) NULL else as.character(layers),
               labels_arg(labels),
               if (is.null(features)) NULL else as.character(features),
               PACKAGE = "basemapper")
  source_layer <- attr(ids, "source_layer")
  source_layer[source_layer == ""] <- NA_character_
  data.frame(id = as.character(ids), type = attr(ids, "type"), source_layer = source_layer,
             feature = attr(ids, "feature"), stringsAsFactors = FALSE)
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

# Attribution per style set, so plots redrawn (resized, re-saved) do not
# fetch the styles again.
.BASEMAPPER_ATTRIBUTION <- new.env(parent = emptyenv())

# The text a plot layer's `attribution` argument asks for: TRUE the styles'
# own (cached; "" when it cannot be read), FALSE / NULL / NA none, a string
# as given.
attribution_text <- function(attribution, style, auth_token = NULL) {
  if (is.null(attribution) || isFALSE(attribution) || identical(attribution, NA)) return("")
  if (is.character(attribution)) return(paste(attribution, collapse = " "))
  if (!isTRUE(attribution)) {
    stop("attribution must be TRUE, FALSE or a character string.", call. = FALSE)
  }
  styles <- style_strings(style)
  key <- paste(c(styles, auth_token), collapse = "\r")
  if (is.null(.BASEMAPPER_ATTRIBUTION[[key]])) {
    .BASEMAPPER_ATTRIBUTION[[key]] <- tryCatch(get_attribution(styles, auth_token), error = function(e) "")
  }
  .BASEMAPPER_ATTRIBUTION[[key]]
}

# The attribution as small text in the bottom-right corner of the current
# viewport, on a translucent white box, wrapped to its width.
attribution_grob <- function(text, width_in, fontsize = 6) {
  if (!nzchar(text)) return(NULL)
  chars <- max(20L, floor(width_in * 72 / (fontsize * 0.5)))
  label <- paste(strwrap(text, width = chars), collapse = "\n")
  pad <- grid::unit(1.5, "pt")
  txt <- grid::textGrob(
    label, x = grid::unit(1, "npc") - pad, y = pad, just = c("right", "bottom"),
    gp = grid::gpar(fontsize = fontsize, col = "grey20", lineheight = 1)
  )
  box <- grid::rectGrob(
    x = grid::unit(1, "npc"), y = grid::unit(0, "npc"), just = c("right", "bottom"),
    width = grid::grobWidth(txt) + 2 * pad, height = grid::grobHeight(txt) + 2 * pad,
    gp = grid::gpar(fill = grDevices::adjustcolor("white", 0.7), col = NA)
  )
  grid::gTree(children = grid::gList(box, txt), name = "basemap_attribution")
}
