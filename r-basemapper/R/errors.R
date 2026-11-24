#' Errors and notes from basemapper
#'
#' Errors carry classes, so code can handle a kind of failure (as Python's
#' `basemapper.ValidationError`, `StyleError` and `NetworkError`):
#'
#' * `basemapper_validation_error`: invalid input, found before anything is
#'   downloaded (a view, style, filter, CRS or clip that cannot work).
#' * `basemapper_style_error`: the style could not be downloaded or read.
#' * `basemapper_network_error`: data could not be downloaded (a wrong or
#'   missing API key, a server down).
#' * `basemapper_render_error`: anything else.
#'
#' Every one also has class `basemapper_error`.
#'
#' Renders also carry **notes** on what basemapper decided, adjusted or could
#' not do (see [render()]). They are shown by their level:
#'
#' * *decided* (a value nothing gave, such as the automatic zoom): in
#'   `print()`, and as messages with `options(basemapper.verbose = TRUE)`;
#' * *adjusted* (something asked for was changed, such as an effective dpi
#'   for a zoom chosen in a plot): a message of class
#'   `basemapper_adjustment`, once per kind per session;
#' * *problem* (the output is incomplete, such as tiles that failed): a
#'   warning of class `basemapper_problem`, every time, or an error with
#'   `fetch_options(on_error = "fail")`.
#'
#' @examples
#' \dontrun{
#' tryCatch(
#'   render(basemap("https://example.com/missing.json")),
#'   basemapper_style_error = function(e) message("no style: ", conditionMessage(e))
#' )
#' }
#' @name basemapper-errors
NULL

# Null-coalescing operator used across the package.
`%||%` <- function(x, y) if (!is.null(x)) x else y

# The kinds the core reports (BasemapError::kind()).
.CORE_ERROR_KINDS <- c("validation", "style", "network", "render")

# The migration guide that removed names point to.
.MIGRATION <- "https://ar-puuk.github.io/basemapper/migrating-to-0.3.html"

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
# core prefixes each message with its kind ("[network] ...").
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

# Values cross into the core as JSON (see src/rust/src/lib.rs): specs here,
# results back. Full precision; NULL and NA as null.
to_json <- function(x) {
  if (is.null(x)) return("")
  as.character(jsonlite::toJSON(x, auto_unbox = TRUE, digits = NA, null = "null", na = "null"))
}

from_json <- function(s) {
  if (is.null(s) || !nzchar(s)) return(NULL)
  jsonlite::fromJSON(s, simplifyVector = TRUE, simplifyDataFrame = FALSE, simplifyMatrix = FALSE)
}

# A clip for the core, flat: ring counts, point counts, x and y. Empty for
# no clip.
clip_args <- function(clip) {
  if (is.null(clip)) return(list("", integer(), integer(), numeric(), numeric()))
  rings <- unlist(clip$polygons, recursive = FALSE)
  list(
    clip$keep,
    as.integer(lengths(clip$polygons)),
    as.integer(vapply(rings, nrow, 1L)),
    as.numeric(unlist(lapply(rings, function(r) r[, 1]))),
    as.numeric(unlist(lapply(rings, function(r) r[, 2])))
  )
}

# A mesh for the core: nx, ny, x, y (nx 0 for none).
mesh_args <- function(mesh) {
  if (is.null(mesh)) return(list(0L, 0L, numeric(), numeric()))
  list(as.integer(mesh$nx), as.integer(mesh$ny), as.numeric(mesh$x), as.numeric(mesh$y))
}

# Render through the core. The pixels come as a raw vector, the rest as
# JSON in its "meta" attribute.
core_render <- function(styles, inputs, fetch, clip = NULL, frame = NULL, mesh = NULL, panel = NULL,
                        call = rlang::caller_env()) {
  args <- c(
    list("wrap__render_raw", to_json(lapply(styles, style_spec)), to_json(inputs), to_json(fetch_spec(fetch))),
    clip_args(clip), list(to_json(frame)), mesh_args(mesh), list(to_json(panel))
  )
  raw <- do.call(call_core, c(args, list(call = call)))
  meta <- from_json(attr(raw, "meta"))
  attributes(raw) <- NULL
  meta$pixels <- raw
  meta
}

# The core's notes (parallel vectors) as a data frame: level, kind,
# message, count, and a list column of details.
notes_df <- function(raw) {
  df <- data.frame(
    level = as.character(raw$level %||% character()),
    kind = as.character(raw$kind %||% character()),
    message = as.character(raw$message %||% character()),
    count = as.integer(raw$count %||% integer()),
    stringsAsFactors = FALSE
  )
  df$details <- I(lapply(as.list(raw$details %||% list()), as.character))
  df
}

# Show notes as their level asks (see ?basemapper-errors).
surface_notes <- function(notes) {
  for (i in seq_len(nrow(notes))) {
    msg <- notes$message[[i]]
    switch(notes$level[[i]],
      decided = if (isTRUE(getOption("basemapper.verbose"))) {
        cli::cli_inform("{msg}", class = "basemapper_decided")
      },
      adjusted = cli::cli_inform(
        c(i = "{msg}"),
        class = "basemapper_adjustment",
        .frequency = "once",
        .frequency_id = paste0("basemapper_", notes$kind[[i]])
      ),
      rlang::warn(msg, class = "basemapper_problem")
    )
  }
  invisible(notes)
}
