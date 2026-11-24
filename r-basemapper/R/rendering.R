# Renderings: one render, many conversions (docs/design/api-review.md §4.4).
# The pixels stay raw bytes, converted on demand: a nativeRaster for plots,
# an array or a SpatRaster on request.

rendering_of <- function(raw, crs) {
  notes <- notes_df(raw$notes)
  surface_notes(notes)
  structure(
    list(
      pixels = raw$pixels,
      width = raw$width,
      height = raw$height,
      bounds = stats::setNames(raw$bounds, c("xmin", "ymin", "xmax", "ymax")),
      crs = crs,
      zoom = raw$zoom,
      tile_zoom = raw$tile_zoom,
      dpi = raw$dpi,
      scale = raw$scale,
      notes = notes
    ),
    class = "basemapper_rendering"
  )
}

check_rendering <- function(x, call = rlang::caller_env()) {
  if (!inherits(x, "basemapper_rendering")) abort_basemapper("`x` must be a rendering: render(m).", call = call)
}

#' Convert a rendering
#'
#' A rendering ([render()]) holds the image once; these convert it.
#'
#' * `write_png()` writes a PNG file (and returns its bytes invisibly).
#' * `as_array()` gives an integer array `[height, width, 4]` (RGBA, 0--255,
#'   straight alpha) with the bounds and CRS as attributes.
#' * `as_raster()` gives a [terra::rast()] in the rendering's CRS (layers red,
#'   green, blue, alpha; terra needed).
#' * `plot()` draws it on the current device; `print()` describes it,
#'   including its notes.
#'
#' A rendering's fields: `width`, `height`, `bounds`, `crs`, `zoom`,
#' `tile_zoom`, `dpi`, `scale` (1:N at the centre, the same at any dpi) and
#' `notes` (a data frame: `level`, `kind`, `message`, `count`, `details`).
#' Python: `img.save()`, `img.to_numpy()`, `img.to_xarray()`.
#'
#' @param x A rendering, from [render()].
#' @param path The file to write.
#' @param ... Unused.
#' @return `write_png()`: the PNG bytes, invisibly. `as_array()`: an integer
#'   array. `as_raster()`: a SpatRaster.
#' @name rendering
#' @examples
#' \dontrun{
#' img <- render(basemap("OpenFreeMap.Liberty", center = c(-111.89, 40.76), zoom = 12))
#' write_png(img, "map.png")
#' dim(as_array(img))
#' img$notes
#' }
NULL

#' @rdname rendering
#' @export
write_png <- function(x, path) {
  check_rendering(x)
  png <- call_core("wrap__encode_png_raw", x$pixels, x$width, x$height)
  writeBin(png, path)
  invisible(png)
}

#' @rdname rendering
#' @export
as_array <- function(x) {
  check_rendering(x)
  a <- aperm(array(as.integer(x$pixels), dim = c(4L, x$width, x$height)), c(3L, 2L, 1L))
  attr(a, "bounds") <- x$bounds
  attr(a, "crs") <- crs_name(x$crs)
  a
}

#' @rdname rendering
#' @export
as_raster <- function(x) {
  check_rendering(x)
  rlang::check_installed("terra", "to return a SpatRaster.")
  b <- x$bounds
  crs <- if (inherits(x$crs, "basemapper_crs")) x$crs$name else x$crs$wkt
  r <- terra::rast(nrows = x$height, ncols = x$width, nlyrs = 4L, xmin = b[["xmin"]], xmax = b[["xmax"]],
                   ymin = b[["ymin"]], ymax = b[["ymax"]], crs = crs)
  # Pixels are row-major RGBA from the top-left: one column per band.
  terra::values(r) <- matrix(as.integer(x$pixels), ncol = 4L, byrow = TRUE)
  names(r) <- c("red", "green", "blue", "alpha")
  terra::RGB(r) <- 1:3
  r
}

# Straight-alpha RGBA bytes as a nativeRaster: R's own packed colour format
# (one integer per pixel, 0xAABBGGRR), which is what RGBA bytes read as
# little-endian integers are. Drawn without any conversion, at a quarter of
# the memory of an integer array.
as_native <- function(x) {
  v <- readBin(x$pixels, what = "integer", n = x$width * x$height, size = 4L, endian = "little")
  structure(v, dim = c(x$height, x$width), class = "nativeRaster", channels = 4L)
}

#' @rdname rendering
#' @export
plot.basemapper_rendering <- function(x, ...) {
  grid::grid.newpage()
  grid::grid.raster(as_native(x), interpolate = TRUE)
  invisible(x)
}

#' @export
print.basemapper_rendering <- function(x, ...) {
  cat(sprintf("<rendering> %d x %d px, %s, zoom %.2f, %g dpi, 1:%s\n", x$width, x$height, crs_name(x$crs),
              x$zoom, x$dpi, format(round(x$scale), big.mark = ",")))
  b <- x$bounds
  cat(sprintf("  bounds %s\n", paste(format(signif(b, 8)), collapse = ", ")))
  for (i in seq_len(nrow(x$notes))) cat(sprintf("  %-8s %s\n", x$notes$level[[i]], x$notes$message[[i]]))
  invisible(x)
}

#' @rdname rendering
#' @param options knitr chunk options.
#' @exportS3Method knitr::knit_print
knit_print.basemapper_rendering <- function(x, options = list(), ...) {
  .BASEMAPPER_KNIT$n <- .BASEMAPPER_KNIT$n + 1L
  path <- knitr::fig_path("-basemap.png", options, number = .BASEMAPPER_KNIT$n)
  dir.create(dirname(path), recursive = TRUE, showWarnings = FALSE)
  write_png(x, path)
  knitr::include_graphics(path, dpi = x$dpi)
}

.BASEMAPPER_KNIT <- new.env(parent = emptyenv())
.BASEMAPPER_KNIT$n <- 0L
