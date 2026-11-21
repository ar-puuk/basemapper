#' Create a basemap view
#'
#' A map view to render headlessly: one style, or several stacked. The view
#' is a centre and MapLibre zoom (512 px tiles; fractional zooms are rendered
#' natively, not resampled), or bounds to fit. `width` and `height` are CSS
#' pixels; images are `ratio` times larger with the same view and label
#' layout (MapLibre's `devicePixelRatio`): `ratio = 2` gives a sharp 2x
#' image, `ratio = dpi / 96` matches a print resolution.
#'
#' Render it with [render_png()], [render_svg()], [render_raster()] or
#' [render_array()].
#'
#' @param style A MapLibre GL style URL or JSON string (or a provider such as
#'   [raster_provider()]), or a character vector / list of those stacked
#'   bottom to top (e.g. a basemap published as separate hillshade, base and
#'   label services).
#' @param width,height View size in CSS pixels.
#' @param center `c(longitude, latitude)` of the view centre.
#' @param zoom MapLibre zoom (0--22, fractional allowed).
#' @param bounds Bounds to fit instead of `center`/`zoom`: `c(xmin, ymin,
#'   xmax, ymax)` in `crs` (default EPSG:4326), an [sf::st_bbox()] result, or
#'   an `sf` / `sfc` object.
#' @param crs CRS of `bounds` (anything [sf::st_crs()] accepts).
#' @param ratio Device pixels per CSS pixel.
#' @param layers Optional layer filter applied to every style: plain IDs keep
#'   only those layers, minus-prefixed IDs drop them.
#' @param auth_token Optional access token for the tile service.
#' @param tile_timeout_ms Per-tile HTTP timeout in milliseconds.
#' @param max_tiles Maximum tiles fetched per style.
#' @param tile_concurrency Parallel tile downloads.
#' @param fail_on_tile_error Abort on a failed tile instead of leaving it
#'   empty.
#'
#' @return A `basemapper_map` object.
#' @export
#' @examples
#' \dontrun{
#' ugrc <- "https://tiles.arcgis.com/tiles/99lidPhWCzftIe9K/arcgis/rest/services"
#' styles <- sprintf("%s/%s/VectorTileServer/resources/styles/root.json", ugrc,
#'                   c("VectorHillshade", "LiteBase", "LiteLabels"))
#' m <- basemap(styles, 800, 600, center = c(-111.8887, 40.7556), zoom = 12.4,
#'              ratio = 2)
#' render_png(m, "slc.png")   # 1600 x 1200
#' render_svg(m, "slc.svg")   # vector
#' r <- render_raster(m)      # terra SpatRaster in EPSG:3857
#'
#' # Fit a layer's extent
#' nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
#' m <- basemap("https://basemaps.cartocdn.com/gl/positron-gl-style/style.json",
#'              bounds = nc)
#' }
basemap <- function(
    style,
    width              = 800L,
    height             = 600L,
    center             = NULL,
    zoom               = NULL,
    bounds             = NULL,
    crs                = NULL,
    ratio              = 1,
    layers             = NULL,
    auth_token         = NULL,
    tile_timeout_ms    = 10000L,
    max_tiles          = 256L,
    tile_concurrency   = 16L,
    fail_on_tile_error = FALSE
) {
  styles <- style_strings(style)
  width  <- as.integer(width)
  height <- as.integer(height)
  if (length(width) != 1L || length(height) != 1L || is.na(width) || is.na(height) ||
      width < 1L || height < 1L) {
    stop("width and height must be positive integers.", call. = FALSE)
  }
  if (!is.numeric(ratio) || length(ratio) != 1L || !(ratio > 0)) {
    stop("ratio must be a single number > 0.", call. = FALSE)
  }
  m <- structure(
    list(
      styles = styles, width = width, height = height, ratio = as.numeric(ratio),
      center = c(0, 0), zoom = 0,
      options = list(
        layers = layers, auth_token = auth_token,
        tile_timeout_ms = as.integer(tile_timeout_ms), max_tiles = as.integer(max_tiles),
        tile_concurrency = as.integer(tile_concurrency),
        fail_on_tile_error = isTRUE(fail_on_tile_error)
      )
    ),
    class = "basemapper_map"
  )
  if (!is.null(bounds)) {
    if (!is.null(center) || !is.null(zoom)) {
      stop("Give either bounds or center/zoom, not both.", call. = FALSE)
    }
    return(set_bounds(m, bounds, crs = crs))
  }
  set_view(m, center = center %||% c(0, 0), zoom = zoom %||% 0)
}

