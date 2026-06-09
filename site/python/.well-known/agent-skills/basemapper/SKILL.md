---
name: basemapper
description: >
  Headless spatial basemap renderer for Python. Use when writing Python code that uses the basemapper package.
license: MIT
compatibility: Requires Python >=3.9.
---

# basemapper

Headless spatial basemap renderer for Python

## Installation

```bash
pip install basemapper
```

## API overview

### Core rendering

Low-level tile fetch and composite.

- `render_basemap_raw`: Fetch and composite map tiles into a raw RGBA pixel buffer

### Plot integrations

Drop a basemap layer into an existing spatial plot. `geom_basemap` works in both ggplot2 (R) and plotnine (Python).


- `add_basemap`: Render a basemap and composite it beneath existing axes content
- `geom_basemap`: Render a styled basemap beneath plotnine spatial layers

### Layer utilities

Inspect and filter layers inside a MapLibre GL style.

- `list_layers`: Return all layer IDs from a MapLibre GL style

### Tile style helpers

Convert bare tile URLs to MapLibre GL Style JSON.

- `RasterProvider`: Generate a MapLibre GL Style JSON string for an XYZ raster tile source
- `VectorProvider`: Generate a MapLibre GL Style JSON string for an MVT vector tile source
- `EsriVectorProvider`: Return the root.json style endpoint URL for an ArcGIS VectorTileServer
- `EsriRasterProvider`: Generate a MapLibre GL Style JSON string for an ArcGIS MapServer raster source

### Exceptions

- `BasemapError`: Raised when the Rust basemapper core returns an error

## Resources

- [Full documentation](https://ar-puuk.github.io/basemapper/python/)
- [llms.txt](llms.txt) — Indexed API reference for LLMs
- [llms-full.txt](llms-full.txt) — Comprehensive documentation for LLMs
