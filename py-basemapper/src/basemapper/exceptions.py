"""Errors and warnings.

Errors carry the core's kind: :class:`ValidationError` for what the caller
gave, :class:`StyleError` for a style that cannot be fetched or read,
:class:`NetworkError` for downloads. Warnings carry a note's level
(docs/design/api-review.md §4.8): :class:`BasemapAdjustment` when something
asked for was changed (shown once per call site, Python's default),
:class:`BasemapProblem` when the output is incomplete (shown every time).
"""

import warnings


class BasemapError(RuntimeError):
    """Base exception for all basemapper errors."""

    def __init__(self, message: str) -> None:
        super().__init__(message)


class ValidationError(BasemapError, ValueError):
    """Invalid input: a view, style, filter, CRS or clip that cannot work.

    Also a :class:`ValueError`, so code catching that keeps working.
    """


class StyleError(BasemapError):
    """A style that cannot be fetched or parsed."""


class NetworkError(BasemapError):
    """A failed download (style, tiles, glyphs, sprites)."""


class BasemapWarning(UserWarning):
    """Base class of basemapper's warnings: the notes of a render."""


class BasemapAdjustment(BasemapWarning):
    """Something you asked for was changed, or settled for less: the zoom
    raised to keep the view inside the world, coarser tiles to stay within
    ``max_tiles``, 300 dpi for a PDF / SVG figure. Shown once per call
    site."""


class BasemapProblem(BasemapWarning):
    """The output is incomplete or likely not what you meant: tiles that
    failed, missing glyphs or sprites, unsupported style features, filter
    names that match nothing. Shown every time; ``FetchOptions(on_error=
    "fail")`` makes it an error instead."""


# Problems are shown every time, not once per call site.
warnings.filterwarnings("always", category=BasemapProblem)
