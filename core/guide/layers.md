# Labels, features and layers

[`list_layers`] lists a style's layers in drawing order, with the source
layer each one draws and the *feature* it shows (background, water,
landcover, parks, buildings, roads, rail, boundaries, places, poi, terrain).
Given a [`LayerFilter`], it lists the layers a render with that filter draws.
The same filters go on a [`Basemap`]: `labels()`, `features()`, and
`layers()` for exact layer ids (with `*` and `?` wildcards, `-` to drop).

