STYLE_JSON <- '{"version":8,"sources":{"s":{"type":"raster","tiles":["http://t/{z}/{x}/{y}.png"]}},"layers":[{"id":"water","type":"raster","source":"s"},{"id":"roads","type":"raster","source":"s"},{"id":"labels","type":"raster","source":"s"}]}'

test_that("list_layers returns a data frame of ids and types", {
  l <- list_layers(STYLE_JSON)
  expect_s3_class(l, "data.frame")
  expect_named(l, c("id", "type", "feature"))
  expect_equal(l$id, c("water", "roads", "labels"))
  expect_equal(l$type, rep("raster", 3))
})

test_that("list_layers keeps style (draw) order", {
  st <- paste0(
    '{"version":8,"sources":{"s":{"type":"raster","tiles":["http://x/{z}/{x}/{y}.png"]}},',
    '"layers":[{"id":"zeta","type":"raster","source":"s"},',
    '{"id":"alpha","type":"background"}]}'
  )
  l <- list_layers(st)
  expect_equal(l$id, c("zeta", "alpha"))
  expect_equal(l$type, c("raster", "background"))
})

test_that("list_layers reports a bad style", {
  expect_error(list_layers("{not json"))
})
