"""Place each supplied orthographic reference beside the matching Riff V1 render."""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).parent
REFERENCE = Image.open(ROOT / "reference-orthographic.png").convert("RGB")
VIEWS = {
    "front": ((0, 0, 468, 826), "riff-v1-front-orthographic.png"),
    "side": ((450, 0, 934, 826), "riff-v1-side-orthographic.png"),
    "back": ((920, 0, 1392, 826), "riff-v1-back-orthographic.png"),
}

try:
    FONT = ImageFont.truetype("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 28)
except OSError:
    FONT = ImageFont.load_default()


def fit(image: Image.Image, size: tuple[int, int]) -> Image.Image:
    image = image.copy()
    image.thumbnail(size, Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", size, (5, 13, 27))
    canvas.paste(image, ((size[0] - image.width) // 2, (size[1] - image.height) // 2))
    return canvas


for view, (crop, render_name) in VIEWS.items():
    reference = fit(REFERENCE.crop(crop), (700, 850))
    render = fit(Image.open(ROOT / render_name).convert("RGB"), (700, 850))
    board = Image.new("RGB", (1400, 910), (5, 13, 27))
    board.paste(reference, (0, 60))
    board.paste(render, (700, 60))
    draw = ImageDraw.Draw(board)
    draw.text((24, 16), f"REFERENCE — {view.upper()}", font=FONT, fill=(116, 190, 255))
    draw.text((724, 16), f"RIFF V1 — {view.upper()}", font=FONT, fill=(255, 185, 82))
    board.save(ROOT / f"compare-{view}.png")

print("RIFF_V1_COMPARISON_BOARDS_WRITTEN")
