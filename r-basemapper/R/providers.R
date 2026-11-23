#' Named basemaps
#'
#' `basemap_provider()` returns the style of a named basemap, for
#' [basemap()] and the other renderers' `style` argument. Names follow
#' leaflet-providers (`"CartoDB.Positron"`, `"Esri.WorldImagery"`,
#' `"OpenStreetMap.HOT"`); [list_providers()] lists them with their forms,
#' and [providers] completes them. A name also works as `style` directly;
#' use this function to pick the form, pass a key, or draw your own tiles
#' with the basemap's style. Implemented in the Rust core, so R and Python
#' (`basemapper.basemap_provider()`) return the same style.
#'
#' @param name Basemap name (case-insensitive).
#' @param type `"vector"` (a MapLibre style) or `"raster"` (tiles). By
#'   default the vector form when there is one, else raster. Asking for a
#'   form a basemap lacks is an error, never a silent fallback.
#' @param api_key API key for CARTO, MapTiler and Stadia, or the quad word
#'   for UGRC Discover's basemaps (`"UGRC.Terrain"`, `"UGRC.Topo"`,
#'   `"UGRC.Hillshade"`, `"UGRC.Imagery"`, ...); by default read from
#'   `CARTO_API_KEY`, `MAPTILER_API_KEY`, `STADIA_API_KEY` or
#'   `UGRC_QUAD_WORD`. The keys are free for moderate use (UGRC's for
#'   Utah's government and partners); [list_providers()] shows which
#'   basemaps need one.
#' @param tiles Your own tile data to draw with this basemap's style: a
#'   `.pmtiles` archive (URL or path), tile URL or TileJSON. It must use the
#'   same schema as the provider's tiles (OpenMapTiles for OpenFreeMap and
#'   Stadia).
#' @param labels `TRUE` keeps every layer, `FALSE` drops the labels,
#'   `"only"` keeps only the labels, to stack this basemap's labels over
#'   another map. Raster tiles switch to the provider's label-free or
#'   label-only tiles (an error when it has none).
#' @param features Vector form: features to keep (`c("water", "roads")`) or
#'   drop (`"-buildings"`); see [list_layers()].
#' @return A style string: a URL, or inline style JSON (the raster form, or
#'   with `tiles`, `labels` or `features`). A basemap published as separate
#'   services (`"UGRC.Lite"`: hillshade, base and labels) returns a character
#'   vector of styles, bottom to top, which [basemap()], [geom_basemap()] and
#'   [tm_basemap()] take as a stack; `labels` and `features` apply to the
#'   stack as one basemap, leaving out a style they empty.
#' @seealso [tile_style()] for your own tile data, [wms_style()] for OGC
#'   services.
#' @export
#' @examples
#' basemap_provider("OpenFreeMap.Liberty")
#' basemap_provider("Esri.WorldGrayCanvas", type = "raster")
#' \dontrun{
#' basemap_provider("CartoDB.Voyager", api_key = "...")   # or set CARTO_API_KEY
#' basemap_provider("OpenFreeMap.Liberty", tiles = "data/area.pmtiles")
#' # Imagery under a vector basemap's labels
#' basemap(c("Esri.WorldImagery", basemap_provider("OpenFreeMap.Liberty", labels = "only")),
#'         center = c(-111.89, 40.76), zoom = 12)
#' }
basemap_provider <- function(name, type = NULL, api_key = NULL, tiles = NULL, labels = TRUE,
                             features = NULL) {
  chr <- function(x) if (is.null(x)) NULL else as.character(x)[1L]
  .Call("wrap__basemap_provider_raw", as.character(name)[1L], chr(type), chr(api_key), chr(tiles),
        labels_arg(labels), if (is.null(features)) NULL else as.character(features),
        PACKAGE = "basemapper")
}

#' The built-in basemaps
#'
#' The basemaps [basemap_provider()] knows, with the forms each comes in.
#' Python: `basemapper.list_providers()`.
#'
#' @param type `"vector"` or `"raster"` to list only basemaps with that
#'   form; `NULL` lists all.
#' @return A data frame with columns `name`; `vector` and `raster` (logical:
#'   the forms it has); `api_key_env` (the environment variable its API key
#'   is read from, `NA` when it needs none); and `attribution`.
#' @export
#' @examples
#' list_providers()
#' list_providers("vector")$name
list_providers <- function(type = NULL) {
  x <- .Call("wrap__list_providers_raw", if (is.null(type)) NULL else as.character(type)[1L],
             PACKAGE = "basemapper")
  key <- attr(x, "api_key_env")
  key[key == ""] <- NA_character_
  data.frame(name = as.vector(x), vector = attr(x, "vector"), raster = attr(x, "raster"),
             api_key_env = key, attribution = attr(x, "attribution"), stringsAsFactors = FALSE)
}

