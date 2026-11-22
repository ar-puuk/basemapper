# Basemap names, for autocompletion

A named list of every basemap name: `providers$CartoDB.Positron` is the
string `"CartoDB.Positron"`, usable as `style` or with
[`basemap_provider()`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md).
The names match
[`leaflet::providers`](https://rstudio.github.io/leaflet/reference/providers.html)
where both have a basemap. Python:
`basemapper.providers.CartoDB.Positron`.

## Usage

``` r
providers
```

## Format

A named list of character strings.

## Examples

``` r
providers$Esri.WorldImagery
#> [1] "Esri.WorldImagery"
```
