# The attribution a style asks maps to show

Every source the styles' layers draw from contributes its attribution
(from the style, the source's TileJSON or PMTiles metadata), as
MapLibre's attribution control collects it: plain text, without
duplicates, joined with `" | "`. Use it for a caption or credit line,
e.g. `labs(caption = get_attribution(style))`. Python:
`basemapper.get_attribution()` and `m.get_attribution()`.

## Usage

``` r
get_attribution(x, auth_token = NULL)
```

## Arguments

- x:

  A view from
  [`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md),
  or a style (anything
  [`basemap()`](https://ar-puuk.github.io/basemapper/r/reference/basemap.md)
  takes) or vector of styles stacked bottom to top.

- auth_token:

  Optional access token for the tile service (for a view, its own is
  used).

## Value

A character string; `""` when the styles give no attribution.

## Examples

``` r
get_attribution("Esri.WorldImagery")
#> [1] "Tiles © Esri — Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community"
if (FALSE) { # \dontrun{
m <- basemap("OpenFreeMap.Liberty", center = c(-111.89, 40.76), zoom = 12)
get_attribution(m)
} # }
```