#' Basemap names, for autocompletion
#'
#' A named list of every basemap name: `providers$CartoDB.Positron` is the
#' string `"CartoDB.Positron"`, usable as `style` or with
#' [basemap_provider()]. The names match `leaflet::providers` where both
#' have a basemap. Python: `basemapper.providers.CartoDB.Positron`.
#'
#' @format A named list of character strings.
#' @export
#' @examples
#' providers$Esri.WorldImagery
providers <- NULL

#' The tile cache
#'
#' Map tiles are cached on disk for as long as the server's caching headers
#' allow (a day when they say nothing), so repeated renders of an area do
#' not download them again. The cache starts in `BASEMAPPER_CACHE_DIR` when
#' that is set (`"off"` disables it), else in
#' `tools::R_user_dir("basemapper", "cache")`. Set `BASEMAPPER_CACHE_DIR` in
#' `.Renviron` (`usethis::edit_r_environ()` opens it) to keep a folder for good,
#' or to share one cache with Python.
#'
#' The cache is kept small: expired tiles are deleted, and past its size
#' limit (250 MB, or `BASEMAPPER_CACHE_MAX_SIZE` in MB) the least recently
#' used tiles go first, in the background as tiles are stored. Python:
#' `basemapper.tile_cache()`.
#'
#' @param dir A directory to move the cache to, or `FALSE` to turn it off.
#'   `NULL` leaves it as it is.
#' @param clear Delete every cached tile.
#' @param max_size The cache's size limit in megabytes (`NULL` leaves it as
#'   it is).
#' @return The cache directory, or `NULL` when the cache is off (invisibly
#'   when `dir` or `clear` is given).
#' @export
#' @examples
#' tile_cache()
tile_cache <- function(dir = NULL, clear = FALSE, max_size = NULL) {
  d <- if (isFALSE(dir)) "" else if (is.null(dir)) NULL else path.expand(as.character(dir)[1L])
  mb <- if (is.null(max_size)) NULL else as.numeric(max_size)[1L]
  out <- .Call("wrap__tile_cache_raw", d, isTRUE(clear), mb, PACKAGE = "basemapper")
  if (is.null(dir) && !isTRUE(clear) && is.null(max_size)) out else invisible(out)
}

.onLoad <- function(libname, pkgname) {
  ns <- asNamespace(pkgname)
  names <- as.vector(.Call("wrap__list_providers_raw", NULL, PACKAGE = "basemapper"))
  assign("providers", stats::setNames(as.list(names), names), envir = ns)
  # CRAN policy: user files go under R_user_dir() unless the user chose a place.
  if (is.na(Sys.getenv("BASEMAPPER_CACHE_DIR", NA_character_))) {
    .Call("wrap__tile_cache_raw", tools::R_user_dir("basemapper", "cache"), FALSE, NULL, PACKAGE = "basemapper")
  }
  invisible()
}

