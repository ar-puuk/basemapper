# Generate a MapLibre GL Style JSON string for an XYZ raster tile source.

Generate a MapLibre GL Style JSON string for an XYZ raster tile source.

## Usage

``` r
raster_provider(url_template, tile_size = 256L)
```

## Arguments

- url_template:

  Character: tile URL with `{z}`, `{x}`, `{y}` placeholders.

- tile_size:

  Integer tile size in pixels (default 256L).

## Value

A JSON character string ready to pass to
[`render_basemap_raw()`](https://ar-puuk.github.io/basemapper/r/reference/render_basemap_raw.md).

## Examples

``` r
style <- raster_provider("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
```
