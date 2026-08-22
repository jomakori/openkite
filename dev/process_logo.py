#!/usr/bin/env python3
"""Square-crop the OpenKite logo to a 1024x1024 white-bg app icon."""
import struct
import zlib

SRC = "/opt/data/cache/images/img_3ec06bef3764.png"
OUT = "icons/icon.png"
SIZE = 1024
PAD = 0.08
WHITE_T = 230


def decode_png(path):
    with open(path, "rb") as f:
        data = f.read()
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
    assert depth == 8 and ctype == 6
    raw = zlib.decompress(idat)
    stride = w * 4
    rows, prev, p = [], bytearray(stride), 0
    for _ in range(h):
        ftype = raw[p]
        p += 1
        line = bytearray(raw[p : p + stride])
        p += stride
        if ftype == 1:  # PNG filter: sub
            for i in range(4, stride):
                line[i] = (line[i] + line[i - 4]) & 0xFF
        elif ftype == 2:  # up
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif ftype == 3:  # average
            for i in range(stride):
                a = line[i - 4] if i >= 4 else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 0xFF
        elif ftype == 4:  # paeth
            for i in range(stride):
                a = line[i - 4] if i >= 4 else 0
                b = prev[i]
                c = prev[i - 4] if i >= 4 else 0
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                line[i] = (line[i] + (a if (pa <= pb and pa <= pc) else (b if pb <= pc else c))) & 0xFF
        rows.append(bytes(line))
        prev = line
    return w, h, rows


def process(w, h, rows):
    # content = non-white pixels (internal white X is enclosed by teal)
    pts = [(x, y) for y in range(h) for x in range(w) if min(rows[y][x * 4 : x * 4 + 3]) < WHITE_T]
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    minx, maxx, miny, maxy = min(xs), max(xs), min(ys), max(ys)
    cx, cy = (minx + maxx) / 2, (miny + maxy) / 2
    side = int(max(maxx - minx, maxy - miny) / (1 - 2 * PAD)) + 1
    x0, y0 = max(0, int(cx - side / 2)), max(0, int(cy - side / 2))
    x1, y1 = min(w, x0 + side), min(h, y0 + side)
    sq = [[(255, 255, 255, 255)] * side for _ in range(side)]
    for y in range(y0, y1):
        for x in range(x0, x1):
            sq[y - y0][x - x0] = rows[y][x * 4 : x * 4 + 4]
    out = [[sq[min(y * side // SIZE, side - 1)][min(x * side // SIZE, side - 1)] for x in range(SIZE)] for y in range(SIZE)]
    return out, (minx, miny, maxx, maxy)


def encode_png(px, w, h):
    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    rows = [bytes([0]) + b"".join(bytes(p) for p in row) for row in px]
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
    return png + chunk(b"IEND", b"")


w, h, rows = decode_png(SRC)
icon, bbox = process(w, h, rows)
with open(OUT, "wb") as f:
    f.write(encode_png(icon, SIZE, SIZE))
print(f"{OUT}: {SIZE}x{SIZE} (content bbox {bbox})")
