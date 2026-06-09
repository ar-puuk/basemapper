## VectorProvider


Generate a MapLibre GL Style JSON string for an MVT vector tile source.


Usage


``` python
VectorProvider()
```


Accepted paint keys: `fill-color`, `fill-opacity`, `fill-outline-color`, `line-color`, `line-width`, `line-opacity`. Unrecognised keys are dropped with a warning. Omitting `paint` applies a light-grey default style.


## Parameters


`url_template: str`  
MVT tile URL with `{z}`, `{x}`, `{y}` placeholders.

`paint: Optional[Dict[str, object]] = None`  
Optional dict of MapLibre GL paint properties.
