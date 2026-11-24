# The 0.3 model in R (docs/design/api-review.md §4): basemaps as values,
# styles, renderings, notes and errors. Offline: every style is inline.

plain <- function(color = "#336699") {
  sprintf('{"version":8,"sources":{},"layers":[{"id":"bg","type":"background","paint":{"background-color":"%s"}}]}', color)
}

with_labels <- function(color = "#336699") {
  sprintf(paste0(
    '{"version":8,"sources":{"g":{"type":"geojson","data":{"type":"FeatureCollection","features":[]}}},',
    '"layers":[{"id":"bg","type":"background","paint":{"background-color":"%s"}},',
    '{"id":"names","type":"symbol","source":"g","layout":{"text-field":"x"}}]}'
  ), color)
}

slc <- c(-112.1, 40.6, -111.7, 40.9)

# ── Errors and removed names ─────────────────────────────────────────────────

test_that("core errors carry their kind as a class", {
  e <- tryCatch(basemap(plain(), center = c(0, 0)), error = function(e) e)
  expect_s3_class(e, "basemapper_validation_error")
  expect_s3_class(e, "basemapper_error")
  expect_match(conditionMessage(e), "around")
})

test_that("removed names and arguments point to the migration guide", {
  for (f in c("render_basemap_raw", "render_png", "render_array", "render_raster", "basemap_provider",
              "tile_style", "wms_style", "wmts_style")) {
    expect_error(getExportedValue("basemapper", f)(), "removed in basemapper 0.3", class = "basemapper_defunct_error")
  }
  expect_error(basemap(plain(), pixel_ratio = 2), "`pixel_ratio` was removed.*dpi.*migrating-to-0.3")
  expect_error(basemap(plain(), labels = FALSE), "features")
  expect_error(basemap(plain(), auth_token = "x"), "key")
  expect_error(basemap(plain(), nonsense = 1), "unknown argument `nonsense`")
  expect_error(basemap(plain(), 800, 600), "must be named, e.g. `width = 800, height = 600`")
})

# ── The view: given, derived, defaulted ──────────────────────────────────────

test_that("the default view is the world, and says so", {
  m <- basemap(plain())
  expect_equal(unname(get_size(m)), c(800, 600))
  expect_equal(get_zoom(m), log2(800 / 512))
  b <- get_bounds(m)
  expect_equal(unname(b[["xmax"]] - b[["xmin"]]), 2 * 20037508.342789244, tolerance = 1e-9)
  expect_equal(m$.prepared$view$provenance$extent, "default")
  out <- capture.output(print(m))
  expect_match(out[2], "800 x 600 px \\(default\\) at 96 dpi \\(default\\)")
  expect_match(out[3], "\\(derived\\)")
})

test_that("any three give the fourth", {
  auto <- basemap(plain(), bounds = slc)
  expect_equal(unname(get_size(auto)), c(800, 600))
  sized <- basemap(plain(), bounds = slc, zoom = 11)
  expect_equal(sized$.prepared$view$provenance$size, "derived")
  dpi <- basemap(plain(), bounds = slc, zoom = 11, width = 800, height = 600)
  expect_equal(dpi$.prepared$view$provenance$dpi, "derived")
  sharp <- basemap(plain(), bounds = slc, dpi = 192)
  expect_equal(unname(get_size(sharp)), c(1600, 1200))
  expect_equal(get_zoom(sharp), get_zoom(auto))
  print_ <- basemap(plain(), bounds = slc, width = 6, height = 4, units = "in", dpi = 300)
  expect_equal(unname(get_size(print_)), c(1800, 1200))
})

test_that("conflicts are errors that say what to drop", {
  expect_error(basemap(plain(), bounds = slc, zoom = 11, width = 800, height = 600, dpi = 96), "over-determine")
  expect_error(basemap(plain(), bounds = slc, center = c(-111.9, 40.7), zoom = 11), "bounds")
  expect_error(basemap(plain(), width = 800), "both `width` and `height`")
  expect_error(basemap(plain(), bounds = c(-12500000, 4900000, -12400000, 5000000)), "longitude / latitude")
})

