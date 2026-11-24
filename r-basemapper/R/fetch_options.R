#' How basemaps download their data
#'
#' Settings for downloading a basemap's data, given to the `fetch` argument
#' of [basemap()]. The defaults suit most maps; change them for slow
#' servers, very large maps, or to stop on missing data. Python:
#' `basemapper.FetchOptions`; Rust: `FetchOptions`.
#'
#' @param timeout Seconds to wait for each download.
#' @param max_tiles Most tiles fetched per style. A view needing more is an
#'   error for vector tiles; raster tiles are fetched one zoom level coarser
#'   instead (an *adjusted* note).
#' @param concurrency Downloads at once.
#' @param on_error `"skip"` draws what there is, each gap a warning of class
#'   `basemapper_problem`; `"fail"` stops with an error for a failed tile or
#'   any other problem.
#'
#' @return A `basemapper_fetch_options` object.
#' @export
#' @examples
#' fetch_options(timeout = 30, on_error = "fail")
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

# As the core reads it.
fetch_spec <- function(fetch) unclass(fetch)
