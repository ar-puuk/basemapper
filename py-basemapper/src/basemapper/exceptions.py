class BasemapError(RuntimeError):
    """Raised when the Rust basemapper core returns an error."""

    def __init__(self, message: str) -> None:
        super().__init__(message)