#' Change a basemap view
#'
#' `set_view()` sets the centre and/or zoom; `set_bounds()` fits bounds
#' (keeping the view's aspect ratio).
#'
#' @param m A [basemap()] object.
#' @param center `c(longitude, latitude)`, or `NULL` to keep it.
#' @param zoom MapLibre zoom, or `NULL` to keep it.
#' @inheritParams basemap
#' @return The modified `basemapper_map`.
#' @export
set_view <- function(m, center = NULL, zoom = NULL) {
  stopifnot(inherits(m, "basemapper_map"))
  if (!is.null(center)) {
    if (!is.numeric(center) || length(center) != 2L) {
      stop("center must be c(longitude, latitude).", call. = FALSE)
    }
    m$center <- as.numeric(center)
  }
  if (!is.null(zoom)) {
    if (!is.numeric(zoom) || length(zoom) != 1L || zoom < 0 || zoom > 22) {
      stop("zoom must be a single number between 0 and 22.", call. = FALSE)
    }
    m$zoom <- as.numeric(zoom)
  }
  m
}

#' @rdname set_view
#' @export
set_bounds <- function(m, bounds, crs = NULL) {
  stopifnot(inherits(m, "basemapper_map"))
  b <- bounds_3857(bounds, crs)
  v <- .Call("wrap__fit_bounds_raw", as.numeric(b), m$width, m$height, PACKAGE = "basemapper")
  m$center <- v[1:2]
  m$zoom   <- min(max(v[3], 0), 22)
  m
}

#' The extent of a basemap view
#'
#' @param m A [basemap()] object.
#' @param lonlat Return degrees (EPSG:4326) instead of EPSG:3857 metres.
#' @return Named numeric `c(xmin, ymin, xmax, ymax)`.
#' @export
basemap_bounds <- function(m, lonlat = FALSE) {
  stopifnot(inherits(m, "basemapper_map"))
  b <- .Call("wrap__bbox_from_center_raw", m$center[1], m$center[2], m$zoom,
             m$width, m$height, PACKAGE = "basemapper")
  names(b) <- c("xmin", "ymin", "xmax", "ymax")
  if (!lonlat) return(b)
  bb <- sf::st_bbox(sf::st_transform(sf::st_as_sfc(sf::st_bbox(b, crs = sf::st_crs(3857))), 4326))
  c(xmin = unname(bb["xmin"]), ymin = unname(bb["ymin"]),
    xmax = unname(bb["xmax"]), ymax = unname(bb["ymax"]))
}

#' @export
print.basemapper_map <- function(x, ...) {
  n <- length(x$styles)
  cat(sprintf(
    "<basemapper_map> %d style%s, %d x %d @ %gx (%d x %d px), center (%.5f, %.5f), zoom %.3f\n",
    n, if (n == 1L) "" else "s", x$width, x$height, x$ratio,
    as.integer(round(x$width * x$ratio)), as.integer(round(x$height * x$ratio)),
    x$center[1], x$center[2], x$zoom
  ))
  invisible(x)
}

#' Render a basemap view
#'
#' * `render_png()` writes or returns a PNG.
#' * `render_svg()` writes or returns a vector SVG document (`width` x
#'   `height` user units; `ratio` does not apply): fills and lines are paths,
#'   labels are glyph outlines.
#' * `render_raster()` returns a [terra::rast()] (EPSG:3857, layers red,
#'   green, blue, alpha).
#' * `render_array()` returns an integer array `[height, width, 4]` (RGBA,
#'   0--255) with the extent as attributes, like [render_basemap_raw()].
#'
#' Image sizes are `width * ratio` by `height * ratio` device pixels.
#'
#' @param m A [basemap()] object.
#' @param file Output path, or `NULL` to return the result.
#' @param text For `render_svg()`: `"editable"` writes labels as `<text>`
#'   elements (editable, selectable) at MapLibre's glyph positions, halos as
#'   strokes, with the style's fonts embedded, and falls back to `"bitmap"`
#'   for labels whose font file is not available; `"bitmap"` draws every
#'   label from the style's own glyph bitmaps, thresholded by SVG filters as
#'   MapLibre's shader does (MapLibre's look at any zoom, not editable);
#'   `"paths"` writes glyph outlines traced from those bitmaps (plain paths).
#' @param embed_fonts For `render_svg()`: `"subset"` (only the glyphs used),
#'   `"full"` (whole fonts, for retyping labels) or `"none"` (fonts by name
#'   only). Fonts come from `font_dirs`, the `BASEMAPPER_FONT_DIRS` folders, a
#'   local cache, then Google Fonts for open fonts; others are referenced by
#'   name.
#' @param font_dirs For `render_svg()`: folders of .ttf / .otf files to use
#'   first.
#' @return `render_png()`: the PNG as a raw vector (invisibly when written).
#'   `render_svg()`: the SVG as a string (invisibly when written).
#' @export
render_png <- function(m, file = NULL) {
  px  <- render_pixels(m)
  png <- .Call("wrap__encode_png_raw", px$pixels, px$width, px$height, PACKAGE = "basemapper")
  if (is.null(file)) return(png)
  writeBin(png, file)
  invisible(png)
}

