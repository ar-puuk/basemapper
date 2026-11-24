# Basemaps in plots (docs/design/api-review.md §4.5, D14, D15, D16):
# ggplot2 `ggplot() + m` / geom_basemap() and tmap tm_basemap(), on the
# core's plot plan. Offline: inline styles.

# A background and an (empty) GeoJSON source that carries a credit line.
credited <- paste0(
  '{"version":8,"sources":{"g":{"type":"geojson","data":{"type":"FeatureCollection","features":[]},',
  '"attribution":"<b>&copy; Test</b>"}},"layers":[',
  '{"id":"bg","type":"background","paint":{"background-color":"#336699"}},',
  '{"id":"d","type":"circle","source":"g"}]}'
)

# Plots below have a geom_sf() layer, as real plots do: coord_sf() takes the
# panel's extent from the data layers (the basemap has none of its own).
slc_box <- function() {
  sf::st_as_sfc(sf::st_bbox(c(xmin = -111.95, ymin = 40.70, xmax = -111.85, ymax = 40.80), crs = sf::st_crs(4326)))
}

# The rendering the basemap grob made when the plot was drawn to `f`.
draw <- function(p, f = tempfile(fileext = ".png"), ...) {
  ggplot2::ggsave(f, p, ...)
  f
}

# Save plot `p` and return the basemap rendering it drew.
with_rendering <- function(p, width, height, dpi = 100, ext = ".png") {
  f <- tempfile(fileext = ext)
  ggplot2::ggsave(f, p, width = width, height = height, dpi = dpi)
  basemapper:::.BASEMAPPER_LAST$rendering
}

test_that("ggplot() + basemap() draws under the layers, sized and placed by the plot", {
  skip_if_not_installed("sf")
  skip_if_not_installed("png")
  p <- ggplot2::ggplot(sf::st_sf(geometry = slc_box())) + basemap(credited) +
    ggplot2::geom_sf(fill = NA, colour = NA) + ggplot2::coord_sf(expand = FALSE, datum = NA) + ggplot2::theme_void()
  f <- draw(p, width = 3, height = 3, dpi = 100)
  img <- png::readPNG(f)
  centre <- img[nrow(img) %/% 2, ncol(img) %/% 2, 1:3]
  expect_equal(round(centre * 255), c(0x33, 0x66, 0x99))
})

test_that("the plot's device sets the dpi (D16): ggsave dpi, and 300 for PDF", {
  skip_if_not_installed("sf")
  r <- with_rendering(ggplot2::ggplot(sf::st_sf(geometry = slc_box())) + basemap(credited, zoom = NULL) +
                        ggplot2::geom_sf(fill = NA, colour = NA) + ggplot2::coord_sf(expand = FALSE, datum = NA) + ggplot2::theme_void(),
                      width = 3, height = 3, dpi = 200)
  expect_equal(r$dpi, 200)
  expect_equal(c(r$width, r$height), c(600, 600), tolerance = 2)
  r96 <- with_rendering(ggplot2::ggplot(sf::st_sf(geometry = slc_box())) + basemap(credited) +
                          ggplot2::geom_sf(fill = NA, colour = NA) + ggplot2::coord_sf(expand = FALSE, datum = NA) + ggplot2::theme_void(),
                        width = 3, height = 3, dpi = 96)
  # The same map, sharper (within the rounding of the panel to whole pixels).
  expect_equal(r$zoom, r96$zoom, tolerance = 0.01)
  pdf <- with_rendering(ggplot2::ggplot(sf::st_sf(geometry = slc_box())) + basemap(credited) +
                          ggplot2::geom_sf(fill = NA, colour = NA) + ggplot2::coord_sf(expand = FALSE, datum = NA) + ggplot2::theme_void(),
                        width = 3, height = 3, ext = ".pdf")
  expect_equal(pdf$dpi, 300)
  expect_true("default_dpi" %in% pdf$notes$kind)
})

test_that("a zoom chosen in a plot sets what is drawn; the device keeps its dpi", {
  skip_if_not_installed("sf")
  r <- with_rendering(ggplot2::ggplot(sf::st_sf(geometry = slc_box())) + basemap(credited, zoom = 14) +
                        ggplot2::geom_sf(fill = NA, colour = NA) + ggplot2::coord_sf(expand = FALSE, datum = NA),
                      width = 3, height = 3, dpi = 100)
  expect_equal(c(r$zoom, r$dpi), c(14, 100))
  # The size of lines and labels it gives is a decided note, without "dpi".
  n <- r$notes[r$notes$kind == "symbol_scale", ]
  expect_equal(nrow(n), 1)
  expect_equal(n$level, "decided")
  expect_false(grepl("dpi", n$message))
})

