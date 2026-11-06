#!/usr/bin/env python3
"""Render the dobukasu mark and banner.

One shape description, three outputs: logo.svg (the original), logo.png and
banner.png (what GitHub shows). Re-run after changing a number:

    python3 assets/render.py

The mark is an original silhouette built from circles and capsules: a figure
with his back to the viewer, head turned so one eye looks back over the
shoulder, throwing a rush punch off the right edge. It borrows the pose and
the mood, not any drawing.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent

INK = (11, 13, 17)
FIGURE = (34, 41, 56)
COLLAR = (22, 27, 38)
PAPER = (240, 236, 228)
GREY = (120, 130, 148)
RED = (216, 58, 58)
RIM = (198, 207, 222)

SIZE = 512

Point = tuple[float, float]


# --------------------------------------------------------------------------
# geometry
# --------------------------------------------------------------------------


def rotate(point: Point, origin: Point, degrees: float) -> Point:
    angle = math.radians(degrees)
    dx, dy = point[0] - origin[0], point[1] - origin[1]
    return (
        origin[0] + dx * math.cos(angle) - dy * math.sin(angle),
        origin[1] + dx * math.sin(angle) + dy * math.cos(angle),
    )


def capsule(start: Point, end: Point, width: float) -> list[Point]:
    """A thick line with round ends, as a polygon (no curves needed)."""
    radius = width / 2
    angle = math.atan2(end[1] - start[1], end[0] - start[0])
    steps = 12
    points: list[Point] = []
    for index in range(steps + 1):
        theta = angle + math.pi / 2 + math.pi * index / steps
        points.append((end[0] + radius * math.cos(theta), end[1] + radius * math.sin(theta)))
    for index in range(steps + 1):
        theta = angle - math.pi / 2 + math.pi * index / steps
        points.append((start[0] + radius * math.cos(theta), start[1] + radius * math.sin(theta)))
    return points


def circle(center: Point, radius: float, steps: int = 28) -> list[Point]:
    return [
        (
            center[0] + radius * math.cos(2 * math.pi * index / steps),
            center[1] + radius * math.sin(2 * math.pi * index / steps),
        )
        for index in range(steps)
    ]


def box(center: Point, half_width: float, half_height: float, degrees: float) -> list[Point]:
    corners = [
        (center[0] - half_width, center[1] - half_height),
        (center[0] + half_width, center[1] - half_height),
        (center[0] + half_width, center[1] + half_height),
        (center[0] - half_width, center[1] + half_height),
    ]
    return [rotate(corner, center, degrees) for corner in corners]


def frame_lines(width: int, height: int, count: int = 24) -> list[tuple[Point, Point, float]]:
    """The 'one second, twenty-four frames' motif."""
    lines = []
    for index in range(count):
        x = width * 0.50 + (width * 0.48) * index / (count - 1)
        lines.append(((x, height * 0.10), (x, height * 0.90), 0.16 if index % 6 == 0 else 0.055))
    return lines


@dataclass
class Mark:
    slash: list[Point]
    torso: list[Point]
    collar: list[Point]
    shoulder: list[Point]
    arm: list[Point]
    fist: list[Point]
    ghosts: list[list[Point]]
    neck: list[Point]
    skull: list[Point]
    jaw: list[Point]
    hair: list[Point]
    eye: list[Point]
    impact: list[tuple[Point, Point]]
    speed: list[tuple[Point, Point]]


def build_mark(size: int = SIZE) -> Mark:
    """The pose, in a 512 box, scaled to `size`."""
    s = size / SIZE

    def p(point: Point) -> Point:
        return (point[0] * s, point[1] * s)

    # Back to the viewer: the torso sits low and the neck is the only bridge
    # to the head, so the silhouette reads as a body rather than one lump.
    torso = box((142, 486), 112, 108, 14)
    collar = box((196, 352), 92, 22, 16)

    neck = capsule((208, 196), (216, 300), 44)
    shoulder: Point = (252, 320)
    fist_center: Point = (452, 258)
    arm = capsule(shoulder, fist_center, 62)
    fist = circle(fist_center, 44)

    # Rush: the fist again, a frame earlier, and again. Fists rather than
    # whole arms, so the icon reads at 32 px.
    ghosts = []
    for step, radius in ((1, 34), (2, 27), (3, 21)):
        end = (fist_center[0] - 46 * step, fist_center[1] + 30 * step)
        ghosts.append(circle(end, radius))

    # Head turned over the shoulder, looking back at the punch.
    skull = circle((176, 152), 56)
    jaw = [
        (200, 190),
        (258, 206),
        (276, 228),
        (262, 244),
        (214, 236),
        (190, 214),
    ]
    hair = [
        (120, 176),
        (114, 132),
        (134, 92),
        (172, 74),
        (214, 78),
        (238, 100),
        (230, 122),
        (196, 102),
        (158, 106),
        (134, 136),
        (132, 172),
    ]
    eye = [(222, 150), (252, 142), (254, 154), (224, 162)]

    slash = [(-40, 436), (552, 214), (552, 288), (-40, 510)]

    impact = []
    for index in range(9):
        angle = math.radians(-50 + index * 14)
        cx, cy = fist_center
        impact.append(
            (
                (cx + 52 * math.cos(angle), cy + 52 * math.sin(angle)),
                (cx + 104 * math.cos(angle), cy + 104 * math.sin(angle)),
            )
        )

    speed = []
    for index in range(3):
        offset = index * 26
        speed.append(
            (
                (shoulder[0] + 40, shoulder[1] - 96 + offset),
                (fist_center[0] + 54, fist_center[1] - 116 + offset),
            )
        )

    return Mark(
        slash=[p(q) for q in slash],
        torso=[p(q) for q in torso],
        collar=[p(q) for q in collar],
        shoulder=[p(q) for q in circle(shoulder, 52)],
        neck=[p(q) for q in neck],
        arm=[p(q) for q in arm],
        fist=[p(q) for q in fist],
        ghosts=[[p(q) for q in ghost] for ghost in ghosts],
        skull=[p(q) for q in skull],
        jaw=[p(q) for q in jaw],
        hair=[p(q) for q in hair],
        eye=[p(q) for q in eye],
        impact=[(p(a), p(b)) for a, b in impact],
        speed=[(p(a), p(b)) for a, b in speed],
    )


# --------------------------------------------------------------------------
# raster
# --------------------------------------------------------------------------


def draw_mark(size: int = 1024) -> Image.Image:
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    mark = build_mark(size)
    s = size / SIZE

    draw.rounded_rectangle([0, 0, size - 1, size - 1], radius=round(96 * s), fill=(*INK, 255))

    for (x1, y1), (x2, y2), alpha in frame_lines(size, size):
        draw.line([(x1, y1), (x2, y2)], fill=(*GREY, round(255 * alpha)), width=max(1, round(1.5 * s)))

    draw.polygon(mark.slash, fill=(*RED, 215))

    for start, end in mark.impact:
        draw.line([start, end], fill=(*RED, 200), width=max(2, round(5 * s)))

    for start, end in mark.speed:
        draw.line([start, end], fill=(*RIM, 70), width=max(1, round(3 * s)))

    for shape in (mark.torso, mark.neck, mark.shoulder, mark.arm, mark.fist, mark.skull, mark.jaw, mark.hair):
        draw.polygon(shape, fill=(*FIGURE, 255))

    draw.polygon(mark.collar, fill=(*COLLAR, 255))

    for ghost in mark.ghosts:
        draw.polygon(ghost, fill=(*FIGURE, 105))

    # Two strokes of rim light are enough to make the silhouette read as a
    # body rather than a blob: the back of the skull, and the shoulder edge.
    draw.line(
        [mark.skull[16], mark.skull[22]],
        fill=(*RIM, 200),
        width=max(1, round(4 * s)),
    )
    draw.line(
        [mark.shoulder[0], mark.shoulder[len(mark.shoulder) // 2]],
        fill=(*RIM, 130),
        width=max(1, round(4 * s)),
    )

    draw.polygon(mark.eye, fill=(*PAPER, 255))

    return Image.alpha_composite(image, layer)


# --------------------------------------------------------------------------
# svg
# --------------------------------------------------------------------------


def svg_polygon(shape: list[Point], fill: str, opacity: float = 1.0) -> str:
    points = " ".join(f"{x:.1f},{y:.1f}" for x, y in shape)
    extra = "" if opacity >= 1.0 else f' fill-opacity="{opacity:.2f}"'
    return f'  <polygon points="{points}" fill="{fill}"{extra}/>'


def svg_mark() -> str:
    mark = build_mark(SIZE)
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}" width="{SIZE}" height="{SIZE}" role="img" aria-label="dobukasu: a figure with his back turned, landing a rush punch behind him, over a grid of frames">',
        f'  <rect width="{SIZE}" height="{SIZE}" rx="96" fill="#0b0d11"/>',
    ]
    for (x1, y1), (x2, y2), alpha in frame_lines(SIZE, SIZE):
        parts.append(
            f'  <line x1="{x1:.0f}" y1="{y1:.0f}" x2="{x2:.0f}" y2="{y2:.0f}" stroke="#788294" stroke-opacity="{alpha:.2f}"/>'
        )
    parts.append(svg_polygon(mark.slash, "#d83a3a", 0.84))
    for start, end in mark.impact:
        parts.append(
            f'  <line x1="{start[0]:.0f}" y1="{start[1]:.0f}" x2="{end[0]:.0f}" y2="{end[1]:.0f}" stroke="#d83a3a" stroke-opacity="0.78" stroke-width="5"/>'
        )
    for start, end in mark.speed:
        parts.append(
            f'  <line x1="{start[0]:.0f}" y1="{start[1]:.0f}" x2="{end[0]:.0f}" y2="{end[1]:.0f}" stroke="#c6cfde" stroke-opacity="0.27" stroke-width="3"/>'
        )
    for shape in (mark.torso, mark.neck, mark.shoulder, mark.arm, mark.fist, mark.skull, mark.jaw, mark.hair):
        parts.append(svg_polygon(shape, "#222936"))
    parts.append(svg_polygon(mark.collar, "#161b26"))
    for ghost in mark.ghosts:
        parts.append(svg_polygon(ghost, "#222936", 0.41))
    parts.append(
        f'  <line x1="{mark.skull[16][0]:.0f}" y1="{mark.skull[16][1]:.0f}" x2="{mark.skull[22][0]:.0f}" y2="{mark.skull[22][1]:.0f}" stroke="#c6cfde" stroke-opacity="0.78" stroke-width="4"/>'
    )
    parts.append(svg_polygon(mark.eye, "#f0ece4"))
    parts.append("</svg>")
    return "\n".join(parts) + "\n"


# --------------------------------------------------------------------------
# banner
# --------------------------------------------------------------------------


def font(path: str, size: int) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(path, size)


def render_banner() -> Image.Image:
    width, height = 1280, 400
    banner = Image.new("RGB", (width, height), INK)
    draw = ImageDraw.Draw(banner)
    for y in range(height):
        t = y / (height - 1)
        colour = tuple(round(INK[i] + (24 - INK[i]) * t) for i in range(3))
        draw.line([(0, y), (width, y)], fill=colour)

    mark = draw_mark(320)
    banner.paste(mark, (56, 40), mark)

    bold = font("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 104)
    mid = font("/System/Library/Fonts/Supplemental/Arial.ttf", 30)
    small = font("/System/Library/Fonts/Menlo.ttc", 20)

    draw.text((424, 112), "dobukasu", font=bold, fill=PAPER)
    draw.text((430, 238), "one query  ·  every engine  ·  one answer", font=mid, fill=GREY)
    draw.line([(430, 286), (740, 286)], fill=RED, width=3)
    draw.text((430, 304), "meta-search CLI for AI agents", font=small, fill=(104, 114, 130))
    return banner


def main() -> None:
    (HERE / "logo.svg").write_text(svg_mark(), encoding="utf-8")
    draw_mark(1024).save(HERE / "logo.png", optimize=True)
    render_banner().save(HERE / "banner.png", optimize=True)
    print("wrote logo.svg, logo.png, banner.png")


if __name__ == "__main__":
    main()
