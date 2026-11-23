# The functions shared with Python (and the Rust core) use the same argument
# names; these tests pin them so the languages stay in step.

shared <- c("style", "pixel_ratio", "layers", "auth_token", "tile_timeout_ms", "max_tiles")

test_that("basemap() has the shared arguments", {
  f <- names(formals(basemap))
  expect_true(all(c(shared, "width", "height", "center", "zoom", "bounds", "crs",
                    "tile_concurrency", "fail_on_tile_error") %in% f))
  expect_false(isTRUE(formals(basemap)$fail_on_tile_error))
})

test_that("render_basemap_raw() has the shared arguments", {
  f <- names(formals(render_basemap_raw))
  expect_equal(f[1:4], c("bounds", "width", "height", "style"))
  expect_true(all(c(shared, "crs", "tile_zoom", "tile_concurrency", "fail_on_tile_error") %in% f))
  expect_false(isTRUE(formals(render_basemap_raw)$fail_on_tile_error))
})

test_that("plot integrations have the shared arguments", {
  for (fn in list(geom_basemap, tm_basemap)) {
    f <- names(formals(fn))
    expect_equal(f[1], "style")
    expect_true(all(c(shared, "tile_zoom", "alpha") %in% f))
  }
  expect_true(all(c("width", "height") %in% names(formals(tm_basemap))))
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