test_that("in a plot the device sets the dpi; symbol_scale sets the size of lines and labels", {
  skip_if_not_installed("sf")
  plot_of <- function(layer) {
    ggplot2::ggplot(sf::st_sf(geometry = slc_box())) + layer +
      ggplot2::geom_sf(fill = NA, colour = NA) + ggplot2::coord_sf(expand = FALSE, datum = NA)
  }
  expect_error(plot_of(basemap(credited, dpi = 168)), "device sets the dpi.*symbol_scale")
  expect_error(geom_basemap(credited, dpi = 168), "device sets the dpi")
  expect_error(plot_of(geom_basemap(credited, zoom = 12, symbol_scale = 0.8)), "zoom or symbol_scale")
  expect_error(render(basemap(credited)), NA)
  # The same map at any dpi; a smaller scale shows more detail.
  r200 <- with_rendering(plot_of(geom_basemap(credited, symbol_scale = 0.84)), width = 3, height = 3, dpi = 200)
  r300 <- with_rendering(plot_of(geom_basemap(credited, symbol_scale = 0.84)), width = 3, height = 3, dpi = 300)
  r1 <- with_rendering(plot_of(basemap(credited)), width = 3, height = 3, dpi = 200)
  expect_equal(c(r200$dpi, r300$dpi), c(200, 300))
  expect_equal(r200$zoom, r300$zoom, tolerance = 1e-3)
  expect_equal(r200$zoom - r1$zoom, log2(1 / 0.84), tolerance = 1e-3)
  # The 1:N scale is the printed one: smaller lines and labels, a larger N.
  expect_equal(r200$scale * 0.84 * 2^(r200$zoom - r1$zoom), r1$scale, tolerance = 1e-3)
  tm <- tm_basemap(credited, symbol_scale = 0.84)
  expect_s3_class(tm, "tmap")
  expect_error(tm_basemap(credited, dpi = 168), "device sets the dpi")
})

test_that("the plot gives the extent and CRS (D15)", {
  expect_error(ggplot2::ggplot() + basemap(credited, bounds = c(-112, 40, -111, 41)), "extent from the plot")
  skip_if_not_installed("sf")
  expect_error(ggplot2::ggplot() + basemap(credited, center = c(-111.9, 40.7), zoom = 10, crs = 32612),
               "CRS from the plot")
  expect_error(geom_basemap(credited, width = 100, height = 100), "size from the plot")
})

test_that("geom_basemap() is an alias, and the credit line is drawn unless disabled", {
  skip_if_not_installed("sf")
  layer <- geom_basemap(credited, zoom = 13)
  expect_s3_class(layer, "LayerInstance")
  expect_error(geom_basemap(basemap(credited), zoom = 3), "already a basemap")
  # The credit box is drawn in the panel's bottom-right corner, and nothing
  # else changes.
  skip_if_not_installed("png")
  drawn <- function(attribution) {
    p <- ggplot2::ggplot(sf::st_sf(geometry = slc_box())) + geom_basemap(credited, attribution = attribution) +
      ggplot2::geom_sf(fill = NA, colour = NA) + ggplot2::coord_sf(expand = FALSE, datum = NA) +
      ggplot2::theme_void()
    png::readPNG(draw(p, width = 3, height = 3, dpi = 100))
  }
  changed <- which(apply(abs(drawn(TRUE) - drawn(FALSE)), c(1, 2), max) > 0.05, arr.ind = TRUE)
  expect_gt(nrow(changed), 20)
  expect_gt(min(changed[, "row"]), 300 * 0.9)   # only at the bottom
  expect_gt(min(changed[, "col"]), 300 * 0.6)   # on the right
})

test_that("tm_basemap() draws under a tmap map, in the map's CRS", {
  skip_if_not_installed("tmap")
  skip_if_not_installed("sf")
  skip_if_not_installed("terra")
  nc <- sf::st_read(system.file("shape/nc.shp", package = "sf"), quiet = TRUE)
  el <- tm_basemap(credited)
  expect_s3_class(el, "tmap")
  expect_true("tm_basemap_rust" %in% class(el[[1]]))
  f <- tempfile(fileext = ".png")
  suppressMessages(tmap::tmap_save(el + tmap::tm_shape(nc) + tmap::tm_borders(), f, width = 4, height = 2,
                                   units = "in", dpi = 100))
  expect_true(file.exists(f))
  expect_error(tm_basemap(basemap(credited), zoom = 3), "already a basemap")
})

test_that("tmap_panel_inches fits the limits' aspect inside the device minus margins", {
  o <- list(devsize = c(6, 4), outer.margins = c(0.02, 0.02, 0.02, 0.02), nrows = 1, ncols = 1)
  p <- basemapper:::tmap_panel_inches(o, c(0, 0, 4, 1))
  expect_equal(p, c(6 * 0.96, 6 * 0.96 / 4))
  p <- basemapper:::tmap_panel_inches(o, c(0, 0, 1, 2))
  expect_equal(p, c(4 * 0.96 / 2, 4 * 0.96))
})

