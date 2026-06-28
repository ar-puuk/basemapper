#' Generate a MapLibre GL Style JSON string for an XYZ raster tile source.
#'
#' @param url_template Character: tile URL with `{z}`, `{x}`, `{y}` placeholders.
#' @param tile_size Integer tile size in pixels (default 256L).
#' @return A JSON character string ready to pass to `render_basemap_raw()`.
#' @export
#' @examples
#' style <- raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
raster_provider <- function(url_template, tile_size = 256L) {
  style <- list(
    version = 8L,
    sources = list(
      `raster-source` = list(
        type     = "raster",
        tiles    = list(url_template),
        tileSize = as.integer(tile_size)
      )
    ),
    layers = list(
      list(id = "raster-layer", type = "raster", source = "raster-source")
    )
  )
  jsonlite::toJSON(style, auto_unbox = TRUE)
}

#' Return the root.json style endpoint URL for an ArcGIS VectorTileServer.
#'
#' The Rust core fetches this URL to obtain the full, ESRI-published MapLibre
#' GL style JSON. Returns a plain URL string, not inline JSON.
#'
#' @param base_url Character: base ArcGIS VectorTileServer URL.
#' @return A URL character string.
#' @export
#' @examples
#' url <- esri_vector_provider("https://basemaps.arcgis.com/arcgis/rest/services/World_Basemap_v2/VectorTileServer")
esri_vector_provider <- function(base_url) {
  paste0(sub("/*$", "", base_url), "/resources/styles/root.json")
}

#' Generate a MapLibre GL Style JSON string for an ArcGIS MapServer raster source.
#'
#' @param url_template Character: ArcGIS tile URL (e.g., `.../MapServer/tile/{z}/{y}/{x}`).
#' @param tile_size Integer tile size in pixels (default 256L).
#' @return A JSON character string ready to pass to `render_basemap_raw()`.
#' @export
#' @examples
#' style <- esri_raster_provider("https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}")
esri_raster_provider <- function(url_template, tile_size = 256L) {
  style <- list(
    version = 8L,
    sources = list(
      `esri-raster-source` = list(
        type     = "raster",
        tiles    = list(url_template),
        tileSize = as.integer(tile_size)
      )
    ),
    layers = list(
      list(id = "esri-raster-layer", type = "raster", source = "esri-raster-source")
    )
  )
  jsonlite::toJSON(style, auto_unbox = TRUE)
}

#' Generate a MapLibre GL Style JSON string for an MVT vector tile source.
#'
#' Recognised paint keys: `fill-color`, `fill-opacity`, `fill-outline-color`,
#' `line-color`, `line-width`, `line-opacity`. Unrecognised keys are dropped
#' with a `warning()`. An empty paint list applies a light-grey default style.
#'
#' @param url_template Character: MVT tile URL with `{z}`, `{x}`, `{y}` placeholders.
#' @param source_layer Character: the name of the layer within each MVT tile to
#'   render. This is tile-server specific (e.g. `"water"`, `"roads"`); it maps
#'   to the MapLibre GL `"source-layer"` property and is required for vector
#'   sources.
#' @param paint Named list of MapLibre GL paint properties.
#' @return A JSON character string ready to pass to `render_basemap_raw()`.
#' @export
#' @examples
#' style <- vector_provider(
#'   "https://example.com/tiles/{z}/{x}/{y}.mvt",
#'   source_layer = "land",
#'   paint = list("fill-color" = "#e8e0d8", "line-color" = "#aaa")
#' )
vector_provider <- function(url_template, source_layer, paint = list()) {
  fill_keys <- c("fill-color", "fill-opacity", "fill-outline-color")
  line_keys <- c("line-color", "line-width", "line-opacity")
  known_keys <- c(fill_keys, line_keys)

  unknown <- setdiff(names(paint), known_keys)
  if (length(unknown) > 0) {
    warning(paste0("vector_provider: unrecognised paint key(s) dropped: ",
                   paste(unknown, collapse = ", ")))
    paint[unknown] <- NULL
  }

  fill_paint <- paint[intersect(names(paint), fill_keys)]
  line_paint <- paint[intersect(names(paint), line_keys)]

  if (is.null(fill_paint[["fill-color"]]))   fill_paint[["fill-color"]]   <- "#e8e0d8"
  if (is.null(fill_paint[["fill-opacity"]])) fill_paint[["fill-opacity"]] <- 1
  if (is.null(line_paint[["line-color"]]))   line_paint[["line-color"]]   <- "#aaaaaa"
  if (is.null(line_paint[["line-width"]]))   line_paint[["line-width"]]   <- 1

  style <- list(
    version = 8L,
    sources = list(
      `vector-source` = list(
        type  = "vector",
        tiles = list(url_template)
      )
    ),
    layers = list(
      list(id = "vector-fill", type = "fill",
           source = "vector-source", `source-layer` = source_layer,
           paint = fill_paint),
      list(id = "vector-line", type = "line",
           source = "vector-source", `source-layer` = source_layer,
           paint = line_paint)
    )
  )
  jsonlite::toJSON(style, auto_unbox = TRUE)
}

