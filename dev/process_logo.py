#!/usr/bin/env python3
"""Process the OpenKite logo into a 1024x1024 app icon.

- decode PNG (RGBA, 8-bit, filters 0-4)
- flood-fill near-white background -> transparent (from borders; the logo's
  internal white X-strokes are enclosed by teal and stay opaque)
- halo pass: light pixels bordering transparency become transparent
- content bbox -> square crop -> nearest-neighbor upscale to 1024
"""
import struct
import zlib

SRC = "/opt/data/cache/images/img_3ec06bef3764.png"
OUT = "icons/icon.png"
SIZE = 1024
FILL_T = 230   # near-white threshold for background flood-fill
HALO_T = 200   # light pixels next to transparency become transparent


def decode_png(path):
    with open(path, "rb") as f:
        data = f.read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, w, h, depth, ctype = 8, b"", 0, 0, 0, 0
    while pos < len(data):
        (ln,) = struct.unpack(">I", data[pos : pos + 4])
        tag = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + ln]
        if tag == b"IHDR":
            w, h, depth, ctype = struct.unpack(">IIBB", body[:10])
        elif tag == b"IDAT":
            idat += body
        pos += 12 + ln
    assert depth == 8 and ctype == 6, f"unsupported {depth}/{ctype}"
    raw = zlib.decompress(idat)
    stride = w * 4
    rows = []
    p, prev = 0, bytearray(stride)
    for _ in range(h):
        ftype = raw[p]
        p += 1
        line = bytearray(raw[p : p + stride])
        p += stride
        if ftype == 1:  # Sub
            for i in range(4, stride):
                line[i] = (line[i] + line[i - 4]) & 0xFF
        elif ftype == 2:  # Up
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif ftype == 3:  # Average
            for i in range(stride):
                a = line[i - 4] if i >= 4 else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 0xFF
        elif ftype == 4:  # Paeth
            for i in range(stride):
                a = line[i - 4] if i >= 4 else 0
                b = prev[i]
                c = prev[i - 4] if i >= 4 else 0
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pred) & 0xFF
        rows.append(bytes(line))
        prev = line
    return w, h, rows


def near_white(px):
    return px[0] >= FILL_T and px[1] >= FILL_T and px[2] >= FILL_T


def process(w, h, rows):
    px = [[(rows[y][x * 4], rows[y][x * 4 + 1], rows[y][x * 4 + 2], rows[y][x * 4 + 3]) for x in range(w)] for y in range(h)]

    # flood-fill background from the border
    seen = [[False] * w for _ in range(h)]
    stack = []
    for x in range(w):
        for y in (0, h - 1):
            if near_white(px[y][x]):
                stack.append((x, y))
    for y in range(h):
        for x in (0, w - 1):
            if near_white(px[y][x]):
                stack.append((x, y))
    while stack:
        x, y = stack.pop()
        if seen[y][x] or not near_white(px[y][x]):
            continue
        seen[y][x] = True
        for nx, ny in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
            if 0 <= nx < w and 0 <= ny < h and not seen[ny][nx]:
                stack.append((nx, ny))
    for y in range(h):
        for x in range(w):
            if seen[y][x]:
                px[y][x] = (0, 0, 0, 0)

    # halo pass: light pixels touching transparency become transparent
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[y][x]
            if a and r >= HALO_T and g >= HALO_T and b >= HALO_T:
                for nx, ny in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
                    if 0 <= nx < w and 0 <= ny < h and px[ny][nx][3] == 0:
                        px[y][x] = (0, 0, 0, 0)
                        break

    # content bbox
    xs = [x for y in range(h) for x in range(w) if px[y][x][3]]
    if not xs:
        raise SystemExit("no content found")
    minx, maxx = min(xs), max(xs)
    ys = [y for y in range(h) for x in range(w) if px[y][x][3]]
    miny, maxy = min(ys), max(ys)
    cw, ch = maxx - minx + 1, maxy - miny + 1
    side = max(cw, ch)
    # crop + center into a square
    ox, oy = minx + (cw - side) // 2, miny + (ch - side) // 2
    sq = [[(0, 0, 0, 0)] * side for _ in range(side)]
    for y in range(side):
        for x in range(side):
            sx, sy = ox + x, oy + y
            if 0 <= sx < w and 0 <= sy < h:
                sq[y][x] = px[sy][sx]

    # nearest-neighbor upscale with 4% padding
    pad = int(side * 0.04)
    inner = side - 2 * pad
    out = [[(0, 0, 0, 0)] * SIZE for _ in range(SIZE)]
    for y in range(SIZE):
        src_y = min(pad + (y * inner // SIZE), inner - 1)
        for x in range(SIZE):
            src_x = min(pad + (x * inner // SIZE), inner - 1)
            out[y][x] = sq[src_y][src_x]
    return out, (minx, miny, maxx, maxy)


def encode_png(px, w, h):
    rows = []
    for y in range(h):
        row = bytearray([0])
        for x in range(w):
            row += bytes(px[y][x])
        rows.append(bytes(row))

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
    png += chunk(b"IEND", b"")
    return png


w, h, rows = decode_png(SRC)
print(f"decoded {w}x{h}")
icon, bbox = process(w, h, rows)
print(f"content bbox: {bbox}")
png = encode_png(icon, SIZE, SIZE)
with open(OUT, "wb") as f:
    f.write(png)
print(f"wrote {OUT} ({len(png)} bytes)")
