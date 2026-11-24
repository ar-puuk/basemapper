# Maps in a CRS other than Web Mercator (docs/design/api-review.md §4.2,
# D18, D19, D20, D21): the map CRS rule, the warp's alignment and clips.

square_style <- function(lon0, lat0, lon1, lat1, color = "#ff0000") {
  ring <- sprintf("[[%g,%g],[%g,%g],[%g,%g],[%g,%g],[%g,%g]]", lon0, lat0, lon1, lat0, lon1, lat1, lon0, lat1, lon0, lat0)
  paste0(
    '{"version":8,"sources":{"sq":{"type":"geojson","data":{"type":"Polygon","coordinates":[', ring, ']}}},',
    '"layers":[{"id":"bg","type":"background","paint":{"background-color":"#ffffff"}},',
    '{"id":"sq","type":"fill","source":"sq","paint":{"fill-color":"', color, '","fill-antialias":false}}]}'
  )
}

red_centroid <- function(a) {
  hit <- which(a[, , 1] > 200 & a[, , 2] < 60 & a[, , 3] < 60, arr.ind = TRUE)
  expect_gt(nrow(hit), 0)
  c(x = mean(hit[, "col"]) - 0.5, y = mean(hit[, "row"]) - 0.5)
}

test_that("a map in UTM puts things where the projection does", {
  skip_if_not_installed("sf")
  style <- square_style(-111.95, 40.72, -111.85, 40.80)
  m <- basemap(style, center = c(-111.9, 40.76), zoom = 11, width = 400, height = 300, crs = 32612)
  expect_equal(m$.prepared$crs$epsg, 32612L)
  r <- render(m)
  expect_equal(c(r$width, r$height), c(400, 300))
  corners <- sf::sf_project("EPSG:4326", "EPSG:32612",
                            rbind(c(-111.95, 40.72), c(-111.85, 40.72), c(-111.85, 40.80), c(-111.95, 40.80)))
  b <- r$bounds
  want <- c((mean(corners[, 1]) - b[["xmin"]]) / (b[["xmax"]] - b[["xmin"]]) * r$width,
            (b[["ymax"]] - mean(corners[, 2])) / (b[["ymax"]] - b[["ymin"]]) * r$height)
  got <- red_centroid(as_array(r))
  expect_lt(sqrt(sum((got - want)^2)), 1)
  # The scale is the centre's, not read from UTM northings.
  expect_equal(r$scale, 2 * 20037508.342789244 / (512 * 2^11) * cos(40.76 * pi / 180) * 96 / 0.0254,
               tolerance = 2e-3)
})

test_that("the map CRS rule (D18, D19)", {
  skip_if_not_installed("sf")
  utm <- sf::st_as_sfc(sf::st_bbox(c(xmin = 415000, ymin = 4500000, xmax = 435000, ymax = 4520000),
                                   crs = sf::st_crs(32612)))
  s <- square_style(0, 0, 1, 1)
  expect_equal(basemap(s, bounds = utm)$.prepared$crs$epsg, 32612L)                 # a projected layer
  expect_equal(basemap(s, bounds = sf::st_transform(utm, 4326))$.prepared$crs$name, "EPSG:3857")  # lon/lat falls back
  expect_equal(basemap(s, bounds = sf::st_transform(utm, 4326), crs = 4326)$.prepared$crs$name, "EPSG:4326")
  expect_equal(basemap(s, bounds = utm, crs = 3857)$.prepared$crs$name, "EPSG:3857")
  b <- get_bounds(basemap(s, bounds = utm, width = 200, height = 200))
  expect_true(b[["xmin"]] <= 415000 && b[["xmax"]] >= 435000 && b[["ymin"]] <= 4500000 && b[["ymax"]] >= 4520000)
})

test_that("web Mercator and lon/lat need no sf", {
  # Plain numbers and around() go through the core's own conversions.
  m <- basemap(square_style(0, 0, 1, 1), bounds = around(c(-111.9, 40.76), 10, units = "km"))
  expect_equal(m$.prepared$crs$name, "EPSG:3857")
  p <- basemapper:::project(-111.9, 40.76, basemapper:::.CRS_4326, basemapper:::.CRS_3857)
  back <- basemapper:::project(p$x, p$y, basemapper:::.CRS_3857, basemapper:::.CRS_4326)
  expect_equal(c(back$x, back$y), c(-111.9, 40.76), tolerance = 1e-12)
})

test_that("clips: inside, outside, and their extent", {
  skip_if_not_installed("sf")
  county <- sf::st_as_sfc(sf::st_bbox(c(xmin = 415000, ymin = 4500000, xmax = 435000, ymax = 4520000),
                                      crs = sf::st_crs(32612)))
  style <- square_style(-120, 30, -100, 50, color = "#00ff00")
  inside <- basemap(style, clip = county, width = 100, height = 100)
  expect_equal(inside$.prepared$crs$epsg, 32612L)        # an inside clip frames the map
  expect_equal(as_array(render(inside))[50, 50, 4], 255L)
  outside_ <- basemap(style, bounds = sf::st_buffer(county, 10000), clip = outside(county), width = 100, height = 100)
  o <- as_array(render(outside_))
  expect_equal(c(o[50, 50, 4], o[2, 2, 4]), c(0L, 255L))
  no_extent <- basemap(style, clip = outside(county))
  expect_equal(no_extent$.prepared$view$provenance$extent, "default")
  expect_equal(no_extent$.prepared$crs$name, "EPSG:3857")
})

test_that("clips need polygons", {
  skip_if_not_installed("sf")
  pts <- sf::st_sfc(sf::st_point(c(-111.9, 40.7)), crs = 4326)
  expect_error(basemap(square_style(0, 0, 1, 1), bounds = c(-112, 40, -111, 41), clip = pts), "st_buffer")
  expect_error(outside(sf::st_sfc(sf::st_point(c(0, 0)))), "no CRS")
})
