# Known bugs of 0.2, fixed by the 0.3.0 redesign (docs/design/api-review.md,
# implementation-plan.md). Each test states the correct behaviour inside
# expect_failure(): it passes while the bug is there, and fails once the bug
# is fixed, as a reminder to remove expect_failure() and keep the test.

# A style that draws a red square (10-12 E, 58-60 N) on white, offline.
square_style <- function() {
  sq <- '{"type":"Polygon","coordinates":[[[10,58],[12,58],[12,60],[10,60],[10,58]]]}'
  paste0(
    '{"version":8,"sources":{"sq":{"type":"geojson","data":', sq, '}},"layers":[',
    '{"id":"bg","type":"background","paint":{"background-color":"#ffffff"}},',
    '{"id":"sq","type":"fill","source":"sq","paint":{"fill-color":"#ff0000","fill-antialias":false}}]}'
  )
}

# Centroid (column, row) of the pixels matching `is_colour` in a PNG file.
colour_centroid <- function(path, is_colour) {
  img <- png::readPNG(path)
  hit <- which(is_colour(img[, , 1], img[, , 2], img[, , 3]), arr.ind = TRUE)
  c(x = mean(hit[, "col"]), y = mean(hit[, "row"]))
}

test_that("F1: geom_basemap lines up with the data on a lon/lat plot (phase 4)", {
  skip_if_not_installed("ggplot2")
  skip_if_not_installed("sf")
  skip_if_not_installed("png")
  sq <- sf::st_sf(geometry = sf::st_sfc(
    sf::st_polygon(list(rbind(c(10, 58), c(12, 58), c(12, 60), c(10, 60), c(10, 58)))), crs = 4326
  ))
  # Europe, in longitude / latitude: the square sits far from the centre.
  frame <- list(ggplot2::coord_sf(crs = 4326, xlim = c(-10, 30), ylim = c(35, 71), expand = FALSE, datum = NA),
                ggplot2::theme_void())
  draw <- function(p) {
    f <- tempfile(fileext = ".png")
    ggplot2::ggsave(f, p, width = 6, height = 6, dpi = 100)
    f
  }
  basemap_png <- draw(ggplot2::ggplot(sq) + geom_basemap(square_style(), attribution = FALSE) + frame)
  data_png <- draw(ggplot2::ggplot(sq) + ggplot2::geom_sf(fill = "#0000ff", colour = NA) + frame)
  red <- colour_centroid(basemap_png, function(r, g, b) r > 0.8 & g < 0.3 & b < 0.3)
  blue <- colour_centroid(data_png, function(r, g, b) b > 0.8 & r < 0.3 & g < 0.3)
  offset <- sqrt(sum((red - blue)^2))
  # Known bug: the Web Mercator image is stretched over the lon/lat panel
  # instead of warped (about 65 px off here on a 600 px panel).
  expect_failure(expect_lt(offset, 2))
})

test_that("F4: a box crossing the date line renders (phase 4)", {
  bg <- '{"version":8,"sources":{},"layers":[{"id":"bg","type":"background"}]}'
  # Documented in the README; the core supports it, R's bbox checks refuse it.
  expect_failure(expect_no_error(
    render_basemap_raw(c(19000000, -1e6, -19000000, 1e6), 100, 50, bg, crs = 3857)
  ))
})
