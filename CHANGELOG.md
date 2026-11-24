# Changelog

Every change to basemapper, from the commit history (written by
`python tools/changelog.py`; do not edit by hand). Each section is a
merge into `main`, newest first, led by its version tag. What changed
for users of each package,
in more words: [R](r-basemapper/NEWS.md) and
[Python](py-basemapper/CHANGELOG.md).

## v0.3.1

Tagged v0.3.1 when merged into `main`; until then on `dev`.

### Fixes

- **core:** draw sizes that are not whole CSS pixels without resampling ([e6e9f96](https://github.com/ar-puuk/basemapper/commit/e6e9f96212229ec67e7606f9b7f8df61df183070))

### Documentation

- keep the audit and review notes ([03beae3](https://github.com/ar-puuk/basemapper/commit/03beae342833dfc74067e84a55413526b3e2d700))

### Maintenance

- release 0.3.1 ([c078ad0](https://github.com/ar-puuk/basemapper/commit/c078ad06ed00a9cb6718f3d611c49d6887f9c659))

## [v0.3.0](https://github.com/ar-puuk/basemapper/releases/tag/v0.3.0) (2026-11-23): [#9](https://github.com/ar-puuk/basemapper/pull/9) Release v0.3.0: one model, one API for R, Python and Rust

### Features

- **r:** default alt text for basemaps shown in knitr ([1118ab9](https://github.com/ar-puuk/basemapper/commit/1118ab9cff78fc6e1f4110de77cb0646e9600e8a))
- **r:** the R package as an adapter over the core (phase 4) ([62b76a1](https://github.com/ar-puuk/basemapper/commit/62b76a10397b434f89c038b196e0b266ed8faf29))
- **python:** the Python package as an adapter over the core (phase 3) ([06e1edf](https://github.com/ar-puuk/basemapper/commit/06e1edf5112d8016e58e5031c1df4204e9b133ed))
- **core:** plot support (phase 2) ([77dea13](https://github.com/ar-puuk/basemapper/commit/77dea13f20e88c36c7c1a7cbed504446197852ec))
- **core:** the Rust facade (phase 1e) ([b3e100a](https://github.com/ar-puuk/basemapper/commit/b3e100a7a7c4141b21d3449c7f33cec3b38e4575))
- **core:** warping and clip masks (phase 1d) ([704511d](https://github.com/ar-puuk/basemapper/commit/704511d6b69e72c86356704e7929a626e033faa3))
- **core:** renderings and notes (phase 1c) ([87def37](https://github.com/ar-puuk/basemapper/commit/87def3702355bc6a66ebf3094d17e86c68c30242))
- **core:** styles as values (phase 1b) ([81d0797](https://github.com/ar-puuk/basemapper/commit/81d079703e58d665a9a2a75690e89796bdf20556))
- **core:** view resolution (phase 1a) ([f5de9a2](https://github.com/ar-puuk/basemapper/commit/f5de9a25ee7dfc116811ed051e734d870fca2d9d))

### Fixes

- **docs,test:** 0.3 reference indexes; draw plotnine figures with Agg ([8377b74](https://github.com/ar-puuk/basemapper/commit/8377b740dfa7e802631e9362f9b25edd16d87f7d))
- views never reach past the poles; the default view renders ([76fdf91](https://github.com/ar-puuk/basemapper/commit/76fdf91eb59d91e2173c4f7ae2a49672909c0087))
- **deps:** read fonts with skrifa instead of the unmaintained ttf-parser ([d2f9008](https://github.com/ar-puuk/basemapper/commit/d2f900826fd07bae3b717bb560487ecb4d28c371))

### Documentation

- **python:** alt text for every figure in the user guide ([af88955](https://github.com/ar-puuk/basemapper/commit/af889550d13198c62fb008671ae6b51644fcea27))
- **r:** alt text for the ggplot2 and tmap figures in the articles ([03b66be](https://github.com/ar-puuk/basemapper/commit/03b66be1ccdea3850a9ca080b8542acefad74a34))
- alt text for the PNG / SVG comparison images ([e17dc60](https://github.com/ar-puuk/basemapper/commit/e17dc60c2afdfeb8de32a16bd5393e095f279cfb))
- **changelog:** the v0.3.0 section, on dev ([f3f38f7](https://github.com/ar-puuk/basemapper/commit/f3f38f7bcc70a2492d74fa31b74eb1b8cc2ab3db))
- **changelog:** the v0.3.0 section ([a2c92d1](https://github.com/ar-puuk/basemapper/commit/a2c92d1702d110552858aa064ef3fbcf0f3d865e))
- the 0.3 contract, guides, migration guide and version 0.3.0 (phase 5) ([4aaa7d5](https://github.com/ar-puuk/basemapper/commit/4aaa7d5b3913b61c0f7fb43d9207d19c39e6bed2))
- **design:** API and design review, and the 0.3.0 implementation plan ([b8ca3d5](https://github.com/ar-puuk/basemapper/commit/b8ca3d52083074b5062400f09a03aa7a31c7bfd5))

### Tests

- phase 0 safety nets for the 0.3.0 redesign ([637b955](https://github.com/ar-puuk/basemapper/commit/637b95536d5600fbf0a29bdc0019662916d276f2))

## [v0.2.0](https://github.com/ar-puuk/basemapper/releases/tag/v0.2.0) (2026-11-23): [#8](https://github.com/ar-puuk/basemapper/pull/8) Release v0.2.0: safer downloads, faster redraws, simpler arguments

### Features

- fetch options, named arguments, classed R errors, optional sf/terra/matplotlib ([a70bc71](https://github.com/ar-puuk/basemapper/commit/a70bc71e7d7a129737c0deb7177aa333db4f7b13))

### Fixes

- **python:** plot integrations import plainly; CI installs the plot extras ([36b11f1](https://github.com/ar-puuk/basemapper/commit/36b11f115906f160b08498dabb164133dde40814))
- **deps:** update dependencies with security advisories; PyO3 0.29 ([bfa32fc](https://github.com/ar-puuk/basemapper/commit/bfa32fc3bf0a8150e63541cde803bffdca16f361))
- **cache:** never keep responses from the local machine in memory ([94ca02c](https://github.com/ar-puuk/basemapper/commit/94ca02caa53ce7fac5a9698a95533cd98061bece))
- enforce max_tiles for vector tiles; scope tokens; harden downloads ([5dcba03](https://github.com/ar-puuk/basemapper/commit/5dcba03aa61101ceef89057c2f7fc6d6d4ab8c3e))

### Performance

- cache resources and decoded tiles, share the HTTP client, decode in parallel ([77fc81c](https://github.com/ar-puuk/basemapper/commit/77fc81c2acce25d7669ec31f414a2583067be2da))

### Documentation

- **changelog:** the v0.2.0 section ([a9d2465](https://github.com/ar-puuk/basemapper/commit/a9d2465309cf4904ef4650f9a0f26d13ce4f3e63))
- **changelog:** the v0.2.0 section ([db73aee](https://github.com/ar-puuk/basemapper/commit/db73aeee1861197937fcde914e43baff91421469))
- install instructions install the latest release, never a pinned version ([ffb22df](https://github.com/ar-puuk/basemapper/commit/ffb22df731dd656e4bf9868b8a2327e47ced7a94))
- CITATION.cff for GitHub's "Cite this repository" ([6a69b01](https://github.com/ar-puuk/basemapper/commit/6a69b01e76eb6847301e861f4e9b47e0c5d8e189))
- Zenodo DOI badge for the package (all versions) ([6e6f112](https://github.com/ar-puuk/basemapper/commit/6e6f112646f10df4a8cd4688b563503a262f1fae))

### CI

- read-only token, clippy bug lints, RustSec audit; R builds --locked ([7ff4e3a](https://github.com/ar-puuk/basemapper/commit/7ff4e3a45e586354292fd19125ced044e03a5fa2))

### Maintenance

- release 0.2.0 ([196ecdb](https://github.com/ar-puuk/basemapper/commit/196ecdbf8e1e27aee356ef05c80adb7b36844d36))

## [v0.1.1](https://github.com/ar-puuk/basemapper/releases/tag/v0.1.1) (2026-11-22): [#7](https://github.com/ar-puuk/basemapper/pull/7) Release v0.1.1: install pins, fresh Python docs builds

### Documentation

- **changelog:** the v0.1.1 section ([3e115af](https://github.com/ar-puuk/basemapper/commit/3e115af4f59d0c63cb5f4ff1856c8294414c3da7))
- stop tracking great-docs' _freeze; CI runs every Python page ([fa0105b](https://github.com/ar-puuk/basemapper/commit/fa0105b6043d863de9074b3880e01913020de453))
- install instructions pinned to the v0.1.0 release ([7cf2ff3](https://github.com/ar-puuk/basemapper/commit/7cf2ff33c09732e871a53da77f7c13f557854a0d))
- **changelog:** link the v0.1.0 section to its tag ([4ae94d3](https://github.com/ar-puuk/basemapper/commit/4ae94d38bf2e77e06287736bbd831a70cffe9226))

### Maintenance

- release 0.1.1 ([b07bd42](https://github.com/ar-puuk/basemapper/commit/b07bd42363907306590d7775cd8f9fb50108deb8))

## [v0.1.0](https://github.com/ar-puuk/basemapper/releases/tag/v0.1.0) (2026-11-22): [#6](https://github.com/ar-puuk/basemapper/pull/6) Release v0.1.0: tile cache upkeep, changelog, one reported version

### Features

- one version for R, Python and Rust, reported and checked ([91534f2](https://github.com/ar-puuk/basemapper/commit/91534f23799b6a9fc091598693d37b10012ddc2b))
- **cache:** keep the tile cache small; document where it lives ([3ca096d](https://github.com/ar-puuk/basemapper/commit/3ca096db19711b06caa2b72e30d898eb5e86e048))

### Documentation

- **changelog:** sections led by their version tags ([74d26e2](https://github.com/ar-puuk/basemapper/commit/74d26e24d6ecd3ffaeffcf226a005d106bbb4c07))
- a root CHANGELOG.md, written from the commit history ([7e40bb1](https://github.com/ar-puuk/basemapper/commit/7e40bb19ec6ba599272512fa1d9fe81b14882117))

### Maintenance

- **r:** remove the unused alternative logo (logo-alt1.svg) ([ab1ac59](https://github.com/ar-puuk/basemapper/commit/ab1ac594e88832fc4026679ee15c136e3d470031))

## [v0.0.6](https://github.com/ar-puuk/basemapper/releases/tag/v0.0.6) (2026-11-22): [#5](https://github.com/ar-puuk/basemapper/pull/5) UGRC basemaps: vector stacks and every UGRC Discover layer

### Features

- UGRC basemaps of Utah: vector stacks and every UGRC Discover layer ([afb468c](https://github.com/ar-puuk/basemapper/commit/afb468ca5715ce9ae47fc2c1788eb61cc65c052e))

### CI

- live UGRC Discover test with the quad word from a repository secret ([64b990a](https://github.com/ar-puuk/basemapper/commit/64b990a52c225b61df1319688f631f7c28d0f2c0))

## [v0.0.5](https://github.com/ar-puuk/basemapper/releases/tag/v0.0.5) (2026-11-22): [#4](https://github.com/ar-puuk/basemapper/pull/4) Refined documentation site, CI-built docs, stack-wide filters

### Fixes

- **core:** label, feature and layer filters treat a stack as one basemap ([845eb2a](https://github.com/ar-puuk/basemapper/commit/845eb2a84a34ba2c0cb8e7e0045a7a466d7a8c0e))

### Documentation

- build the site in CI; freeze the Python guide; document the process ([8840d51](https://github.com/ar-puuk/basemapper/commit/8840d51404f2a237bd242b956515217c9fbc85b1))
- refined site with executed examples in R, Python and Rust ([04132e2](https://github.com/ar-puuk/basemapper/commit/04132e22ffcda42d2f90ff707b4ce51e2b906013))

### CI

- **docs:** build and deploy the site from CI; stop committing site/ ([95a96f0](https://github.com/ar-puuk/basemapper/commit/95a96f0f1c4c5c7d8b8549b7d78189911329169e))

## [v0.0.4](https://github.com/ar-puuk/basemapper/releases/tag/v0.0.4) (2026-11-22): [#3](https://github.com/ar-puuk/basemapper/pull/3) CPU renderer, one API across R/Python/Rust, basemap catalogue and filters

### Features

- plot attribution by default; layer wildcards and filter preview ([9a21bd8](https://github.com/ar-puuk/basemapper/commit/9a21bd878d48d394281f35d504a7473e418efd81))
- get_attribution() for a map's credit line ([0c78682](https://github.com/ar-puuk/basemapper/commit/0c78682644d21de2414b7831bf4d54b909cfa27c))
- labels and features to stack parts of basemaps ([1d141fa](https://github.com/ar-puuk/basemapper/commit/1d141fab781dc2c7e8c1e11704c12663f8438246))
- basemap catalogue with leaflet names; disk tile cache ([35cfed9](https://github.com/ar-puuk/basemapper/commit/35cfed94962086cd63282d8c594a185fa4cef610))
- accept tile data as style; add tile_style() ([a38c8f5](https://github.com/ar-puuk/basemapper/commit/a38c8f57cc113f1fb4bf82e300f98702124f9092))
- one API across R, Python and Rust ([d2f02e9](https://github.com/ar-puuk/basemapper/commit/d2f02e907715a16bb6e7ef49e8561a07fc93ac74))
- **core:** vertical text ([6172adf](https://github.com/ar-puuk/basemapper/commit/6172adf53e3f773449728dc49ae2f323e1801ff0))
- **core:** right-to-left labels ([fd01b82](https://github.com/ar-puuk/basemapper/commit/fd01b8250b98547f13228fb45e50c79130a38f5c))
- **core:** PMTiles sources in vector styles ([f8e98c3](https://github.com/ar-puuk/basemapper/commit/f8e98c393786c7e026481b5e019ecf81792c2872))
- **core:** GeoJSON clustering ([cf4b950](https://github.com/ar-puuk/basemapper/commit/cf4b9504420b9157effa24fbf8b44c8aa6aa0806))
- **core:** line-gradient over GeoJSON lineMetrics ([bdda261](https://github.com/ar-puuk/basemapper/commit/bdda261499c81a7a298060c3afdab0c3b02e9198))
- **core:** fill-extrusion-pattern ([339f417](https://github.com/ar-puuk/basemapper/commit/339f4175fd6151db04cb25b24c850ca53df72f21))
- **core:** line-pattern ([ce366b2](https://github.com/ar-puuk/basemapper/commit/ce366b2bfd684e260374d5858df55dc5a50b3e56))
- **core:** heatmap layers; straight-alpha color interpolation ([6abd6e0](https://github.com/ar-puuk/basemapper/commit/6abd6e0ea2432f0bc6695a1c4cd6c07232e43dd7))
- **core:** hillshade layers over raster-dem sources ([e71bad5](https://github.com/ar-puuk/basemapper/commit/e71bad5745cb7ad97a90b8810b67476e7f7f23cf))
- **core:** GeoJSON sources (geojson-vt port) ([451944e](https://github.com/ar-puuk/basemapper/commit/451944e80ea7d7652e340bf1311e4358b5432c2d))
- **svg:** bitmap glyphs from the style's SDFs; fallback for editable text ([61598de](https://github.com/ar-puuk/basemapper/commit/61598dedbc817233f098d0f17ae46c981af8cdb8))
- **core:** text-variable-anchor and text-variable-anchor-offset ([921431f](https://github.com/ar-puuk/basemapper/commit/921431facd7416dbaa854b72d2ae308062df1f69))
- **svg:** editable text with embedded fonts (true font outlines) ([3b907f0](https://github.com/ar-puuk/basemapper/commit/3b907f07b70e199c995969175352a2ebf35e1366))
- **core:** icon-text-fit with stretchable (nine-slice) sprite images ([bc65668](https://github.com/ar-puuk/basemapper/commit/bc6566801e57554696f8532f6a719e8e68d59fd9))
- **svg:** line-blur; fix legacy function text-field tokens ([06bcfdc](https://github.com/ar-puuk/basemapper/commit/06bcfdc7df98e13bf673aa29d023e3f9e72ecc9d))
- **core:** fill-extrusion; expression-valued layout enums ([ee01e08](https://github.com/ar-puuk/basemapper/commit/ee01e0829513f9813575edec963595d288946227))
- **core:** fill-pattern (PNG and SVG) ([8504c6b](https://github.com/ar-puuk/basemapper/commit/8504c6b3ea3c12d55ef07c8ddc1f2f9837cd7e7c))
- **core:** styles with several sources; raster layers; multi-sprite; more expressions ([0923c8f](https://github.com/ar-puuk/basemapper/commit/0923c8f3e52021fcd2a81509d3d71a70a5add22a))
- **svg:** soft text edges, MapLibre dash patterns, join-shaped offsets ([5e8852c](https://github.com/ar-puuk/basemapper/commit/5e8852c0d3ba245956e55b71f18042cb94f02f54))
- **r:** tm_basemap renders at the device resolution; renv snapshot ([dfefef4](https://github.com/ar-puuk/basemapper/commit/dfefef47ca4346a6de4fc6c7eb406b65ca303ec3))
- Map / basemap() API, stacked styles, resolution-aware plotting ([2121111](https://github.com/ar-puuk/basemapper/commit/2121111b4cf5199095e4bfc5784892077bebbcdf))
- **svg:** closer to MapLibre when rasterised ([d2350b5](https://github.com/ar-puuk/basemapper/commit/d2350b56c3d712d384a4e96cc55e20f1606ece89))
- **core:** pixel_ratio (MapLibre devicePixelRatio) ([e013015](https://github.com/ar-puuk/basemapper/commit/e013015880e9f5cba717ca4954c61941991eb132))
- **core:** true-vector SVG export, measured against MapLibre ([b9fa071](https://github.com/ar-puuk/basemapper/commit/b9fa07113599f12c7863aa5842b37f5b4684dc3d))
- **core:** line-placed symbols — port of MapLibre's line label pipeline ([4dde4ad](https://github.com/ar-puuk/basemapper/commit/4dde4adf56da97692c4a31612bb1fb605aa146a4))
- **core:** symbol icons (sprites) with MapLibre's atlas, filtering and placement ([ba064a8](https://github.com/ar-puuk/basemapper/commit/ba064a81a941a2671a65474c2a4ea178747997dc))
- **core:** point text labels — port of MapLibre's symbol pipeline ([d0497bd](https://github.com/ar-puuk/basemapper/commit/d0497bd28f319c3d37a9b43a830f8800b392c77f))
- **core:** model MapLibre's WebGL pipeline in software for fills/lines/circles ([fcc2060](https://github.com/ar-puuk/basemapper/commit/fcc2060de104a4911c9610ecce37df6d82cc6bf0))
- **core:** in-house CPU vector renderer; golden tests gate on MapLibre GL JS ([68ac73e](https://github.com/ar-puuk/basemapper/commit/68ac73e7ecce684a580bd12115fb3297369305fe))
- **core:** anti-meridian bbox support via split-and-stitch ([186a017](https://github.com/ar-puuk/basemapper/commit/186a0176be20d5466cb9f42a19ab50acbbaf6de0))
- **core:** real PMTiles fetch + in-session tile cache ([72ac92f](https://github.com/ar-puuk/basemapper/commit/72ac92f7ad27479d7fcc501da759e74b63c29b61))
- WebP decoding, WMS/WMTS, PMTiles support + named provider catalog ([62f45cc](https://github.com/ar-puuk/basemapper/commit/62f45ccf3261349582fa17776b9326f30b7813ef))
- named provider catalog for R and Python ([394824a](https://github.com/ar-puuk/basemapper/commit/394824a31abd7207880a336bba6ce3808541e6ef))

### Fixes

- **core:** raster tiles meet without seams at fractional pixel ratios ([82f5e08](https://github.com/ar-puuk/basemapper/commit/82f5e0884aee05f4b8f5b6f1757ce5782673b933))
- **core:** request tile URLs as MapLibre does ([cd7023e](https://github.com/ar-puuk/basemapper/commit/cd7023eb73fc75633879223d35b772e2d6002a5b))
- **core:** snap vertices to 1/16 px as the reference GPU does; add OpenFreeMap Positron and Bright ([575fda8](https://github.com/ar-puuk/basemapper/commit/575fda8fadae06abb15cd9746562dd6fe3f80c9a))
- **core:** sample map-pitched icons LINEAR, as draw_symbol does ([8d4ffb4](https://github.com/ar-puuk/basemapper/commit/8d4ffb49341e969a0155fc14b06e154892de1d7e))
- **core:** store point collision boxes as MapLibre does (Int16) ([61a3204](https://github.com/ar-puuk/basemapper/commit/61a3204f7389da9ea000a16529c682223b64a82d))
- **r:** clear R CMD check warnings and notes ([1abd34d](https://github.com/ar-puuk/basemapper/commit/1abd34dfab71fd53871c2f23d6592cd35c2005fb))
- **core:** viewport-aligned line labels draw upright at their anchor ([652849e](https://github.com/ar-puuk/basemapper/commit/652849e7f97af153b6cf8cf95b00d2c2a5bb55cc))
- **py:** plotnine geom_basemap renders; README corrections ([d5c12c1](https://github.com/ar-puuk/basemapper/commit/d5c12c1dccac790e9cfaf6f9477d8d70ef8b2809))
- **py:** add_basemap(crs=), plotnine 0.15 compatibility; real render tests ([e808929](https://github.com/ar-puuk/basemapper/commit/e8089299aed0b21272eb1e7888bb396a94756b7c))
- **build:** drop out-of-tree maplibre-rs [patch]; build from the pinned fork ([0420dbc](https://github.com/ar-puuk/basemapper/commit/0420dbce6762f42d97729775dc3741ab94a806fd))
- **core:** source selection, maxzoom clamping, auth token propagation ([e397cf3](https://github.com/ar-puuk/basemapper/commit/e397cf3499ca09a7048a357fb29029336bac29ec))
- address code-review action plan (12/13 items) ([60166d1](https://github.com/ar-puuk/basemapper/commit/60166d14ecdcb7f2524c3e53f86c2cc7ad137d41))

### Refactoring

- **core:** drop data-visualisation features; warn on terrain ([7f38f38](https://github.com/ar-puuk/basemapper/commit/7f38f3840cc1748a714f5899b1f942f7262c3b8d))
- **core:** remove the maplibre-rs/wgpu engine ([0298f27](https://github.com/ar-puuk/basemapper/commit/0298f274e0c4760bdfc78241ba3ba50b5c2a30f3))

### Documentation

- install with pak, remotes or devtools; pip, uv or conda ([c117eb2](https://github.com/ar-puuk/basemapper/commit/c117eb2f508045ff1fbe1d4b300e9bc77506fe1e))
- rebuild site; list the package topic and site URL ([4441827](https://github.com/ar-puuk/basemapper/commit/4441827b8d36d9410bce1df60568076192b2c66e))
- current architecture in SPEC.md and plan.md; NEWS and CHANGELOG ([a908221](https://github.com/ar-puuk/basemapper/commit/a908221474ef31b14c36820804af13a650b927d4))
- update roxygen2 man pages, site reference, and READMEs ([c7168a4](https://github.com/ar-puuk/basemapper/commit/c7168a496f99fc5c887615d1b9d3260901c9c118))

### Tests

- **r:** update two outdated testthat expectations ([229c1a7](https://github.com/ar-puuk/basemapper/commit/229c1a7900f41f074435cb124f2bfd6831c1078e))
- **golden:** every MapLibre zoom 1-22 for CARTO and UGRC; world copies; fixes ([085d8f8](https://github.com/ar-puuk/basemapper/commit/085d8f873532a4e709a7c7c72ec8eb22a1d8ce23))
- **golden:** UGRC full basemap and its services at MapLibre zooms 4/7/10/16 ([dcbf51a](https://github.com/ar-puuk/basemapper/commit/dcbf51a45effe10cf378a8cf66e9cc4a9ea4eac3))
- **golden:** full UGRC basemap (Hillshade + Lite Base + Lite Labels); fix gzip ([ea5f787](https://github.com/ar-puuk/basemapper/commit/ea5f787ae9ca3fc1aef4ff93e828759556f6c333))
- **golden:** CARTO styles at 5 zooms, real glyphs/sprites, check/ viewer ([2cd992d](https://github.com/ar-puuk/basemapper/commit/2cd992d2cd2664578dc813ea5a41bd1c09b5aec6))
- **core:** golden-image render tests with MapLibre GL JS reference + CI ([98c916b](https://github.com/ar-puuk/basemapper/commit/98c916bd8dcebadcb0b5b1a25b5ecba39c6ec464))

### Build

- link the Python extension with dynamic lookup on macOS ([eee9954](https://github.com/ar-puuk/basemapper/commit/eee9954eda9798ec71ff3b014a733f7cc895b5cc))

### Maintenance

- ignore Rplots.pdf ([d2ecc83](https://github.com/ar-puuk/basemapper/commit/d2ecc839d27406c4e5ae8661ec7dfdcced7c6088))
- gitignore .claude/worktrees (ephemeral agent scratch space) ([21b3381](https://github.com/ar-puuk/basemapper/commit/21b3381e01b6dcd23e5ca44273072dfdf251dfd1))

## [v0.0.3](https://github.com/ar-puuk/basemapper/releases/tag/v0.0.3) (2026-06-09 to 2026-06-15): commits on `main`

### Features

- **core:** full-scene vector rendering — eliminates tile seams ([953a0a8](https://github.com/ar-puuk/basemapper/commit/953a0a83f0f7778a9a8c69d48bf98fd77177b03e))
- **core:** Track B Part 2 — vector tile rendering via maplibre-rs ([ab5e536](https://github.com/ar-puuk/basemapper/commit/ab5e536e710d6e7ddfca17141493dc85fe9cae95))
- **core:** Track B Part 1 — VectorProvider source_layer + vector routing stub ([3655d9b](https://github.com/ar-puuk/basemapper/commit/3655d9bdb79573fcdd772a31bbdbc38eaf9a6abf))
- flexible bbox input for render_basemap_raw (R + Python) ([37c2471](https://github.com/ar-puuk/basemapper/commit/37c24716b121715aabbe97dfc0576dcc21d4c6af))
- **tmap:** deferred tm_basemap — no bbox arg, mirrors tmap's own tile mechanism ([da7c98b](https://github.com/ar-puuk/basemapper/commit/da7c98bd7ac29d82c3f9589b5e0a3d38d846e467))

### Fixes

- **core:** fill rendering — legacy color stops + alpha blending ([7f15dfa](https://github.com/ar-puuk/basemapper/commit/7f15dfab63fc50541185a52bc7b7d3410bbeb7c5))
- **core:** relative tile URL resolution for ESRI VectorTileServer styles ([8a8f1c5](https://github.com/ar-puuk/basemapper/commit/8a8f1c56ca4a3d4b8cee963287bd98ad99001270))
- **example:** clap hyphen values + zoom override for headless_render ([e47b21d](https://github.com/ar-puuk/basemapper/commit/e47b21d7d5008afbd7a2e19c9bb916df2c88ea8e))
- **track-a:** host-language correctness and API hardening ([4d82a7f](https://github.com/ar-puuk/basemapper/commit/4d82a7f0555ede43278b392a8155a2efc19bec03))
- **py:** basemap visibility and CRS detection ([a73afec](https://github.com/ar-puuk/basemapper/commit/a73afecebfb9b3be283b3c8c5bb308829b021ae5))
- **tmap:** working tm_basemap using terra SpatRaster ([959bce4](https://github.com/ar-puuk/basemapper/commit/959bce409ab1f24bc23285cb9b201e2abc512ab0))
- **r-pkg:** detect CRS from panel_params instead of coord$crs ([64a9620](https://github.com/ar-puuk/basemapper/commit/64a962048653606e7937f570c25abf3258693cb4))
- **core:** set User-Agent header on all HTTP requests ([a341617](https://github.com/ar-puuk/basemapper/commit/a341617c42d2294d3888e215ab2e2bbd2b36aef2))
- **r-pkg:** fix Windows DLL load crash and geom_basemap rendering ([56cf355](https://github.com/ar-puuk/basemapper/commit/56cf355cc4d429555e0dc13dffdae908f668a85d))
- **r-pkg:** add missing Windows system libs to PKG_LIBS ([651b9cd](https://github.com/ar-puuk/basemapper/commit/651b9cd5e12927dc35a700a525d6a5bd5b721005))
- **r-pkg:** add entrypoint.c so R can build basemapper.dll ([712b9fa](https://github.com/ar-puuk/basemapper/commit/712b9fa3376c25b4a1dc1cab8d8ce234b8509a71))
- **r-pkg:** create libgcc_eh.a stub for RTools45 GCC 14 compatibility ([c6b1745](https://github.com/ar-puuk/basemapper/commit/c6b1745da7b75d585045b628a0fc55e6e38f929f))
- **py-docs:** use relative URL for R docs cross-link ([0258421](https://github.com/ar-puuk/basemapper/commit/0258421578890432c65f639a7e940278b4dcdfc8))
- **py-docs:** move R docs button to far right of navbar ([db2d670](https://github.com/ar-puuk/basemapper/commit/db2d6705bc591c178d799c25804f50ede6232ab2))
- **docs:** move R docs button to far right of Python navbar ([d6b23a0](https://github.com/ar-puuk/basemapper/commit/d6b23a0bbb3cf19e5d1370f5d88350db3b023569))
- **docs:** full-height rounded-rect Python docs button, larger text, spaced right ([a7a7899](https://github.com/ar-puuk/basemapper/commit/a7a78997b6aa8ed1ce53362b4bdcf594dd31015b))
- **docs:** make Python docs button full navbar height ([7fa3c0c](https://github.com/ar-puuk/basemapper/commit/7fa3c0ce1a494c6499c5b7f8ee45cdbb498fef99))
- **docs:** style Python docs as a teal pill button in R navbar ([95b18e6](https://github.com/ar-puuk/basemapper/commit/95b18e65ab3719213703229fe5f5dbfeffb79430))
- **docs:** align Python/R theming — Inter, JetBrains Mono, orange links, code bg ([1161787](https://github.com/ar-puuk/basemapper/commit/1161787c32ca6b11973e2bb827b89cf6d3c38122))
- **docs:** restore search/lightswitch, fix navbar bg, add README, fix R docs link ([cf92271](https://github.com/ar-puuk/basemapper/commit/cf92271bbe246eb4ae79c7b53500b2d8f3212952))
- **docs:** replace banner with navbar-native R/Python switcher links ([df9331c](https://github.com/ar-puuk/basemapper/commit/df9331ce1b762dcc24fbaf8b7fa545a8debfd262))
- **docs:** insert cross-lang banner after navbar, not before body ([6f310fe](https://github.com/ar-puuk/basemapper/commit/6f310fede5acc811b9f4b65d881024278b363d5f))
- **r-pkg:** fix src/ nesting in sync-core, track Cargo.lock files ([e83535c](https://github.com/ar-puuk/basemapper/commit/e83535c72959141f99209bc2a15f7484cc49ba15))
- **r-pkg:** remove duplicate examples/ nesting in vendored core ([400ef54](https://github.com/ar-puuk/basemapper/commit/400ef540ec0505cea5b6448546014447a163f25e))
- **r-pkg:** sync examples/ in Makevars and drop invalid bslib vars ([295076f](https://github.com/ar-puuk/basemapper/commit/295076f41d8bcaac476cf1ef96acd10ad6f51673))
- **r-pkg:** vendor core snapshot and apply portfolio theming ([dcac47a](https://github.com/ar-puuk/basemapper/commit/dcac47ac0d8382b4a6347d9ad9cd186e323151bf))
- **r-pkg:** use workspace target dir in Makevars to avoid AppControl block ([bc9449e](https://github.com/ar-puuk/basemapper/commit/bc9449e2b3644c560d5da349eab7e1b06fc2116a))
- **r-pkg:** add Makevars to enable install_github() compilation ([ec3b137](https://github.com/ar-puuk/basemapper/commit/ec3b137f4d3b04b115d57ee90bb8963eabc59a3c))

### Refactoring

- **docs:** replace post-build banner script with native config includes ([6a9aef1](https://github.com/ar-puuk/basemapper/commit/6a9aef1567c200d96d961f781475f02aa1d3db5f))

### Documentation

- **r:** regenerate man pages for flexible bbox API ([2e5fbf5](https://github.com/ar-puuk/basemapper/commit/2e5fbf54db8b030f8a42c098539f9dd486b4201d))
- update tm_basemap vignette and plotnine draw_panel docstring ([1ab6c60](https://github.com/ar-puuk/basemapper/commit/1ab6c6021aeede1ee7c0a3edcca73ea3eb761191))
- **r:** rebuild site after Makevars.win fix ([28819f8](https://github.com/ar-puuk/basemapper/commit/28819f828ddd546c699bb66eb876f85153df7b2b))
- regenerate R and Python docs ([87fb1be](https://github.com/ar-puuk/basemapper/commit/87fb1be3020f120982dd7ed800c7f0aa556243de))
- **py:** R docs callout, teal navbar button, fix install instructions ([1c817d2](https://github.com/ar-puuk/basemapper/commit/1c817d2e14ecf950251cf9256c64393e1a5c1b7b))
- **r:** use {pkg} syntax for ggplot2/tmap/sf, let downlit auto-link ([e235512](https://github.com/ar-puuk/basemapper/commit/e2355124b4db4b0b93567e863b4f66416fa684be))
- **r:** hyperlink ggplot2, tmap, sf package names in README and vignette ([7b81afd](https://github.com/ar-puuk/basemapper/commit/7b81afdb39e7fec7ed134b1208e09d1ad5dda74e))
- **r:** add Python docs callout above Installation in README ([b0b34dd](https://github.com/ar-puuk/basemapper/commit/b0b34dd3fe87c8bc1bb70305efeb4f25715441e6))
- **r:** add winget install instructions for Rust and RTools45 ([84a7d31](https://github.com/ar-puuk/basemapper/commit/84a7d3139c8a923333b54f75be4f6b181c581dbd))
- add README to py-basemapper, rebuild Python site ([5ef1eb5](https://github.com/ar-puuk/basemapper/commit/5ef1eb576e7fc8c83b3d144e370592c419fa92a5))
- clean rebuild site ([e757dbb](https://github.com/ar-puuk/basemapper/commit/e757dbb9db8d5bff61eb4a37ae8dc11910e152fe))
- rebuild site with portfolio theming and native cross-lang banners ([9dc9efd](https://github.com/ar-puuk/basemapper/commit/9dc9efd2ff8410f731e0091c5b3a964966bcda62))
- Bootstrap 5 theming, hex logo, improved banners, disable skills/MCP ([70690c4](https://github.com/ar-puuk/basemapper/commit/70690c4901f7dbd27f52748dd822b02a0e840675))

### Maintenance

- bump maplibre-rs rev to 1a930af (color/fill expression fix) ([0603b7a](https://github.com/ar-puuk/basemapper/commit/0603b7adedbb10424794f270ddcb9f43681a07ca))
- sync vendored r-basemapper core with full-scene vector renderer ([48eac1d](https://github.com/ar-puuk/basemapper/commit/48eac1dd666e8536e1bf76622be0a03bdd09649b))
- update site build timestamps from cold rebuild ([045f322](https://github.com/ar-puuk/basemapper/commit/045f3224d043e9c09fd775c38d2b80ed1c79b4f9))
- commit Cargo.lock and clean up .gitignore ([259e1e2](https://github.com/ar-puuk/basemapper/commit/259e1e280fe33256e3d9128c56cdebf10746d20b))

## [v0.0.2](https://github.com/ar-puuk/basemapper/releases/tag/v0.0.2) (2026-06-09): merge of docs-ci-2

### Features

- **docs:** build and commit initial docs site (R + Python) ([15355cc](https://github.com/ar-puuk/basemapper/commit/15355cc05663e286ecdfa4b90328fc520344b69a))
- **docs:** switch to pre-built docs workflow ([1e73f05](https://github.com/ar-puuk/basemapper/commit/1e73f058c98d77b97ab8b993fb68d48834cd92e0))

### Fixes

- **docs-ci-2:** fix R roxygenize compile error and Python alias resolution ([3da3216](https://github.com/ar-puuk/basemapper/commit/3da3216b533612b63bdc4a44cf04ea0bebec28bb))

### Documentation

- **readme:** add documentation update workflow section ([db1afe6](https://github.com/ar-puuk/basemapper/commit/db1afe6e2c40cb45798fc40a5944a40dc2aa5b1f))

## 2026-06-08: [#2](https://github.com/ar-puuk/basemapper/pull/2) fix(docs-ci): fix R renv conflict and Python great-docs config

### Fixes

- **docs-ci:** fix R renv conflict and Python great-docs config ([80968ee](https://github.com/ar-puuk/basemapper/commit/80968ee53e00639d65038a486a938d50d34f4bc5))

## [v0.0.1](https://github.com/ar-puuk/basemapper/releases/tag/v0.0.1) (2026-06-08): [#1](https://github.com/ar-puuk/basemapper/pull/1) 001 spatial basemap renderer

### Features

- **validate:** all 54 tests pass across Rust, Python, and R (T037) ([5dce83c](https://github.com/ar-puuk/basemapper/commit/5dce83c509dba1ac69c00f67921e18596a2bb9e8))
- **impl:** implement full basemapper workspace across Phases 1-9 ([c4710a5](https://github.com/ar-puuk/basemapper/commit/c4710a587fb31ff3a28e795d42d3821b8bdfaf2d))
- **spec:** add US5–US7 — layer filtering, tmap, and plotnine integrations ([36b1919](https://github.com/ar-puuk/basemapper/commit/36b19199b6925cbdc3cd6cd420109b06c4783b3d))
- **spec:** restore EsriRasterProvider as fourth provider (additive) ([3c27a48](https://github.com/ar-puuk/basemapper/commit/3c27a4818a64c2f378cf0d4cf02fbb3aeff5c748))
- **spec:** add US4 Provider Generator Helpers to spec and tasks ([17cc039](https://github.com/ar-puuk/basemapper/commit/17cc039c20469a4def69f7c0472dbb69405b5242))

### Fixes

- **r-binding:** fix R DLL crashes and nested-runtime panic on Windows ([4e88124](https://github.com/ar-puuk/basemapper/commit/4e88124397b8eb28bc21be8928038a165d500284))
- **python:** add python-source = \"src\" to maturin config; mark T036 done ([97f9488](https://github.com/ar-puuk/basemapper/commit/97f9488427a548834310bfa40e46ade923d6cc01))
- **lint:** resolve all cargo clippy -D warnings across workspace (T036) ([867b469](https://github.com/ar-puuk/basemapper/commit/867b46950d797c2b3d47652dd0a5ee25d201b2b9))
- **build:** generate R.lib and wire up linker search path for Windows ([fdb95e1](https://github.com/ar-puuk/basemapper/commit/fdb95e135758eb702540a3b89fbccfc3354a5373))
- **build:** resolve cargo check errors across all three workspace crates ([dc30f80](https://github.com/ar-puuk/basemapper/commit/dc30f80473ea19474650216722f0d70c1ff5b20b))
- **spec:** rename EsriRasterProvider → EsriVectorProvider; update behavior ([4f0ee52](https://github.com/ar-puuk/basemapper/commit/4f0ee52e483c9e4401b8f8a7e01b6b2ea97134f7))

### Documentation

- add unified R+Python documentation site with language-switch banner ([bcf4a88](https://github.com/ar-puuk/basemapper/commit/bcf4a8899c7b9c4bbe148a2d2e8b9d373e37dd5c))
- **spec:** add implementation hazards to plan.md and tasks.md ([2c02a04](https://github.com/ar-puuk/basemapper/commit/2c02a04bc86c0db375e4c861b9b78b52c62ce738))

### Maintenance

- **lockfiles:** add Cargo.lock, renv.lock, and renv bootstrap files ([9d8b40b](https://github.com/ar-puuk/basemapper/commit/9d8b40b37ba9d6cfafca0b09730a5c7ace0992ea))
- **docs:** add roxygen2 (R) and great-docs (Python) documentation tooling ([3182900](https://github.com/ar-puuk/basemapper/commit/3182900d30cc5002704ccf0dac4f1f14553dd9cc))

### Other

- [Spec Kit] Add basemapper constitution, spec, plan, and tasks (v1.0.0) ([f836be0](https://github.com/ar-puuk/basemapper/commit/f836be0803818381e4132ec9f6a714da4d3a8075))

## 2026-06-08: commits on `main`

### Other

- Initial commit ([61ac899](https://github.com/ar-puuk/basemapper/commit/61ac899a8e1929f7028020ea99583339852ba84a))
