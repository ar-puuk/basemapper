# The view-resolution contract (core/tests/fixtures/contract/cases.json),
# run from R: the same cases the Rust and Python suites run, so the
# languages resolve every view the same way.

contract_path <- function() {
  # tests/testthat -> r-basemapper -> repository
  p <- file.path(testthat::test_path(), "..", "..", "..", "core", "tests", "fixtures", "contract", "cases.json")
  if (file.exists(p)) normalizePath(p) else NULL
}

contract_inputs <- function(i) {
  out <- i[intersect(names(i), c("center", "zoom", "width", "height", "units", "dpi", "symbol_scale"))]
  if (!is.null(i$bounds_lonlat)) {
    b <- unlist(i$bounds_lonlat)
    p <- basemapper:::project(b[c(1, 3)], b[c(2, 4)], basemapper:::.CRS_4326, basemapper:::.CRS_3857)
    out$bounds <- c(p$x[1], p$y[1], p$x[2], p$y[2])
  }
  if (length(out) == 0L) out <- structure(list(), names = character())
  out
}

contract_resolve <- function(c) {
  j <- basemapper:::to_json(contract_inputs(c$inputs))
  if (is.null(c$context)) {
    return(basemapper:::from_json(.Call("wrap__resolve_raw", j, "", PACKAGE = "basemapper")))
  }
  p <- c$context$plot
  device <- switch(p$device,
    screen = list(kind = "screen", pixel_ratio = p$pixel_ratio %||% 1),
    raster_export = list(kind = "raster_export", dpi = p$dpi),
    vector_export = list(kind = "vector_export")
  )
  panel <- list(limits = unlist(p$extent_3857), crs = "EPSG:3857", px = unlist(p$panel_px), device = device)
  basemapper:::from_json(.Call("wrap__plan_raw", j, basemapper:::to_json(panel), 0L, 0L, numeric(), numeric(),
                               PACKAGE = "basemapper"))
}

`%||%` <- function(x, y) if (is.null(x)) y else x

# Within an absolute tolerance (the contract's tolerances are absolute).
expect_near <- function(got, want, tol, info) {
  got <- as.numeric(unlist(got))
  want <- as.numeric(unlist(want))
  expect_equal(length(got), length(want), info = info)
  expect_true(all(abs(got - want) <= tol), info = paste(info, ":", paste(got, collapse = ", "), "vs", paste(want, collapse = ", ")))
}

test_that("every contract case resolves as specified", {
  path <- contract_path()
  skip_if(is.null(path), "the contract needs a repository checkout")
  cases <- jsonlite::fromJSON(path, simplifyVector = FALSE)
  for (c in cases$views) {
    if (!is.null(c$error)) {
      e <- tryCatch(contract_resolve(c), error = function(e) e)
      expect_true(inherits(e, "error"), info = c$name)
      for (word in unlist(c$error$contains)) expect_match(conditionMessage(e), word, fixed = TRUE, info = c$name)
      next
    }
    v <- contract_resolve(c)
    want <- c$expect
    tol <- c$tolerance
    expect_near(v$width, want$width, tol$px, c$name)
    expect_near(v$height, want$height, tol$px, c$name)
    expect_near(v$dpi, want$dpi, tol$dpi, c$name)
    expect_near(v$symbol_scale, want$symbol_scale, 1e-9, c$name)
    expect_near(v$zoom, want$zoom, tol$zoom, c$name)
    expect_near(v$bounds, unlist(want$bounds_3857), tol$metres, c$name)
    by_name <- function(l) l[order(names(l))]
    expect_equal(by_name(v$provenance), by_name(want$provenance), info = c$name)
    got_notes <- paste(v$notes$level, v$notes$kind)
    want_notes <- vapply(want$notes, function(n) paste(n$level, n$kind), "")
    expect_equal(got_notes, want_notes, info = c$name)
  }
  for (a in cases$around) {
    span <- unlist(a$span)
    got <- around(unlist(a$center), span, units = a$units %||% "m")$bounds
    expect_near(got, unlist(a$expect_lonlat), 1e-9, a$name)
  }
})
