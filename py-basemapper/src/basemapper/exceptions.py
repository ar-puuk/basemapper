class BasemapError(RuntimeError):
    """Base exception for all basemapper errors."""

    def __init__(self, message: str) -> None:
        super().__init__(message)


class ValidationError(BasemapError):
    """Raised for invalid bbox, dimensions, zoom, or layer filter."""


class StyleError(BasemapError):
    """Raised when a style cannot be fetched or parsed."""


class NetworkError(BasemapError):
    """Raised when tile or style network requests fail."""
