## list_layers()


Return all layer IDs from a MapLibre GL style.


Usage


``` python
list_layers(style_input)
```


Also prints a two-column table of `id` and `type` to stdout for interactive exploration.


## Parameters


`style_input: str`  
A MapLibre GL style URL or inline JSON string.


## Returns


`List[str]`  
Sorted list of layer `id` values.


## Raises


`BasemapError`  
If the style cannot be fetched or parsed.
