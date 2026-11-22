# The tile cache

Map tiles are cached on disk for as long as the server's caching headers
allow (a day when they say nothing), so repeated renders of an area do
not download them again. The cache starts in `BASEMAPPER_CACHE_DIR` when
that is set (`"off"` disables it), else in
`tools::R_user_dir("basemapper", "cache")`. Python:
`basemapper.tile_cache()`.

## Usage

``` r
tile_cache(dir = NULL, clear = FALSE)
```

## Arguments

- dir:

  A directory to move the cache to, or `FALSE` to turn it off. `NULL`
  leaves it as it is.

- clear:

  Delete every cached tile.

## Value

The cache directory, or `NULL` when the cache is off (invisibly when
`dir` or `clear` is given).

## Examples

``` r
tile_cache()
#> [1] "C:\\Users\\Pukar.Bhandari\\AppData\\Local/R/cache/R/basemapper"
```