#' Look up a named basemap provider.
#'
#' Returns either a style URL string or inline MapLibre GL JSON suitable for
#' passing directly to \code{\link{render_basemap_raw}()}.
#'
#' @param name Character: provider name. See *Details* for the full catalog.
#' @param api_key Character or \code{NULL}: API key required by some providers
#'   (e.g. MapTiler). Ignored for providers that do not use a key.  Stadia
#'   providers work without a key at low volume; supply one to lift rate limits.
#' @param ... Reserved for future arguments; currently unused.
#' @return A character string — either a style URL or inline MapLibre GL JSON —
#'   ready to pass to \code{\link{render_basemap_raw}()}.
#' @details
#' **Available provider names:**
#'
#' *Vector GL styles (no key required):*
#' `"OpenFreeMap.liberty"`, `"OpenFreeMap.bright"`, `"OpenFreeMap.positron"`,
#' `"Carto.Positron"`, `"Carto.DarkMatter"`, `"Carto.Voyager"`,
#' `"ESRI.WorldBasemap"`
#'
#' *MapTiler GL styles (require `api_key`):*
#' `"MapTiler.Streets"`, `"MapTiler.Satellite"`, `"MapTiler.Topo"`,
#' `"MapTiler.Outdoor"`
#'
#' *ESRI raster tile services (no key required):*
#' `"ESRI.WorldImagery"`, `"ESRI.WorldStreetMap"`, `"ESRI.WorldTopoMap"`,
#' `"ESRI.WorldShadedRelief"`, `"ESRI.WorldGrayCanvas"`,
#' `"ESRI.NatGeoWorldMap"`
#'
#' *OSM raster (no key required):*
#' `"OSM.Standard"`, `"OSM.CyclOSM"`
#'
#' *Stadia/Stamen (key optional):*
#' `"Stadia.AlidadeSmooth"`, `"Stadia.AlidadeSmoothDark"`,
#' `"Stadia.OSMBright"`, `"Stadia.StamenToner"`, `"Stadia.StamenTerrain"`,
#' `"Stadia.StamenWatercolor"`
#' @export
#' @examples
#' style <- basemap_provider("OpenFreeMap.liberty")
#' style <- basemap_provider("Carto.DarkMatter")
#' style <- basemap_provider("OSM.Standard")
#' \dontrun{
#' style <- basemap_provider("MapTiler.Streets", api_key = "YOUR_KEY")
#' }
basemap_provider <- function(name, api_key = NULL, ...) {
  switch(name,

    # --- Vector GL styles (return style URL string) ---
    "OpenFreeMap.liberty"   = "https://tiles.openfreemap.org/styles/liberty",
    "OpenFreeMap.bright"    = "https://tiles.openfreemap.org/styles/bright",
    "OpenFreeMap.positron"  = "https://tiles.openfreemap.org/styles/positron",
    "Carto.Positron"        = "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json",
    "Carto.DarkMatter"      = "https://basemaps.cartocdn.com/gl/dark-matter-gl-style/style.json",
    "Carto.Voyager"         = "https://basemaps.cartocdn.com/gl/voyager-gl-style/style.json",

    # --- MapTiler GL styles (require api_key; embed key in URL) ---
    "MapTiler.Streets" = {
      if (is.null(api_key)) stop("Provider 'MapTiler.Streets' requires an api_key argument")
      paste0("https://api.maptiler.com/maps/streets-v2/style.json?key=", api_key)
    },
    "MapTiler.Satellite" = {
      if (is.null(api_key)) stop("Provider 'MapTiler.Satellite' requires an api_key argument")
      paste0("https://api.maptiler.com/maps/satellite/style.json?key=", api_key)
    },
    "MapTiler.Topo" = {
      if (is.null(api_key)) stop("Provider 'MapTiler.Topo' requires an api_key argument")
      paste0("https://api.maptiler.com/maps/topo-v2/style.json?key=", api_key)
    },
    "MapTiler.Outdoor" = {
      if (is.null(api_key)) stop("Provider 'MapTiler.Outdoor' requires an api_key argument")
      paste0("https://api.maptiler.com/maps/outdoor-v2/style.json?key=", api_key)
    },

    # --- ESRI raster tile services ---
    "ESRI.WorldImagery"      = esri_raster_provider("https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"),
    "ESRI.WorldStreetMap"    = esri_raster_provider("https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{z}/{y}/{x}"),
    "ESRI.WorldTopoMap"      = esri_raster_provider("https://server.arcgisonline.com/ArcGIS/rest/services/World_Topo_Map/MapServer/tile/{z}/{y}/{x}"),
    "ESRI.WorldShadedRelief" = esri_raster_provider("https://server.arcgisonline.com/ArcGIS/rest/services/World_Shaded_Relief/MapServer/tile/{z}/{y}/{x}"),
    "ESRI.WorldGrayCanvas"   = esri_raster_provider("https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Base/MapServer/tile/{z}/{y}/{x}"),
    "ESRI.NatGeoWorldMap"    = esri_raster_provider("https://server.arcgisonline.com/ArcGIS/rest/services/NatGeo_World_Map/MapServer/tile/{z}/{y}/{x}"),

    # --- ESRI vector tile services ---
    "ESRI.WorldBasemap" = esri_vector_provider("https://basemaps.arcgis.com/arcgis/rest/services/World_Basemap_v2/VectorTileServer"),

    # --- OSM raster ---
    "OSM.Standard" = raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png"),
    "OSM.CyclOSM"  = raster_provider("https://tile-cyclosm.openstreetmap.fr/cyclosm/{z}/{x}/{y}.png"),

    # --- Stadia (Stamen successors; key optional for low-volume use) ---
    "Stadia.AlidadeSmooth" = {
      url <- "https://tiles.stadiamaps.com/tiles/alidade_smooth/{z}/{x}/{y}.png"
      if (!is.null(api_key)) url <- paste0(url, "?api_key=", api_key)
      raster_provider(url)
    },
    "Stadia.AlidadeSmoothDark" = {
      url <- "https://tiles.stadiamaps.com/tiles/alidade_smooth_dark/{z}/{x}/{y}.png"
      if (!is.null(api_key)) url <- paste0(url, "?api_key=", api_key)
      raster_provider(url)
    },
    "Stadia.OSMBright" = {
      url <- "https://tiles.stadiamaps.com/tiles/osm_bright/{z}/{x}/{y}.png"
      if (!is.null(api_key)) url <- paste0(url, "?api_key=", api_key)
      raster_provider(url)
    },
    "Stadia.StamenToner" = {
      url <- "https://tiles.stadiamaps.com/tiles/stamen_toner/{z}/{x}/{y}.png"
      if (!is.null(api_key)) url <- paste0(url, "?api_key=", api_key)
      raster_provider(url)
    },
    "Stadia.StamenTerrain" = {
      url <- "https://tiles.stadiamaps.com/tiles/stamen_terrain/{z}/{x}/{y}.png"
      if (!is.null(api_key)) url <- paste0(url, "?api_key=", api_key)
      raster_provider(url)
    },
    "Stadia.StamenWatercolor" = {
      url <- "https://tiles.stadiamaps.com/tiles/stamen_watercolor/{z}/{x}/{y}.png"
      if (!is.null(api_key)) url <- paste0(url, "?api_key=", api_key)
      raster_provider(url)
    },

    # --- fallthrough: unknown name ---
    stop(sprintf("Unknown provider: '%s'. See ?basemap_provider for available names.", name))
  )
}

