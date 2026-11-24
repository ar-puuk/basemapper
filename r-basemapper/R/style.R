#' A style: what to draw
#'
#' A style is a source and what to draw of it: its own filters and
#' credential. A character string works wherever a style is expected and
#' means `style(string)`. Each style of a stack is filtered on its own; a
#' catalogue stack (`"UGRC.Lite"`) is one style. The core checks a style
#' when it is made, so a bad filter, unknown catalogue name or missing key
#' is an error here. Python: `basemapper.style()`.
#'
#' @param source A catalogue name (`"OpenFreeMap.Liberty"`; see
#'   [list_providers()]), a MapLibre style URL, file or JSON, tile data (a
#'   tile URL, TileJSON, PMTiles archive, ArcGIS service), a source helper
#'   ([provider()], [tiles()], [wms()], [wmts()]), or a style (to change its
#'   filters or key).
#' @param ... Not used: the arguments after `source` must be named.
#' @param layers Layer ids to keep, or (`-`-prefixed) to drop; `*` and `?`
#'   are wildcards (`"-label_*"`). See [list_layers()].
#' @param features Feature classes to keep or (`-`-prefixed) drop:
#'   background, water, landcover, parks, buildings, roads, rail, boundaries,
#'   places, poi, terrain, other, and `"labels"` (`"labels"` keeps only the
#'   labels, `"-labels"` drops them; `c("roads", "labels")` keeps the road
#'   labels).
#' @param key The credential: an API key for catalogue basemaps (default from
#'   their environment variable), else a token sent only to the style's own
#'   server.
#' @return A `basemapper_style` object.
#' @export
#' @examples
#' style("OpenFreeMap.Liberty", features = "-labels")
#' \dontrun{
#' imagery <- style("Esri.WorldImagery")
#' labels <- style("OpenFreeMap.Liberty", features = "labels")
#' basemap(list(imagery, labels), center = c(-111.89, 40.76), zoom = 13)
#' }
style <- function(source, ..., layers = NULL, features = NULL, key = NULL) {
  check_named_dots(list(...), "style", "source")
  if (inherits(source, "basemapper_style")) {
    layers <- layers %||% source$layers
    features <- features %||% source$features
    key <- key %||% source$key
    source <- source$source
  } else if (!inherits(source, "basemapper_source")) {
    if (!is.character(source) || length(source) != 1L || is.na(source)) {
      abort_basemapper("A style's source must be a single string or a source helper (provider(), tiles(), ...).")
    }
  }
  s <- structure(
    list(
      source = source,
      layers = if (is.null(layers)) NULL else as.character(layers),
      features = if (is.null(features)) NULL else as.character(features),
      key = if (is.null(key)) NULL else as.character(key)[1L]
    ),
    class = "basemapper_style"
  )
  call_core("wrap__check_style_raw", to_json(style_spec(s)), call = rlang::caller_env())
  s
}

#' Sources with their own options
#'
#' Helpers for [style()]'s `source`, each with that source's own options.
#' A catalogue name or tile data also works as a plain string.
#'
#' * `provider()`: a catalogue basemap, in a chosen form, or drawing your own
#'   tile data with its style.
#' * `tiles()`: tile data, with a style built for it and overrides.
#' * `wms()`, `wmts()`: OGC services, as 256 px tiles in Web Mercator. A
#'   RESTful WMTS template (`{TileMatrix}/{TileRow}/{TileCol}`) needs no
#'   helper: give it as the style.
#'
#' Python: `basemapper.provider()`, `tiles()`, `wms()`, `wmts()`.
#'
#' @param name The basemap's name (case-insensitive).
#' @param form `"vector"` (a MapLibre style) or `"raster"` (tiles); by
#'   default the vector form when there is one. A form the basemap lacks is an
#'   error, never a silent fallback.
#' @param tiles For `provider()`: your own tile data drawn with the
#'   basemap's style (a `.pmtiles` archive, tile URL or TileJSON), in the
#'   same schema as its tiles.
#' @param input For `tiles()`: a tile URL template (`{z}/{x}/{y}`,
#'   `{quadkey}`, `{bbox-epsg-3857}`), a TileJSON, a PMTiles archive (URL or
#'   path), or an ArcGIS `.../MapServer` or `.../VectorTileServer`.
#' @param type `"raster"` or `"vector"`; detected when `NULL`.
#' @param tile_size Raster tile size in px (default 256).
#' @param scheme `"xyz"` or `"tms"` (rows counted from the south).
#' @param minzoom,maxzoom The tiles' zoom range (enlarged beyond `maxzoom`).
#' @param attribution Attribution text for the source.
#' @param source_layers Vector tiles: the source layers to draw (default all).
#' @param paint Vector tiles: a named list of paint properties for the
#'   generated `fill-*`, `line-*` and `circle-*` layers, e.g.
#'   `list("fill-color" = "#cccccc")`. A key that matches no layer is a
#'   problem note when rendering.
#' @param url The service URL, without query parameters.
#' @param layers The WMS `LAYERS` parameter.
#' @param styles The WMS `STYLES` parameter.
#' @param version The service version (WMS 1.1.x sends `SRS`, 1.3.0 `CRS`).
#' @param format The image MIME type.
#' @param transparent Request transparent WMS images.
#' @param extra_params More query parameters, appended verbatim.
#' @param layer The WMTS `LAYER` parameter.
#' @param tilematrixset A Web Mercator WMTS tile matrix set.
#' @param style The WMTS `STYLE` parameter.
#' @param ... Not used: the arguments after the first must be named.
#' @return A `basemapper_source` object, for [style()] or as a style itself.
#' @name sources
#' @examples
#' style(provider("CartoDB.Positron", form = "raster"), key = "...")
#' style(tiles("https://example.com/{z}/{x}/{y}.mvt", source_layers = "land",
#'             paint = list("fill-color" = "#e8e0d8")))
#' style(wms("https://example.com/wms", layers = "relief"))
NULL

