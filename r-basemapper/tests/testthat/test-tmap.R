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

test_that("tmap_panel_inches fits the bbox aspect inside the device minus margins", {
  o <- list(devsize = c(6, 4), outer.margins = c(0.02, 0.02, 0.02, 0.02), nrows = 1, ncols = 1)
  wide <- c(xmin = 0, ymin = 0, xmax = 4, ymax = 1)
  p <- basemapper:::tmap_panel_inches(o, wide)
  expect_equal(p[1], 6 * 0.96)
  expect_equal(p[2], 6 * 0.96 / 4)
  tall <- c(xmin = 0, ymin = 0, xmax = 1, ymax = 2)
  p <- basemapper:::tmap_panel_inches(o, tall)
  expect_equal(p, c(4 * 0.96 / 2, 4 * 0.96))
  o$ncols <- 2
  expect_equal(basemapper:::tmap_panel_inches(o, wide)[1], 3 * 0.96)
})

test_that("tm_basemap renders at the device resolution (tmap_save dpi)", {
  skip_on_cran()
  skip_if_offline()
  skip_if_not_installed("tmap")
  nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
  style <- "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json"
  m <- tmap::tm_shape(nc) + tm_basemap(style) + tmap::tm_borders()
  f <- tempfile(fileext = ".png")
  suppressMessages(tmap::tmap_save(m, f, width = 6, height = 4, units = "in", dpi = 200))
  expect_true(file.exists(f))
  # The cached basemap raster has about panel inches * 200 px columns.
  dt <- basemapper:::.BASEMAPPER_TMAP$bmaps_dts
  expect_true(length(dt) > 0)
  n <- nrow(dt[[length(dt)]][[1]])
  expect_gt(n, (6 * 0.96 * 200) * (6 * 0.96 * 200 / 2.67) * 0.5)
})
