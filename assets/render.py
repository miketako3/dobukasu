#!/usr/bin/env python3
"""Render the dobukasu mark and banner.

One shape description, two outputs: the SVG is the original (sharp at any
size, diffable in git) and the PNG is what GitHub can show. Run it after
changing the geometry:

    python3 assets/render.py

The mark is an original silhouette - a figure looking down, cut by a slash,
over a grid of 24 frames. It borrows an idea, not any drawing: no character
design, no copied pose, no traced line from someone else's picture.
"""

from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent

INK = (11, 13, 17)
INK_SOFT = (18, 21, 28)
FIGURE = (30, 36, 50)
PAPER = (240, 236, 228)
GREY = (120, 130, 148)
RED = (216, 58, 58)
RIM = (86, 96, 116)

SIZE = 512


def frame_lines(width: int, height: int, count: int = 24) -> list[tuple[tuple[int, int], tuple[int, int], int, int]]:
    """The 'one second, twenty-four frames' motif: thin vertical rules."""
    lines = []
    span = count - 1
    for index in range(count):
        x = int(width * 0.52 + (width * 0.46) * index / span)
        alpha = 44 if index % 6 == 0 else 15
        lines.append(((x, int(height * 0.14)), (x, int(height * 0.86)), 1, alpha))
    return lines


def mark_shapes(size: int) -> dict[str, list]:
    """Geometry of the mark, expressed in a 512 box and scaled by `size`.

    Composition: tight on the face, tilted, a hand pushing into the frame from
    below, a slash and a radial burst behind. Everything is a silhouette - the
    eyes and mouth are the only light, and they are holes, not features.
    """
    s = size / SIZE

    def p(x: float, y: float) -> tuple[int, int]:
        # A slight counter-clockwise tilt keeps the face from feeling static.
        angle = math.radians(-7)
        cx, cy = 256, 256
        dx, dy = x - cx, y - cy
        rx = cx + dx * math.cos(angle) - dy * math.sin(angle)
        ry = cy + dx * math.sin(angle) + dy * math.cos(angle)
        return (round(rx * s), round(ry * s))

    slash = [p(-30, 400), p(542, 150), p(542, 232), p(-30, 482)]

    shoulders = [
        p(-20, 540),
        p(40, 470),
        p(120, 424),
        p(190, 404),
        p(208, 386),
        p(304, 386),
        p(322, 404),
        p(392, 424),
        p(472, 470),
        p(532, 540),
    ]
    neck = [p(212, 336), p(300, 336), p(306, 392), p(206, 392)]
    head = [
        p(256, 52),
        p(330, 70),
        p(378, 138),
        p(384, 226),
        p(362, 300),
        p(316, 352),
        p(256, 368),
        p(196, 352),
        p(150, 300),
        p(128, 226),
        p(134, 138),
        p(182, 70),
    ]
    hair = [
        p(126, 176),
        p(140, 104),
        p(182, 58),
        p(240, 36),
        p(300, 40),
        p(352, 66),
        p(384, 118),
        p(392, 178),
        p(352, 140),
        p(306, 118),
        p(256, 112),
        p(206, 120),
        p(162, 146),
    ]
    hand = [
        p(-30, 470),
        p(40, 428),
        p(92, 442),
        p(118, 476),
        p(116, 540),
        p(-30, 540),
    ]
    fingers = [
        [p(46, 442), p(92, 396), p(108, 406), p(76, 452)],
        [p(76, 452), p(128, 414), p(142, 426), p(102, 466)],
        [p(100, 472), p(154, 448), p(162, 462), p(118, 490)],
    ]
    eyes = [
        [p(186, 236), p(238, 222), p(240, 234), p(188, 249)],
        [p(274, 222), p(326, 236), p(324, 249), p(272, 234)],
    ]
    mouth = [p(228, 296), p(286, 288), p(288, 297), p(230, 305)]

    burst = []
    for index in range(24):
        angle = math.radians(index * 15 + 7)
        inner, outer = 210, 460
        burst.append(
            (
                p(256 + inner * math.cos(angle), 210 + inner * math.sin(angle)),
                p(256 + outer * math.cos(angle), 210 + outer * math.sin(angle)),
            )
        )

    return {
        "slash": slash,
        "burst": burst,
        "shoulders": shoulders,
        "neck": neck,
        "head": head,
        "hair": hair,
        "hand": hand,
        "fingers": fingers,
        "eyes": eyes,
        "mouth": mouth,
    }


