class BasemapError(RuntimeError):
    """Base exception for all basemapper errors."""

    def __init__(self, message: str) -> None:
        super().__init__(message)


class ValidationError(BasemapError, ValueError):
    """Raised for invalid bbox, dimensions, zoom, or layer filter.

    Subclasses both :class:`BasemapError` and :class:`ValueError` so that
    callers that catch the built-in ``ValueError`` continue to work unchanged.
    """


class StyleError(BasemapError):
    """Raised when a style cannot be fetched or parsed."""


class NetworkError(BasemapError):
    """Raised when tile or style network requests fail."""
