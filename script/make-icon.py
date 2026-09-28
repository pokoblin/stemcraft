#!/usr/bin/env python3
"""Draw the placeholder app icon: six coloured stem bars on a rounded tile.

Writes a 1024x1024 RGBA PNG to the path given as the only argument.
"""
import struct
import sys
import zlib

SIZE = 1024
MARGIN = 100  # macOS icon grid leaves transparent space around the tile
RADIUS = 185
BACKGROUND = (28, 27, 38)
BARS = [
    ((0xD8, 0x5A, 0x30), 0.42),  # drums
    ((0xBA, 0x75, 0x17), 0.62),  # bass
    ((0x7F, 0x77, 0xDD), 0.80),  # vocals
    ((0x37, 0x8A, 0xDD), 0.56),  # piano
    ((0x1D, 0x9E, 0x75), 0.70),  # guitar
    ((0xD4, 0x53, 0x7E), 0.36),  # other
]
BAR_W, GAP = 78, 38


def inside_tile(x, y):
    lo, hi = MARGIN, SIZE - 1 - MARGIN
    if not (lo <= x <= hi and lo <= y <= hi):
        return False
    cx = min(max(x, lo + RADIUS), hi - RADIUS)
    cy = min(max(y, lo + RADIUS), hi - RADIUS)
    return (x - cx) ** 2 + (y - cy) ** 2 <= RADIUS ** 2


def pixel(x, y):
    if not inside_tile(x, y):
        return (0, 0, 0, 0)
    left = (SIZE - (len(BARS) * BAR_W + (len(BARS) - 1) * GAP)) // 2
    if x >= left:
        i, offset = divmod(x - left, BAR_W + GAP)
        if i < len(BARS) and offset < BAR_W:
            colour, height = BARS[i]
            h = int(height * 520)
            top = SIZE // 2 - h // 2
            if top <= y < top + h:
                return colour + (255,)
    return BACKGROUND + (255,)


def chunk(tag, data):
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


def main():
    rows = b"".join(
        b"\x00" + bytes(c for x in range(SIZE) for c in pixel(x, y)) for y in range(SIZE)
    )
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(rows, 9))
        + chunk(b"IEND", b"")
    )
    with open(sys.argv[1], "wb") as f:
        f.write(png)


if __name__ == "__main__":
    main()
