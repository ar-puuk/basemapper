# Generate a MapLibre GL Style JSON string for an ArcGIS MapServer raster source.

Generate a MapLibre GL Style JSON string for an ArcGIS MapServer raster
source.

## Usage

``` r
esri_raster_provider(url_template, tile_size = 256L)
```

## Arguments

- url_template:

  Character: ArcGIS tile URL (e.g., `.../MapServer/tile/{z}/{y}/{x}`).

- tile_size:

  Integer tile size in pixels (default 256L).

## Value

A JSON character string ready to pass to
[`render_basemap_raw()`](https://ar-puuk.github.io/basemapper/r/reference/render_basemap_raw.md).

## Examples

``` r
style <- esri_raster_provider("https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}")
```
