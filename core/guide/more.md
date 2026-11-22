# More

* [`tile_style()`](crate::tile_style()) builds a style for your own tile data
  (raster or vector, with paint overrides); [`providers::wms_style`] and
  [`providers::wmts_style`] for OGC services.
* [`attribution`](crate::attribution()) and [`Basemap::get_attribution`]
  return the credit line the providers ask maps to show.
* Tiles are cached on disk ([`cache`]), for as long as the servers' caching
  headers allow; `BASEMAPPER_CACHE_DIR=off` turns the cache off.
* Errors are [`BasemapError`]; [`BasemapError::kind`] names the kind for
  bindings.
* The [R](../../r/index.html) and [Python](../../python/index.html)
  documentation have more examples, with plots in ggplot2, tmap, matplotlib
  and plotnine.

