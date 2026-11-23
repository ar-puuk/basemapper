positron <- "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json"

test_that("basemap() view: centre/zoom and fitted bounds round-trip", {
  m <- basemap(positron, 800, 600, center = c(-111.8887, 40.7556), zoom = 12.4)
  expect_s3_class(m, "basemap")
  b <- get_bounds(m)
  expect_named(b, c("xmin", "ymin", "xmax", "ymax"))
  # 800 CSS px at zoom 12.4 is 14.48 m/px.
  expect_equal(unname((b["xmax"] - b["xmin"]) / 800), 14.482108758, tolerance = 1e-8)

  m2 <- basemap(positron, 800, 600, bounds = b, crs = 3857)
  expect_equal(get_zoom(m2), 12.4, tolerance = 1e-9)
  expect_equal(get_center(m2), get_center(m), tolerance = 1e-9)
  expect_named(get_center(m), c("longitude", "latitude"))

  m3 <- set_view(m, zoom = 10)
  expect_equal(get_zoom(m3), 10)
  expect_equal(get_center(m3), get_center(m))
  m4 <- set_bounds(m, b, crs = 3857)
  expect_equal(get_zoom(m4), 12.4, tolerance = 1e-9)

  lonlat <- get_bounds(m, lonlat = TRUE)
  expect_true(lonlat[["xmin"]] < -111.8887 && lonlat[["xmax"]] > -111.8887)

  expect_equal(get_size(m), c(width = 800L, height = 600L))
  # Rounded as the core rounds: halves away from zero.
  expect_equal(get_size(basemap(positron, 5, 3, pixel_ratio = 1.5)), c(width = 8L, height = 5L))

  expect_output(print(m), "800 x 600 @ 1x")
  expect_error(basemap(positron, center = c(0, 0), zoom = 1, bounds = c(0, 0, 1, 1)))
  expect_error(basemap(positron, pixel_ratio = 0))
  expect_error(set_view(m, zoom = 23))
  expect_error(get_bounds(list()), "basemap")
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
  m <- basemap(positron, 200, 150, center = c(-111.8887, 40.7556), zoom = 12, pixel_ratio = 2)

  png <- render_png(m)
  expect_type(png, "raw")
  expect_equal(png[2:4], charToRaw("PNG"))
  f <- tempfile(fileext = ".png")
  render_png(m, path = f)
  expect_true(file.size(f) > 1000)

  a <- render_array(m)
  expect_equal(dim(a), c(300L, 400L, 4L))
  expect_equal(unname(get_size(m)), dim(a)[2:1])
  expect_equal(attr(a, "crs_epsg"), 3857L)
  expect_true(all(a[, , 4] == 255L))  # Positron has an opaque background

  if (requireNamespace("terra", quietly = TRUE)) {
    r <- render_raster(m)
    expect_s4_class(r, "SpatRaster")
    expect_equal(dim(r), c(300, 400, 4))
    expect_equal(as.vector(terra::values(r)[1, ]), as.numeric(a[1, 1, ]))
  }

  svg <- render_svg(m)
  expect_match(svg, "^<svg")
  expect_match(svg, 'viewBox="0 0 200 150"', fixed = TRUE)
  expect_match(svg, "<text ", fixed = TRUE)          # editable labels
  expect_match(svg, "@font-face", fixed = TRUE)      # open fonts embedded (subset)
  paths <- render_svg(m, text = "paths")
  expect_false(grepl("<text", paths, fixed = TRUE))  # traced outlines
  bitmap <- render_svg(m, text = "bitmap")
  expect_match(bitmap, "feComponentTransfer", fixed = TRUE)   # glyph bitmaps + filters
  expect_error(render_svg(m, text = "raster"))

  # Two styles stacked: the top (opaque) one wins.
  dark <- "https://basemaps.cartocdn.com/gl/dark-matter-gl-style/style.json"
  s <- render_array(basemap(c(positron, dark), 200, 150, center = c(-111.8887, 40.7556), zoom = 12))
  d <- render_array(basemap(dark, 200, 150, center = c(-111.8887, 40.7556), zoom = 12))
  expect_equal(s, d)

  # The one-shot call returns the same pixels as the view.
  raw <- render_basemap_raw(get_bounds(m), 200, 150, positron, crs = 3857, pixel_ratio = 2)
  expect_equal(dim(raw), dim(a))
})

test_that("geom_basemap renders natively at ggsave() dpi", {
  skip_on_cran()
  skip_if_offline()
  skip_if_not_installed("ggplot2")
  skip_if_not_installed("sf")
  pts <- sf::st_sf(geometry = sf::st_sfc(
    sf::st_point(c(-111.95, 40.70)), sf::st_point(c(-111.80, 40.80)), crs = 4326))
  p <- ggplot2::ggplot(pts) + geom_basemap(positron) + ggplot2::geom_sf() +
    ggplot2::coord_sf(expand = FALSE) + ggplot2::theme_void()
  f <- tempfile(fileext = ".png")
  ggplot2::ggsave(f, p, width = 4, height = 3, dpi = 200)
  expect_true(file.size(f) > 10000)
})

test_that("the default view fits the world and renders (as MapLibre shows it)", {
  bg <- '{"version":8,"sources":{},"layers":[{"id":"bg","type":"background","paint":{"background-color":"#abcdef"}}]}'
  m <- basemap(bg)  # 800 x 600 at zoom 0: taller than the world
  expect_equal(get_zoom(m), log2(800 / 512))
  expect_equal(unname(get_center(m)), c(0, 0))
  b <- get_bounds(m)
  expect_lte(b[["ymax"]], 20037508.342789244)
  expect_equal(dim(render_array(m)), c(600L, 800L, 4L))
  # A centre near a pole moves so the view stops at it.
  n <- basemap(bg, 800, 600, center = c(10, 84), zoom = 3)
  expect_lt(get_center(n)[["latitude"]], 84)
  expect_equal(get_bounds(n)[["ymax"]], 20037508.342789244)
  expect_output(print(m), "zoom 0.644")
})
