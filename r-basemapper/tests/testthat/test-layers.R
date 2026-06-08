STYLE_JSON <- '{"version":8,"sources":{"s":{"type":"raster","tiles":["http://t/{z}/{x}/{y}.png"]}},"layers":[{"id":"water","type":"raster","source":"s"},{"id":"roads","type":"raster","source":"s"},{"id":"labels","type":"raster","source":"s"}]}'

test_that("list_layers returns correct IDs from inline JSON", {
  ids <- list_layers(STYLE_JSON)
  expect_setequal(ids, c("water", "roads", "labels"))
})

test_that("list_layers returns sorted character vector", {
  ids <- list_layers(STYLE_JSON)
  expect_equal(ids, sort(ids))
})

test_that("list_layers prints id/type table to console", {
  out <- capture.output(list_layers(STYLE_JSON))
  expect_true(any(grepl("water", out)))
  expect_true(any(grepl("raster", out)))
})
