Renders styled spatial basemaps headlessly using a Rust core
(maplibre-rs + wgpu) and exposes the result as raw RGBA pixel arrays to
R. Integrates with ggplot2 via a lazy GeomBasemap ggproto layer and with
tmap v4 via tm_basemap().
