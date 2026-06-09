# Return the root.json style endpoint URL for an ArcGIS VectorTileServer.

The Rust core fetches this URL to obtain the full, ESRI-published
MapLibre GL style JSON. Returns a plain URL string, not inline JSON.

## Usage

``` r
esri_vector_provider(base_url)
```

## Arguments

- base_url:

  Character: base ArcGIS VectorTileServer URL.

## Value

A URL character string.

## Examples

``` r
url <- esri_vector_provider("https://basemaps.arcgis.com/arcgis/rest/services/World_Basemap_v2/VectorTileServer")
```
