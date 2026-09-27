"""Parse time ranges such as ``1:30-3:00``, ``90-180``, ``1:30-`` or ``-2:00``."""
from __future__ import annotations

import re

_RANGE = re.compile(r"^\s*([\d:.]*)\s*-\s*([\d:.]*)\s*$")


def parse_clock(text: str) -> float:
    """``SS``, ``M:SS`` or ``H:MM:SS`` (seconds may be fractional) → seconds."""
    parts = text.strip().split(":")
    if not 1 <= len(parts) <= 3 or any(p == "" for p in parts):
        raise ValueError(f"invalid time: {text!r}")
    try:
        nums = [float(p) for p in parts]
    except ValueError:
        raise ValueError(f"invalid time: {text!r}") from None
    seconds = 0.0
    for n in nums:
        seconds = seconds * 60 + n
    return seconds


def parse_range(text: str) -> tuple[float, float | None]:
    """Return ``(start, end)``; ``end`` is ``None`` for "until the end of the file"."""
    m = _RANGE.match(text)
    if not m:
        raise ValueError(f"invalid range {text!r}, expected e.g. 1:30-3:00")
    start = parse_clock(m.group(1)) if m.group(1) else 0.0
    end = parse_clock(m.group(2)) if m.group(2) else None
    if end is not None and end <= start:
        raise ValueError("range end must be after its start")
    return start, end
