"""Writes site/media/contours.svg: contour lines of a smooth noise field, the site's cloud map.
It is a plain file, so the browser only has to move it. Run:  python tools/contours.py
"""
import math
import pathlib
import random

OUT = pathlib.Path(__file__).resolve().parent.parent / "site" / "media" / "contours.svg"
W, H, CELL = 1600, 1000, 12
LEVELS = 13
random.seed(7)
WAVES = [(random.uniform(.0012, .0052), random.uniform(0, math.tau), random.uniform(0, math.tau), random.uniform(.5, 1)) for _ in range(9)]


def field(x, y):
    return sum(a * math.sin(f * (x * math.cos(d) + y * math.sin(d)) + p) for f, d, p, a in WAVES)


grid = [[field(x * CELL, y * CELL) for x in range(W // CELL + 2)] for y in range(H // CELL + 2)]
low = min(map(min, grid))
high = max(map(max, grid))


def crossing(a, b, level):
    return (level - a) / (b - a)


def segments(level):
    """Marching squares: the line pieces of one level, as pairs of points."""
    out = []
    for j in range(len(grid) - 1):
        for i in range(len(grid[0]) - 1):
            a, b, c, d = grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]
            edges = []
            if (a < level) != (b < level):
                edges.append(((i + crossing(a, b, level)) * CELL, j * CELL))
            if (b < level) != (c < level):
                edges.append(((i + 1) * CELL, (j + crossing(b, c, level)) * CELL))
            if (d < level) != (c < level):
                edges.append(((i + crossing(d, c, level)) * CELL, (j + 1) * CELL))
            if (a < level) != (d < level):
                edges.append((i * CELL, (j + crossing(a, d, level)) * CELL))
            if len(edges) == 2:
                out.append((edges[0], edges[1]))
            elif len(edges) == 4:
                out += [(edges[0], edges[1]), (edges[2], edges[3])]
    return out


def key(p):
    return round(p[0], 1), round(p[1], 1)


def chains(pieces):
    """Joins line pieces that touch into long lines."""
    ends = {}
    for n, (p, q) in enumerate(pieces):
        ends.setdefault(key(p), []).append(n)
        ends.setdefault(key(q), []).append(n)
    used = [False] * len(pieces)
    lines = []
    for start in range(len(pieces)):
        if used[start]:
            continue
        used[start] = True
        line = list(pieces[start])
        for grow_front in (False, True):
            while True:
                tip = line[0] if grow_front else line[-1]
                next_piece = next((n for n in ends[key(tip)] if not used[n]), None)
                if next_piece is None:
                    break
                used[next_piece] = True
                p, q = pieces[next_piece]
                other = q if key(p) == key(tip) else p
                if grow_front:
                    line.insert(0, other)
                else:
                    line.append(other)
        lines.append(line)
    return lines


def smooth(line):
    """A curve through the midpoints, which hides the grid and needs few numbers."""
    points = line[::3] + [line[-1]]
    if len(points) < 4:
        return ""
    d = f"M{points[0][0]:.0f} {points[0][1]:.0f}"
    for p, q in zip(points[1:-1], points[2:]):
        d += f"Q{p[0]:.0f} {p[1]:.0f} {(p[0] + q[0]) / 2:.0f} {(p[1] + q[1]) / 2:.0f}"
    return d


paths = []
for n in range(1, LEVELS):
    level = low + (high - low) * n / LEVELS
    paths += [smooth(line) for line in chains(segments(level)) if len(line) > 20]
body = "".join(p for p in paths if p)
OUT.write_text(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" preserveAspectRatio="xMidYMid slice"><path fill="none" stroke="#fff" stroke-width="1" d="{body}"/></svg>\n', encoding="utf-8")
print(f"{len(paths)} lines, {OUT.stat().st_size // 1024} KB")
