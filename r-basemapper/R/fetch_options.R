#' How basemaps download tiles
#'
#' Settings for downloading a basemap's tiles, given to the `fetch`
#' argument of [basemap()], [render_basemap_raw()], [geom_basemap()] and
#' [tm_basemap()]. The defaults suit most maps; change them for slow
#' servers, very large maps, or to stop on a missing tile. Python:
#' `basemapper.FetchOptions`.
#'
#' @param timeout Seconds to wait for each tile.
#' @param max_tiles Most tiles fetched per style. A view needing more is an
#'   error for vector tiles; raster tiles are fetched one zoom level
#'   coarser instead.
#' @param concurrency Tiles downloaded at once.
#' @param on_error What a tile that fails to download does: `"skip"` leaves
#'   it empty (the map is drawn without it), `"fail"` stops with an error.
#'
#' @return A `basemapper_fetch_options` object.
#' @export
#' @examples
#' fetch_options(timeout = 30, on_error = "fail")
#' \dontrun{
#' m <- basemap("OpenFreeMap.Liberty", center = c(-111.89, 40.76), zoom = 12,
#'              fetch = fetch_options(timeout = 30))
#' }
fetch_options <- function(timeout = 10, max_tiles = 256L, concurrency = 16L,
                          on_error = c("skip", "fail")) {
  call <- rlang::current_env()
  on_error <- rlang::arg_match(on_error)
  if (!is.numeric(timeout) || length(timeout) != 1L || is.na(timeout) || timeout <= 0) {
    abort_basemapper("`timeout` must be a number of seconds > 0.", call = call)
  }
  count <- function(x, name) {
    if (!is.numeric(x) || length(x) != 1L || is.na(x) || x < 1 || x != round(x)) {
      abort_basemapper(sprintf("`%s` must be a whole number of at least 1.", name), call = call)
    }
    as.integer(x)
  }
  structure(
    list(
      timeout = as.numeric(timeout),
      max_tiles = count(max_tiles, "max_tiles"),
      concurrency = count(concurrency, "concurrency"),
      on_error = on_error
    ),
    class = "basemapper_fetch_options"
  )
}

#' @export
print.basemapper_fetch_options <- function(x, ...) {
  cat(sprintf(
    "<fetch_options> timeout %gs, max_tiles %d, concurrency %d, on_error \"%s\"\n",
    x$timeout, x$max_tiles, x$concurrency, x$on_error
  ))
  invisible(x)
}

check_fetch <- function(fetch, call = rlang::caller_env()) {
  if (!inherits(fetch, "basemapper_fetch_options")) {
    abort_basemapper("`fetch` must be made with fetch_options().", call = call)
  }
  fetch
}

# The core's names for the fetch settings.
fetch_core <- function(fetch) {
  list(
    tile_timeout_ms = as.integer(min(round(fetch$timeout * 1000), .Machine$integer.max)),
    max_tiles = fetch$max_tiles,
    tile_concurrency = fetch$concurrency,
    fail_on_tile_error = identical(fetch$on_error, "fail")
  )
}

# The arguments `fetch` replaces, and the fetch_options() setting each maps to.
.DEPRECATED_FETCH_ARGS <- c(
  tile_timeout_ms = "timeout", max_tiles = "max_tiles",
  tile_concurrency = "concurrency", fail_on_tile_error = "on_error"
)

# `fetch` with the deprecated arguments found in `dots` (a list) applied, and
# the other dots. `fn` names the function for messages, `after` its last
# positional argument; `allow_other` passes other named dots through
# (geom_basemap() hands them to ggplot2::layer()), else they are an error.
fetch_with_deprecated <- function(fetch, dots, fn, after = "height", allow_other = FALSE,
                                  call = rlang::caller_env()) {
  check_fetch(fetch, call)
  nms <- names(dots) %||% rep("", length(dots))
  if (any(!nzchar(nms))) {
    abort_basemapper(
      sprintf("Arguments after `%s` in %s() must be named (e.g. `zoom = 12`).", after, fn),
      call = call
    )
  }
  old <- intersect(nms, names(.DEPRECATED_FETCH_ARGS))
  for (arg in old) {
    lifecycle::deprecate_warn(
      "0.2.0", sprintf("%s(%s)", fn, arg), sprintf("%s(fetch)", fn),
      details = sprintf("Use `fetch = fetch_options(%s = ...)`%s.", .DEPRECATED_FETCH_ARGS[[arg]],
                        if (arg == "tile_timeout_ms") " (in seconds)" else ""),
      env = call
    )
    v <- dots[[arg]]
    switch(arg,
      tile_timeout_ms = fetch$timeout <- as.numeric(v) / 1000,
      max_tiles = fetch$max_tiles <- v,
      tile_concurrency = fetch$concurrency <- v,
      fail_on_tile_error = fetch$on_error <- if (isTRUE(v)) "fail" else "skip"
    )
  }
  if (length(old)) {
    fetch <- fetch_options(fetch$timeout, fetch$max_tiles, fetch$concurrency, fetch$on_error)
  }
  rest <- dots[!nms %in% old]
  if (!allow_other && length(rest)) {
    abort_basemapper(
      sprintf("Unknown argument%s to %s(): %s.", if (length(rest) > 1) "s" else "", fn,
              paste0("`", names(rest), "`", collapse = ", ")),
      call = call
    )
  }
  list(fetch = fetch, dots = rest)
}
