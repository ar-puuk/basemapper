"""Draw the basemapper logo, mark and favicons.

The logo is a regular hexagon holding three map plates in isometric view
(raster tiles, imagery, a vector map: layers stacked into one basemap, the way
basemapper stacks styles) above the name. The plates are drawn at 30 degrees,
so their edges run parallel to the hexagon's top edges. The name is set as
outlines from Manrope (fonts/, SIL OFL 1.1), so it looks the same everywhere,
and its position is measured: the stack is made as large as it can be while
the name keeps CLEARANCE units from the inside of the border.

Writes:
  r-basemapper/man/figures/logo.svg, py-basemapper/logo.svg   hex logo
  py-basemapper/mark.svg                                      plates only
  py-basemapper/favicon.svg, r-basemapper/pkgdown/favicon/*   favicons

The PNG and ICO favicons are rasterised with headless Chrome or Edge (and
Pillow for the ICO); without them only the SVGs are written.

Usage: python tools/logo/build_logo.py      (needs fontTools)
"""

import math
import pathlib
import shutil
import subprocess
import tempfile

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent
FONT = HERE / "fonts" / "Manrope[wght].ttf"

INK, ACCENT, CREAM, SKY = "#0E171E", "#007EA7", "#FFFBF2", "#5EC2E6"
WORD = [("base", 500, SKY), ("mapper", 800, CREAM)]  # (text, Manrope weight, colour)
CLEARANCE = 12      # name to the inside of the border, at its closest
TOP_GAP = 16        # top plate's upper edges to the border (they are parallel)
THICKNESS, SPACING = 7, 19

# The hexagon: pointy-top, regular, 220 wide (232 x 268 with the 6px border).
SIDE = 220 / math.sqrt(3)
CX, CY = 116, 134
HEX = [(CX, CY - SIDE), (CX + 110, CY - SIDE / 2), (CX + 110, CY + SIDE / 2),
       (CX, CY + SIDE), (CX - 110, CY + SIDE / 2), (CX - 110, CY - SIDE / 2)]
BORDER = 6
T30 = math.tan(math.radians(30))

# What each plate shows, drawn in a 100 x 100 map square. Bottom to top.
RASTER = ('<g stroke="#0E171E" stroke-width="1.6" opacity="0.55" fill="none">'
          '<path d="M25 0V100M50 0V100M75 0V100M0 25H100M0 50H100M0 75H100" vector-effect="non-scaling-stroke"/></g>')
IMAGERY = ('<path d="M0 0H50V50H0Z" fill="#1592BA"/><path d="M50 50H100V100H50Z" fill="#1592BA"/>'
           '<path d="M50 0H75V25H50ZM25 75H50V100H25Z" fill="#3AA9CC"/>')
VECTOR = ('<path d="M0 62C14 58 22 70 34 72C46 74 52 62 62 64C72 66 74 82 80 100H0Z" fill="#9CCFE3"/>'
          '<path d="M66 6C78 2 92 8 94 20C96 32 84 38 74 34C64 30 56 12 66 6Z" fill="#BFDDA6"/>'
          '<g fill="none" stroke-linecap="round">'
          '<path d="M-5 30C20 34 40 24 58 40S88 58 105 52" stroke="#E8833A" stroke-width="3.2" vector-effect="non-scaling-stroke"/>'
          '<path d="M30 -5C34 20 28 40 42 58S48 90 50 105" stroke="#0E171E" stroke-width="1.6" opacity="0.55" vector-effect="non-scaling-stroke"/>'
          '<path d="M-5 12C10 16 22 10 30 14" stroke="#0E171E" stroke-width="1.2" opacity="0.4" vector-effect="non-scaling-stroke"/></g>')
PLATES = [("#0A5068", "#06303D", RASTER), ("#007EA7", "#05566F", IMAGERY), (CREAM, "#C9C1AE", VECTOR)]


def num(v):
    return f"{v:.2f}".rstrip("0").rstrip(".")


