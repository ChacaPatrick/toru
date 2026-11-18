"""Writes toru.ico from the icon drawings beside it. Run it by hand after changing a drawing:

    python assets/make_ico.py

Needs Pillow, and Microsoft Edge to rasterise the SVGs. toru.ico is committed; no build runs this.
"""
import subprocess
import tempfile
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"

# (frame size, drawing, scale). Up to 32 px each size has a drawing made on its own pixel grid.
# icon.svg's card edge lands on a whole pixel at 48, 64 and 256; at 40 it would fall on a half
# pixel, and 36/35 moves it onto the grid.
FRAMES = [
    (16, "icon-16.svg", 1),
    (20, "icon-20.svg", 1),
    (24, "icon-24.svg", 1),
    (32, "icon-32.svg", 1),
    (40, "icon.svg", 36 / 35),
    (48, "icon.svg", 1),
    (64, "icon.svg", 1),
    (256, "icon.svg", 1),
]
GAP = 4


def rasterise() -> list[Image.Image]:
    cells, boxes, left = [], [], 0
    for size, drawing, scale in FRAMES:
        shown = 1 / scale
        inset = (1 - shown) / 2
        cells.append(
            f'<svg style="position:absolute;left:{left}px;top:0" width="{size}" height="{size}" '
            f'viewBox="{inset} {inset} {shown} {shown}">'
            f'<image href="{(HERE / drawing).as_uri()}" width="1" height="1"/></svg>')
        boxes.append((left, 0, left + size, size))
        left += size + GAP
    width, height = left, max(size for size, _, _ in FRAMES)
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as temp:
        page, shot = Path(temp, "frames.html"), Path(temp, "frames.png")
        page.write_text(f'<!doctype html><body style="margin:0">{"".join(cells)}', encoding="utf-8")
        subprocess.run(
            [EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
             f"--user-data-dir={Path(temp, 'profile')}", f"--window-size={width},{height}",
             "--default-background-color=00000000", f"--screenshot={shot}", page.as_uri()],
            check=True, capture_output=True, timeout=120)
        sheet = Image.open(shot).convert("RGBA")
    if sheet.width < width or sheet.height < height:
        raise SystemExit(f"Edge drew {sheet.size}, smaller than the {width}x{height} sheet of frames")
    return [sheet.crop(box) for box in boxes]


if __name__ == "__main__":
    *smaller, largest = rasterise()
    largest.save(HERE / "toru.ico", sizes=[(size, size) for size, _, _ in FRAMES], append_images=smaller)
