"""Shared test setup."""

import pytest


@pytest.fixture(autouse=True, scope="session")
def temporary_tile_cache(tmp_path_factory):
    """Tests use a temporary tile cache, never the user's."""
    import basemapper

    basemapper.tile_cache(str(tmp_path_factory.mktemp("tiles")))
    yield
