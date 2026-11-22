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
    vector_provider("https://x.com/{z}/{x}/{y}.mvt", "land",
                    paint = list("fill-color" = "#e8e0d8", "line-color" = "#aaa")),
    simplifyVector = FALSE
  )
  fill_layer <- Filter(function(l) l$type == "fill", style$layers)[[1]]
  line_layer <- Filter(function(l) l$type == "line", style$layers)[[1]]
  expect_equal(fill_layer$paint[["fill-color"]], "#e8e0d8")
  expect_equal(line_layer$paint[["line-color"]], "#aaa")
  expect_equal(fill_layer[["source-layer"]], "land")
  expect_equal(line_layer[["source-layer"]], "land")
})

test_that("vector_provider unknown paint key triggers warning", {
  expect_warning(
    vector_provider("https://x.com/{z}/{x}/{y}.mvt", "land", paint = list("circle-radius" = 5)),
    "circle-radius"
  )
})

test_that("vector_provider with empty paint uses default grey values", {
  style <- jsonlite::fromJSON(
    vector_provider("https://x.com/{z}/{x}/{y}.mvt", source_layer = "roads"),
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

test_that("basemap_provider resolves every listed provider", {
  p <- list_providers()
  expect_true(length(p) >= 20)
  expect_true(all(c("OpenFreeMap.liberty", "Carto.Positron", "ESRI.WorldImagery") %in% p))
  for (name in p) {
    key <- if (startsWith(name, "MapTiler.")) "KEY" else NULL
    s <- basemap_provider(name, api_key = key)
    expect_type(s, "character")
    expect_length(s, 1L)
  }
  expect_match(basemap_provider("Carto.Positron"), "positron-gl-style")
  expect_error(basemap_provider("MapTiler.Streets"), "api_key")
  expect_error(basemap_provider("Nope"), "list_providers")
})

test_that("wms / wmts / pmtiles providers build raster styles", {
  w <- jsonlite::fromJSON(wms_provider("https://x.com/wms", layers = "a b"), simplifyVector = FALSE)
  tile <- w$sources[[1]]$tiles[[1]]
  expect_match(tile, "LAYERS=a%20b", fixed = TRUE)
  expect_match(tile, "BBOX={xmin},{ymin},{xmax},{ymax}", fixed = TRUE)
  t <- jsonlite::fromJSON(wmts_provider("https://x.com/wmts", "lyr"), simplifyVector = FALSE)
  expect_match(t$sources[[1]]$tiles[[1]], "LAYER=lyr", fixed = TRUE)
  p <- jsonlite::fromJSON(pmtiles_provider("https://x.com/a.pmtiles"), simplifyVector = FALSE)
  expect_equal(p$sources[[1]]$tiles[[1]], "pmtiles://https://x.com/a.pmtiles")
})
