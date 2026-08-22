#!/usr/bin/env python3
"""Generate a placeholder OpenKite app icon (1024x1024 RGBA PNG, no deps).

Design: slate background, teal kite (diamond) with a white center node —
placeholder until real branding lands.
"""
import os
import struct
import zlib

W = H = 1024
CX = CY = W / 2

def pixel(x, y):
    # kite diamond: |dx|/rx + |dy|/ry <= 1
    dx = abs(x - CX) / (W * 0.42)
    dy = abs(y - CY) / (H * 0.30)
    in_diamond = dx + dy <= 1.0
    in_dot = (x - CX) ** 2 + (y - CY) ** 2 < (W * 0.055) ** 2
    if in_dot:
        return (255, 255, 255, 255)   # white center node
    if in_diamond:
        return (20, 184, 166, 255)    # teal
    return (15, 23, 42, 255)          # slate

rows = []
for y in range(H):
    row = bytearray([0])  # filter: none
    for x in range(W):
        row += bytes(pixel(x, y))
    rows.append(bytes(row))

def chunk(tag, data):
    body = tag + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
png += chunk(b"IEND", b"")

os.makedirs("icons", exist_ok=True)
with open("icons/icon.png", "wb") as f:
    f.write(png)
print(f"icons/icon.png written ({len(png)} bytes)")
