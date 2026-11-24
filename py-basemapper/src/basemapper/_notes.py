"""Notes: what basemapper decided, adjusted, or could not do (§4.8)."""

from __future__ import annotations

import logging
import warnings
from typing import Iterable, NamedTuple, Tuple

from .exceptions import BasemapAdjustment, BasemapProblem

logger = logging.getLogger("basemapper")


class Note(NamedTuple):
    """One note of a view or rendering.

    Attributes:
        level: ``"decided"`` (a value chosen because nothing gave it),
            ``"adjusted"`` (something asked for was changed) or ``"problem"``
            (the output is incomplete).
        kind: A stable identifier, e.g. ``"zoom_auto"``, ``"tile_skipped"``.
        message: For people.
        details: Each occurrence's message, when it happened several times.
        count: How many times it happened.
    """

    level: str
    kind: str
    message: str
    details: Tuple[str, ...] = ()
    count: int = 1


def notes_of(raw: Iterable[tuple]) -> Tuple[Note, ...]:
    """The core's ``(level, kind, message, details, count)`` tuples as notes."""
    return tuple(Note(level, kind, message, tuple(details), count) for level, kind, message, details, count in raw)


def surface(notes: Iterable[Note], stacklevel: int = 3) -> None:
    """Show notes as their level asks: *decided* on the ``basemapper`` logger
    at INFO, *adjusted* as a :class:`BasemapAdjustment` warning, *problem*
    as a :class:`BasemapProblem` warning."""
    for n in notes:
        if n.level == "decided":
            logger.info(n.message)
        elif n.level == "adjusted":
            warnings.warn(n.message, BasemapAdjustment, stacklevel=stacklevel)
        else:
            warnings.warn(n.message, BasemapProblem, stacklevel=stacklevel)