#' @title WMS provider style
#'
#' Build a MapLibre GL Style JSON string that uses a WMS GetMap endpoint as a
#' raster tile source.  The Rust tile engine substitutes \code{\{xmin\}},
#' \code{\{ymin\}}, \code{\{xmax\}}, \code{\{ymax\}} with EPSG:3857 coordinates
#' for each 256×256 tile.
#'
#' @param url Character: base WMS URL (without query parameters).
#' @param layers Character: value for the \code{LAYERS} parameter.
#' @param styles Character: value for the \code{STYLES} parameter (default \code{""}).
#' @param version Character: WMS version string (default \code{"1.3.0"}).
#' @param format Character: image MIME type (default \code{"image/png"}).
#' @param transparent Logical: request transparent PNG tiles (default \code{FALSE}).
#' @param extra_params Character or \code{NULL}: additional query parameters appended verbatim.
#' @return A JSON character string ready to pass to \code{render_basemap_raw()}.
#' @export
wms_provider <- function(url, layers, styles = "", version = "1.3.0",
                         format = "image/png", transparent = FALSE,
                         extra_params = NULL) {
  tile_url <- paste0(
    url, "?SERVICE=WMS&VERSION=", version, "&REQUEST=GetMap",
    "&LAYERS=", utils::URLencode(layers, reserved = TRUE),
    "&STYLES=", utils::URLencode(styles, reserved = TRUE),
    "&CRS=EPSG:3857",
    "&BBOX={xmin},{ymin},{xmax},{ymax}",
    "&WIDTH=256&HEIGHT=256",
    "&FORMAT=", utils::URLencode(format, reserved = TRUE),
    "&TRANSPARENT=", if (transparent) "TRUE" else "FALSE"
  )
  if (!is.null(extra_params)) tile_url <- paste0(tile_url, "&", extra_params)
  raster_provider(tile_url, tile_size = 256L)
}

