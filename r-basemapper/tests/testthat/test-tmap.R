test_that("tm_basemap raises informative error when no bbox can be inferred", {
  skip_if_not_installed("tmap")
  skip_if_not_installed("stars")
  expect_error(
    tm_basemap("https://demotiles.maplibre.org/style.json"),
    regexp = "bounding box|bbox"
  )
})

test_that("tm_basemap is a function with expected formals", {
  expect_true(is.function(tm_basemap))
  fn <- formals(tm_basemap)
  expect_true("style_input" %in% names(fn))
  expect_true("alpha"       %in% names(fn))
  expect_true("layers"      %in% names(fn))
})
