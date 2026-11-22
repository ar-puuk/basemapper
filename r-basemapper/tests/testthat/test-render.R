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