#' @title WMTS provider style
#'
#' Build a MapLibre GL Style JSON string that uses a WMTS GetTile KVP endpoint
#' as a raster tile source.
#'
#' @param url Character: base WMTS URL (without query parameters).
#' @param layer Character: value for the \code{LAYER} parameter.
#' @param tilematrixset Character: tile matrix set (default \code{"GoogleMapsCompatible"}).
#' @param format Character: image MIME type (default \code{"image/png"}).
#' @param style Character: style identifier (default \code{"default"}).
#' @param version Character: WMTS version string (default \code{"1.0.0"}).
#' @param extra_params Character or \code{NULL}: additional query parameters appended verbatim.
#' @return A JSON character string ready to pass to \code{render_basemap_raw()}.
#' @export
wmts_provider <- function(url, layer, tilematrixset = "GoogleMapsCompatible",
                          format = "image/png", style = "default",
                          version = "1.0.0", extra_params = NULL) {
  tile_url <- paste0(
    url, "?SERVICE=WMTS&REQUEST=GetTile&VERSION=", version,
    "&LAYER=", utils::URLencode(layer, reserved = TRUE),
    "&STYLE=", utils::URLencode(style, reserved = TRUE),
    "&FORMAT=", utils::URLencode(format, reserved = TRUE),
    "&TILEMATRIXSET=", utils::URLencode(tilematrixset, reserved = TRUE),
    "&TILEMATRIX={z}&TILEROW={y}&TILECOL={x}"
  )
  if (!is.null(extra_params)) tile_url <- paste0(tile_url, "&", extra_params)
  raster_provider(tile_url, tile_size = 256L)
}

#' @title PMTiles provider style
#'
#' Build a MapLibre GL Style JSON string for a PMTiles raster archive served
#' over HTTP.  The Rust core detects the \code{pmtiles://} scheme in the
#' \code{tiles} array and routes tile fetching through the PMTiles pipeline.
#'
#' @param url Character: HTTPS URL of the \code{.pmtiles} archive file.
#' @return A JSON character string ready to pass to \code{render_basemap_raw()}.
#' @export
pmtiles_provider <- function(url) {
  style <- list(
    version = 8L,
    sources = list(
      pmtiles = list(
        type     = "raster",
        tiles    = list(paste0("pmtiles://", url)),
        tileSize = 256L
      )
    ),
    layers = list(
      list(id = "pmtiles-layer", type = "raster", source = "pmtiles")
    )
  )
  jsonlite::toJSON(style, auto_unbox = TRUE)
}
