"""PewPew Blitz shell chrome (BRL-0114, BRL-0116, BRL-0156, BRL-4jdx). Python 3 standard library only.

Run from the repository root:  python3 asset_src/player-shell/build_shell_art.py

Writes the editable SVG source (asset_src/player-shell/<name>.svg) and the runtime PNG
(assets/brawler/ui/shell/<name>.png) for each piece of code-authored shell chrome a screen loads:

- panel-identity: the Dashboard identity plate (angled ink panel on a sand offset lip), 2x export;
- underline: the Dashboard title underline arrow, 2x export;
- game-selection-background: the shared BRL-0140 `shell-bg` gradient with two low-contrast tonal
  planes, 1x export (the Dashboard shader draws the same planes natively).

The PNGs come from an anti-aliased signed-distance rasterizer over the same geometry the SVG
describes, so output is deterministic and needs no Sharp. `generate-map-overviews.py` reuses
`Canvas` for the Practice map overviews.
"""
import math
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SVG_DIR = Path(__file__).resolve().parent
PNG_DIR = ROOT / "assets/brawler/ui/shell"

INK = "#061a25"
SAND = "#c3b398"


def hex_rgb(value):
    return tuple(int(value[i:i + 2], 16) / 255 for i in (1, 3, 5))


# ---- geometry ---------------------------------------------------------------------------------
def flatten(commands, steps=16):
    """Absolute M/L/Q commands -> closed polygon points."""
    pts = []
    for op, *args in commands:
        if op in "ML":
            pts.append(tuple(args))
        else:  # Q cx cy x y
            (x0, y0), (cx, cy, x1, y1) = pts[-1], args
            for k in range(1, steps + 1):
                t = k / steps
                u = 1 - t
                pts.append((u * u * x0 + 2 * u * t * cx + t * t * x1,
                            u * u * y0 + 2 * u * t * cy + t * t * y1))
    if pts[0] == pts[-1]:
        pts.pop()
    return pts


def path_d(commands):
    def fmt(v):
        return f"{v:g}"
    return " ".join(op + " ".join(fmt(v) for v in args) for op, *args in commands) + " Z"


def _seg(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    ll = dx * dx + dy * dy
    t = 0.0 if ll == 0 else max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / ll))
    ex, ey = px - ax - dx * t, py - ay - dy * t
    return math.sqrt(ex * ex + ey * ey)


def polygon_sdf(pts, px, py):
    d = float("inf")
    inside = False
    for i in range(len(pts)):
        ax, ay = pts[i]
        bx, by = pts[i - 1]
        d = min(d, _seg(px, py, ax, ay, bx, by))
        if (ay > py) != (by > py) and px < (bx - ax) * (py - ay) / (by - ay) + ax:
            inside = not inside
    return -d if inside else d


def bounds(pts, pad=0.0):
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    return min(xs) - pad, min(ys) - pad, max(xs) + pad, max(ys) + pad


