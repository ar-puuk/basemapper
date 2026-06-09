# Scenario 5 & 6 smoke-test stubs.
# Full network tests require a running GPU; these validate the R wrapper logic.

test_that("render_basemap_raw signature is callable", {
  # Verify the function exists and accepts the expected parameters.
  expect_true(is.function(render_basemap_raw))
  formals_names <- names(formals(render_basemap_raw))
  expect_true("bbox_3857"   %in% formals_names)
  expect_true("style_input" %in% formals_names)
  expect_true("layers"      %in% formals_names)
})

test_that("geom_basemap returns a ggplot2 layer", {
  skip_if_not_installed("ggplot2")
  layer <- geom_basemap("https://demotiles.maplibre.org/style.json")
  expect_s3_class(layer, "LayerInstance")
})
