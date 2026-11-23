// Write Protomaps basemap styles for a PMTiles archive.
//
// Usage: node generate.cjs PACKAGE_DIR OUT_DIR ARCHIVE_URL
//
// PACKAGE_DIR is an unpacked protomaps-themes-base 3.1.0 npm package (BSD-3-
// Clause; styles for Protomaps basemap tiles v3), for example:
//   curl -sLO https://registry.npmjs.org/protomaps-themes-base/-/protomaps-themes-base-3.1.0.tgz
//   tar xzf protomaps-themes-base-3.1.0.tgz        # -> package/
// For tiles v4 (archives built from late 2024 on) use @protomaps/basemaps 5.x
// instead, whose layers() takes namedFlavor(name) and { lang }.
const path = require("path");
const fs = require("fs");
const [pkg, out, archive] = process.argv.slice(2);
if (!archive) throw new Error("usage: node generate.cjs PACKAGE_DIR OUT_DIR ARCHIVE_URL");
const mod = require(path.resolve(pkg, "dist/cjs/index.cjs"));
const layers = mod.default || mod;
for (const flavor of ["light", "dark"]) {
  const style = {
    version: 8,
    name: `Protomaps ${flavor}`,
    sources: {
      protomaps: {
        type: "vector",
        url: `pmtiles://${archive}`,
        attribution: '<a href="https://github.com/protomaps/basemaps">Protomaps</a> © <a href="https://openstreetmap.org/copyright">OpenStreetMap</a>',
      },
    },
    glyphs: "https://protomaps.github.io/basemaps-assets/fonts/{fontstack}/{range}.pbf",
    sprite: `https://protomaps.github.io/basemaps-assets/sprites/v3/${flavor}`,
    layers: layers("protomaps", flavor),
  };
  const file = path.join(out, `protomaps-${flavor}.json`);
  fs.writeFileSync(file, JSON.stringify(style, null, 1) + "\n");
  console.log(file, style.layers.length, "layers");
}
