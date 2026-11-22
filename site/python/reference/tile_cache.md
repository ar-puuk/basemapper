## tile_cache()


The disk cache for map tiles.


Usage


``` python
tile_cache(
    dir=None,
    clear=False,
)
```


Tiles are kept as long as the server's caching headers allow (a day when they say nothing), so repeated renders of an area do not download them again. The cache starts in `BASEMAPPER_CACHE_DIR` when set (`off` disables it), else in the platform cache directory.


## Parameters


`dir: Optional[str] = None`  
Move the cache to this directory; `False` or `""` turns it off. None leaves it as it is.

`clear: bool = ``False`  
Delete every cached tile.


## Returns


`Optional[str]`  
The cache directory, or None when the cache is off.
