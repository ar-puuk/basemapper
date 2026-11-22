# Show a basemap view

[`plot()`](https://rdrr.io/r/graphics/plot.default.html) renders the
view and draws it on the current graphics device. In R Markdown and
Quarto documents, a view printed by a chunk (`m` on its own line)
renders and shows as an image at its CSS pixel size, sharp at any
`pixel_ratio`; `print(m)` describes the view instead.

## Usage

``` r
# S3 method for class 'basemap'
plot(x, ...)

# S3 method for class 'basemap'
knit_print(x, options = list(), ...)
```

## Arguments

- x:

  A view from
  [`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md).

- ...:

  Unused.

- options:

  knitr chunk options.

## Value

`x`, invisibly
([`plot()`](https://rdrr.io/r/graphics/plot.default.html)); a knitr
image (`knit_print()`).

## Examples

``` r
if (FALSE) { # \dontrun{
m <- basemap("OpenFreeMap.Positron", 800, 450, center = c(-111.8887, 40.7556), zoom = 12.4)
plot(m)
} # }
```