def clearance(x, y):
    """Distance from (x, y) to the inside edge of the border; negative outside."""
    best = math.inf
    for (x1, y1), (x2, y2) in zip(HEX, HEX[1:] + HEX[:1]):
        nx, ny = y2 - y1, x1 - x2
        best = min(best, -((x - x1) * nx + (y - y1) * ny) / math.hypot(nx, ny))
    return best - BORDER / 2


def stack(a, top, cx=CX):
    """The plates, half-width a, the top plate's top corner at y=top. Returns (svg, bottom y)."""
    h = a * T30
    out = []
    for i, (face, side, content) in enumerate(PLATES):
        y = top + (len(PLATES) - 1 - i) * SPACING
        lx, rx, ly, by = cx - a, cx + a, y + h, y + 2 * h
        t = THICKNESS
        out.append(f'  <path d="M{num(lx)} {num(ly)} {num(cx)} {num(by)} {num(rx)} {num(ly)} {num(rx)} {num(ly + t)} '
                   f'{num(cx)} {num(by + t)} {num(lx)} {num(ly + t)}Z" fill="{side}"/>')
        out.append(f'  <path d="M{num(cx)} {num(y)} {num(rx)} {num(ly)} {num(cx)} {num(by)} {num(lx)} {num(ly)}Z" fill="{face}"/>')
        out.append(f'  <clipPath id="p{i}"><path d="M0 0H100V100H0Z"/></clipPath>')
        out.append(f'  <g transform="matrix({num(a / 100)} {num(h / 100)} {num(-a / 100)} {num(h / 100)} {num(cx)} {num(y)})" '
                   f'clip-path="url(#p{i})">{content}</g>')
    return "\n".join(out), top + (len(PLATES) - 1) * SPACING + 2 * h + THICKNESS


_fonts = {}


def word(size, track=-0.01):
    """The name as outlines, baseline at y=0. Returns (svg, (x0, y0, x1, y1))."""
    paths, xs, ys, x = [], [], [], 0.0
    for text, weight, fill in WORD:
        if weight not in _fonts:
            _fonts[weight] = instancer.instantiateVariableFont(TTFont(FONT), {"wght": weight})
        font = _fonts[weight]
        glyphs, cmap = font.getGlyphSet(), font.getBestCmap()
        scale = size / font["head"].unitsPerEm
        d = []
        for ch in text:
            g = glyphs[cmap[ord(ch)]]
            m = (scale, 0, 0, -scale, x, 0)
            pen = SVGPathPen(glyphs, num)
            g.draw(TransformPen(pen, m))
            d.append(pen.getCommands())
            bounds = BoundsPen(glyphs)
            g.draw(TransformPen(bounds, m))
            if bounds.bounds:
                xs += [bounds.bounds[0], bounds.bounds[2]]
                ys += [bounds.bounds[1], bounds.bounds[3]]
            x += g.width * scale + track * size
        paths.append(f'<path fill="{fill}" d="{" ".join(d)}"/>')
    return "".join(paths), (min(xs), min(ys), max(xs), max(ys))


def layout(a, size, gap=9):
    top = CY - SIDE + BORDER / 2 + TOP_GAP / math.cos(math.radians(30))
    plates, bottom = stack(a, top)
    svg, (x0, y0, x1, y1) = word(size)
    base = bottom + gap - y0
    tx = CX - (x1 - x0) / 2 - x0
    corners = [(tx + x, base + y) for x in (x0, x1) for y in (y0, y1)]
    plate_corner = (CX - a, top + a * T30 + (len(PLATES) - 1) * SPACING + THICKNESS)
    fits = min(clearance(*c) for c in corners) >= CLEARANCE and clearance(*plate_corner) >= CLEARANCE
    return fits, dict(plates=plates, word=svg, tx=tx, base=base, top=top, bottom=bottom)


