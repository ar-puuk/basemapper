# More

* [`Source::tiles`] draws your own tile data (raster or vector, with paint
  overrides); [`Source::wms`] and [`Source::wmts`] OGC services, and
  [`Source::provider`] a catalogue basemap in a chosen form.
* [`Basemap::attribution`] returns the credit line the providers ask maps
  to show.
* A map in another CRS: [`Basemap::view`], then [`Basemap::render_view`]
  with a [`Warp`] (the host computes its [`Mesh`]). [`Basemap::clip`] draws
  only inside, or outside, polygons ([`Shape`]).
* Tiles are cached on disk ([`cache`]), for as long as the servers' caching
  headers allow; `BASEMAPPER_CACHE_DIR=off` turns the cache off.
* Errors are [`BasemapError`]; [`BasemapError::kind`] names the kind for
  bindings.
* The [R](../../r/index.html) and [Python](../../python/index.html)
  documentation have more examples, with plots in ggplot2, tmap, matplotlib
  and plotnine.