test_that("alpha fades the basemap on any device, and a clip masks it in a plot", {
  skip_if_not_installed("sf")
  skip_if_not_installed("png")
  red <- '{"version":8,"sources":{},"layers":[{"id":"bg","type":"background","paint":{"background-color":"#ff0000"}}]}'
  inner <- sf::st_as_sfc(sf::st_bbox(c(xmin = -111.93, ymin = 40.72, xmax = -111.87, ymax = 40.78), crs = sf::st_crs(4326)))
  p <- ggplot2::ggplot(sf::st_sf(geometry = slc_box())) +
    geom_basemap(red, alpha = 0.3, attribution = FALSE) +
    geom_basemap(red, clip = inner, attribution = FALSE) +
    ggplot2::geom_sf(fill = NA, colour = NA) + ggplot2::coord_sf(expand = FALSE, datum = NA) + ggplot2::theme_void()
  # R's own png() device: on Windows it ignores a raster's alpha, so the
  # opacity must be in the image itself.
  f <- tempfile(fileext = ".png")
  grDevices::png(f, width = 300, height = 300)
  print(p)
  grDevices::dev.off()
  img <- png::readPNG(f)
  green <- img[, , 2]
  row <- nrow(img) %/% 2
  cols <- which(img[row, , 1] > 0.9)            # the panel's columns (red, faded or not)
  centre <- green[row, round(mean(range(cols)))]
  edge <- green[row, min(cols) + 2]
  expect_lt(centre, 0.05)                         # inside the clip: full red
  expect_gt(edge, 0.6)                            # outside: 30% red over white
})

# The WFRC tract view's Web Mercator limits: a 3.6 x 5 in panel, 720 x
# 1000 px at 200 dpi, at symbol_scale 0.84 (1.75 map pixels per CSS pixel).
tract <- c(xmin = -12469979, ymin = 5026396.805555556, xmax = -12466414, ymax = 5031348.194444444)

# A white map with a black vertical line `width_css` CSS px wide at Web
# Mercator `x`.
line_style <- function(x, width_css) {
  lon <- x / 20037508.342789244 * 180
  sprintf(paste0(
    '{"version":8,"sources":{"l":{"type":"geojson","data":{"type":"LineString","coordinates":[[%.12g,-80],[%.12g,80]]}}},',
    '"layers":[{"id":"bg","type":"background","paint":{"background-color":"#ffffff"}},',
    '{"id":"l","type":"line","source":"l","paint":{"line-color":"#000000","line-width":%.12g}}]}'
  ), lon, lon, width_css)
}

dark_centre <- function(v, cols) {
  d <- 255 - v[cols]
  sum(d * (cols - 1)) / sum(d)
}

# The panel's pixels in a saved PNG, and the rendering the plot drew.
expect_panel_is_the_rendering <- function(f, r) {
  img <- round(png::readPNG(f) * 255)
  expect_equal(c(r$width, r$height), c(720, 1000))
  expect_equal(dim(img)[1:2], c(1000, 720))
  a <- as_array(r)
  expect_identical(as.integer(img[, , 1:3]), as.integer(a[, , 1:3]))
  # A line one device pixel wide at the centre of column 300 (0-based) is
  # there, to a tenth of a pixel.
  for (y in c(11, 501, 991)) expect_lt(abs(dark_centre(img[y, , 1], 291:311) - 300), 0.1)
}

test_that("a Web Mercator ggplot2 panel shows the rendering pixel for pixel", {
  skip_if_not_installed("sf")
  skip_if_not_installed("png")
  mx <- (tract[["xmax"]] - tract[["xmin"]]) / 720
  style <- line_style(tract[["xmin"]] + 300.5 * mx, 1 / 1.75)
  box <- sf::st_as_sfc(sf::st_bbox(tract, crs = sf::st_crs(3857)))
  # 168 dpi of map pixels on a 200 dpi device: symbol_scale 0.84.
  p <- ggplot2::ggplot(sf::st_sf(geometry = box)) + geom_basemap(style, symbol_scale = 0.84) +
    ggplot2::geom_sf(fill = NA, colour = NA) +
    ggplot2::coord_sf(crs = 3857, expand = FALSE, datum = NA) + ggplot2::theme_void() +
    ggplot2::theme(plot.margin = ggplot2::margin(0, 0, 0, 0))
  f <- tempfile(fileext = ".png")
  ggplot2::ggsave(f, p + ggplot2::theme(legend.position = "none"), width = 3.6, height = 5, dpi = 200)
  r <- basemapper:::.BASEMAPPER_LAST$rendering
  expect_equal(r$dpi, 200)
  expect_equal(unname(r$bounds), unname(tract), tolerance = 1e-12)
  expect_panel_is_the_rendering(f, r)
})

