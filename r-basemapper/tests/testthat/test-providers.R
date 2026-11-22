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

test_that("list_providers is a table of forms", {
  p <- list_providers()
  expect_s3_class(p, "data.frame")
  expect_named(p, c("name", "vector", "raster", "api_key_env", "attribution"))
  expect_true(nrow(p) >= 40)
  row <- function(n) p[p$name == n, ]
  expect_true(row("Esri.WorldGrayCanvas")$vector && row("Esri.WorldGrayCanvas")$raster)
  expect_false(row("OpenFreeMap.Liberty")$raster)
  expect_equal(row("CartoDB.Positron")$api_key_env, "CARTO_API_KEY")
  expect_true(is.na(row("OpenFreeMap.Liberty")$api_key_env))
  expect_true(all(list_providers("vector")$vector))
  expect_true(all(list_providers("raster")$raster))
})

test_that("basemap_provider resolves every form of every basemap", {
  p <- list_providers()
  for (i in seq_len(nrow(p))) {
    for (kind in c("vector", "raster")[c(p$vector[i], p$raster[i])]) {
      s <- basemap_provider(p$name[i], type = kind, api_key = "KEY")
      expect_type(s, "character")
      expect_length(s, 1L)
    }
  }
})

test_that("basemap_provider picks forms, reads keys and suggests names", {
  expect_match(basemap_provider("OpenFreeMap.Positron"), "/styles/positron$")
  expect_equal(basemap_provider("openfreemap.positron"), basemap_provider("OpenFreeMap.Positron"))
  expect_match(basemap_provider("Carto.Positron", api_key = "K"), "positron-gl-style/style.json?key=K", fixed = TRUE)
  src <- json(basemap_provider("CartoDB.Positron", type = "raster", api_key = "K"))$sources$raster
  expect_length(src$tiles, 4L)
  expect_match(src$attribution, "CARTO")
  expect_error(basemap_provider("Esri.WorldImagery", type = "vector"), "only as raster tiles")
  expect_error(basemap_provider("CartoDB.Positon"), "did you mean CartoDB.Positron")
  withr_env <- Sys.getenv("MAPTILER_API_KEY", NA)
  on.exit(if (is.na(withr_env)) Sys.unsetenv("MAPTILER_API_KEY") else Sys.setenv(MAPTILER_API_KEY = withr_env))
  Sys.unsetenv("MAPTILER_API_KEY")
  expect_error(basemap_provider("MapTiler.Streets"), "MAPTILER_API_KEY")
  Sys.setenv(MAPTILER_API_KEY = "ENVKEY")
  expect_match(basemap_provider("MapTiler.Streets"), "key=ENVKEY$")
  tiles <- json(basemap_provider("Esri.WorldGrayCanvas", type = "raster", tiles = "https://x/{z}/{x}/{y}.png"))
  expect_equal(tiles$sources$raster$tiles[[1]], "https://x/{z}/{x}/{y}.png")
})

test_that("providers completes names, and names work as style", {
  expect_equal(providers$CartoDB.Positron, "CartoDB.Positron")
  expect_equal(providers$OpenTopoMap, "OpenTopoMap")
  expect_equal(list_layers(providers$Esri.WorldImagery)$id, "raster")
})

test_that("tile_cache moves, turns off and clears the cache", {
  before <- tile_cache()
  on.exit(tile_cache(if (is.null(before)) FALSE else before))
  d <- tempfile("tiles")
  expect_equal(tile_cache(d), d)
  expect_null(tile_cache(FALSE))
  expect_equal(tile_cache(d, clear = TRUE), d)
  expect_match(before, "basemapper", fixed = TRUE)
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

test_that("list_layers reports each layer's feature", {
  l <- list_layers("Esri.WorldImagery")
  expect_named(l, c("id", "type", "feature"))
  expect_equal(l$feature, "other")
})

test_that("basemap_provider labels pick raster tiles", {
  tiles <- function(name, labels) {
    json(basemap_provider(name, type = "raster", api_key = "K", labels = labels))$sources$raster$tiles[[1]]
  }
  expect_match(tiles("CartoDB.Positron", FALSE), "/light_nolabels/", fixed = TRUE)
  expect_match(tiles("CartoDB.Positron", "only"), "/light_only_labels/", fixed = TRUE)
  expect_match(tiles("Esri.WorldImagery", "only"), "World_Boundaries_and_Places", fixed = TRUE)
  expect_error(tiles("OpenStreetMap.Mapnik", FALSE), "labels drawn in")
  expect_error(basemap_provider("Esri.WorldImagery", labels = "some"), "labels must be")
})

test_that("labels and features filter a style", {
  style <- '{"version":8,"sources":{},"layers":[{"id":"bg","type":"background","paint":{"background-color":"#ff0000"}}]}'
  a <- render_array(basemap(style, 8, 8, labels = FALSE))
  expect_equal(as.vector(a[1, 1, ]), c(255L, 0L, 0L, 255L))
  expect_error(render_array(basemap(style, 8, 8, labels = "only")), "its features are background")
  expect_error(render_array(basemap(style, 8, 8, features = "highways")), "unknown feature")
  b <- render_array(basemap(style, 8, 8, features = "background"))
  expect_equal(as.vector(b[1, 1, ]), c(255L, 0L, 0L, 255L))
})

test_that("get_attribution collects sources as plain text", {
  expect_match(get_attribution("Esri.WorldImagery"), "^Tiles \u00a9 Esri")
  style <- paste0('{"version":8,"sources":{"a":{"type":"raster","tiles":["https://t/{z}/{x}/{y}.png"],',
                  '"attribution":"<a href=\'x\'>&copy; A</a>"}},"layers":[{"id":"a","type":"raster","source":"a"}]}')
  expect_equal(get_attribution(c(style, style)), "\u00a9 A")
  expect_match(get_attribution(basemap(c(style, "Esri.WorldImagery"), 8, 8)), "^\u00a9 A \\| Tiles")
  expect_equal(get_attribution('{"version":8,"sources":{},"layers":[]}'), "")
})
