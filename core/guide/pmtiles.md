# PMTiles

A [PMTiles](https://docs.protomaps.com/pmtiles/) archive holds a whole
tileset in one file, read with HTTP range requests or from disk: no tile
server needed. A raster archive works directly as a style. A vector archive
gets a default style over every source layer, or draws with a full basemap
style whose source points at it: here the
[Protomaps](https://github.com/protomaps/basemaps) light and dark styles
(in the crate's `guide/styles/`, which explains how they were made) over an
archive of Florence.

