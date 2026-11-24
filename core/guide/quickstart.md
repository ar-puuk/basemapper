# A basemap in a few lines

A [`Basemap`] is what to draw: a style (or several, stacked), what you give
of the view (an extent, or a centre and zoom; the zoom; the image size; the
dpi), how to fetch it, and an optional clip. Any three of extent, zoom, image
size and dpi give the fourth ([`view`]); what nothing gives is defaulted, and
said so. A catalogue name (`"OpenFreeMap.Positron"`, see [`providers`]), a
MapLibre style URL, style JSON, tile URL, TileJSON, PMTiles archive or ArcGIS
service works as a style.

