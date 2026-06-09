# Generate a MapLibre GL Style JSON string for an MVT vector tile source.

Recognised paint keys: `fill-color`, `fill-opacity`,
`fill-outline-color`, `line-color`, `line-width`, `line-opacity`.
Unrecognised keys are dropped with a
[`warning()`](https://rdrr.io/r/base/warning.html). An empty paint list
applies a light-grey default style.

## Usage

``` r
vector_provider(url_template, paint = list())
```

## Arguments

- url_template:

  Character: MVT tile URL with `{z}`, `{x}`, `{y}` placeholders.

- paint:

  Named list of MapLibre GL paint properties.

## Value

A JSON character string ready to pass to
[`render_basemap_raw()`](https://ar-puuk.github.io/basemapper/r/reference/render_basemap_raw.md).

## Examples

``` r
style <- vector_provider(
  "https://example.com/tiles/{z}/{x}/{y}.mvt",
  paint = list("fill-color" = "#e8e0d8", "line-color" = "#aaa")
)
```