test_that("basemaps are values: set_view() and set_bounds() return new ones", {
  m <- basemap(plain(), center = c(-111.89, 40.76), zoom = 12)
  n <- set_view(m, zoom = 10)
  expect_equal(c(get_zoom(m), get_zoom(n)), c(12, 10))
  expect_equal(unname(get_center(n)), c(-111.89, 40.76), tolerance = 1e-9)
  b <- set_bounds(m, slc)
  expect_null(b$center)
  expect_equal(get_zoom(b), 12)  # the zoom given before stays: the size follows
  expect_equal(b$.prepared$view$provenance$size, "derived")
  c2 <- set_view(b, center = c(-111.5, 40.5))
  expect_null(c2$bounds)
  expect_equal(unname(get_center(c2)), c(-111.5, 40.5), tolerance = 1e-9)
  # Unsetting the zoom: the extent at the default size, the zoom automatic again.
  auto <- set_view(b, zoom = NULL)
  expect_equal(auto$.prepared$view$provenance$zoom, "derived")
  expect_equal(get_zoom(auto), get_zoom(basemap(plain(), bounds = slc)))
})

test_that("around() is a ground distance in its own units", {
  a <- around(c(-111.9, 40.76), c(20, 15), units = "km")
  expect_s3_class(a, "basemapper_around")
  expect_equal(basemap(plain(), bounds = a)$.prepared$crs$name, "EPSG:3857")
  expect_error(around(c(0, 0), 1, units = "furlongs"), "units")
})

test_that("a box across the date line is an extent", {
  m <- basemap(plain(), bounds = c(170, -10, -170, 10), width = 100, height = 50)
  b <- get_bounds(m)
  expect_gt(b[["xmin"]], b[["xmax"]])
  expect_equal(abs(get_center(m)[["longitude"]]), 180, tolerance = 1e-9)
})

# ── Styles ───────────────────────────────────────────────────────────────────

test_that("styles are checked when made", {
  s <- style("OpenFreeMap.Liberty", features = c("roads", "labels"))
  expect_s3_class(s, "basemapper_style")
  expect_equal(style(s, key = "k")$features, c("roads", "labels"))
  expect_error(style(plain(), features = "highways"), "unknown feature")
  expect_error(style(plain(), features = c("labels", "-labels")), 'both "labels" and "-labels"')
  expect_error(style(provider("Esri.WorldImagery", form = "vector")), "only as raster")
  expect_error(style(provider("CartoDB.Positon")), "did you mean CartoDB.Positron")
  withr::with_envvar(c(MAPTILER_API_KEY = NA), expect_error(style("MapTiler.Streets"), "MAPTILER_API_KEY"))
  expect_equal(style("MapTiler.Streets", key = "K")$key, "K")
  expect_false(any(grepl("K", capture.output(print(style("MapTiler.Streets", key = "K"))))))
})

test_that("each style of a stack has its own filters", {
  base <- style(plain("#ff0000"))
  top <- style(with_labels("#0000ff"), features = "labels")
  img <- as_array(render(basemap(list(base, top), center = c(0, 0), zoom = 2, width = 64, height = 48)))
  expect_equal(as.integer(img[1, 1, ]), c(255L, 0L, 0L, 255L))
  expect_error(
    render(basemap(list(style(plain(), features = "labels"), with_labels()), center = c(0, 0), zoom = 2,
                   width = 8, height = 8)),
    "nothing of this style of the stack"
  )
})

# ── Renderings ───────────────────────────────────────────────────────────────

test_that("a rendering is one render with many conversions", {
  m <- basemap(plain("#123456"), center = c(-111.89, 40.76), zoom = 12, width = 40, height = 20, dpi = 192)
  r <- render(m)
  expect_s3_class(r, "basemapper_rendering")
  expect_equal(c(r$width, r$height, r$dpi, r$zoom), c(40, 20, 192, 12))
  expect_equal(unname(r$bounds), unname(get_bounds(m)))
  a <- as_array(r)
  expect_equal(dim(a), c(20L, 40L, 4L))
  expect_equal(as.integer(a[1, 1, ]), c(0x12L, 0x34L, 0x56L, 255L))
  expect_equal(attr(a, "crs"), "EPSG:3857")
  f <- tempfile(fileext = ".png")
  png <- write_png(r, f)
  expect_identical(readBin(f, "raw", 8L), as.raw(c(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a)))
  expect_match(capture.output(print(r))[1], "40 x 20 px, EPSG:3857")
  # render(dpi =): the same map, sharper, when the size is not fixed.
  world <- basemap(plain(), bounds = slc)
  sharp <- render(world, dpi = 192)
  expect_equal(c(sharp$width, sharp$height), c(1600, 1200))
  expect_equal(sharp$zoom, get_zoom(world))
})

