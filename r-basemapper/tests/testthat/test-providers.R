json <- function(s) jsonlite::fromJSON(s, simplifyVector = FALSE)

test_that("tile_style builds a raster style for a tile URL", {
  url <- "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
  style <- json(tile_style(url, maxzoom = 19, attribution = "(c) OSM"))
  src <- style$sources$tiles
  expect_equal(style$version, 8L)
  expect_equal(src$type, "raster")
  expect_equal(src$tiles[[1]], url)
  expect_equal(src$tileSize, 256L)
  expect_equal(src$maxzoom, 19L)
  expect_equal(src$attribution, "(c) OSM")
  expect_equal(style$layers[[1]]$type, "raster")
})

test_that("tile_style honours scheme and tile_size", {
  src <- json(tile_style("https://t/{z}/{x}/{y}.jpg", scheme = "tms", tile_size = 512))$sources$tiles
  expect_equal(src$scheme, "tms")
  expect_equal(src$tileSize, 512L)
})

test_that("tile_style reads ArcGIS MapServer URLs", {
  base <- "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer"
  src <- json(tile_style(paste0(base, "/")))$sources$tiles
  expect_equal(src$tiles[[1]], paste0(base, "/tile/{z}/{y}/{x}"))
  expect_error(tile_style(base, type = "vector"), "raster")
})

test_that("tile_style draws vector source layers with a default style", {
  style <- json(tile_style("https://x.com/{z}/{x}/{y}.mvt", source_layers = c("land", "roads"),
                           paint = list("fill-color" = "#e8e0d8", "line-width" = 2)))
  ids <- vapply(style$layers, `[[`, "", "id")
  expect_equal(ids, c("land-fill", "roads-fill", "land-line", "roads-line", "land-circle", "roads-circle"))
  expect_equal(style$layers[[1]]$paint[["fill-color"]], "#e8e0d8")
  expect_equal(style$layers[[3]]$paint[["line-width"]], 2L)
  expect_equal(style$layers[[1]][["source-layer"]], "land")
})

test_that("tile_style warns about paint keys that match no layer", {
  expect_warning(
    tile_style("https://x.com/{z}/{x}/{y}.pbf", source_layers = "land", paint = list("text-color" = "#000")),
    "text-color"
  )
})

test_that("tile_style validates type and reports missing styles", {
  expect_error(tile_style("https://x.com/{z}/{x}/{y}.png", type = "imagery"), "raster")
  expect_error(tile_style("no/such/style.json"), "not a style")
})

test_that("a local style file gets absolute URLs and works as a style", {
  path <- tempfile(fileext = ".json")
  writeLines(jsonlite::toJSON(list(
    version = 8,
    sources = list(s = list(type = "raster", tiles = list("tiles/{z}/{x}/{y}.png"))),
    layers = list(list(id = "r", type = "raster", source = "s"))
  ), auto_unbox = TRUE), path)
  tile <- json(tile_style(path))$sources$s$tiles[[1]]
  expect_match(tile, "^file://")
  expect_match(tile, "/tiles/\\{z\\}/\\{x\\}/\\{y\\}\\.png$")
  expect_equal(list_layers(path)$id, "r")
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

test_that("wms_style and wmts_style build raster styles", {
  tile <- json(wms_style("https://x.com/wms", layers = "a b"))$sources[[1]]$tiles[[1]]
  expect_match(tile, "LAYERS=a%20b", fixed = TRUE)
  expect_match(tile, "BBOX={bbox-epsg-3857}", fixed = TRUE)
  expect_match(tile, "&CRS=EPSG:3857", fixed = TRUE)
  tile <- json(wms_style("https://x.com/wms", "a", version = "1.1.1"))$sources[[1]]$tiles[[1]]
  expect_match(tile, "&SRS=EPSG:3857", fixed = TRUE)
  tile <- json(wmts_style("https://x.com/wmts", "lyr"))$sources[[1]]$tiles[[1]]
  expect_match(tile, "LAYER=lyr", fixed = TRUE)
})
