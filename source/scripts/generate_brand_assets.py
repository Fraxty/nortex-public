"""Vektorisiert die vier NORTEX-JPG-Vorlagen und erzeugt Branding- und Icon-Assets.

Voraussetzung: pip install pillow numpy potracer
Aufruf (Repository-Wurzel): python scripts/generate_brand_assets.py

SVG-Masterdateien, PNG und ICO entstehen aus denselben getracten Pfaden;
es gibt pro Icon genau eine Geometrie.
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import potrace
from PIL import Image, ImageChops, ImageDraw

ICO_SIZES = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
SUPERSAMPLE = 8
WHITE = (255, 255, 255)

Point = tuple[float, float]
Contour = list[Point]


@dataclass
class Layer:
    """Eine Füllfarbe mit allen Konturen (even-odd, Löcher inklusive)."""

    color: tuple[int, int, int]
    commands: list[list[tuple]]  # pro Kontur: ("M"|"L"|"C", Punkte...)


@dataclass
class Artwork:
    width: int
    height: int
    layers: list[Layer]


def hex_color(color: tuple[int, int, int]) -> str:
    return "#{:02X}{:02X}{:02X}".format(*color)


def trace_mask(mask: np.ndarray, smooth: bool, turdsize: int = 8) -> list[list[tuple]]:
    bitmap = potrace.Bitmap(~mask)
    paths = bitmap.trace(
        turdsize=turdsize,
        turnpolicy=potrace.POTRACE_TURNPOLICY_MINORITY,
        alphamax=1.0 if smooth else 0.0,
        opticurve=smooth,
        opttolerance=0.2,
    )
    contours = []
    for curve in paths:
        start = curve.start_point
        commands: list[tuple] = [("M", (start.x, start.y))]
        for segment in curve.segments:
            if segment.is_corner:
                commands.append(("L", (segment.c.x, segment.c.y)))
                commands.append(("L", (segment.end_point.x, segment.end_point.y)))
            else:
                commands.append(
                    (
                        "C",
                        (segment.c1.x, segment.c1.y),
                        (segment.c2.x, segment.c2.y),
                        (segment.end_point.x, segment.end_point.y),
                    )
                )
        contours.append(commands)
    return contours


def median_color(rgb: np.ndarray, mask: np.ndarray) -> tuple[int, int, int]:
    pixels = rgb[mask]
    return tuple(int(v) for v in np.median(pixels, axis=0))


def enclosed_region(dark: np.ndarray) -> np.ndarray:
    """Alles, was nicht vom Bildrand aus erreichbar ist (füllt Löcher der Kachel)."""
    height, width = dark.shape
    outside = np.zeros_like(dark)
    stack = [(y, x) for y in (0, height - 1) for x in range(width)]
    stack += [(y, x) for x in (0, width - 1) for y in range(height)]
    while stack:
        y, x = stack.pop()
        if 0 <= y < height and 0 <= x < width and not dark[y, x] and not outside[y, x]:
            outside[y, x] = True
            stack += [(y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)]
    return ~outside


def load(path: Path) -> tuple[np.ndarray, np.ndarray]:
    rgb = np.asarray(Image.open(path).convert("RGB")).astype(np.int32)
    luma = (rgb[..., 0] * 299 + rgb[..., 1] * 587 + rgb[..., 2] * 114) // 1000
    return rgb, luma


def build_symbol(source: Path) -> Artwork:
    """Weißes N auf Schwarz -> weißes N ohne Hintergrund (Polygon)."""
    rgb, luma = load(source)
    shape = luma > 128
    return Artwork(rgb.shape[1], rgb.shape[0], [Layer(WHITE, trace_mask(shape, smooth=False))])


def build_application(source: Path) -> Artwork:
    """Dunkle, abgerundete Kachel mit weißem N."""
    rgb, luma = load(source)
    dark = luma < 128
    tile = enclosed_region(dark)
    glyph = tile & ~dark
    tile_color = median_color(rgb, dark)
    return Artwork(
        rgb.shape[1],
        rgb.shape[0],
        [
            Layer(tile_color, trace_mask(tile, smooth=True, turdsize=20)),
            Layer(WHITE, trace_mask(glyph, smooth=False, turdsize=20)),
        ],
    )


def build_general_file(source: Path) -> Artwork:
    """Schwarzes N-Zeichen mit blauem Quadrat auf Weiß."""
    rgb, luma = load(source)
    blue = (rgb[..., 2] > 180) & (rgb[..., 0] < 120)
    black = (luma < 128) & ~blue
    return Artwork(
        rgb.shape[1],
        rgb.shape[0],
        [
            Layer((0, 0, 0), trace_mask(black, smooth=True)),
            Layer(median_color(rgb, blue), trace_mask(blue, smooth=True)),
        ],
    )


def build_code_file(source: Path) -> Artwork:
    """Schwarzes Dokument; N und Zeilen sind Aussparungen."""
    rgb, luma = load(source)
    black = luma < 128
    return Artwork(rgb.shape[1], rgb.shape[0], [Layer((0, 0, 0), trace_mask(black, smooth=True))])


def svg_path(commands: list[tuple]) -> str:
    parts = []
    for command in commands:
        kind, *points = command
        coords = " ".join(f"{x:.2f} {y:.2f}" for x, y in points)
        parts.append(f"{kind}{coords}")
    return "".join(parts) + "Z"


def write_svg(art: Artwork, target: Path, title: str, recolor: dict | None = None) -> None:
    body = []
    for layer in art.layers:
        color = (recolor or {}).get(layer.color, layer.color)
        data = "".join(svg_path(contour) for contour in layer.commands)
        body.append(f'  <path fill="{hex_color(color)}" fill-rule="evenodd" d="{data}"/>')
    target.parent.mkdir(parents=True, exist_ok=True)
    svg = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {art.width} {art.height}" '
        f'width="{art.width}" height="{art.height}">\n'
        f"  <title>{title}</title>\n" + "\n".join(body) + "\n</svg>\n"
    )
    target.write_text(svg, encoding="utf-8", newline="\n")


def flatten(commands: list[tuple], steps: int = 16) -> Contour:
    points: Contour = []
    current: Point = (0.0, 0.0)
    for command in commands:
        kind, *args = command
        if kind in ("M", "L"):
            current = args[0]
            points.append(current)
        else:
            p0, (c1, c2, end) = current, args
            for index in range(1, steps + 1):
                t = index / steps
                u = 1 - t
                x = u**3 * p0[0] + 3 * u * u * t * c1[0] + 3 * u * t * t * c2[0] + t**3 * end[0]
                y = u**3 * p0[1] + 3 * u * u * t * c1[1] + 3 * u * t * t * c2[1] + t**3 * end[1]
                points.append((x, y))
            current = end
    return points


def render(art: Artwork, size: int, recolor: dict | None = None, padding: float = 0.0) -> Image.Image:
    """Rendert quadratisch mit transparentem Hintergrund (Supersampling, even-odd)."""
    scale_source = max(art.width, art.height)
    canvas_size = size * SUPERSAMPLE
    inner = canvas_size * (1 - 2 * padding)
    scale = inner / scale_source
    offset_x = (canvas_size - art.width * scale) / 2
    offset_y = (canvas_size - art.height * scale) / 2
    result = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    for layer in art.layers:
        mask = Image.new("1", (canvas_size, canvas_size), 0)
        for contour in layer.commands:
            polygon = Image.new("1", (canvas_size, canvas_size), 0)
            points = [(x * scale + offset_x, y * scale + offset_y) for x, y in flatten(contour)]
            ImageDraw.Draw(polygon).polygon(points, fill=1)
            mask = ImageChops.logical_xor(mask, polygon)
        color = (recolor or {}).get(layer.color, layer.color)
        fill = Image.new("RGBA", mask.size, (*color, 255))
        result.paste(fill, (0, 0), mask.convert("L"))
    return result.resize((size, size), Image.LANCZOS)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", type=Path, default=Path("assets/source"), help="Ordner mit den vier JPGs")
    parser.add_argument("--assets", type=Path, default=Path("assets"))
    args = parser.parse_args()

    sources = {
        "logo": args.sources / "Nortex-Logo.jpg",
        "application": args.sources / "Nortex Anwendung.jpg",
        "general": args.sources / "Nortex-Datein.jpg",
        "code": args.sources / "Nortex-Code-Dateien.jpg",
    }
    missing = [str(path) for path in sources.values() if not path.is_file()]
    if missing:
        print("Fehlende Quelldateien: " + ", ".join(missing), file=sys.stderr)
        return 1

    assets = args.assets
    symbol = build_symbol(sources["logo"])
    application = build_application(sources["application"])
    general = build_general_file(sources["general"])
    code = build_code_file(sources["code"])

    write_svg(symbol, assets / "branding" / "nortex-symbol.svg", "NORTEX Symbol")
    write_svg(application, assets / "icons" / "app" / "nortex-code.svg", "NORTEX Code")
    write_svg(general, assets / "icons" / "files" / "nortex-file.svg", "NORTEX Datei")
    write_svg(code, assets / "icons" / "files" / "nortex-code-file.svg", "NORTEX Code Datei")

    app_dir = assets / "icons" / "app"
    render(application, 256).save(app_dir / "nortex-code-256.png", optimize=True)
    render(application, 256).save(app_dir / "nortex-code.ico", sizes=ICO_SIZES)

    # UI-Laufzeitvarianten: gleiche Geometrie, hell eingefärbt, damit sie auf dunklem Grund
    # lesbar sind und per egui-Tint pro Dateityp eingefärbt werden können.
    runtime = assets / "icons" / "runtime"
    runtime.mkdir(parents=True, exist_ok=True)
    light = {(0, 0, 0): WHITE}
    render(symbol, 128, padding=0.0).save(runtime / "nortex-symbol-128.png", optimize=True)
    render(general, 64, recolor=light, padding=0.08).save(runtime / "nortex-file-64.png", optimize=True)
    render(code, 64, recolor=light, padding=0.08).save(runtime / "nortex-code-file-64.png", optimize=True)
    print("Assets erzeugt in", assets)
    return 0


if __name__ == "__main__":
    sys.exit(main())