def logo_svg():
    for size in (23, 22.5, 22):
        for a in (x / 2 for x in range(200, 140, -1)):  # largest plates first
            fits, lay = layout(a, size)
            if fits:
                break
        if fits:
            break
    points = " ".join(f"{num(x)},{num(y)}" for x, y in HEX)
    glow_y = (lay["top"] + lay["bottom"]) / 2
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 232 268" width="232" height="268" role="img" aria-label="basemapper">
  <!-- basemapper: a basemap is layers rendered into one image. Three map
       plates in isometric view (raster tiles, imagery, a vector map), the way
       basemapper stacks styles. The plates' edges run at 30 degrees, parallel
       to the hexagon's. Drawn by tools/logo/build_logo.py; the name is
       Manrope (SIL OFL 1.1). -->
  <defs><clipPath id="hex"><polygon points="{points}"/></clipPath></defs>
  <polygon points="{points}" fill="{INK}"/>
  <circle cx="{CX}" cy="{num(glow_y)}" r="104" fill="{ACCENT}" opacity="0.10" clip-path="url(#hex)"/>
{lay["plates"]}
  <g transform="translate({num(lay["tx"])} {num(lay["base"])})">{lay["word"]}</g>
  <polygon points="{points}" fill="none" stroke="{ACCENT}" stroke-width="{BORDER}" stroke-linejoin="round"/>
</svg>
'''


def mark_svg(favicon=False, a=80):
    plates, bottom = stack(a, 0)
    pad = 14 if favicon else 4
    side = max(2 * a + 2 * pad, bottom + 2 * pad)
    vx, vy = CX - side / 2, bottom / 2 - side / 2
    bg = (f'<rect x="{num(vx)}" y="{num(vy)}" width="{num(side)}" height="{num(side)}" '
          f'rx="{num(side * 0.2)}" fill="{INK}"/>\n') if favicon else ""
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="{num(vx)} {num(vy)} {num(side)} {num(side)}" width="160" height="160" role="img" aria-label="basemapper">
  <!-- The basemapper mark: the logo's map plates, for small spaces. Drawn by tools/logo/build_logo.py. -->
{bg}{plates}
</svg>
'''


def find_browser():
    for exe in ("chrome", "msedge", "google-chrome", "chromium",
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"):
        path = shutil.which(exe) or (exe if pathlib.Path(exe).exists() else None)
        if path:
            return path
    return None


def rasterise(svg_path, sizes):
    """{size: PNG path} for the SVG, rendered by headless Chrome/Edge; {} without one."""
    browser = find_browser()
    if not browser:
        return {}
    tmp = pathlib.Path(tempfile.mkdtemp())
    out = {}
    for n in sizes:
        page = tmp / f"f{n}.html"
        page.write_text(f'<html><body style="margin:0;background:transparent">'
                        f'<img src="{svg_path.as_uri()}" width="{n}" height="{n}" style="display:block"></body></html>')
        png = tmp / f"f{n}.png"
        subprocess.run([browser, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--allow-file-access-from-files",
                        "--default-background-color=00000000", f"--window-size={n},{n}",
                        f"--screenshot={png}", page.as_uri()], check=True, capture_output=True)
        out[n] = png
    return out


def main():
    logo, mark, favicon = logo_svg(), mark_svg(), mark_svg(favicon=True)
    write = lambda path, text: (ROOT / path).write_text(text, encoding="utf-8", newline="\n")
    write("r-basemapper/man/figures/logo.svg", logo)
    write("py-basemapper/logo.svg", logo)
    write("py-basemapper/mark.svg", mark)
    write("py-basemapper/favicon.svg", favicon)
    fav_dir = ROOT / "r-basemapper" / "pkgdown" / "favicon"
    write("r-basemapper/pkgdown/favicon/favicon.svg", favicon)
    pngs = rasterise(fav_dir / "favicon.svg", (96, 180, 192, 512))
    if not pngs:
        print("No Chrome or Edge found: wrote the SVGs only, not the PNG/ICO favicons.")
        return
    for n, name in ((96, "favicon-96x96.png"), (180, "apple-touch-icon.png"),
                    (192, "web-app-manifest-192x192.png"), (512, "web-app-manifest-512x512.png")):
        shutil.copyfile(pngs[n], fav_dir / name)
    try:
        from PIL import Image
        Image.open(pngs[512]).convert("RGBA").save(fav_dir / "favicon.ico", sizes=[(16, 16), (32, 32), (48, 48)])
    except ImportError:
        print("Pillow not installed: favicon.ico not updated.")
    print("Wrote the logo, mark and favicons.")


if __name__ == "__main__":
    main()