#' A style for tile data
#'
#' Builds a style for tile data, with overrides. `source` is anything you
#' could pass as `style` directly; use this function to override what is
#' detected, or to get the style JSON to edit:
#'
#' * a tile URL template (`{z}/{x}/{y}`, `{quadkey}`, `{bbox-epsg-3857}`,
#'   WMTS `{TileMatrix}/{TileRow}/{TileCol}`);
#' * a TileJSON URL or file;
#' * a PMTiles archive: URL, local path or `pmtiles://` URL;
#' * an ArcGIS `.../MapServer` (raster tiles) or `.../VectorTileServer` (its
#'   published style);
#' * a style URL or file (returned with relative URLs made absolute).
#'
#' Raster tiles get one raster layer. Vector tiles get a default style that
#' draws every source layer (polygons as translucent fills, lines and
#' outlines, points as circles, a colour per source layer) over a
#' transparent background. The same function exists in Python
#' (`basemapper.tile_style()`).
#'
#' @param source The tile data, as above.
#' @param type `"raster"` or `"vector"`; detected when `NULL` (from the
#'   archive header, the TileJSON, the URL's extension, or one fetched tile).
#' @param tile_size Raster tile size in px (default 256 for tile URLs and
#'   archives).
#' @param scheme `"xyz"` or `"tms"` (rows counted from the south).
#' @param minzoom,maxzoom Zoom range of the tiles (overzoomed beyond
#'   `maxzoom`).
#' @param attribution Attribution text for the source.
#' @param source_layers Vector tiles: the source layers to draw (default
#'   all).
#' @param paint Vector tiles: a named list of paint properties for the
#'   generated `fill-*`, `line-*` and `circle-*` layers, e.g.
#'   `list("fill-color" = "#cccccc")`. Others are dropped with a warning.
#' @param auth_token Optional access token for the tile service.
#' @return The style JSON string.
#' @export
#' @examples
#' style <- tile_style("https://tile.openstreetmap.org/{z}/{x}/{y}.png",
#'                     maxzoom = 19, attribution = "© OpenStreetMap contributors")
#' style <- tile_style("https://example.com/tiles/{z}/{x}/{y}.mvt",
#'                     source_layers = "land", paint = list("fill-color" = "#e8e0d8"))
#' \dontrun{
#' # Detected from the data; also usable directly as `style`
#' tile_style("data/area.pmtiles")
#' basemap("data/area.pmtiles", 800, 600, bounds = nc)
#' }
tile_style <- function(source, type = NULL, tile_size = NULL, scheme = NULL, minzoom = NULL,
                       maxzoom = NULL, attribution = NULL, source_layers = NULL, paint = list(),
                       auth_token = NULL) {
  int_or_null <- function(x) if (is.null(x)) NULL else as.integer(x)
  chr_or_null <- function(x) if (is.null(x)) NULL else as.character(x)
  paint_json <- if (length(paint) == 0L) "{}" else as.character(jsonlite::toJSON(paint, auto_unbox = TRUE))
  s <- .Call("wrap__tile_style_raw", as.character(source)[1L], chr_or_null(type), int_or_null(tile_size),
             chr_or_null(scheme), int_or_null(minzoom), int_or_null(maxzoom), chr_or_null(attribution),
             chr_or_null(source_layers), paint_json, chr_or_null(auth_token), PACKAGE = "basemapper")
  dropped <- attr(s, "dropped")
  if (length(dropped) > 0L) {
    warning("tile_style: paint key(s) that match no generated layer dropped: ",
            paste(dropped, collapse = ", "), call. = FALSE)
  }
  attributes(s) <- NULL
  s
}

#' Styles for OGC services
#'
#' Styles for WMS GetMap and WMTS GetTile (KVP) endpoints, as 256 px tiles in
#' EPSG:3857 (WMS through MapLibre's `{bbox-epsg-3857}` token). A RESTful
#' WMTS template (`{TileMatrix}/{TileRow}/{TileCol}`) needs no builder: pass
#' it as `style`. Python: `basemapper.wms_style()`, `basemapper.wmts_style()`.
#'
#' @param url Base service URL without query parameters.
#' @param layers WMS `LAYERS` parameter.
#' @param styles WMS `STYLES` parameter.
#' @param version Service version (WMS 1.1.x sends `SRS`, 1.3.0 `CRS`).
#' @param format Image MIME type.
#' @param transparent Request transparent WMS images.
#' @param extra_params Additional query parameters appended verbatim, or
#'   `NULL`.
#' @param layer WMTS `LAYER` parameter.
#' @param tilematrixset WMTS tile matrix set (a Web Mercator one).
#' @param style WMTS `STYLE` parameter.
#' @return A style JSON string.
#' @name ogc_styles
#' @examples
#' style <- wms_style("https://example.com/wms", layers = "relief")
#' style <- wmts_style("https://example.com/wmts", layer = "imagery")
NULL

#' @rdname ogc_styles
#' @export
wms_style <- function(url, layers, styles = "", version = "1.3.0", format = "image/png",
                      transparent = FALSE, extra_params = NULL) {
  .Call("wrap__wms_style_raw", as.character(url), as.character(layers), as.character(styles),
        as.character(version), as.character(format), isTRUE(transparent),
        if (is.null(extra_params)) NULL else as.character(extra_params), PACKAGE = "basemapper")
}

#' @rdname ogc_styles
#' @export
wmts_style <- function(url, layer, tilematrixset = "GoogleMapsCompatible", format = "image/png",
                       style = "default", version = "1.0.0", extra_params = NULL) {
  .Call("wrap__wmts_style_raw", as.character(url), as.character(layer), as.character(tilematrixset),
        as.character(format), as.character(style), as.character(version),
        if (is.null(extra_params)) NULL else as.character(extra_params), PACKAGE = "basemapper")
}
