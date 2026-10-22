"""Builds hydra's icons from the logo in assets/logo.

Run from the repo root:  python assets/make_icon.py   (needs Pillow)
Writes assets/hydra.ico (16-256 px) and assets/hydra.png (512 px) from the app icon,
assets/hydra-64.png (hydraui's title bar) from the bare mark, which reads better than a tile at
18 px on the dark bar, and site/favicon.png (64 px) from the app icon.
"""
from PIL import Image

APP_ICON = "assets/logo/hydra-app-icon.png"
MARK = "assets/logo/hydra-mark-on-dark.png"


def sized(path: str, size: int) -> Image.Image:
    return Image.open(path).convert("RGBA").resize((size, size), Image.LANCZOS)


def main() -> None:
    sized(APP_ICON, 512).save("assets/hydra.png")
    sized(MARK, 64).save("assets/hydra-64.png")
    sized(APP_ICON, 64).save("site/favicon.png")
    sized(APP_ICON, 256).save(
        "assets/hydra.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)]
    )


if __name__ == "__main__":
    main()