def draw_mark(size: int, *, background: bool = True, with_slash: bool = True) -> Image.Image:
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    shapes = mark_shapes(size)
    s = size / SIZE

    if background:
        draw.rounded_rectangle([0, 0, size - 1, size - 1], radius=round(96 * s), fill=(*INK, 255))

    for (x1, y1), (x2, y2), width, alpha in frame_lines(size, size):
        draw.line([(x1, y1), (x2, y2)], fill=(*GREY, alpha), width=max(1, round(width * s)))

    for (x1, y1), (x2, y2) in shapes["burst"]:
        draw.line([(x1, y1), (x2, y2)], fill=(*GREY, 46), width=max(1, round(3 * s)))

    if with_slash:
        draw.polygon(shapes["slash"], fill=(*RED, 225))

    body = (*FIGURE, 255)
    for key in ("shoulders", "neck", "head", "hair", "hand"):
        draw.polygon(shapes[key], fill=body)
    for finger in shapes["fingers"]:
        draw.polygon(finger, fill=body)

    # Rim light on the left contour so the silhouette separates from the ink.
    draw.line(shapes["head"][:4], fill=(201, 209, 224, 210), width=max(1, round(4 * s)))
    draw.line(shapes["shoulders"][1:3], fill=(*RIM, 150), width=max(1, round(3 * s)))

    for eye in shapes["eyes"]:
        draw.polygon(eye, fill=(*PAPER, 255))
    draw.polygon(shapes["mouth"], fill=(*PAPER, 170))

    return Image.alpha_composite(image, layer)


def svg_mark() -> str:
    shapes = mark_shapes(SIZE)
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}" width="{SIZE}" height="{SIZE}" role="img" aria-label="dobukasu mark: a silhouette looking down, cut by a slash, over a grid of frames">',
        f'  <rect width="{SIZE}" height="{SIZE}" rx="96" fill="#0b0d11"/>',
    ]
    for (x1, y1), (x2, y2), _width, alpha in frame_lines(SIZE, SIZE):
        parts.append(
            f'  <line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#788294" stroke-opacity="{alpha / 255:.2f}"/>'
        )
    points = " ".join(f"{x},{y}" for x, y in shapes["slash"])
    parts.append(f'  <polygon points="{points}" fill="#d83a3a" fill-opacity="0.92"/>')
    for key in ("shoulders", "neck", "head", "hair", "hand"):
        points = " ".join(f"{x},{y}" for x, y in shapes[key])
        parts.append(f'  <polygon points="{points}" fill="#1e2432"/>')
    for finger in shapes["fingers"]:
        points = " ".join(f"{x},{y}" for x, y in finger)
        parts.append(f'  <polygon points="{points}" fill="#1e2432"/>')
    rim = " ".join(f"{x},{y}" for x, y in shapes["head"][:4])
    parts.append(f'  <polyline points="{rim}" fill="none" stroke="#8b97ad" stroke-opacity="0.9" stroke-width="3"/>')
    for eye in shapes["eyes"]:
        points = " ".join(f"{x},{y}" for x, y in eye)
        parts.append(f'  <polygon points="{points}" fill="#f0ece4"/>')
    points = " ".join(f"{x},{y}" for x, y in shapes["mouth"])
    parts.append(f'  <polygon points="{points}" fill="#f0ece4" fill-opacity="0.66"/>')
    parts.append("</svg>")
    return "\n".join(parts) + "\n"


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
    banner.paste(mark, (64, 40), mark)

    bold = font("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 108)
    mid = font("/System/Library/Fonts/Supplemental/Arial.ttf", 30)
    small = font("/System/Library/Fonts/Menlo.ttc", 20)

    draw.text((432, 118), "dobukasu", font=bold, fill=PAPER)
    draw.text((438, 246), "one query  ·  every engine  ·  one answer", font=mid, fill=GREY)
    draw.line([(438, 292), (748, 292)], fill=RED, width=3)
    draw.text((438, 308), "meta-search CLI for AI agents", font=small, fill=(104, 114, 130))
    return banner


def main() -> None:
    svg = svg_mark()
    (HERE / "logo.svg").write_text(svg, encoding="utf-8")

    mark = draw_mark(1024)
    mark.save(HERE / "logo.png", optimize=True)

    render_banner().save(HERE / "banner.png", optimize=True)
    print("wrote logo.svg, logo.png, banner.png")


if __name__ == "__main__":
    main()
