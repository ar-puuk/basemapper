# Your own tiles and services

Besides named basemaps and MapLibre styles, `style` takes tile data: a tile URL template, TileJSON, a PMTiles archive (URL or local file), or an ArcGIS service. basemapper detects whether the tiles are raster or vector and builds a style for them; [tile_style()](reference/tile_style.html#basemapper.tile_style) builds one with your overrides. OGC services (WMS, WMTS) have their own style builders.


``` python
import basemapper as bm
```


# A tile URL

An XYZ template, as raster tiles:


``` python
bm.basemap("https://tile.opentopomap.org/{z}/{x}/{y}.png", 720, 405,
           center=(-111.65, 40.60), zoom=11)
```


<figure class="figure">
<p><img src="own-data_files/figure-html/cell-3-output-1.png" class="figure-img" width="720" height="405" /></p>
</figure>


# PMTiles

A [PMTiles](https://docs.protomaps.com/pmtiles/) archive is read with HTTP range requests, or from a local file. Raster archives draw as they are; here a USGS topographic map of Mount Whitney, fitted to the archive's extent:


``` python
bm.basemap("https://pmtiles.io/usgs-mt-whitney-8-15-webp-512.pmtiles", 720, 405,
           bounds=[-118.3198, 36.5611, -118.2607, 36.5930])
```


<figure class="figure">
<p><img src="own-data_files/figure-html/cell-4-output-1.png" class="figure-img" width="720" height="405" /></p>
</figure>


Vector archives get a default style over every source layer:


``` python
firenze = "https://pmtiles.io/protomaps(vector)ODbL_firenze.pmtiles"
bm.basemap(firenze, 720, 405, center=(11.255, 43.773), zoom=13.5)
```


<figure class="figure">
<p><img src="own-data_files/figure-html/cell-5-output-1.png" class="figure-img" width="720" height="405" /></p>
</figure>


[tile_style()](reference/tile_style.html#basemapper.tile_style) sets which source layers it draws, and its paint:


``` python
print(sorted({l["source_layer"] for l in bm.list_layers(firenze)}))

styled = bm.tile_style(
    firenze,
    source_layers=["earth", "water", "landuse", "roads", "buildings"],
    paint={"fill-color": "#f2efe9", "line-color": "#8a8a8a"},
)
bm.basemap(styled, 720, 405, center=(11.255, 43.773), zoom=13.5)
```


    ['boundaries', 'buildings', 'earth', 'landcover', 'landuse', 'natural', 'physical_line', 'physical_point', 'places', 'pois', 'roads', 'transit', 'water']


<figure class="figure">
<p><img src="own-data_files/figure-html/cell-6-output-2.png" class="figure-img" width="720" height="405" /></p>
</figure>


## With a basemap style

A vector archive can also be drawn with a full basemap style whose source points at it. [protomaps-light.json](../../styles/protomaps-light.json) and [protomaps-dark.json](../../styles/protomaps-dark.json) are the [Protomaps](https://github.com/protomaps/basemaps) basemap styles with the Florence archive as their source (`"url": "pmtiles://https://pmtiles.io/protomaps(vector)ODbL_firenze.pmtiles"`), and a style file's path works as `style`:


``` python
bm.basemap("styles/protomaps-light.json", 720, 405, center=(11.2551, 43.7710), zoom=15.2)
```


<figure class="figure">
<p><img src="own-data_files/figure-html/cell-7-output-1.png" class="figure-img" width="720" height="405" /></p>
</figure>


``` python
bm.basemap("styles/protomaps-dark.json", 720, 405, center=(11.2551, 43.7710), zoom=15.2)
```


<figure class="figure">
<p><img src="own-data_files/figure-html/cell-8-output-1.png" class="figure-img" width="720" height="405" /></p>
</figure>


For your own Protomaps archive, change the source's `url` to `pmtiles://` and the archive's URL or path. These styles are for Protomaps tiles v3; archives built from late 2024 on use tiles v4, with the styles of `@protomaps/basemaps` 5 ([how the styles were made](../../styles/README.md)).


# TileJSON

A TileJSON URL works the same way; this is OpenFreeMap's planet tileset, with the default vector style:


``` python
bm.basemap("https://tiles.openfreemap.org/planet", 720, 405,
           center=(-111.8887, 40.7556), zoom=12)
```


<figure class="figure">
<p><img src="own-data_files/figure-html/cell-9-output-1.png" class="figure-img" width="720" height="405" /></p>
</figure>


# Your tiles in a basemap's style

`basemap_provider(name, tiles=)` keeps a basemap's style and swaps in your own tiles, which must use the same schema (OpenFreeMap's styles use OpenMapTiles): self-hosted tiles, or an extract for offline use.


``` python
style = bm.basemap_provider("OpenFreeMap.Positron", tiles="https://tiles.openfreemap.org/planet")
bm.basemap(style, 720, 405, center=(-111.8887, 40.7556), zoom=12)
```


<figure class="figure">
<p><img src="own-data_files/figure-html/cell-10-output-1.png" class="figure-img" width="720" height="405" /></p>
</figure>


# OGC services

[wms_style()](reference/wms_style.html#basemapper.wms_style) and [wmts_style()](reference/wmts_style.html#basemapper.wmts_style) build styles for OGC services. NASA's Blue Marble, from GIBS:


``` python
gibs = bm.wms_style("https://gibs.earthdata.nasa.gov/wms/epsg3857/best/wms.cgi",
                    layers="BlueMarble_ShadedRelief_Bathymetry", format="image/jpeg")
bm.basemap(gibs, 720, 360, center=(0, 20), zoom=0.6)
```


<figure class="figure">
<p><img src="own-data_files/figure-html/cell-11-output-1.png" class="figure-img" width="720" height="360" /></p>
</figure>


# In plots

Any of these goes in [add_basemap()](reference/add_basemap.html#basemapper.add_basemap) and [geom_basemap()](reference/geom_basemap.html#basemapper.geom_basemap):


``` python
import geodatasets
import geopandas as gpd
import matplotlib.pyplot as plt

nc = gpd.read_file(geodatasets.get_path("geoda.sids"))
fig, ax = plt.subplots(figsize=(8, 4.4))
nc.boundary.plot(ax=ax, color="black", linewidth=0.4)
ax.set_xlim(-84.4, -81.2)
ax.set_ylim(34.9, 36.6)
bm.add_basemap(ax, "https://tile.opentopomap.org/{z}/{x}/{y}.png", alpha=0.8, crs=nc.crs)
ax.set_axis_off()
```


<figure class="figure">
<p><img src="own-data_files/figure-html/cell-12-output-1.png" class="figure-img" width="519" height="344" /></p>
</figure>


# Tile cache

Tiles are cached on disk, for as long as the servers' caching headers allow. [tile_cache()](reference/tile_cache.html#basemapper.tile_cache) shows where; `tile_cache(clear=True)` empties it, and `tile_cache(False)` turns it off for the session.
