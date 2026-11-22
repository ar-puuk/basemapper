## ValidationError


Raised for invalid bbox, dimensions, zoom, or layer filter.


Usage


``` python
ValidationError(message)
```


Subclasses both [BasemapError](BasemapError.md#basemapper.BasemapError) and `ValueError` so that callers that catch the built-in `ValueError` continue to work unchanged.
