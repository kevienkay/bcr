#!/usr/bin/env python3
"""Analyze bcr UI screenshots: colors, layout boundaries, tofu detection."""
import sys
from PIL import Image
from collections import Counter

def top_colors(img, n=14, scale=2):
    im = img.convert("RGB")
    if scale > 1:
        im = im.resize((im.width // scale, im.height // scale))
    cnt = Counter(im.getdata())
    return cnt.most_common(n)

def hexc(c):
    return "#%02X%02X%02X" % c

def panel_map(img, bg, tol=18):
    """Return per-column and per-row fraction of non-bg pixels."""
    w, h = img.size
    px = img.convert("RGB").load()
    cols = []
    for x in range(0, w, 4):
        n = sum(1 for y in range(0, h, 4) if _diff(px[x, y], bg) > tol)
        cols.append((x, n / (h / 4)))
    rows = []
    for y in range(0, h, 4):
        n = sum(1 for x in range(0, w, 4) if _diff(px[x, y], bg) > tol)
        rows.append((y, n / (w / 4)))
    return cols, rows

def _diff(a, b):
    return sum(abs(x - y) for x, y in zip(a, b)) / 3

def runs(vals, thresh, min_len=2):
    """Find contiguous runs above threshold; vals = list of (coord, value)."""
    out = []
    cur = None
    for c, v in vals:
        if v > thresh:
            if cur is None:
                cur = [c, c]
            else:
                cur[1] = c
        else:
            if cur and (cur[1] - cur[0]) >= min_len:
                out.append(tuple(cur))
            cur = None
    if cur and (cur[1] - cur[0]) >= min_len:
        out.append(tuple(cur))
    return out

def analyze(path):
    img = Image.open(path)
    print(f"\n{'='*72}\n{path}  {img.size}")
    print("-- top colors --")
    for c, n in top_colors(img):
        print(f"  {hexc(c)}  {n:8d}")

def contrast(fg, bg):
    def lum(c):
        def ch(v):
            v /= 255.0
            return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
        return 0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2])
    l1, l2 = lum(fg), lum(bg)
    if l1 < l2: l1, l2 = l2, l1
    return (l1 + 0.05) / (l2 + 0.05)

def sample(img, x0, y0, x1, y1):
    """Average color of a region."""
    px = img.convert("RGB")
    xs = px.crop((x0, y0, x1, y1)).resize((1, 1))
    return xs.getpixel((0, 0))

def row_colors(img, y0, y1, x0=0, x1=None, step=8):
    """Sample colors across a horizontal band."""
    if x1 is None: x1 = img.width
    px = img.convert("RGB")
    out = []
    for x in range(x0, x1, step):
        # average small window
        acc = [0, 0, 0]
        for yy in range(y0, y1):
            p = px[x, yy]
            acc[0] += p[0]; acc[1] += p[1]; acc[2] += p[2]
        n = (y1 - y0)
        out.append((x, tuple(v // n for v in acc)))
    return out

if __name__ == "__main__":
    for p in sys.argv[1:]:
        analyze(p)
