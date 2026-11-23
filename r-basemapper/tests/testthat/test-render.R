# The functions shared with Python (and the Rust core) use the same argument
# names; these tests pin them so the languages stay in step.

shared <- c("style", "pixel_ratio", "layers", "auth_token", "fetch")

test_that("basemap() has the shared arguments", {
  f <- names(formals(basemap))
  expect_equal(f[1:4], c("style", "width", "height", "..."))
  expect_true(all(c(shared, "center", "zoom", "bounds", "crs") %in% f))
})

test_that("render_basemap_raw() has the shared arguments", {
  f <- names(formals(render_basemap_raw))
  expect_equal(f[1:5], c("bounds", "width", "height", "style", "..."))
  expect_true(all(c(shared, "crs", "tile_zoom") %in% f))
})

test_that("plot integrations have the shared arguments", {
  for (fn in list(geom_basemap, tm_basemap)) {
    f <- names(formals(fn))
    expect_equal(f[1:2], c("style", "..."))
    expect_true(all(c(shared, "tile_zoom", "alpha") %in% f))
  }
  expect_true(all(c("width", "height") %in% names(formals(tm_basemap))))
})

test_that("fetch_options() validates and maps to the core's settings", {
  f <- fetch_options()
  expect_s3_class(f, "basemapper_fetch_options")
  expect_equal(basemapper:::fetch_core(f),
               list(tile_timeout_ms = 10000L, max_tiles = 256L, tile_concurrency = 16L,
                    fail_on_tile_error = FALSE))
  expect_equal(basemapper:::fetch_core(fetch_options(timeout = 2.5, on_error = "fail"))$tile_timeout_ms, 2500L)
  expect_true(basemapper:::fetch_core(fetch_options(on_error = "fail"))$fail_on_tile_error)
  expect_error(fetch_options(timeout = 0), class = "basemapper_validation_error")
  expect_error(fetch_options(max_tiles = 1.5), class = "basemapper_validation_error")
  expect_error(fetch_options(concurrency = 0), class = "basemapper_validation_error")
  expect_error(fetch_options(on_error = "ignore"))
  expect_output(print(f), "timeout 10s")
  expect_error(basemap("x", fetch = list(timeout = 1)), class = "basemapper_validation_error")
})

test_that("the replaced fetch arguments still work, with a deprecation warning", {
  rlang::local_options(lifecycle_verbosity = "warning")
  w <- testthat::capture_warnings(m <- basemap("x", tile_timeout_ms = 2000L, max_tiles = 12L))
  expect_length(w, 2)
  expect_match(w[1], "fetch_options(timeout = ...)` (in seconds)", fixed = TRUE)
  expect_equal(m$fetch$timeout, 2)
  expect_equal(m$fetch$max_tiles, 12L)
  expect_equal(m$options$tile_timeout_ms, 2000L)
  expect_length(testthat::capture_warnings(m <- basemap("x", fail_on_tile_error = TRUE, tile_concurrency = 3L)), 2)
  expect_equal(m$fetch$on_error, "fail")
  expect_equal(m$fetch$concurrency, 3L)
  expect_warning(l <- geom_basemap("x", max_tiles = 9L))
  expect_equal(l$geom_params$fetch$max_tiles, 9L)
})

test_that("optional arguments must be named; unknown ones are errors", {
  expect_error(basemap("x", 800, 600, c(-111, 40)), "must be named", class = "basemapper_validation_error")
  expect_error(basemap("x", zoo = 3), "Unknown argument", class = "basemapper_validation_error")
  expect_error(render_basemap_raw(c(0, 0, 1, 1), 10, 10, "x", 4326), "after `style`")
  expect_error(tm_basemap("x", tile_zom = 3), "Unknown argument")
})

test_that("core errors carry their kind as a class", {
  bad <- basemap("{not json", 64, 64, center = c(0, 0), zoom = 2)
  expect_error(render_array(bad), class = "basemapper_style_error")
  expect_error(render_array(bad), class = "basemapper_error")
  s <- '{"version":8,"sources":{},"layers":[{"id":"bg","type":"background"}]}'
  expect_error(basemapper:::render_styles_3857(c(-1e6, -1e6, 1e6, 1e6), 8, 8, s, 1, tile_zoom = 99L),
               "tile_zoom must be between", class = "basemapper_validation_error")
  # A message holding braces (a tile URL) comes through intact.
  expect_error(basemapper:::abort_basemapper("https://t/{z}/{x}/{y}.png"), "{z}/{x}/{y}", fixed = TRUE)
})

