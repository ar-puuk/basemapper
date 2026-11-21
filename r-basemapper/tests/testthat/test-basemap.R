positron <- "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json"

test_that("basemap() view: centre/zoom and fitted bounds round-trip", {
  m <- basemap(positron, 800, 600, center = c(-111.8887, 40.7556), zoom = 12.4)
  expect_s3_class(m, "basemapper_map")
  b <- basemap_bounds(m)
  expect_named(b, c("xmin", "ymin", "xmax", "ymax"))
  # 800 CSS px at zoom 12.4 is 14.48 m/px.
  expect_equal(unname((b["xmax"] - b["xmin"]) / 800), 14.482108758, tolerance = 1e-8)

  m2 <- basemap(positron, 800, 600, bounds = b, crs = 3857)
  expect_equal(m2$zoom, 12.4, tolerance = 1e-9)
  expect_equal(m2$center, m$center, tolerance = 1e-9)

  m3 <- set_view(m, zoom = 10)
  expect_equal(m3$zoom, 10)
  expect_output(print(m), "800 x 600 @ 1x")
  expect_error(basemap(positron, center = c(0, 0), zoom = 1, bounds = c(0, 0, 1, 1)))
  expect_error(basemap(positron, ratio = 0))
})

test_that("device_dpi() reads raster device resolution; vector devices get 300", {
  f <- tempfile(fileext = ".png")
  grDevices::png(f, width = 400, height = 300, res = 192)
  expect_equal(basemapper:::device_dpi(), 192)
  grDevices::dev.off()
  g <- tempfile(fileext = ".pdf")
  grDevices::pdf(g)
  expect_equal(basemapper:::device_dpi(), 300)
  grDevices::dev.off()
})

test_that("render_png / render_array / render_raster / render_svg (live tiles)", {
  skip_on_cran()
  skip_if_offline()
  m <- basemap(positron, 200, 150, center = c(-111.8887, 40.7556), zoom = 12, ratio = 2)

  png <- render_png(m)
  expect_type(png, "raw")
  expect_equal(png[2:4], charToRaw("PNG"))
  f <- tempfile(fileext = ".png")
  render_png(m, f)
  expect_true(file.size(f) > 1000)

  a <- render_array(m)
  expect_equal(dim(a), c(300L, 400L, 4L))
  expect_equal(attr(a, "crs_epsg"), 3857L)
  expect_true(all(a[, , 4] == 255L))  # Positron has an opaque background

  r <- render_raster(m)
  expect_s4_class(r, "SpatRaster")
  expect_equal(dim(r), c(300, 400, 4))
  expect_equal(as.vector(terra::values(r)[1, ]), as.numeric(a[1, 1, ]))

  svg <- render_svg(m)
  expect_match(svg, "^<svg")
  expect_match(svg, 'viewBox="0 0 200 150"', fixed = TRUE)
  expect_match(svg, "<text ", fixed = TRUE)          # editable labels
  expect_match(svg, "@font-face", fixed = TRUE)      # open fonts embedded (subset)
  paths <- render_svg(m, text = "paths")
  expect_false(grepl("<text", paths, fixed = TRUE))  # traced outlines
  expect_error(render_svg(m, text = "bitmap"))

  # Two styles stacked: the top (opaque) one wins.
  dark <- "https://basemaps.cartocdn.com/gl/dark-matter-gl-style/style.json"
  s <- render_array(basemap(c(positron, dark), 200, 150, center = c(-111.8887, 40.7556), zoom = 12))
  d <- render_array(basemap(dark, 200, 150, center = c(-111.8887, 40.7556), zoom = 12))
  expect_equal(s, d)
})

test_that("geom_basemap renders natively at ggsave() dpi", {
  skip_on_cran()
  skip_if_offline()
  skip_if_not_installed("ggplot2")
  pts <- sf::st_sf(geometry = sf::st_sfc(
    sf::st_point(c(-111.95, 40.70)), sf::st_point(c(-111.80, 40.80)), crs = 4326))
  p <- ggplot2::ggplot(pts) + geom_basemap(positron) + ggplot2::geom_sf() +
    ggplot2::coord_sf(expand = FALSE) + ggplot2::theme_void()
  f <- tempfile(fileext = ".png")
  ggplot2::ggsave(f, p, width = 4, height = 3, dpi = 200)
  expect_true(file.size(f) > 10000)
})
