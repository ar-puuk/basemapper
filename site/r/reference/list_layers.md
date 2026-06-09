# List all layer IDs from a MapLibre GL style.

Fetches (if URL) or parses (if inline JSON) a MapLibre GL style and
returns all layer `id` values. Also prints a two-column data.frame of id
and type to the console for interactive exploration.

## Usage

``` r
list_layers(style_input)
```

## Arguments

- style_input:

  Character: a MapLibre GL style URL or inline JSON string.

## Value

A character vector of all layer `id` values.

## Examples

``` r
if (FALSE) { # \dontrun{
ids <- list_layers("https://demotiles.maplibre.org/style.json")
} # }
```
