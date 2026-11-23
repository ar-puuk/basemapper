# A basemap in a few lines

A [`Basemap`] is a view: a style (or several, stacked), a size in CSS pixels,
a centre and MapLibre zoom (or bounds to fit), and a device pixel ratio.
[`providers::basemap_provider`] turns a
[leaflet-providers](https://leaflet-extras.github.io/leaflet-providers/preview/)
name into a style; a MapLibre style URL, style JSON, tile URL, TileJSON,
PMTiles archive or ArcGIS service works directly as a style too.

