# The R package against the API contract (docs/api-contract.json, described
# in docs/api.md): every function and argument it lists, by name. The Python
# suite checks Python against the same file, so the two packages cannot
# drift apart.

`%||%` <- function(x, y) if (is.null(x)) y else x

api_contract <- function() {
  p <- file.path(testthat::test_path(), "..", "..", "..", "docs", "api-contract.json")
  if (!file.exists(p)) return(NULL)
  jsonlite::fromJSON(p, simplifyVector = FALSE)
}

test_that("every function has the contract's arguments", {
  c <- api_contract()
  skip_if(is.null(c), "the contract needs a repository checkout")
  ignore <- unlist(c$language_only_arguments$r)
  ns <- asNamespace("basemapper")
  exports <- getNamespaceExports("basemapper")
  for (f in c$functions) {
    name <- f$r
    if (is.null(name)) next
    expect_true(name %in% exports, info = paste(name, "is not exported"))
    got <- setdiff(names(formals(get(name, envir = ns))), ignore)
    want <- as.character(c(unlist(f$args), unlist(f$r_also)))
    expect_setequal(got, want)
  }
})

test_that("view readers, rendering conversions and defunct names are as listed", {
  c <- api_contract()
  skip_if(is.null(c), "the contract needs a repository checkout")
  exports <- getNamespaceExports("basemapper")
  for (name in c(unlist(c$view$r), unlist(c$rendering$r))) expect_true(name %in% exports, info = name)
  for (name in unlist(c$defunct$r)) {
    expect_error(get(name, envir = asNamespace("basemapper"))(), "migrating-to-0.3", info = name)
  }
})

test_that("nothing exported is left out of the contract", {
  c <- api_contract()
  skip_if(is.null(c), "the contract needs a repository checkout")
  listed <- c(vapply(c$functions, function(f) f$r %||% "", ""), unlist(c$view$r), unlist(c$rendering$r),
              unlist(c$defunct$r))
  # Described in prose: the catalogue names object.
  unlisted <- setdiff(getNamespaceExports("basemapper"), c(listed, "providers"))
  expect_equal(sort(unlisted), character())
})
