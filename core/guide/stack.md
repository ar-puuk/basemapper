# Stacking styles

[`Basemap::stack`] draws several styles bottom to top, as one image. Each
style has its own filters: with `features = ["labels"]`, a vector basemap
contributes just its labels, on a transparent background: here over
satellite imagery.

