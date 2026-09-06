#!/usr/bin/env python3
"""Rasterize the prototype's cell SVG with an installed monospace font.

The Rust renderer supplies every cell, position and color. This helper only paints
that export; it contains no screen composition. Requires Pillow, for PNG export only.
"""

import argparse
from pathlib import Path
import xml.etree.ElementTree as ET

from PIL import Image, ImageDraw, ImageFont


def render(source: Path, destination: Path, font_path: str) -> None:
    root = ET.parse(source).getroot()
    width, height = int(root.attrib["width"]), int(root.attrib["height"])
    scale = 5
    image = Image.new("RGB", (width * scale, height * scale), "#0c121b")
    draw = ImageDraw.Draw(image)
    fonts = {"normal": ImageFont.truetype(font_path, 83, index=0)}
    for index in range(4):
        try:
            font = ImageFont.truetype(font_path, 83, index=index)
            if font.getname()[1].lower() == "bold":
                fonts["bold"] = font
        except OSError:
            break
    fonts.setdefault("bold", fonts["normal"])
    for node in root.iter():
        kind = node.tag.rsplit("}", 1)[-1]
        if kind == "rect":
            if "%" in node.attrib["width"]:
                continue
            x, y = float(node.attrib.get("x", 0)), float(node.attrib.get("y", 0))
            w, h = float(node.attrib["width"]), float(node.attrib["height"])
            draw.rectangle((x * scale, y * scale, (x + w) * scale - 1, (y + h) * scale - 1), fill=node.attrib["fill"])
        elif kind == "text":
            draw.text((float(node.attrib["x"]) * scale, float(node.attrib["y"]) * scale), node.text or "", font=fonts[node.attrib["font-weight"]], fill=node.attrib["fill"], anchor="ls")
    destination.parent.mkdir(parents=True, exist_ok=True)
    image.resize((width, height), Image.Resampling.LANCZOS).save(destination)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--font", default="/System/Library/Fonts/Menlo.ttc")
    args = parser.parse_args()
    render(args.source, args.destination, args.font)