test_that("a Web Mercator tmap panel shows the rendering pixel for pixel", {
  skip_if_not_installed("sf")
  skip_if_not_installed("png")
  skip_if_not_installed("tmap")
  mx <- (tract[["xmax"]] - tract[["xmin"]]) / 720
  style <- line_style(tract[["xmin"]] + 300.5 * mx, 1 / 1.75)
  box <- sf::st_as_sfc(sf::st_bbox(tract, crs = sf::st_crs(3857)))

  tm <- tmap::tm_shape(sf::st_sf(geometry = box), crs = 3857, bbox = sf::st_bbox(box)) + tmap::tm_borders(col = NA, lwd = 0) +
    basemapper::tm_basemap(style, symbol_scale = 0.84, attribution = FALSE) +
    tmap::tm_layout(outer.margins = 0, inner.margins = 0, frame = FALSE, asp = 0)
  f <- tempfile(fileext = ".png")
  tmap::tmap_save(tm, f, width = 3.6, height = 5, units = "in", dpi = 200)
  r <- basemapper:::.BASEMAPPER_LAST$rendering
  expect_equal(r$dpi, 200)
  expect_panel_is_the_rendering(f, r)
})

# A Web Mercator plot of the tract view, `width` inches wide, saved at 200
# dpi; the rendering it drew.
tract_plot <- function(layer, width) {
  box <- sf::st_as_sfc(sf::st_bbox(tract, crs = sf::st_crs(3857)))
  p <- ggplot2::ggplot(sf::st_sf(geometry = box)) + layer + ggplot2::geom_sf(fill = NA, colour = NA) +
    ggplot2::coord_sf(crs = 3857, expand = FALSE, datum = NA) + ggplot2::theme_void() +
    ggplot2::theme(plot.margin = ggplot2::margin(0, 0, 0, 0))
  f <- tempfile(fileext = ".png")
  height <- width * (tract[["ymax"]] - tract[["ymin"]]) / (tract[["xmax"]] - tract[["xmin"]])
  ggplot2::ggsave(f, p, width = width, height = height, dpi = 200)
  list(file = f, rendering = last_rendering())
}

test_that("zoom_for() is the zoom a plot of that width draws; last_rendering() reads it", {
  skip_if_not_installed("sf")
  box <- sf::st_as_sfc(sf::st_bbox(tract, crs = sf::st_crs(3857)))
  for (s in c(1, 0.84)) {
    d <- tract_plot(geom_basemap(credited, symbol_scale = s, attribution = FALSE), 3.6)
    expect_equal(d$rendering$symbol_scale, s, tolerance = 1e-9)
    # The panel's width as drawn (ggplot2 may round it to a pixel less).
    expect_equal(zoom_for(box, d$rendering$width / 200, symbol_scale = s), d$rendering$zoom, tolerance = 1e-6)
  }
  expect_s3_class(last_rendering(), "basemapper_rendering")
  expect_equal(zoom_for(box, 3.6 * 2.54, units = "cm"), zoom_for(box, 3.6), tolerance = 1e-9)
  expect_error(zoom_for(box, 720, units = "px"), "printed width")
})

test_that("the same zoom draws the same picture at two panel sizes", {
  skip_if_not_installed("sf")
  skip_if_not_installed("png")
  box <- sf::st_as_sfc(sf::st_bbox(tract, crs = sf::st_crs(3857)))
  z <- zoom_for(box, 3.6, symbol_scale = 0.84)
  style <- line_style(tract[["xmin"]] + 0.4 * (tract[["xmax"]] - tract[["xmin"]]), 1)
  page <- tract_plot(geom_basemap(style, zoom = z, attribution = FALSE), 3.6)
  thumb <- tract_plot(geom_basemap(style, zoom = z, attribution = FALSE), 2.27)
  expect_equal(c(page$rendering$zoom, thumb$rendering$zoom), c(z, z), tolerance = 1e-9)
  expect_equal(page$rendering$symbol_scale, 0.84, tolerance = 0.005)
  expect_equal(thumb$rendering$symbol_scale / page$rendering$symbol_scale,
               thumb$rendering$width / page$rendering$width, tolerance = 1e-9)
  # The line is at the same place, relative to the width.
  line_at <- function(f) {
    img <- png::readPNG(f)
    row <- 1 - img[nrow(img) %/% 2, , 1]
    sum(row * seq_along(row)) / sum(row) / length(row)
  }
  # Within a pixel of the thumbnail.
  expect_lt(abs(line_at(page$file) - line_at(thumb$file)), 1 / thumb$rendering$width)
})