# ---- raster -----------------------------------------------------------------------------------
class Canvas:
    """Premultiplied float RGBA canvas in SVG user units: `scale` pixels per unit, or
    `scale` across and `scale_y` down when a fit-inside export rounds its size."""

    def __init__(self, width, height, scale=1.0, scale_y=None):
        self.w, self.h = width, height
        self.sx, self.sy = scale, scale_y or scale
        self.scale = min(self.sx, self.sy)
        n = width * height
        self.r, self.g, self.b, self.a = [0.0] * n, [0.0] * n, [0.0] * n, [0.0] * n

    def paint(self, box, coverage, color, opacity=1.0):
        """Composite `color` over the pixels whose centres fall in `box` (units).

        `coverage(px, py)` returns 0..1 at a pixel centre in units; `opacity` is a constant or a
        function of the same point (gradient opacity)."""
        sx, sy = self.sx, self.sy
        x0 = max(0, int(math.floor(box[0] * sx)))
        y0 = max(0, int(math.floor(box[1] * sy)))
        x1 = min(self.w, int(math.ceil(box[2] * sx)) + 1)
        y1 = min(self.h, int(math.ceil(box[3] * sy)) + 1)
        cr, cg, cb = hex_rgb(color) if isinstance(color, str) else color
        flat = not callable(opacity)
        r, g, b, a = self.r, self.g, self.b, self.a
        for j in range(y0, y1):
            py = (j + 0.5) / sy
            row = j * self.w
            for i in range(x0, x1):
                px = (i + 0.5) / sx
                c = coverage(px, py)
                if c <= 0.0:
                    continue
                c *= opacity if flat else opacity(px, py)
                k = 1.0 - c
                n = row + i
                r[n] = cr * c + r[n] * k
                g[n] = cg * c + g[n] * k
                b[n] = cb * c + b[n] * k
                a[n] = c + a[n] * k

    def cov(self, d):
        """Coverage of a pixel whose centre is `d` units outside an edge."""
        return max(0.0, min(1.0, 0.5 - d * self.scale))

    def fill_polygon(self, pts, color, opacity=1.0):
        self.paint(bounds(pts, 1 / self.scale), lambda x, y: self.cov(polygon_sdf(pts, x, y)),
                   color, opacity)

    def stroke_polygon(self, pts, width, color):
        half = width / 2
        self.paint(bounds(pts, half + 1 / self.scale),
                   lambda x, y: self.cov(abs(polygon_sdf(pts, x, y)) - half), color)

    def butt_line(self, a, b, width, color, opacity=1.0):
        """Straight stroke with butt caps (SVG default)."""
        (ax, ay), (bx, by) = a, b
        length = math.hypot(bx - ax, by - ay)
        ux, uy = (bx - ax) / length, (by - ay) / length

        def coverage(x, y):
            along = (x - ax) * ux + (y - ay) * uy
            across = abs((x - ax) * -uy + (y - ay) * ux)
            return self.cov(across - width / 2) * self.cov(max(-along, along - length))

        self.paint(bounds([a, b], width / 2 + 1 / self.scale), coverage, color, opacity)

    def fill_rect(self, x, y, w, h, color, opacity=1.0):
        """Exact-area coverage of an axis-aligned rectangle."""
        sx, sy = self.sx, self.sy
        x0, x1, y0, y1 = x * sx, (x + w) * sx, y * sy, (y + h) * sy

        def coverage(px, py):
            i, j = px * sx - 0.5, py * sy - 0.5
            cx = min(i + 1, x1) - max(i, x0)
            cy = min(j + 1, y1) - max(j, y0)
            return cx * cy if cx > 0 and cy > 0 else 0.0

        self.paint((x, y, x + w, y + h), coverage, color, opacity)

    def stroke_rect(self, x, y, w, h, width, color, opacity=1.0):
        """Centred rectangle outline as outer area minus inner area."""
        sx, sy = self.sx, self.sy
        hw = width / 2
        outer = (x - hw, y - hw, x + w + hw, y + h + hw)
        inner = (x + hw, y + hw, x + w - hw, y + h - hw)

        def area(box, i, j):
            cx = min(i + 1, box[2] * sx) - max(i, box[0] * sx)
            cy = min(j + 1, box[3] * sy) - max(j, box[1] * sy)
            return cx * cy if cx > 0 and cy > 0 else 0.0

        def coverage(px, py):
            i, j = px * sx - 0.5, py * sy - 0.5
            return area(outer, i, j) - area(inner, i, j)

        self.paint(outer, coverage, color, opacity)

    def circle(self, cx, cy, radius, color, opacity=1.0, stroke=None, stroke_width=0.0):
        pad = stroke_width / 2 + 1 / self.scale
        box = (cx - radius - pad, cy - radius - pad, cx + radius + pad, cy + radius + pad)
        self.paint(box, lambda x, y: self.cov(math.hypot(x - cx, y - cy) - radius), color,
                   opacity)
        if stroke:
            self.paint(box, lambda x, y: self.cov(
                abs(math.hypot(x - cx, y - cy) - radius) - stroke_width / 2), stroke)

    def write_png(self, path):
        """8-bit RGBA, or RGB when every pixel is opaque."""
        opaque = all(round(a * 255) == 255 for a in self.a)
        rows = []
        for j in range(self.h):
            row = bytearray()
            for n in range(j * self.w, (j + 1) * self.w):
                a = self.a[n]
                if a <= 0.0:
                    row += b"\x00\x00\x00\x00"
                    continue
                row += bytes((round(self.r[n] / a * 255), round(self.g[n] / a * 255),
                              round(self.b[n] / a * 255)))
                if not opaque:
                    row.append(round(a * 255))
            rows.append(b"\x00" + bytes(row))

        def chunk(tag, data):
            return struct.pack(">I", len(data)) + tag + data + struct.pack(
                ">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

        Path(path).write_bytes(
            b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", self.w, self.h, 8, 2 if opaque else 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(b"".join(rows), 9)) + chunk(b"IEND", b""))


def write_svg(name, width, height, body):
    (SVG_DIR / f"{name}.svg").write_text(
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}">{body}</svg>')


# ---- artwork ----------------------------------------------------------------------------------
def panel_identity():
    """650x80 angled plate: sand offset lip, ink fill fading to 94 %, ink outline, faint top
    highlight. The retired BRL-0114 panel family shared this silhouette."""
    w, h, slant = 650, 80, 18
    commands = [("M", slant + 8, 7), ("L", w - 16, 3), ("Q", w - 7, 3, w - 9, 13),
                ("L", w - slant - 5, h - 20), ("Q", w - slant - 6, h - 13, w - slant - 14, h - 13),
                ("L", 10, h - 9), ("Q", 3, h - 9, 5, h - 18), ("L", slant, 15),
                ("Q", slant + 1, 7, slant + 8, 7)]
    d = path_d(commands)
    highlight = ((slant + 9, 12), (w - 20, 8))
    write_svg("panel-identity", w, h,
              f'<defs><linearGradient id="f" x2=".25" y2="1"><stop stop-color="{INK}"/>'
              f'<stop offset="1" stop-color="{INK}" stop-opacity=".94"/></linearGradient></defs>'
              f'<path d="{d}" fill="{SAND}" transform="translate(6 8)"/>'
              f'<path d="{d}" fill="url(#f)" stroke="{INK}" stroke-width="6" '
              f'stroke-linejoin="round"/><path d="M{highlight[0][0]} {highlight[0][1]} '
              f'L{highlight[1][0]} {highlight[1][1]}" stroke="white" stroke-opacity=".2" '
              f'stroke-width="2"/>')
    pts = flatten(commands)
    canvas = Canvas(w * 2, h * 2, 2.0)
    canvas.fill_polygon([(x + 6, y + 8) for x, y in pts], SAND)
    # objectBoundingBox gradient from (0,0) to (.25,1): opacity 1 -> .94.
    bx0, by0, bx1, by1 = bounds(pts)

    def fade(x, y):
        u, v = (x - bx0) / (bx1 - bx0), (y - by0) / (by1 - by0)
        t = max(0.0, min(1.0, (0.25 * u + v) / (0.25 * 0.25 + 1)))
        return 1.0 - 0.06 * t

    canvas.fill_polygon(pts, INK, fade)
    canvas.stroke_polygon(pts, 6, INK)
    canvas.butt_line(*highlight, 2, "#ffffff", 0.2)
    return "panel-identity", canvas


def underline():
    """400x20 ink arrow under the Dashboard title."""
    pts = [(4, 8), (366, 8), (359, 1), (396, 8), (359, 20), (366, 13), (4, 13)]
    write_svg("underline", 400, 20, f'<path fill="{INK}" d="M4 8h362l-7-7 37 7-37 12 7-7H4Z"/>')
    canvas = Canvas(800, 40, 2.0)
    canvas.fill_polygon(pts, INK)
    return "underline", canvas


def game_selection_background():
    """1600x900 BRL-0156 `shell-bg`: vertical #2e7bff -> #1b3fa8 gradient, a 14 % navy plane
    from the left edge and an 8 % white plane along the bottom."""
    w, h = 1600, 900
    top, bottom = hex_rgb("#2e7bff"), hex_rgb("#1b3fa8")
    navy = [(0, 180), (1100, 900), (0, 900)]
    light = [(0, 900), (1600, 670), (1600, 900)]
    write_svg("game-selection-background", w, h,
              '<defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">'
              '<stop offset="0" stop-color="#2e7bff"/><stop offset="1" stop-color="#1b3fa8"/>'
              f'</linearGradient></defs><path fill="url(#bg)" d="M0 0h{w}v{h}H0z"/>'
              '<path fill="#0e1638" fill-opacity=".14" d="M0 180 1100 900H0z"/>'
              '<path fill="#ffffff" fill-opacity=".08" d="m0 900 1600-230v230z"/>')
    canvas = Canvas(w, h, 1.0)
    for j in range(h):
        t = (j + 0.5) / h
        row = slice(j * w, (j + 1) * w)
        for channel, c in zip((canvas.r, canvas.g, canvas.b), range(3)):
            channel[row] = [top[c] + (bottom[c] - top[c]) * t] * w
        canvas.a[row] = [1.0] * w
    canvas.fill_polygon(navy, "#0e1638", 0.14)
    canvas.fill_polygon(light, "#ffffff", 0.08)
    return "game-selection-background", canvas


def main():
    for build in (panel_identity, underline, game_selection_background):
        name, canvas = build()
        canvas.write_png(PNG_DIR / f"{name}.png")
        print(f"{name}: {canvas.w}x{canvas.h}")


if __name__ == "__main__":
    main()
