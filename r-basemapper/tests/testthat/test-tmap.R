test_that("tm_basemap is a function with expected formals", {
  expect_true(is.function(tm_basemap))
  fn <- formals(tm_basemap)
  expect_true("style_input" %in% names(fn))
  expect_true("alpha"       %in% names(fn))
  expect_true("layers"      %in% names(fn))
})

test_that("tm_basemap returns a tmap object with tm_basemap_rust dispatch class", {
  skip_if_not_installed("tmap")
  el <- tm_basemap("https://demotiles.maplibre.org/style.json")
  expect_s3_class(el, "tmap")
  # S3 dispatch for our render methods requires this subclass on the element.
  first_elem <- el[[1]]
  expect_true("tm_basemap_rust" %in% class(first_elem))
})

test_that("tmapGridAuxPrepare and tmapGridAuxPlot S3 methods are registered", {
  skip_if_not_installed("tmap")
  expect_true(
    exists("tmapGridAuxPrepare.tm_basemap_rust",
           envir = asNamespace("basemapper"), inherits = FALSE)
  )
  expect_true(
    exists("tmapGridAuxPlot.tm_basemap_rust",
           envir = asNamespace("basemapper"), inherits = FALSE)
  )
})

test_that("tm_basemap render-phase integration works with valid sf shape", {
  skip_if_not_installed("tmap")
  skip_if_not_installed("sf")
  skip_if_not_installed("terra")
  skip_on_cran()
  skip_if_offline()

  nc    <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
  style <- raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")

  map <- tm_basemap(style) + tmap::tm_shape(nc) + tmap::tm_sf()
  expect_no_error(tmap::tmap_grob(map))
})
