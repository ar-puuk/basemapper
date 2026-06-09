test_that("raster_provider returns parseable JSON with correct tile URL", {
  url <- "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
  style <- jsonlite::fromJSON(raster_provider(url), simplifyVector = FALSE)
  expect_equal(style$version, 8L)
  expect_equal(style$sources[["raster-source"]]$tiles[[1]], url)
  expect_equal(style$layers[[1]]$type, "raster")
})

test_that("esri_vector_provider constructs correct URL", {
  base <- "https://example.com/VectorTileServer"
  expect_equal(
    esri_vector_provider(base),
    "https://example.com/VectorTileServer/resources/styles/root.json"
  )
})

test_that("esri_vector_provider strips trailing slash without double-slash", {
  result <- esri_vector_provider("https://example.com/VectorTileServer/")
  expect_equal(result, "https://example.com/VectorTileServer/resources/styles/root.json")
  expect_false(grepl("//resources", result))
})

test_that("vector_provider with paint produces correct layer entries", {
  style <- jsonlite::fromJSON(
    vector_provider("https://x.com/{z}/{x}/{y}.mvt",
                    paint = list("fill-color" = "#e8e0d8", "line-color" = "#aaa")),
    simplifyVector = FALSE
  )
  fill_layer <- Filter(function(l) l$type == "fill", style$layers)[[1]]
  line_layer <- Filter(function(l) l$type == "line", style$layers)[[1]]
  expect_equal(fill_layer$paint[["fill-color"]], "#e8e0d8")
  expect_equal(line_layer$paint[["line-color"]], "#aaa")
})

test_that("vector_provider unknown paint key triggers warning", {
  expect_warning(
    vector_provider("https://x.com/{z}/{x}/{y}.mvt", paint = list("circle-radius" = 5)),
    "circle-radius"
  )
})

test_that("vector_provider with empty paint uses default grey values", {
  style <- jsonlite::fromJSON(
    vector_provider("https://x.com/{z}/{x}/{y}.mvt"),
    simplifyVector = FALSE
  )
  fill_layer <- Filter(function(l) l$type == "fill", style$layers)[[1]]
  expect_false(is.null(fill_layer$paint[["fill-color"]]))
})

test_that("esri_raster_provider returns parseable JSON with tileSize 256", {
  url <- "https://example.com/MapServer/tile/{z}/{y}/{x}"
  style <- jsonlite::fromJSON(esri_raster_provider(url), simplifyVector = FALSE)
  expect_equal(style$version, 8L)
  expect_equal(style$sources[["esri-raster-source"]]$tileSize, 256L)
  expect_equal(style$sources[["esri-raster-source"]]$tiles[[1]], url)
})
