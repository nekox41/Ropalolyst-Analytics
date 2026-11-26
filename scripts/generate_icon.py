from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
source = ROOT / "logo.png"
target = ROOT / "logo.ico"
tauri_icons = ROOT / "src-tauri" / "icons"
frontend = ROOT / "ui" / "logo.png"

if not source.is_file():
    raise SystemExit("logo.png was not found in the repository root")

with Image.open(source) as image:
    image = image.convert("RGBA")
    sizes = [(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)]
    image.save(target, format="ICO", sizes=sizes)
    tauri_icons.mkdir(parents=True, exist_ok=True)
    image.save(tauri_icons / "icon.ico", format="ICO", sizes=sizes)
    image.resize((128, 128), Image.Resampling.LANCZOS).save(tauri_icons / "128x128.png")
    image.thumbnail((320, 320), Image.Resampling.LANCZOS)
    image.save(frontend, format="PNG", optimize=True)

print(f"Generated Windows and Tauri icons from {source}")
