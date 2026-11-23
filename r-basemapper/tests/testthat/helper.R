# Load the package under test.
library(basemapper)

# Tests use a temporary tile cache, never the user's (CRAN policy).
tile_cache(file.path(tempdir(), "basemapper-tiles"))
