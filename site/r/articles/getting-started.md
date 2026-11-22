# Getting started with basemapper

> **Python user?** The Python package has the same functions and
> arguments; R’s `verb(m, ...)` is `m.verb(...)` there. See the [Python
> documentation](https://ar-puuk.github.io/basemapper/python/).

basemapper renders MapLibre GL basemaps headlessly with a Rust core: no
browser, display server, GPU or GDAL. Vector styles are drawn by a
software port of MapLibre GL JS’s renderer, so maps look as they do in
MapLibre.

## Installation

\
`remotes``::`[`install_github`](https://remotes.r-lib.org/reference/install_github.html)`(``"ar-puuk/basemapper"``, subdir ``=`` ``"r-basemapper"``)`

## Step 1 — Choose a style

Every function takes a `style`: a MapLibre GL style (URL, file or JSON),
a named basemap (`"CartoDB.Voyager"`, `"Esri.WorldImagery"`: the
leaflet-providers names), or tile data, whose style is built for you: a
tile URL, TileJSON, PMTiles archive (URL or local file) or ArcGIS
service. A vector of styles is stacked bottom to top.

\
[`library`](https://rdrr.io/r/base/library.html)`(`[`basemapper`](https://ar-puuk.github.io/basemapper/r/)`)`\
\
[`list_providers`](https://ar-puuk.github.io/basemapper/r/reference/list_providers.md)`(``)``                  ``# name, forms, API key variable, attribution`\
`positron`` ``<-`` ``"OpenFreeMap.Positron"``  ``# a name works as a style`\
`positron`` ``<-`` ``providers``$``OpenFreeMap.Positron``   ``# the same, with autocompletion`\
\
`# Pick the form of a basemap that has both; CARTO, MapTiler and Stadia need`\
`# a free API key (api_key =, or CARTO_API_KEY etc.)`\
`gray`` ``<-`` `[`basemap_provider`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)`(``"Esri.WorldGrayCanvas"``, type ``=`` ``"raster"``)`\
\
`# Your own tiles with a basemap's style (same schema, here OpenMapTiles)`\
`mine`` ``<-`` `[`basemap_provider`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)`(``"OpenFreeMap.Liberty"``, tiles ``=`` ``"data/omt.pmtiles"``)`\
\
`# Tile data works as a style directly`\
`osm``  ``<-`` ``"https://tile.openstreetmap.org/{z}/{x}/{y}.png"`\
`esri`` ``<-`` ``"https://basemaps.arcgis.com/arcgis/rest/services/World_Basemap_v2/VectorTileServer"`\
`area`` ``<-`` ``"data/area.pmtiles"``   ``# raster or vector, read from the archive`\
\
`# tile_style() overrides what is detected, e.g. the paint of the default`\
`# vector style, or which source layers it draws`\
`mvt`` ``<-`` `[`tile_style`](https://ar-puuk.github.io/basemapper/r/reference/tile_style.md)`(``"https://example.com/tiles/{z}/{x}/{y}.mvt"``, source_layers ``=`` ``"land"``,`\
`                  paint ``=`` `[`list`](https://rdrr.io/r/base/list.html)`(``"fill-color"`` ``=`` ``"#e8e0d8"``, ``"line-color"`` ``=`` ``"#aaaaaa"``)``)`\
\
`# OGC services`\
`relief`` ``<-`` `[`wms_style`](https://ar-puuk.github.io/basemapper/r/reference/ogc_styles.md)`(``"https://example.com/wms"``, layers ``=`` ``"relief"``)`

[`list_layers()`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md)
lists a style’s layers (id and type) in drawing order:

\
[`list_layers`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md)`(``positron``)`

## Step 2 — A map on its own

[`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)
creates a view: a centre and MapLibre zoom, or bounds to fit. `width`
and `height` are CSS pixels; `pixel_ratio` makes the image larger with
the same view and label layout (`pixel_ratio = 2` for a sharp 2x image,
`dpi / 96` for print).

\
`m`` ``<-`` `[`basemap`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)`(``positron``, ``800``, ``600``, center ``=`` `[`c`](https://rdrr.io/r/base/c.html)`(``-``111.8887``, ``40.7556``)``, zoom ``=`` ``12.4``,`\
`             pixel_ratio ``=`` ``2``)`\
`m`\
`#> <basemap> 1 style, 800 x 600 @ 2x (1600 x 1200 px), center (-111.88870, 40.75560), zoom 12.400`\
\
[`render_png`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)`(``m``, path ``=`` ``"slc.png"``)`\
[`render_svg`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)`(``m``, path ``=`` ``"slc.svg"``)``   ``# true vector, labels as editable text`\
`r`` ``<-`` `[`render_raster`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)`(``m``)``             ``# terra SpatRaster in EPSG:3857`\
`a`` ``<-`` `[`render_array`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)`(``m``)``              ``# [1200, 1600, 4] integer array`

Change the view with
[`set_view()`](https://ar-puuk.github.io/basemapper/r/reference/set_view.md)
and
[`set_bounds()`](https://ar-puuk.github.io/basemapper/r/reference/set_view.md),
and read it with
[`get_bounds()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md),
[`get_center()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md),
[`get_zoom()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md)
and
[`get_size()`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md):

\
`nc`` ``<-`` ``sf``::`[`st_read`](https://r-spatial.github.io/sf/reference/st_read.html)`(`[`system.file`](https://rdrr.io/r/base/system.file.html)`(``"shape/nc.shp"``, package ``=`` ``"sf"``)``, quiet ``=`` ``TRUE``)`\
`m`` ``<-`` `[`set_bounds`](https://ar-puuk.github.io/basemapper/r/reference/set_view.md)`(``m``, ``nc``)``            ``# fit an sf object's extent`\
[`get_zoom`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md)`(``m``)`\
[`get_bounds`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md)`(``m``, lonlat ``=`` ``TRUE``)`\
`m`` ``<-`` `[`set_view`](https://ar-puuk.github.io/basemapper/r/reference/set_view.md)`(``m``, zoom ``=`` `[`get_zoom`](https://ar-puuk.github.io/basemapper/r/reference/get_bounds.md)`(``m``)`` ``+`` ``1``)`

## Step 3 — Render with {ggplot2}

[`geom_basemap()`](https://ar-puuk.github.io/basemapper/r/reference/geom_basemap.md)
renders when the plot is drawn, for the panel’s extent and at the
device’s resolution, so `ggsave(dpi = 300)` gets a basemap rendered at
300 dpi:

\
[`library`](https://rdrr.io/r/base/library.html)`(`[`ggplot2`](https://ggplot2.tidyverse.org)`)`\
\
[`ggplot`](https://ggplot2.tidyverse.org/reference/ggplot.html)`(``nc``)`` ``+`\
`  `[`geom_basemap`](https://ar-puuk.github.io/basemapper/r/reference/geom_basemap.md)`(``positron``)`` ``+`\
`  `[`geom_sf`](https://ggplot2.tidyverse.org/reference/ggsf.html)`(``fill ``=`` ``NA``, colour ``=`` ``"steelblue"``, linewidth ``=`` ``0.4``)`` ``+`\
`  `[`coord_sf`](https://ggplot2.tidyverse.org/reference/ggsf.html)`(``)`` ``+`\
`  `[`theme_void`](https://ggplot2.tidyverse.org/reference/ggtheme.html)`(``)`

## Step 4 — Render with {tmap}

[`tm_basemap()`](https://ar-puuk.github.io/basemapper/r/reference/tm_basemap.md)
is a {tmap} v4 element that takes its extent from the active
[`tm_shape()`](https://r-tmap.github.io/tmap/reference/tm_shape.html):

\
[`library`](https://rdrr.io/r/base/library.html)`(`[`tmap`](https://github.com/r-tmap/tmap)`)`\
\
[`tm_shape`](https://r-tmap.github.io/tmap/reference/tm_shape.html)`(``nc``)`` ``+`\
`  `[`tm_basemap`](https://ar-puuk.github.io/basemapper/r/reference/tm_basemap.md)`(``positron``)`` ``+`\
`  `[`tm_borders`](https://r-tmap.github.io/tmap/reference/tm_polygons.html)`(``col ``=`` ``"steelblue"``)`

The plot layers share these arguments (in Python too):

| Argument | Default | Description |
|----|----|----|
| `style` | — | Style URL / JSON, or a vector stacked bottom to top |
| `tile_zoom` | `NULL` (as MapLibre) | Tile detail override on the 256 px grid |
| `alpha` | `1` | Basemap opacity |
| `layers` | `NULL` (all) | Layer filter — see below |
| `labels` | `TRUE` | `FALSE` drops labels, `"only"` keeps just them — see below |
| `features` | `NULL` (all) | Feature filter — see below |
| `pixel_ratio` | device dpi / 96 | Device pixels per CSS pixel |
| `auth_token` | `NULL` | Access token for the tile service |
| `tile_timeout_ms` | `10000` | Per-tile HTTP timeout |
| `max_tiles` | `256` | Cap on tiles fetched per style |
| `attribution` | `TRUE` | The providers’ credit line in the corner; `FALSE` or your own text |

## Step 5 — Labels, features and layers

`labels = FALSE` draws a style without its labels (text and icons);
`labels = "only"` draws just the labels, on a transparent background, to
go over another map. `features` keeps or drops parts of the map by what
they show: background, water, landcover, parks, buildings, roads, rail,
boundaries, places, poi and terrain.
[`list_layers()`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md)
shows each layer’s feature.

\
[`list_layers`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md)`(``positron``)``                       ``# id, type, source_layer, feature`\
[`list_layers`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md)`(``positron``, labels ``=`` ``FALSE``, features ``=`` `[`c`](https://rdrr.io/r/base/c.html)`(``"water"``, ``"roads"``)``)``   ``# preview`\
\
`# Water and roads only, without their names`\
[`render_png`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)`(`[`basemap`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)`(``positron``, ``800``, ``600``, bounds ``=`` ``nc``, labels ``=`` ``FALSE``,`\
`                   features ``=`` `[`c`](https://rdrr.io/r/base/c.html)`(``"water"``, ``"roads"``)``)``, path ``=`` ``"nc-lines.png"``)`\
\
`# Everything but buildings and points of interest`\
[`ggplot`](https://ggplot2.tidyverse.org/reference/ggplot.html)`(``nc``)`` ``+`` `[`geom_basemap`](https://ar-puuk.github.io/basemapper/r/reference/geom_basemap.md)`(``positron``, features ``=`` `[`c`](https://rdrr.io/r/base/c.html)`(``"-buildings"``, ``"-poi"``)``)`

On the renderers these apply to every style of a stack. To filter one
style of a stack, pass them to
[`basemap_provider()`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md);
this puts satellite imagery under a vector basemap’s labels:

\
`hybrid`` ``<-`` `[`c`](https://rdrr.io/r/base/c.html)`(``"Esri.WorldImagery"``,`\
`            `[`basemap_provider`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)`(``"OpenFreeMap.Liberty"``, labels ``=`` ``"only"``)``)`\
[`render_png`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)`(`[`basemap`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)`(``hybrid``, ``800``, ``600``, bounds ``=`` ``nc``)``, path ``=`` ``"nc-hybrid.png"``)`\
\
`# Raster basemaps switch to their label-free or label-only tiles`\
`relief`` ``<-`` `[`c`](https://rdrr.io/r/base/c.html)`(``"Esri.WorldShadedRelief"``, `[`basemap_provider`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)`(``"Esri.WorldShadedRelief"``, labels ``=`` ``"only"``)``)`

Features are a convenience, sorted from each layer’s source layer (see
the `source_layer` and `feature` columns of
[`list_layers()`](https://ar-puuk.github.io/basemapper/r/reference/list_layers.md)).
For exact control, pass layer IDs to `layers` to keep only those layers,
or prefix IDs with `"-"` to drop them; `*` and `?` are wildcards.
Keeping and dropping cannot be mixed. Names that match no layer warn.

\
`# Without town and village labels`\
[`render_png`](https://ar-puuk.github.io/basemapper/r/reference/render_png.md)`(`[`basemap`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)`(``positron``, ``800``, ``600``, bounds ``=`` ``nc``, layers ``=`` `[`c`](https://rdrr.io/r/base/c.html)`(``"-label_town"``, ``"-label_village"``)``)``,`\
`           path ``=`` ``"nc.png"``)`\
\
[`ggplot`](https://ggplot2.tidyverse.org/reference/ggplot.html)`(``nc``)`` ``+`` `[`geom_basemap`](https://ar-puuk.github.io/basemapper/r/reference/geom_basemap.md)`(``positron``, layers ``=`` `[`c`](https://rdrr.io/r/base/c.html)`(``"background"``, ``"water"``)``)`

Tiles are cached on disk
([`tile_cache()`](https://ar-puuk.github.io/basemapper/r/reference/tile_cache.md)
shows where, moves the cache or clears it), for as long as the servers’
caching headers allow.

Basemap providers ask maps to credit them. The plot layers draw the
credit line in the bottom-right corner (`attribution = FALSE` turns it
off, `attribution = "..."` replaces it), and
[`get_attribution()`](https://ar-puuk.github.io/basemapper/r/reference/get_attribution.md)
returns it, for a caption:

\
[`ggplot`](https://ggplot2.tidyverse.org/reference/ggplot.html)`(``nc``)`` ``+`\
`  `[`geom_basemap`](https://ar-puuk.github.io/basemapper/r/reference/geom_basemap.md)`(``positron``, attribution ``=`` ``FALSE``)`` ``+`\
`  `[`geom_sf`](https://ggplot2.tidyverse.org/reference/ggsf.html)`(``fill ``=`` ``NA``)`` ``+`\
`  `[`labs`](https://ggplot2.tidyverse.org/reference/labs.html)`(``caption ``=`` `[`get_attribution`](https://ar-puuk.github.io/basemapper/r/reference/get_attribution.md)`(``positron``)``)`

## Step 6 — One-shot rendering

[`render_basemap_raw()`](https://ar-puuk.github.io/basemapper/r/reference/render_basemap_raw.md)
renders bounds in one call and returns the pixels with their extent, for
custom compositing or export:

\
`a`` ``<-`` `[`render_basemap_raw`](https://ar-puuk.github.io/basemapper/r/reference/render_basemap_raw.md)`(``nc``, ``800``, ``600``, ``positron``)`\
\
[`dim`](https://rdrr.io/r/base/dim.html)`(``a``)``                ``# 600 800 4`\
[`attr`](https://rdrr.io/r/base/attr.html)`(``a``, ``"crs_epsg"``)``   ``` # 4267, the CRS of `nc` ``\
`png``::`[`writePNG`](https://rdrr.io/pkg/png/man/writePNG.html)`(``a`` ``/`` ``255``, ``"basemap.png"``)`
