# PMTiles provider style

Build a MapLibre GL Style JSON string for a PMTiles raster archive served
over HTTP. The Rust core detects the `pmtiles://` scheme in the `tiles`
array and routes tile fetching through the PMTiles HTTP range-request
pipeline.

## Usage

``` r
pmtiles_provider(url)
```

## Arguments

- url:

  Character: HTTPS URL of the `.pmtiles` archive file.

## Value

A JSON character string ready to pass to
[`render_basemap_raw()`](render_basemap_raw.md).

## Examples

``` r
if (FALSE) { # \dontrun{
style <- pmtiles_provider("https://example.com/tiles/world.pmtiles")
m <- render_basemap_raw(
  bbox        = c(-122.5, 37.7, -122.4, 37.8),
  width       = 400L,
  height      = 300L,
  style_input = style
)
} # }
```
