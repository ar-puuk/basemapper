# Look up a named basemap provider.

Returns either a style URL string or inline MapLibre GL JSON suitable for
passing directly to [`render_basemap_raw()`](render_basemap_raw.md).

## Usage

``` r
basemap_provider(name, api_key = NULL, ...)
```

## Arguments

- name:

  Character: provider name. See *Details* for the full catalog.

- api_key:

  Character or `NULL`: API key required by some providers (e.g. MapTiler).
  Ignored for providers that do not use a key. Stadia providers work
  without a key at low volume; supply one to lift rate limits.

- ...:

  Reserved for future arguments; currently unused.

## Value

A character string — either a style URL or inline MapLibre GL JSON —
ready to pass to [`render_basemap_raw()`](render_basemap_raw.md).

## Details

**Available provider names:**

*Vector GL styles (no key required):*
`"OpenFreeMap.liberty"`, `"OpenFreeMap.bright"`, `"OpenFreeMap.positron"`,
`"Carto.Positron"`, `"Carto.DarkMatter"`, `"Carto.Voyager"`,
`"ESRI.WorldBasemap"`

*MapTiler GL styles (require `api_key`):*
`"MapTiler.Streets"`, `"MapTiler.Satellite"`, `"MapTiler.Topo"`,
`"MapTiler.Outdoor"`

*ESRI raster tile services (no key required):*
`"ESRI.WorldImagery"`, `"ESRI.WorldStreetMap"`, `"ESRI.WorldTopoMap"`,
`"ESRI.WorldShadedRelief"`, `"ESRI.WorldGrayCanvas"`,
`"ESRI.NatGeoWorldMap"`

*OSM raster (no key required):*
`"OSM.Standard"`, `"OSM.CyclOSM"`

*Stadia/Stamen (key optional):*
`"Stadia.AlidadeSmooth"`, `"Stadia.AlidadeSmoothDark"`,
`"Stadia.OSMBright"`, `"Stadia.StamenToner"`, `"Stadia.StamenTerrain"`,
`"Stadia.StamenWatercolor"`

## Examples

``` r
style <- basemap_provider("OpenFreeMap.liberty")
style <- basemap_provider("Carto.DarkMatter")
style <- basemap_provider("OSM.Standard")

if (FALSE) { # \dontrun{
style <- basemap_provider("MapTiler.Streets", api_key = "YOUR_KEY")
} # }
```