#' @rdname render_png
#' @export
render_svg <- function(m, file = NULL, text = c("editable", "bitmap", "paths"),
                       embed_fonts = c("subset", "full", "none"), font_dirs = NULL) {
  text <- match.arg(text)
  embed_fonts <- match.arg(embed_fonts)
  stopifnot(inherits(m, "basemapper_map"))
  o <- m$options
  svg <- .Call(
    "wrap__render_svg_raw",
    as.numeric(basemap_bounds(m)), m$width, m$height, m$styles,
    NULL, o$tile_timeout_ms, o$max_tiles,
    if (is.null(o$layers)) NULL else as.character(o$layers),
    if (is.null(o$auth_token)) NULL else as.character(o$auth_token),
    o$fail_on_tile_error, o$tile_concurrency,
    text, embed_fonts,
    if (is.null(font_dirs)) NULL else normalizePath(as.character(font_dirs), mustWork = FALSE),
    PACKAGE = "basemapper"
  )
  if (is.null(file)) return(svg)
  writeLines(svg, file, useBytes = TRUE, sep = "")
  invisible(svg)
}

#' @rdname render_png
#' @export
render_array <- function(m) {
  px <- render_pixels(m)
  a  <- pixels_to_array(px)
  b  <- basemap_bounds(m)
  attr(a, "xmin") <- unname(b["xmin"])
  attr(a, "ymin") <- unname(b["ymin"])
  attr(a, "xmax") <- unname(b["xmax"])
  attr(a, "ymax") <- unname(b["ymax"])
  attr(a, "crs_epsg") <- 3857L
  a
}

#' @rdname render_png
#' @export
render_raster <- function(m) {
  px <- render_pixels(m)
  b  <- basemap_bounds(m)
  r  <- terra::rast(
    nrows = px$height, ncols = px$width, nlyrs = 4L,
    xmin = unname(b["xmin"]), xmax = unname(b["xmax"]),
    ymin = unname(b["ymin"]), ymax = unname(b["ymax"]),
    crs = "EPSG:3857"
  )
  # Pixels are row-major RGBA from the top-left: one column per band.
  v <- matrix(as.integer(px$pixels), ncol = 4L, byrow = TRUE)
  terra::values(r) <- v
  names(r) <- c("red", "green", "blue", "alpha")
  terra::RGB(r) <- 1:3
  r
}

# ── Internals ─────────────────────────────────────────────────────────────────

# One style or a vector / list of styles as character strings.
style_strings <- function(style) {
  styles <- if (is.list(style)) style else as.list(style)
  styles <- vapply(styles, function(s) as.character(s)[1L], character(1))
  if (length(styles) == 0L || anyNA(styles)) stop("At least one style is required.", call. = FALSE)
  unname(styles)
}

bounds_3857 <- function(bounds, crs = NULL) {
  norm <- normalize_bbox(bounds, crs)
  v <- norm$vals
  if (!is.na(norm$crs_obj$epsg) && norm$crs_obj$epsg == 3857L) {
    return(unname(v[c("xmin", "ymin", "xmax", "ymax")]))
  }
  unname(reproject_bbox_to_3857(v[["xmin"]], v[["ymin"]], v[["xmax"]], v[["ymax"]],
                                from_crs = norm$crs_obj))
}

render_pixels <- function(m) {
  stopifnot(inherits(m, "basemapper_map"))
  render_styles_3857(basemap_bounds(m), m$width, m$height, m$styles, m$ratio,
                     zoom = NULL, options = m$options)
}

# Render styles over an EPSG:3857 bbox; width/height in CSS px.
render_styles_3857 <- function(bbox_3857, width, height, styles, ratio,
                               zoom = NULL, options = list()) {
  o <- utils::modifyList(
    list(layers = NULL, auth_token = NULL, tile_timeout_ms = 10000L, max_tiles = 256L,
         tile_concurrency = 16L, fail_on_tile_error = FALSE),
    options
  )
  raw <- .Call(
    "wrap__render_raw",
    as.numeric(bbox_3857), as.integer(width), as.integer(height), style_strings(styles),
    as.numeric(ratio),
    if (is.null(zoom)) NULL else as.integer(zoom),
    as.integer(o$tile_timeout_ms), as.integer(o$max_tiles),
    if (is.null(o$layers)) NULL else as.character(o$layers),
    if (is.null(o$auth_token)) NULL else as.character(o$auth_token),
    isTRUE(o$fail_on_tile_error), as.integer(o$tile_concurrency),
    PACKAGE = "basemapper"
  )
  w <- attr(raw, "width")
  h <- attr(raw, "height")
  attributes(raw) <- NULL
  list(pixels = raw, width = w, height = h)
}

pixels_to_array <- function(px) {
  a <- array(as.integer(px$pixels), dim = c(4L, px$width, px$height))
  aperm(a, c(3L, 2L, 1L))  # [height, width, channel]
}
