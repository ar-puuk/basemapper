# Styles and their inspection: list_layers(), get_attribution(), the
# catalogue and the tile cache (implemented in the Rust core, shared with
# Python). Offline.

filter_style <- paste0(
  '{"version":8,"sources":{"s":{"type":"vector","tiles":["https://t/{z}/{x}/{y}.pbf"]}},"layers":[',
  '{"id":"bg","type":"background","paint":{"background-color":"#336699"}},',
  '{"id":"water","type":"fill","source":"s","source-layer":"water"},',
  '{"id":"road_major","type":"line","source":"s","source-layer":"transportation"},',
  '{"id":"road_name","type":"symbol","source":"s","source-layer":"transportation_name"}]}'
)

ids <- function(s) list_layers(s)$id

test_that("list_layers() lists a style's layers and previews its filters", {
  rows <- list_layers(filter_style)
  expect_equal(names(rows), c("id", "type", "source_layer", "feature"))
  expect_equal(rows[2, ], data.frame(id = "water", type = "fill", source_layer = "water", feature = "water",
                                     row.names = 2L))
  expect_true(is.na(rows$source_layer[1]))
  expect_equal(ids(style(filter_style, layers = "road_*")), c("road_major", "road_name"))
  expect_equal(ids(style(filter_style, layers = "-road_?ame")), c("bg", "water", "road_major"))
  expect_equal(ids(style(filter_style, features = "labels")), "road_name")
  expect_equal(ids(style(filter_style, features = "-roads")), c("bg", "water"))
  expect_equal(ids(style(filter_style, features = c("roads", "labels"))), "road_name")
  expect_equal(nrow(list_layers(style(filter_style, features = c("water", "labels")))), 0L)
  expect_error(list_layers("{not json"), class = "basemapper_error")
})

test_that("a local style file works anywhere a style does", {
  f <- tempfile(fileext = ".json")
  writeLines('{"version":8,"sources":{"s":{"type":"raster","tiles":["tiles/{z}/{x}/{y}.png"]}},"layers":[{"id":"r","type":"raster","source":"s"}]}', f)
  expect_equal(list_layers(f)$id, "r")
})

test_that("source helpers build styles", {
  t <- style(tiles("https://x.com/{z}/{x}/{y}.mvt", source_layers = c("land", "roads"),
                   paint = list("fill-color" = "#e8e0d8")))
  expect_equal(ids(t), c("land-fill", "roads-fill", "land-line", "roads-line", "land-circle", "roads-circle"))
  expect_equal(ids(tiles("https://x.com/{z}/{x}/{y}.png")), "raster")
  expect_length(ids(wms("https://x.com/wms", "a")), 1L)
  expect_length(ids(wmts("https://x.com/wmts", "lyr")), 1L)
  expect_equal(ids(provider("Esri.WorldImagery")), "raster")   # built locally
  expect_error(style(tiles("https://x.com/{z}/{x}/{y}.png", type = "imagery")), "raster")
  expect_output(print(style(provider("CartoDB.Positron", form = "raster"), key = "k")), "provider\\(CartoDB.Positron\\)")
})

test_that("the catalogue", {
  rows <- list_providers()
  expect_gte(nrow(rows), 40L)
  expect_equal(names(rows), c("name", "vector", "raster", "api_key_env", "attribution"))
  by <- function(n) rows[rows$name == n, ]
  expect_true(by("Esri.WorldGrayCanvas")$vector && by("Esri.WorldGrayCanvas")$raster)
  expect_false(by("OpenFreeMap.Liberty")$raster)
  expect_true(is.na(by("OpenFreeMap.Liberty")$api_key_env))
  expect_equal(by("MapTiler.Streets")$api_key_env, "MAPTILER_API_KEY")
  expect_true(all(list_providers("vector")$vector))
  expect_equal(providers$CartoDB.Positron, "CartoDB.Positron")
  # A catalogue stack is one style of several services.
  expect_gt(nrow(list_layers("UGRC.Lite")), nrow(list_layers(provider("Esri.WorldImagery"))))
})

test_that("get_attribution() takes styles and basemaps", {
  credited <- paste0('{"version":8,"sources":{"a":{"type":"raster","tiles":["https://t/{z}/{x}/{y}.png"],',
                     '"attribution":"<a href=\'x\'>&copy; A</a>"}},"layers":[{"id":"a","type":"raster","source":"a"}]}')
  expect_match(get_attribution("Esri.WorldImagery"), "^Tiles © Esri")
  expect_equal(get_attribution(list(credited, credited)), "© A")
  expect_match(get_attribution(basemap(list(credited, "Esri.WorldImagery"))), "^© A \\| Tiles © Esri")
  expect_equal(get_attribution('{"version":8,"sources":{},"layers":[]}'), "")
})

test_that("tile_cache() moves, clears and turns off the cache", {
  before <- tile_cache()
  on.exit(tile_cache(if (is.null(before)) FALSE else before))
  d <- tempfile("tiles")
  expect_equal(tile_cache(d), path.expand(d))
  expect_null(tile_cache(FALSE))
  expect_equal(tile_cache(d, clear = TRUE), path.expand(d))
})
