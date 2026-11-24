# Bugs of 0.2 that the 0.3.0 redesign fixed (docs/design/api-review.md F1,
# F4), kept as regression tests. Written in phase 0 as expected failures;
# they pass since phase 4.

# A style that draws a red square (10-12 E, 58-60 N) on white, offline.
square_style <- function() {
  sq <- '{"type":"Polygon","coordinates":[[[10,58],[12,58],[12,60],[10,60],[10,58]]]}'
  paste0(
    '{"version":8,"sources":{"sq":{"type":"geojson","data":', sq, '}},"layers":[',
    '{"id":"bg","type":"background","paint":{"background-color":"#ffffff"}},',
    '{"id":"sq","type":"fill","source":"sq","paint":{"fill-color":"#ff0000","fill-antialias":false}}]}'
  )
}

square_sf <- function() {
  sf::st_sf(geometry = sf::st_sfc(
    sf::st_polygon(list(rbind(c(10, 58), c(12, 58), c(12, 60), c(10, 60), c(10, 58)))), crs = 4326
  ))
}

# Centroid (column, row) of the pixels matching `is_colour` in a PNG file.
colour_centroid <- function(path, is_colour) {
  img <- png::readPNG(path)
  hit <- which(is_colour(img[, , 1], img[, , 2], img[, , 3]), arr.ind = TRUE)
  c(x = mean(hit[, "col"]), y = mean(hit[, "row"]))
}

red <- function(r, g, b) r > 0.8 & g < 0.3 & b < 0.3
blue <- function(r, g, b) b > 0.8 & r < 0.3 & g < 0.3

test_that("F1: a basemap lines up with the data on a lon/lat ggplot", {
  skip_if_not_installed("sf")
  skip_if_not_installed("png")
  sq <- square_sf()
  # Europe, in longitude / latitude: the square sits far from the centre.
  frame <- list(ggplot2::coord_sf(crs = 4326, xlim = c(-10, 30), ylim = c(35, 71), expand = FALSE, datum = NA),
                ggplot2::theme_void())
  draw <- function(p) {
    f <- tempfile(fileext = ".png")
    ggplot2::ggsave(f, p, width = 6, height = 6, dpi = 100)
    f
  }
  basemap_png <- draw(ggplot2::ggplot(sq) + basemap(square_style()) + frame)
  data_png <- draw(ggplot2::ggplot(sq) + ggplot2::geom_sf(fill = "#0000ff", colour = NA) + frame)
  offset <- sqrt(sum((colour_centroid(basemap_png, red) - colour_centroid(data_png, blue))^2))
  expect_lt(offset, 2)  # 0.2: about 65 px off, the Web Mercator image stretched
})

test_that("F1: a basemap lines up with the data in a tmap map", {
  skip_if_not_installed("tmap")
  skip_if_not_installed("sf")
  skip_if_not_installed("terra")
  skip_if_not_installed("png")
  sq <- square_sf()
  europe <- sf::st_bbox(c(xmin = -10, ymin = 35, xmax = 30, ymax = 71), crs = sf::st_crs(4326))
  draw <- function(m) {
    f <- tempfile(fileext = ".png")
    suppressMessages(tmap::tmap_save(m, f, width = 6, height = 6, units = "in", dpi = 100))
    f
  }
  layout <- tmap::tm_layout(frame = FALSE, inner.margins = 0, outer.margins = 0)
  basemap_png <- draw(tm_basemap(square_style(), attribution = FALSE) +
                        tmap::tm_shape(sq, bbox = europe) + tmap::tm_borders(col = NA) + layout)
  data_png <- draw(tmap::tm_shape(sq, bbox = europe) + tmap::tm_polygons(fill = "#0000ff", col = NA) + layout)
  offset <- sqrt(sum((colour_centroid(basemap_png, red) - colour_centroid(data_png, blue))^2))
  expect_lt(offset, 2)
})

test_that("F4: a box crossing the date line renders", {
  bg <- '{"version":8,"sources":{},"layers":[{"id":"bg","type":"background","paint":{"background-color":"#123456"}}]}'
  m <- basemap(bg, bounds = c(170, -10, -170, 10), width = 100, height = 50)
  b <- get_bounds(m)
  expect_gt(b[["xmin"]], b[["xmax"]])  # west > east: across the date line
  a <- as_array(render(m))
  expect_equal(dim(a), c(50L, 100L, 4L))
  expect_equal(as.integer(a[25, 50, ]), c(0x12L, 0x34L, 0x56L, 255L))
})