test_that("EPSG:4326 and 3857 bounds need no sf, and match sf's result", {
  b <- c(-111.95, 40.70, -111.80, 40.80)
  mine <- basemapper:::reproject_bbox_to_3857(b[1], b[2], b[3], b[4], 4326L)
  expect_equal(basemapper:::reproject_bbox_to_3857(1, 2, 3, 4, "EPSG:3857"), c(xmin = 1, ymin = 2, xmax = 3, ymax = 4))
  expect_identical(basemapper:::simple_epsg(NULL), 4326L)
  expect_identical(basemapper:::simple_epsg("epsg:3857"), 3857L)
  expect_true(is.na(basemapper:::simple_epsg(2163)))
  skip_if_not_installed("sf")
  ref <- sf::st_bbox(sf::st_transform(sf::st_as_sfc(sf::st_bbox(
    c(xmin = b[1], ymin = b[2], xmax = b[3], ymax = b[4]), crs = sf::st_crs(4326))), 3857))
  expect_equal(unname(mine), unname(as.numeric(ref)), tolerance = 1e-9)
  # Round trip through get_bounds(lonlat = TRUE), which no longer uses sf.
  m <- basemap("x", 800, 600, bounds = b)
  ll <- get_bounds(m, lonlat = TRUE)
  expect_equal(unname(get_center(m)), unname(c((ll[1] + ll[3]) / 2, get_center(m)[2])), tolerance = 1e-9)
})

test_that("render functions take `path`", {
  expect_equal(names(formals(render_png)), c("m", "path"))
  expect_equal(names(formals(render_svg))[1:2], c("m", "path"))
})

test_that("geom_basemap returns a ggplot2 layer", {
  skip_if_not_installed("ggplot2")
  layer <- geom_basemap("https://demotiles.maplibre.org/style.json")
  expect_s3_class(layer, "LayerInstance")
})

# A style that renders offline (a background) and credits a source.
credited_style <- function(credit = "<b>&copy; Test</b>") {
  paste0('{"version":8,"sources":{"s":{"type":"raster","tiles":["http://127.0.0.1:9/{z}/{x}/{y}.png"],',
         '"attribution":"', credit, '"}},"layers":[',
         '{"id":"bg","type":"background","paint":{"background-color":"#dddddd"}},',
         '{"id":"r","type":"raster","source":"s"}]}')
}

test_that("attribution_text resolves TRUE, FALSE and custom text", {
  s <- credited_style()
  expect_equal(basemapper:::attribution_text(TRUE, s), "\u00a9 Test")
  expect_equal(basemapper:::attribution_text(FALSE, s), "")
  expect_equal(basemapper:::attribution_text("Mine", s), "Mine")
  expect_error(basemapper:::attribution_text(1, s), "attribution must be")
})

test_that("attribution_text caches style sets longer than a variable name", {
  # Inline style JSON (e.g. a labels-only basemap_provider()) is often over
  # the 10000 bytes R allows a name; the cache must not use it as one.
  long <- credited_style(paste0("&copy; Long", strrep(" ", 12000)))
  expect_equal(basemapper:::attribution_text(TRUE, c(long, credited_style())), "© Long | © Test")
  expect_equal(basemapper:::attribution_text(TRUE, c(long, credited_style())), "© Long | © Test")
})

test_that("geom_basemap draws the attribution unless disabled", {
  skip_if_not_installed("ggplot2")
  skip_if_not_installed("sf")
  pts <- sf::st_sf(geometry = sf::st_sfc(sf::st_point(c(-111.9, 40.7)), sf::st_point(c(-111.8, 40.8)), crs = 4326))
  draw <- function(...) {
    p <- ggplot2::ggplot(pts) + geom_basemap(credited_style(), ...) + ggplot2::geom_sf() + ggplot2::coord_sf()
    f <- tempfile(fileext = ".png")
    grDevices::png(f, 300, 200)
    on.exit({ grDevices::dev.off(); unlink(f) })
    print(p)
    grid::grid.force()
    grid::grid.ls(print = FALSE)$name
  }
  expect_true(any(grepl("basemap_attribution", draw())))
  expect_false(any(grepl("basemap_attribution", draw(attribution = FALSE))))
})

test_that("tm_basemap adds tm_credits unless disabled", {
  skip_if_not_installed("tmap")
  with_credit <- tm_basemap(credited_style())
  without <- tm_basemap(credited_style(), attribution = FALSE)
  expect_gt(length(with_credit), length(without))
})

test_that("plot() draws a view, and knitr shows it as an image at CSS size", {
  m <- basemap(credited_style(), 16, 8, center = c(0, 0), zoom = 1, pixel_ratio = 2)
  f <- tempfile(fileext = ".png")
  grDevices::png(f, 64, 32)
  expect_invisible(plot(m))
  grDevices::dev.off()
  expect_gt(file.size(f), 0)

  skip_if_not_installed("knitr")
  img <- knitr::knit_print(m, options = list(fig.path = file.path(tempdir(), "knit-"), label = "chunk"))
  expect_s3_class(img, "knit_image_paths")
  expect_true(file.exists(img[[1]]))
  expect_equal(attr(img, "dpi"), 192)  # 32 x 16 device px, shown at 16 x 8 CSS px
  skip_if_not_installed("png")
  expect_equal(dim(png::readPNG(img[[1]]))[1:2], c(16L, 32L))
})