test_that("as_raster() is georeferenced", {
  skip_if_not_installed("terra")
  r <- render(basemap(plain(), bounds = c(-1, -1, 1, 1), width = 20, height = 20))
  ras <- as_raster(r)
  expect_equal(terra::nlyr(ras), 4)
  expect_equal(as.vector(terra::ext(ras))[["xmin"]], unname(r$bounds[["xmin"]]))
})

test_that("svg is a vector document, in Web Mercator only", {
  m <- basemap(plain(), center = c(0, 0), zoom = 3, width = 64, height = 32)
  f <- tempfile(fileext = ".svg")
  svg <- render_svg(m, f)
  expect_match(svg, '^<svg')
  expect_match(svg, 'viewBox="0 0 64 32"', fixed = TRUE)
  expect_equal(readChar(f, file.size(f)), svg)
  skip_if_not_installed("sf")
  utm <- basemap(plain(), center = c(-111.9, 40.7), zoom = 10, crs = 32612)
  expect_error(render_svg(utm), "SVG output is in Web Mercator only")
})

test_that("plot() draws a rendering, and knitr shows it at its CSS size", {
  m <- basemap(plain(), center = c(0, 0), zoom = 2, width = 16, height = 8, dpi = 192)
  f <- tempfile(fileext = ".png")
  grDevices::png(f)
  expect_invisible(plot(m))
  grDevices::dev.off()
  skip_if_not_installed("knitr")
  out <- knit_print.basemap(m, options = list(fig.path = file.path(tempdir(), "fig-"), label = "x"))
  expect_true(file.exists(out))
})

# ── Notes ────────────────────────────────────────────────────────────────────

test_that("notes are surfaced by level", {
  # Decided: quiet, unless verbose.
  expect_no_message(render(basemap(plain(), width = 8, height = 8)))
  withr::with_options(list(basemapper.verbose = TRUE),
                      expect_message(render(basemap(plain(), width = 8, height = 8)), "dpi \\(default\\)"))
  # Adjusted: a message of its class (once per kind per session).
  rlang::reset_message_verbosity("basemapper_view_constrained")
  expect_message(render(basemap(plain(), center = c(0, 89), zoom = 1, width = 64, height = 64)),
                 class = "basemapper_adjustment")
  # Problem: a warning, every time.
  m <- basemap(style(plain(), layers = "-label_*"), width = 8, height = 8)
  expect_warning(render(m), "matches no layer", class = "basemapper_problem")
  expect_warning(render(m), "matches no layer", class = "basemapper_problem")
})

test_that("on_error = \"fail\" turns problems into errors", {
  video <- paste0(
    '{"version":8,"sources":{"g":{"type":"geojson","data":{"type":"FeatureCollection","features":[]}},',
    '"v":{"type":"video","urls":[]}},"layers":[{"id":"bg","type":"background"},',
    '{"id":"d","type":"circle","source":"g"},{"id":"m","type":"raster","source":"v"}]}'
  )
  expect_warning(r <- render(basemap(video, width = 8, height = 8)), "video", class = "basemapper_problem")
  expect_true(any(r$notes$level == "problem"))
  expect_error(render(basemap(video, width = 8, height = 8, fetch = fetch_options(on_error = "fail"))), "on_error")
})

test_that("fetch_options() validates", {
  expect_error(fetch_options(timeout = 0), "timeout")
  expect_error(fetch_options(on_error = "retry"))
  expect_error(basemap(plain(), fetch = list(timeout = 3)), "fetch_options")
  expect_output(print(fetch_options()), "timeout 10s")
})
