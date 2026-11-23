#' Errors raised by basemapper
#'
#' Errors carry classes, so code can handle a kind of failure (as Python's
#' `basemapper.ValidationError`, `StyleError` and `NetworkError`):
#'
#' * `basemapper_validation_error`: invalid input, found before anything is
#'   downloaded (bounds, sizes, filters, `max_tiles` exceeded).
#' * `basemapper_style_error`: the style could not be downloaded or read.
#' * `basemapper_network_error`: tiles could not be downloaded (a wrong or
#'   missing API key, a server down).
#' * `basemapper_render_error`: anything else (an undecodable tile, ...).
#'
#' Every one also has class `basemapper_error`.
#'
#' @examples
#' \dontrun{
#' tryCatch(
#'   render_png(basemap("https://example.com/missing.json")),
#'   basemapper_style_error = function(e) message("no style: ", conditionMessage(e))
#' )
#' }
#' @name basemapper-errors
NULL

# The kinds the core reports (BasemapError::kind()).
.CORE_ERROR_KINDS <- c("validation", "style", "network", "render")

# Raise a basemapper error of `kind`. `message` is shown as is (it may hold
# URLs with `{z}`: it is passed as a value, never as a template).
abort_basemapper <- function(message, kind = "validation", call = rlang::caller_env()) {
  cli::cli_abort(
    "{message}",
    class = c(paste0("basemapper_", kind, "_error"), "basemapper_error"),
    call = call
  )
}

# .Call() into the Rust core, raising its errors as classed conditions. The
# core prefixes each message with its kind ("[network] ..."). No call is
# shown: the caller here is an internal helper, not the user's function.
call_core <- function(.NAME, ..., call = NULL) {
  tryCatch(
    .Call(.NAME, ..., PACKAGE = "basemapper"),
    error = function(e) {
      msg <- conditionMessage(e)
      # (?s): a message may span lines.
      m <- regmatches(msg, regexec("(?s)^\\[([a-z]+)\\] (.*)$", msg, perl = TRUE))[[1L]]
      if (length(m) == 3L && m[2L] %in% .CORE_ERROR_KINDS) {
        abort_basemapper(m[3L], m[2L], call = call)
      }
      abort_basemapper(msg, "render", call = call)
    }
  )
}
