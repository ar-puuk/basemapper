"""Layer discovery utility."""

from __future__ import annotations

import json
from typing import List


def list_layers(style_input: str) -> List[str]:
    """Return all layer IDs from a MapLibre GL style.

    Also prints a two-column table of ``id`` and ``type`` to stdout for
    interactive exploration.

    Args:
        style_input: A MapLibre GL style URL or inline JSON string.

    Returns:
        Sorted list of layer ``id`` values.

    Raises:
        BasemapError: If the style cannot be fetched or parsed.
    """
    from .exceptions import BasemapError

    if style_input.strip().startswith("{"):
        try:
            style = json.loads(style_input)
        except json.JSONDecodeError as exc:
            raise BasemapError(f"style parse error: {exc}") from exc
    else:
        import requests

        try:
            resp = requests.get(style_input, timeout=10)
            resp.raise_for_status()
            style = resp.json()
        except Exception as exc:
            raise BasemapError(f"style fetch failed: {exc}") from exc

    layers = style.get("layers", [])
    ids = [layer.get("id", "") for layer in layers if "id" in layer]

    # Print interactive table.
    print(f"{'id':<40} {'type':<20}")
    print("-" * 61)
    for layer in layers:
        lid = layer.get("id", "")
        ltype = layer.get("type", "")
        print(f"{lid:<40} {ltype:<20}")

    return sorted(ids)