new_source <- function(kind, ...) {
  opts <- list(...)
  opts <- opts[!vapply(opts, is.null, logical(1))]
  structure(c(list(kind = kind), opts), class = "basemapper_source")
}

chr1 <- function(x) if (is.null(x)) NULL else as.character(x)[1L]
num1 <- function(x) if (is.null(x)) NULL else as.numeric(x)[1L]

#' @rdname sources
#' @export
provider <- function(name, form = NULL, tiles = NULL) {
  new_source("provider", name = chr1(name), form = chr1(form), tiles = chr1(tiles))
}

#' @rdname sources
#' @export
tiles <- function(input, ..., type = NULL, tile_size = NULL, scheme = NULL, minzoom = NULL, maxzoom = NULL,
                  attribution = NULL, source_layers = NULL, paint = NULL) {
  check_named_dots(list(...), "tiles", "input")
  new_source(
    "tiles", input = chr1(input), type = chr1(type), tile_size = num1(tile_size), scheme = chr1(scheme),
    minzoom = num1(minzoom), maxzoom = num1(maxzoom), attribution = chr1(attribution),
    source_layers = if (is.null(source_layers)) NULL else as.character(source_layers),
    paint = if (length(paint)) as.character(jsonlite::toJSON(paint, auto_unbox = TRUE)) else NULL
  )
}

#' @rdname sources
#' @export
wms <- function(url, layers, ..., styles = "", version = "1.3.0", format = "image/png",
                transparent = FALSE, extra_params = NULL) {
  check_named_dots(list(...), "wms", "layers")
  new_source("wms", url = chr1(url), layers = chr1(layers), styles = chr1(styles), version = chr1(version),
             format = chr1(format), transparent = isTRUE(transparent), extra_params = chr1(extra_params))
}

#' @rdname sources
#' @export
wmts <- function(url, layer, ..., tilematrixset = "GoogleMapsCompatible", format = "image/png",
                 style = "default", version = "1.0.0", extra_params = NULL) {
  check_named_dots(list(...), "wmts", "layer")
  new_source("wmts", url = chr1(url), layer = chr1(layer), tilematrixset = chr1(tilematrixset),
             format = chr1(format), style = chr1(style), version = chr1(version),
             extra_params = chr1(extra_params))
}

#' @export
print.basemapper_style <- function(x, ...) {
  src <- if (is.character(x$source)) x$source else sprintf("%s(%s)", x$source$kind,
                                                            x$source$name %||% x$source$input %||% x$source$url)
  extra <- c(
    if (!is.null(x$layers)) sprintf("layers %s", paste(x$layers, collapse = ", ")),
    if (!is.null(x$features)) sprintf("features %s", paste(x$features, collapse = ", ")),
    if (!is.null(x$key)) "a key"
  )
  cat(sprintf("<style> %s%s\n", if (nchar(src) > 60) paste0(substr(src, 1, 57), "...") else src,
              if (length(extra)) paste0(" (", paste(extra, collapse = "; "), ")") else ""))
  invisible(x)
}

#' @export
print.basemapper_source <- function(x, ...) {
  cat(sprintf("<source> %s(%s)\n", x$kind, x$name %||% x$input %||% x$url))
  invisible(x)
}

# A style as the core reads it.
style_spec <- function(s) {
  src <- s$source
  source <- if (is.character(src)) list(kind = "auto", value = src) else unclass(src)
  spec <- list(source = source)
  if (!is.null(s$layers)) spec$layers <- s$layers
  if (!is.null(s$features)) spec$features <- s$features
  if (!is.null(s$key)) spec$key <- s$key
  spec
}

# A style, or anything style() takes.
style_arg <- function(x, call = rlang::caller_env()) {
  if (inherits(x, "basemapper_style")) x else style(x)
}

# One style or several (a character vector or a list), stacked bottom to top.
styles_arg <- function(x, call = rlang::caller_env()) {
  items <- if (inherits(x, c("basemapper_style", "basemapper_source"))) list(x) else as.list(x)
  if (length(items) == 0L) abort_basemapper("A basemap needs at least one style.", call = call)
  lapply(items, style_arg, call = call)
}

# Dots must be empty: the arguments after `after` are named.
check_named_dots <- function(dots, fn, after, call = rlang::caller_env(2)) {
  if (length(dots) == 0L) return(invisible())
  nms <- names(dots) %||% rep("", length(dots))
  if (any(!nzchar(nms))) {
    abort_basemapper(sprintf("Arguments after `%s` in %s() must be named (e.g. `zoom = 12`).", after, fn), call = call)
  }
  removed_arguments(fn, nms, call = call)
}
