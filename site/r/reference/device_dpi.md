# Output resolution of the current graphics device, in dpi.

Raster devices report their resolution (e.g. `ggsave(dpi = 300)`);
vector devices (PDF, SVG, PostScript) have none, so their embedded
basemap is rendered at 300 dpi.

## Usage

``` r
device_dpi()
```
