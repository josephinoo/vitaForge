#!/usr/bin/env python3
"""Writes the Cargo.toml version into the LiveArea background (static/sce_sys/livearea/contents/bg0.png).

The image is a baked picture, so the old version text is painted over with the gradient
slice just above it and the new one drawn on top. Always starts from the template.
"""
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

root = Path(__file__).resolve().parent.parent
version = re.search(r'^version\s*=\s*"([^"]+)"', (root / "Cargo.toml").read_text(), re.M).group(1)
img = Image.open(root / "assets/images/livearea_bg0_template.png").convert("RGB")

# Where the template has "v0.1.9" (x 227..271, y 445..458); wider than the old text so longer versions fit.
x0, y0, x1, y1 = 224, 438, 330, 466
img.paste(img.crop((x0, y0 - 34, x1, y1 - 34)), (x0, y0))

font = None
for path in ("/System/Library/Fonts/Avenir.ttc", "/System/Library/Fonts/Helvetica.ttc", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"):
    try:
        font = ImageFont.truetype(path, 17)
        break
    except OSError:
        pass
if font is None:
    sys.exit("stamp_livearea: no usable font found")
draw = ImageDraw.Draw(img)
# Anchor on the old text's left edge and baseline.
draw.text((227, 458), f"v{version}", font=font, fill=(255, 255, 255), anchor="ls")

out = root / "static/sce_sys/livearea/contents/bg0.png"
# The Vita only loads 256-colour indexed PNGs here (the template is one); RGB shows nothing.
img = img.quantize(colors=256, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.FLOYDSTEINBERG)
img.save(out, optimize=True)
print(f"livearea: stamped v{version} into {out.relative_to(root)}")
