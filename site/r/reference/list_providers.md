# The built-in basemaps

The basemaps
[`basemap_provider()`](https://ar-puuk.github.io/basemapper/r/reference/basemap_provider.md)
knows, with the forms each comes in. Python:
`basemapper.list_providers()`.

## Usage

``` r
list_providers(type = NULL)
```

## Arguments

- type:

  `"vector"` or `"raster"` to list only basemaps with that form; `NULL`
  lists all.

## Value

A data frame with columns `name`; `vector` and `raster` (logical: the
forms it has); `api_key_env` (the environment variable its API key is
read from, `NA` when it needs none); and `attribution`.

## Examples

``` r
list_providers()
#>                        name vector raster      api_key_env
#> 1      OpenStreetMap.Mapnik  FALSE   TRUE             <NA>
#> 2          OpenStreetMap.DE  FALSE   TRUE             <NA>
#> 3      OpenStreetMap.France  FALSE   TRUE             <NA>
#> 4         OpenStreetMap.HOT  FALSE   TRUE             <NA>
#> 5               OpenTopoMap  FALSE   TRUE             <NA>
#> 6                   CyclOSM  FALSE   TRUE             <NA>
#> 7       OpenFreeMap.Liberty   TRUE  FALSE             <NA>
#> 8        OpenFreeMap.Bright   TRUE  FALSE             <NA>
#> 9      OpenFreeMap.Positron   TRUE  FALSE             <NA>
#> 10         OpenFreeMap.Dark   TRUE  FALSE             <NA>
#> 11        OpenFreeMap.Fiord   TRUE  FALSE             <NA>
#> 12         CartoDB.Positron   TRUE   TRUE    CARTO_API_KEY
#> 13       CartoDB.DarkMatter   TRUE   TRUE    CARTO_API_KEY
#> 14          CartoDB.Voyager   TRUE   TRUE    CARTO_API_KEY
#> 15      Esri.WorldStreetMap   TRUE   TRUE             <NA>
#> 16        Esri.WorldTopoMap   TRUE   TRUE             <NA>
#> 17     Esri.WorldGrayCanvas   TRUE   TRUE             <NA>
#> 18 Esri.WorldDarkGrayCanvas   TRUE   TRUE             <NA>
#> 19        Esri.WorldImagery  FALSE   TRUE             <NA>
#> 20        Esri.WorldTerrain  FALSE   TRUE             <NA>
#> 21   Esri.WorldShadedRelief  FALSE   TRUE             <NA>
#> 22       Esri.WorldPhysical  FALSE   TRUE             <NA>
#> 23        Esri.OceanBasemap  FALSE   TRUE             <NA>
#> 24      Esri.NatGeoWorldMap  FALSE   TRUE             <NA>
#> 25              USGS.USTopo  FALSE   TRUE             <NA>
#> 26           USGS.USImagery  FALSE   TRUE             <NA>
#> 27     Stadia.AlidadeSmooth   TRUE   TRUE   STADIA_API_KEY
#> 28 Stadia.AlidadeSmoothDark   TRUE   TRUE   STADIA_API_KEY
#> 29  Stadia.AlidadeSatellite   TRUE   TRUE   STADIA_API_KEY
#> 30         Stadia.OSMBright   TRUE   TRUE   STADIA_API_KEY
#> 31          Stadia.Outdoors   TRUE   TRUE   STADIA_API_KEY
#> 32       Stadia.StamenToner   TRUE   TRUE   STADIA_API_KEY
#> 33   Stadia.StamenTonerLite   TRUE   TRUE   STADIA_API_KEY
#> 34     Stadia.StamenTerrain   TRUE   TRUE   STADIA_API_KEY
#> 35  Stadia.StamenWatercolor   TRUE   TRUE   STADIA_API_KEY
#> 36         MapTiler.Streets   TRUE   TRUE MAPTILER_API_KEY
#> 37           MapTiler.Basic   TRUE   TRUE MAPTILER_API_KEY
#> 38          MapTiler.Bright   TRUE   TRUE MAPTILER_API_KEY
#> 39         MapTiler.Outdoor   TRUE   TRUE MAPTILER_API_KEY
#> 40            MapTiler.Topo   TRUE   TRUE MAPTILER_API_KEY
#> 41         MapTiler.Dataviz   TRUE   TRUE MAPTILER_API_KEY
#> 42       MapTiler.Satellite   TRUE   TRUE MAPTILER_API_KEY
#>                                                                                                                                                                                                                               attribution
#> 1                                                                                                                                                                                                            © OpenStreetMap contributors
#> 2                                                                                                                                                                                                            © OpenStreetMap contributors
#> 3                                                                                                                                                                                   © OpenStreetMap France | © OpenStreetMap contributors
#> 4                                                                                                                             © OpenStreetMap contributors, Tiles style by Humanitarian OpenStreetMap Team hosted by OpenStreetMap France
#> 5                                                                                                                                                      Map data: © OpenStreetMap contributors, SRTM | Map style: © OpenTopoMap (CC-BY-SA)
#> 6                                                                                                                                                                                        CyclOSM | Map data: © OpenStreetMap contributors
#> 7                                                                                                                                                                                      OpenFreeMap © OpenMapTiles Data from OpenStreetMap
#> 8                                                                                                                                                                                      OpenFreeMap © OpenMapTiles Data from OpenStreetMap
#> 9                                                                                                                                                                                      OpenFreeMap © OpenMapTiles Data from OpenStreetMap
#> 10                                                                                                                                                                                     OpenFreeMap © OpenMapTiles Data from OpenStreetMap
#> 11                                                                                                                                                                                     OpenFreeMap © OpenMapTiles Data from OpenStreetMap
#> 12                                                                                                                                                                                                   © OpenStreetMap contributors © CARTO
#> 13                                                                                                                                                                                                   © OpenStreetMap contributors © CARTO
#> 14                                                                                                                                                                                                   © OpenStreetMap contributors © CARTO
#> 15                   Tiles © Esri — Source: Esri, HERE, Garmin, USGS, Intermap, INCREMENT P, NRCan, Esri Japan, METI, Esri China (Hong Kong), Esri Korea, Esri (Thailand), NGCC, © OpenStreetMap contributors, and the GIS User Community
#> 16 Tiles © Esri — Esri, HERE, Garmin, Intermap, INCREMENT P, GEBCO, USGS, FAO, NPS, NRCAN, GeoBase, IGN, Kadaster NL, Ordnance Survey, Esri Japan, METI, Esri China (Hong Kong), © OpenStreetMap contributors, and the GIS User Community
#> 17                                                                                                                                            Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors, and the GIS User Community
#> 18                                                                                                                                            Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors, and the GIS User Community
#> 19                                                                                                     Tiles © Esri — Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community
#> 20                                                                                                                                                                              Tiles © Esri — Source: USGS, Esri, TANA, DeLorme, and NPS
#> 21                                                                                                                                                                                                            Tiles © Esri — Source: Esri
#> 22                                                                                                                                                                                        Tiles © Esri — Source: US National Park Service
#> 23                                                                                                                              Tiles © Esri — Sources: GEBCO, NOAA, CHS, OSU, UNH, CSUMB, National Geographic, DeLorme, NAVTEQ, and Esri
#> 24                                                                                                                   Tiles © Esri — National Geographic, Esri, DeLorme, NAVTEQ, UNEP-WCMC, USGS, NASA, ESA, METI, NRCAN, GEBCO, NOAA, iPC
#> 25                                                                                                                                                                                           Tiles courtesy of the U.S. Geological Survey
#> 26                                                                                                                                                                                           Tiles courtesy of the U.S. Geological Survey
#> 27                                                                                                                                                                              © Stadia Maps © OpenMapTiles © OpenStreetMap contributors
#> 28                                                                                                                                                                              © Stadia Maps © OpenMapTiles © OpenStreetMap contributors
#> 29                                                                                                                                                                              © Stadia Maps © OpenMapTiles © OpenStreetMap contributors
#> 30                                                                                                                                                                              © Stadia Maps © OpenMapTiles © OpenStreetMap contributors
#> 31                                                                                                                                                                              © Stadia Maps © OpenMapTiles © OpenStreetMap contributors
#> 32                                                                                                                                                              © Stadia Maps © Stamen Design © OpenMapTiles © OpenStreetMap contributors
#> 33                                                                                                                                                              © Stadia Maps © Stamen Design © OpenMapTiles © OpenStreetMap contributors
#> 34                                                                                                                                                              © Stadia Maps © Stamen Design © OpenMapTiles © OpenStreetMap contributors
#> 35                                                                                                                                                              © Stadia Maps © Stamen Design © OpenMapTiles © OpenStreetMap contributors
#> 36                                                                                                                                                                                                © MapTiler © OpenStreetMap contributors
#> 37                                                                                                                                                                                                © MapTiler © OpenStreetMap contributors
#> 38                                                                                                                                                                                                © MapTiler © OpenStreetMap contributors
#> 39                                                                                                                                                                                                © MapTiler © OpenStreetMap contributors
#> 40                                                                                                                                                                                                © MapTiler © OpenStreetMap contributors
#> 41                                                                                                                                                                                                © MapTiler © OpenStreetMap contributors
#> 42                                                                                                                                                                                                © MapTiler © OpenStreetMap contributors
list_providers("vector")$name
#>  [1] "OpenFreeMap.Liberty"      "OpenFreeMap.Bright"      
#>  [3] "OpenFreeMap.Positron"     "OpenFreeMap.Dark"        
#>  [5] "OpenFreeMap.Fiord"        "CartoDB.Positron"        
#>  [7] "CartoDB.DarkMatter"       "CartoDB.Voyager"         
#>  [9] "Esri.WorldStreetMap"      "Esri.WorldTopoMap"       
#> [11] "Esri.WorldGrayCanvas"     "Esri.WorldDarkGrayCanvas"
#> [13] "Stadia.AlidadeSmooth"     "Stadia.AlidadeSmoothDark"
#> [15] "Stadia.AlidadeSatellite"  "Stadia.OSMBright"        
#> [17] "Stadia.Outdoors"          "Stadia.StamenToner"      
#> [19] "Stadia.StamenTonerLite"   "Stadia.StamenTerrain"    
#> [21] "Stadia.StamenWatercolor"  "MapTiler.Streets"        
#> [23] "MapTiler.Basic"           "MapTiler.Bright"         
#> [25] "MapTiler.Outdoor"         "MapTiler.Topo"           
#> [27] "MapTiler.Dataviz"         "MapTiler.Satellite"      
```
