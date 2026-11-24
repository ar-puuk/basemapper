# The 0.2 names and arguments 0.3 removed (D1: no shims). Each is an error
# that names its replacement and links the migration guide.

# Arguments 0.3 removed, and what replaces each.
.REMOVED_ARGS <- c(
  pixel_ratio = "use `dpi` (dpi = 96 x pixel_ratio)",
  tile_zoom = "use `zoom` (the MapLibre zoom)",
  labels = 'filter the style: style(s, features = "-labels"), or features = "labels" for only the labels',
  layers = "filter the style: style(s, layers = ...)",
  features = "filter the style: style(s, features = ...)",
  auth_token = "give the style its key: style(s, key = ...)",
  tile_timeout_ms = "use fetch = fetch_options(timeout = seconds)",
  max_tiles = "use fetch = fetch_options(max_tiles = ...)",
  tile_concurrency = "use fetch = fetch_options(concurrency = ...)",
  fail_on_tile_error = 'use fetch = fetch_options(on_error = "fail")'
)

# An error for named arguments `fn` does not take, saying what replaces the
# ones 0.3 removed.
removed_arguments <- function(fn, names, call = rlang::caller_env()) {
  for (name in names) {
    if (name %in% names(.REMOVED_ARGS)) {
      abort_basemapper(sprintf("%s(): `%s` was removed in basemapper 0.3; %s. See %s", fn, name,
                               .REMOVED_ARGS[[name]], .MIGRATION), call = call)
    }
    abort_basemapper(sprintf("%s(): unknown argument `%s`.", fn, name), call = call)
  }
}

defunct <- function(old, new, call = rlang::caller_env()) {
  cli::cli_abort(
    "{old}() was removed in basemapper 0.3; use {new}. See {.url {(.MIGRATION)}}",
    class = c("basemapper_defunct_error", "basemapper_error"), call = call
  )
}

#' Removed in basemapper 0.3
#'
#' These 0.2 functions are gone; each now stops with an error that names
#' its replacement. See the migration guide,
#' <https://ar-puuk.github.io/basemapper/migrating-to-0.3.html>.
#'
#' * `render_basemap_raw()`: `as_array(render(basemap(style, bounds = ..., width = ..., height = ...)))`.
#' * `render_png()`, `render_array()`, `render_raster()`: [render()], then
#'   [write_png()], [as_array()] or [as_raster()].
#' * `basemap_provider()`: `style(provider(name, form = ...), features = ..., key = ...)`.
#' * `tile_style()`: `style(tiles(source, ...))`.
#' * `wms_style()`, `wmts_style()`: `style(wms(...))`, `style(wmts(...))`.
#'
#' @param ... Ignored.
#' @return Nothing: they stop with an error.
#' @name basemapper-defunct
#' @keywords internal
NULL

#' @rdname basemapper-defunct
#' @export
render_basemap_raw <- function(...) defunct("render_basemap_raw", "as_array(render(basemap(style, bounds = ...)))")

#' @rdname basemapper-defunct
#' @export
render_png <- function(...) defunct("render_png", "write_png(render(m), path)")

#' @rdname basemapper-defunct
#' @export
render_array <- function(...) defunct("render_array", "as_array(render(m))")

#' @rdname basemapper-defunct
#' @export
render_raster <- function(...) defunct("render_raster", "as_raster(render(m))")

#' @rdname basemapper-defunct
#' @export
basemap_provider <- function(...) defunct("basemap_provider", "style(provider(name, form = ...), key = ...)")

#' @rdname basemapper-defunct
#' @export
tile_style <- function(...) defunct("tile_style", "style(tiles(source, ...))")

#' @rdname basemapper-defunct
#' @export
wms_style <- function(...) defunct("wms_style", "style(wms(url, layers, ...))")

#' @rdname basemapper-defunct
#' @export
wmts_style <- function(...) defunct("wmts_style", "style(wmts(url, layer, ...))")
