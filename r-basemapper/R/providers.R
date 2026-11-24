#' The built-in basemaps
#'
#' The named basemaps: their names work as styles (`"CartoDB.Positron"`),
#' and with [provider()] for a chosen form. Python:
#' `basemapper.list_providers()`.
#'
#' @param type `"vector"` or `"raster"` to list only basemaps with that
#'   form; `NULL` lists all.
#' @return A data frame with columns `name`; `vector` and `raster` (logical:
#'   the forms it has); `api_key_env` (the environment variable its API key
#'   is read from, `NA` when it needs none); and `attribution`.
#' @export
#' @examples
#' list_providers()
#' list_providers("vector")$name
list_providers <- function(type = NULL) {
  x <- from_json(call_core("wrap__list_providers_raw", if (is.null(type)) "" else as.character(type)[1L]))
  data.frame(name = as.character(x$name), vector = as.logical(x$vector), raster = as.logical(x$raster),
             api_key_env = json_chr(x$api_key_env), attribution = as.character(x$attribution),
             stringsAsFactors = FALSE)
}

#' Basemap names, for autocompletion
#'
#' A named list of every basemap name: `providers$CartoDB.Positron` is the
#' string `"CartoDB.Positron"`, usable as a style or with [provider()]. The names match `leaflet::providers` where both
#' have a basemap. Python: `basemapper.providers.CartoDB.Positron`.
#'
#' @format A named list of character strings.
#' @export
#' @examples
#' providers$Esri.WorldImagery
providers <- NULL

#' The tile cache
#'
#' Map tiles are cached on disk for as long as the server's caching headers
#' allow (a day when they say nothing), so repeated renders of an area do
#' not download them again. The cache starts in `BASEMAPPER_CACHE_DIR` when
#' that is set (`"off"` disables it), else in
#' `tools::R_user_dir("basemapper", "cache")`. Set `BASEMAPPER_CACHE_DIR` in
#' `.Renviron` (`usethis::edit_r_environ()` opens it) to keep a folder for good,
#' or to share one cache with Python.
#'
#' The cache is kept small: expired tiles are deleted, and past its size
#' limit (250 MB, or `BASEMAPPER_CACHE_MAX_SIZE` in MB) the least recently
#' used tiles go first, in the background as tiles are stored. Python:
#' `basemapper.tile_cache()`.
#'
#' @param dir A directory to move the cache to, or `FALSE` to turn it off.
#'   `NULL` leaves it as it is.
#' @param clear Delete every cached tile.
#' @param max_size The cache's size limit in megabytes (`NULL` leaves it as
#'   it is).
#' @return The cache directory, or `NULL` when the cache is off (invisibly
#'   when `dir` or `clear` is given).
#' @export
#' @examples
#' tile_cache()
tile_cache <- function(dir = NULL, clear = FALSE, max_size = NULL) {
  d <- if (isFALSE(dir)) "off" else if (is.null(dir)) "" else path.expand(as.character(dir)[1L])
  mb <- if (is.null(max_size)) 0 else as.numeric(max_size)[1L]
  out <- call_core("wrap__tile_cache_raw", d, isTRUE(clear), mb)
  if (!nzchar(out)) out <- NULL
  if (is.null(dir) && !isTRUE(clear) && is.null(max_size)) out else invisible(out)
}

.onLoad <- function(libname, pkgname) {
  ns <- asNamespace(pkgname)
  names <- as.character(from_json(call_core("wrap__list_providers_raw", ""))$name)
  assign("providers", stats::setNames(as.list(names), names), envir = ns)
  # CRAN policy: user files go under R_user_dir() unless the user chose a place.
  if (is.na(Sys.getenv("BASEMAPPER_CACHE_DIR", NA_character_))) {
    call_core("wrap__tile_cache_raw", tools::R_user_dir("basemapper", "cache"), FALSE, 0)
  }
  invisible()
}
