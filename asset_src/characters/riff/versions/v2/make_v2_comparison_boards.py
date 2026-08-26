"""Place the supplied orthographic reference beside matching Riff V2 renders."""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).parent
OUTPUT = ROOT / "comparison"
REFERENCE = Image.open(ROOT.parent / "v1" / "comparison" / "reference-orthographic.png").convert("RGB")
VIEWS = {
    "front": ((43, 98, 441, 783), "riff-v2-front-orthographic.png"),
    "side": ((590, 80, 838, 781), "riff-v2-side-orthographic.png"),
    "back": ((937, 97, 1318, 783), "riff-v2-back-orthographic.png"),
}

try:
    FONT = ImageFont.truetype("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 28)
except OSError:
    FONT = ImageFont.load_default()


def model_subject(image):
    """Crop the render to lit character pixels; the orthographic background is near-black."""
    rgb = image.convert("RGB")
    mask = Image.new("1", rgb.size)
    mask.putdata([max(pixel) > 22 for pixel in rgb.getdata()])
    bbox = mask.getbbox()
    if bbox is None:
        raise RuntimeError("orthographic render contains no visible subject")
    return rgb.crop(bbox)


def fit_subject(image, size, subject_height=760, ground_y=810):
    scale = subject_height / image.height
    resized = image.resize((round(image.width * scale), subject_height), Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", size, (5, 13, 27))
    canvas.paste(resized, ((size[0] - resized.width) // 2, ground_y - resized.height))
    return canvas


for view, (crop, render_name) in VIEWS.items():
    reference = fit_subject(REFERENCE.crop(crop), (700, 850))
    render = fit_subject(model_subject(Image.open(OUTPUT / render_name)), (700, 850))
    board = Image.new("RGB", (1400, 910), (5, 13, 27))
    board.paste(reference, (0, 60))
    board.paste(render, (700, 60))
    draw = ImageDraw.Draw(board)
    draw.text((24, 16), f"REFERENCE — {view.upper()}", font=FONT, fill=(116, 190, 255))
    draw.text((724, 16), f"RIFF V2 — {view.upper()}", font=FONT, fill=(255, 185, 82))
    board.save(OUTPUT / f"compare-v2-{view}.png")

print("RIFF_V2_COMPARISON_BOARDS_WRITTEN")
