STYLE_JSON <- '{"version":8,"sources":{"s":{"type":"raster","tiles":["http://t/{z}/{x}/{y}.png"]}},"layers":[{"id":"water","type":"raster","source":"s"},{"id":"roads","type":"raster","source":"s"},{"id":"labels","type":"raster","source":"s"}]}'

test_that("list_layers returns a data frame of ids and types", {
  l <- list_layers(STYLE_JSON)
  expect_s3_class(l, "data.frame")
  expect_named(l, c("id", "type", "source_layer", "feature"))
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

filter_style <- paste0(
  '{"version":8,"sources":{"s":{"type":"vector","tiles":["https://t/{z}/{x}/{y}.pbf"]}},"layers":[',
  '{"id":"bg","type":"background","paint":{"background-color":"#336699"}},',
  '{"id":"water","type":"fill","source":"s","source-layer":"water"},',
  '{"id":"road_major","type":"line","source":"s","source-layer":"transportation"},',
  '{"id":"road_name","type":"symbol","source":"s","source-layer":"transportation_name"}]}'
)

test_that("list_layers previews filters and shows source layers", {
  l <- list_layers(filter_style)
  expect_equal(l$source_layer, c(NA, "water", "transportation", "transportation_name"))
  expect_equal(list_layers(filter_style, layers = "road_*")$id, c("road_major", "road_name"))
  expect_equal(list_layers(filter_style, labels = "only")$id, "road_name")
  expect_equal(list_layers(filter_style, features = "-roads")$id, c("bg", "water"))
  expect_equal(nrow(list_layers(filter_style, labels = "only", features = "water")), 0L)
})

test_that("filter names that match no layer warn", {
  bg <- '{"version":8,"sources":{},"layers":[{"id":"bg","type":"background","paint":{"background-color":"#336699"}}]}'
  expect_warning(render_array(basemap(bg, 8, 8, features = "-water")), 'features: "water" matches no layer', fixed = TRUE)
  expect_warning(render_array(basemap(bg, 8, 8, layers = "-label_*")), 'layers: "-label_*" matches no layer', fixed = TRUE)
  expect_no_warning(render_array(basemap(bg, 8, 8, features = "background")))
})
