from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
source = ROOT / "logo.png"
target = ROOT / "logo.ico"

if not source.is_file():
    raise SystemExit("logo.png was not found in the repository root")

with Image.open(source) as image:
    image = image.convert("RGBA")
    image.save(
        target,
        format="ICO",
        sizes=[(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)],
    )

print(f"Generated {target}")
