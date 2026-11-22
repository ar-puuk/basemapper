# Protomaps basemap styles

`protomaps-light.json` and `protomaps-dark.json` are the Protomaps basemap
styles, light and dark, pointed at a small public PMTiles archive of Florence
(`https://pmtiles.io/protomaps(vector)ODbL_firenze.pmtiles`, Protomaps tiles
v3, © OpenStreetMap contributors, ODbL). The documentation's PMTiles examples
in R, Python and Rust render them.

To use one with your own Protomaps archive, change the source's `url` to
`pmtiles://` followed by your archive's URL or path.

They were written by `tools/protomaps-style/generate.cjs` from
[protomaps-themes-base](https://github.com/protomaps/basemaps) 3.1.0, the
styles for Protomaps tiles v3. Archives built from late 2024 on use tiles v4
and need the styles of `@protomaps/basemaps` 5.x instead (same script, see its
comments). The styles are BSD-3-Clause and their visual design CC0: see
[LICENSE-protomaps.md](LICENSE-protomaps.md).
