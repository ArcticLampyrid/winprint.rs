#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import math
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.pagesizes import A4
from reportlab.pdfgen import canvas
from reportlab.lib.units import mm

# ---------------------------------------------------------------------------
# Layout
# ---------------------------------------------------------------------------

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent.parent
VR_DIR = REPO_ROOT / "test_data" / "visual_regression"

PAGE_SIZE = A4
PAGE_W, PAGE_H = PAGE_SIZE

# Baseline rendering DPI.
BASELINE_DPI = 300
TIFF_DOWN_SCALE_FACTOR = 4

# Pin reportlab / Ghostscript output timestamps for byte-stable output.
os.environ.setdefault("SOURCE_DATE_EPOCH", "1700000000")


# ---------------------------------------------------------------------------
# Tool discovery
# ---------------------------------------------------------------------------


REQUIRED_TOOLS = ("gs", "tiffcp")


def _require_tools() -> None:
    missing = [t for t in REQUIRED_TOOLS if shutil.which(t) is None]
    if missing:
        raise SystemExit("Missing required tools: {}\n".format(", ".join(missing)))


def _run(cmd: list[str]) -> None:
    """Run a subprocess, surfacing a readable error on failure."""
    subprocess.run(cmd, capture_output=True, text=True, check=True)


# ---------------------------------------------------------------------------
# data.pdf — reportlab drawing
# ---------------------------------------------------------------------------


def _draw_page_header(c: canvas.Canvas, page_no: int, title: str) -> None:
    c.setFillColor(colors.HexColor("#1f3a5f"))
    c.rect(0, PAGE_H - 22 * mm, PAGE_W, 22 * mm, fill=1, stroke=0)
    c.setFillColor(colors.white)
    c.setFont("Helvetica-Bold", 18)
    c.drawString(15 * mm, PAGE_H - 14 * mm, title)
    c.setFont("Helvetica", 10)
    c.drawRightString(PAGE_W - 15 * mm, PAGE_H - 14 * mm, f"Page {page_no}")


def _draw_footer(c: canvas.Canvas, page_no: int) -> None:
    c.setFillColor(colors.HexColor("#777777"))
    c.setFont("Helvetica-Oblique", 8)
    c.drawCentredString(
        PAGE_W / 2,
        10 * mm,
        f"winprint.rs visual regression fixture — page {page_no}",
    )


def _page_color_swatches(c: canvas.Canvas) -> None:
    """Page 1: a grid of saturated primary / secondary colour swatches."""
    _draw_page_header(c, 1, "Color Swatches")

    swatches = [
        ("Red", "#E53935"),
        ("Green", "#43A047"),
        ("Blue", "#1E88E5"),
        ("Cyan", "#00ACC1"),
        ("Magenta", "#D81B60"),
        ("Yellow", "#FDD835"),
        ("Orange", "#FB8C00"),
        ("Purple", "#8E24AA"),
        ("Teal", "#00897B"),
        ("Pink", "#EC407A"),
        ("Lime", "#C0CA33"),
        ("Indigo", "#3949AB"),
    ]

    margin = 20 * mm
    grid_w = PAGE_W - 2 * margin
    grid_h = PAGE_H - 60 * mm
    cols, rows = 3, 4
    cell_w = grid_w / cols
    cell_h = grid_h / rows

    for i, (name, hexv) in enumerate(swatches):
        col = i % cols
        row = i // cols
        x = margin + col * cell_w
        y = PAGE_H - 40 * mm - (row + 1) * cell_h
        c.setFillColor(colors.HexColor(hexv))
        c.rect(x + 3, y + 3, cell_w - 6, cell_h - 14, fill=1, stroke=0)
        c.setFillColor(colors.black)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(x + 6, y, f"{name}  {hexv}")

    _draw_footer(c, 1)


def _page_text_styles(c: canvas.Canvas) -> None:
    """Page 2: varied fonts, sizes, weights, colours, alignments."""
    _draw_page_header(c, 2, "Text Styles")

    y = PAGE_H - 40 * mm

    c.setFillColor(colors.black)
    c.setFont("Helvetica", 8)
    c.drawString(
        20 * mm, y, "Helvetica 8pt — The quick brown fox jumps over the lazy dog."
    )
    y -= 8 * mm
    c.setFont("Helvetica", 12)
    c.drawString(
        20 * mm, y, "Helvetica 12pt — The quick brown fox jumps over the lazy dog."
    )
    y -= 10 * mm
    c.setFont("Helvetica-Bold", 16)
    c.drawString(20 * mm, y, "Helvetica-Bold 16pt — SHARP EDGES")
    y -= 12 * mm
    c.setFont("Helvetica-Oblique", 14)
    c.drawString(20 * mm, y, "Helvetica-Oblique 14pt — slanted")
    y -= 12 * mm
    c.setFont("Courier", 12)
    c.drawString(20 * mm, y, "Courier 12pt — monospace 0123456789")
    y -= 12 * mm
    c.setFont("Times-Roman", 14)
    c.drawString(20 * mm, y, "Times-Roman 14pt — serif")
    y -= 14 * mm

    # Coloured text.
    c.setFillColor(colors.HexColor("#C62828"))
    c.setFont("Helvetica-Bold", 14)
    c.drawString(20 * mm, y, "Red bold text")
    c.setFillColor(colors.HexColor("#2E7D32"))
    c.drawString(70 * mm, y, "Green bold text")
    c.setFillColor(colors.HexColor("#1565C0"))
    c.drawString(120 * mm, y, "Blue bold text")
    y -= 14 * mm

    # Alignment samples.
    c.setFillColor(colors.black)
    c.setFont("Helvetica", 10)
    c.drawString(20 * mm, y, "Left aligned")
    c.drawCentredString(PAGE_W / 2, y, "Centre aligned")
    c.drawRightString(PAGE_W - 20 * mm, y, "Right aligned")
    y -= 14 * mm

    # Extended-Latin & symbols — reportlab's built-in fonts don't cover CJK,
    # so we stay Latin-only to avoid needing external font files.
    c.setFont("Helvetica", 10)
    for line in (
        "Latin-1 supplement: À Á Â Ã Ä Å Æ Ç È É Ê Ë Ì Í Î Ï",
        "Symbols: € £ ¥ © ® ™ ± × ÷ ≈ ≠ ≤ ≥",
        "Numerals: 0 1 2 3 4 5 6 7 8 9",
    ):
        c.drawString(20 * mm, y, line)
        y -= 8 * mm

    # Filled paragraph block (coloured rectangle with clipped text).
    c.setFillColor(colors.HexColor("#FFF3E0"))
    c.rect(20 * mm, 30 * mm, PAGE_W - 40 * mm, 35 * mm, fill=1, stroke=1)
    c.setFillColor(colors.HexColor("#333333"))
    c.setFont("Helvetica", 9)
    txt = c.beginText(22 * mm, 55 * mm)
    for line in (
        "This paragraph is drawn inside a coloured rectangle to exercise",
        "anti-aliased edges, fill patterns, and clipped text rendering on the",
        "PWG raster path. Any visual regression in glyph kerning, colour fill,",
        "or stroke width should show up as an SSIM drop on this page.",
    ):
        txt.textLine(line)
    c.drawText(txt)

    _draw_footer(c, 2)


def _page_shapes(c: canvas.Canvas) -> None:
    """Page 3: filled circles, stroked rectangles, diagonal lines, polygons."""
    _draw_page_header(c, 3, "Shapes & Strokes")

    # Filled circles.
    cx0 = 50 * mm
    cy = PAGE_H - 70 * mm
    for i, (r, hv) in enumerate(
        (
            (14 * mm, "#FFB300"),
            (11 * mm, "#039BE5"),
            (8 * mm, "#6A1B9A"),
            (5 * mm, "#00897B"),
        )
    ):
        c.setFillColor(colors.HexColor(hv))
        c.circle(cx0 + i * 35 * mm, cy, r, fill=1, stroke=0)

    # Stroked rectangles with varying line widths.
    y = PAGE_H - 110 * mm
    for i, w in enumerate((0.5, 1.0, 2.0, 3.5, 6.0)):
        c.setStrokeColor(colors.HexColor("#37474F"))
        c.setLineWidth(w)
        c.rect(20 * mm + i * 32 * mm, y, 25 * mm, 15 * mm, fill=0, stroke=1)
        c.setFillColor(colors.HexColor("#37474F"))
        c.setFont("Helvetica", 8)
        c.drawString(20 * mm + i * 32 * mm, y - 5 * mm, f"{w} pt")

    # Diagonal lines.
    c.setStrokeColor(colors.HexColor("#E53935"))
    c.setLineWidth(1.2)
    for i in range(12):
        x1 = 20 * mm + i * 14 * mm
        c.line(x1, 100 * mm, x1 + 10 * mm, 140 * mm)

    # Triangle via path.
    p = c.beginPath()
    p.moveTo(60 * mm, 70 * mm)
    p.lineTo(110 * mm, 70 * mm)
    p.lineTo(85 * mm, 110 * mm)
    p.close()
    c.setFillColor(colors.HexColor("#FFD54F"))
    c.setStrokeColor(colors.HexColor("#5D4037"))
    c.setLineWidth(2)
    c.drawPath(p, fill=1, stroke=1)

    # Hexagon via path.
    p = c.beginPath()
    p.moveTo(120 * mm, 70 * mm)
    for n in range(1, 7):
        angle = 2 * math.pi * n / 6
        p.lineTo(
            150 * mm + 20 * mm * math.cos(angle),
            90 * mm + 20 * mm * math.sin(angle),
        )
    p.close()
    c.setFillColor(colors.HexColor("#81C784"))
    c.setStrokeColor(colors.HexColor("#1B5E20"))
    c.drawPath(p, fill=1, stroke=1)

    _draw_footer(c, 3)


def _page_gradient(c: canvas.Canvas) -> None:
    """Page 4: faux gradients, overlapping translucent circles, grey ramp."""
    _draw_page_header(c, 4, "Gradients & Overlaps")

    strips = 200
    x0 = 20 * mm
    w = PAGE_W - 40 * mm
    step = w / strips

    # Horizontal gradient strip: red → yellow → green.
    y0 = PAGE_H - 80 * mm
    h = 30 * mm
    for i in range(strips):
        t = i / (strips - 1)
        if t < 0.5:
            u = t / 0.5
            r = 0xE5 * (1 - u) + 0xFD * u
            g = 0x39 * (1 - u) + 0xD8 * u
            b = 0x35 * (1 - u) + 0x35 * u
        else:
            u = (t - 0.5) / 0.5
            r = 0xFD * (1 - u) + 0x43 * u
            g = 0xD8 * (1 - u) + 0xA0 * u
            b = 0x35 * (1 - u) + 0x47 * u
        c.setFillColorRGB(r / 255, g / 255, b / 255)
        c.rect(x0 + i * step, y0, step + 0.5, h, fill=1, stroke=0)

    # Vertical-ish gradient strip: blue → white.
    y0 = PAGE_H - 130 * mm
    for i in range(strips):
        t = i / (strips - 1)
        r = 0x1E * (1 - t) + 0xFF * t
        g = 0x88 * (1 - t) + 0xFF * t
        b = 0xE5 * (1 - t) + 0xFF * t
        c.setFillColorRGB(r / 255, g / 255, b / 255)
        c.rect(x0 + i * step, y0, step + 0.5, 20 * mm, fill=1, stroke=0)

    # Overlapping translucent circles (alpha blending).
    c.saveState()
    c.setFillAlpha(0.6)
    for i, hv in enumerate(("#E53935", "#1E88E5", "#43A047")):
        c.setFillColor(colors.HexColor(hv))
        c.circle(70 * mm + i * 25 * mm, 65 * mm, 25 * mm, fill=1, stroke=0)
    c.restoreState()

    # Black-to-white grayscale ramp.
    ramp_y = 30 * mm
    ramp_h = 12 * mm
    steps = 16
    sw = w / steps
    for i in range(steps):
        v = i / (steps - 1)
        c.setFillColorRGB(v, v, v)
        c.rect(20 * mm + i * sw, ramp_y, sw + 0.5, ramp_h, fill=1, stroke=0)

    _draw_footer(c, 4)


def generate_pdf(out_path: Path) -> None:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    c = canvas.Canvas(
        str(out_path),
        pagesize=PAGE_SIZE,
        invariant=True,  # strip dynamic metadata for byte-stable output
    )
    c.setTitle("winprint.rs visual regression fixture")
    c.setAuthor("winprint.rs")
    c.setSubject("Visual regression test input")

    _page_color_swatches(c)
    c.showPage()
    _page_text_styles(c)
    c.showPage()
    _page_shapes(c)
    c.showPage()
    _page_gradient(c)
    c.showPage()

    c.save()


# ---------------------------------------------------------------------------
# data.tiff — Ghostscript tiff24nc
# ---------------------------------------------------------------------------


def convert_pdf_to_tiff(pdf_path: Path, out_path: Path) -> None:
    """Render the PDF into a multi-page TIFF via Ghostscript's tiff24nc device."""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile() as gs_out:
        _run(
            [
                "gs",
                "-dNOPAUSE",
                "-dBATCH",
                "-dSAFER",
                "-dQUIET",
                "-sDEVICE=tiffscaled24",
                "-dTIFFDateTime=false",
                f"-r{BASELINE_DPI*TIFF_DOWN_SCALE_FACTOR}",
                f"-dDownScaleFactor={TIFF_DOWN_SCALE_FACTOR}",
                f"-sOutputFile={gs_out.name}",
                str(pdf_path),
            ]
        )
        _run(["tiffcp", "-c", "zip", str(gs_out.name), str(out_path)])


# ---------------------------------------------------------------------------
# data.xps — Ghostscript xpswrite
# ---------------------------------------------------------------------------


def convert_pdf_to_xps(pdf_path: Path, out_path: Path) -> None:
    """Convert the PDF into an XPS package via Ghostscript.

    Ghostscript's xpswrite device honours SOURCE_DATE_EPOCH for the ZIP
    entries' mtime, which keeps the resulting .xps byte-stable. -dSAFER
    disables any surprising PostScript side-effects from the input.
    """
    out_path.parent.mkdir(parents=True, exist_ok=True)
    _run(
        [
            "gs",
            "-dNOPAUSE",
            "-dBATCH",
            "-dSAFER",
            "-dQUIET",
            "-sDEVICE=xpswrite",
            f"-sOutputFile={out_path}",
            str(pdf_path),
        ]
    )


# ---------------------------------------------------------------------------
# Utilities
# ---------------------------------------------------------------------------


def calculate_sha256(filename):
    """Calculate the SHA256 hash of a file."""
    h = hashlib.sha256()
    with open(filename, "rb") as file:
        # Read file in chunks to handle large files efficiently
        while chunk := file.read(8192):
            h.update(chunk)
    return h.hexdigest()


# ---------------------------------------------------------------------------
# Orchestration
# ---------------------------------------------------------------------------


def main() -> int:
    _require_tools()

    VR_DIR.mkdir(parents=True, exist_ok=True)
    files_before = {
        p.name: calculate_sha256(p) for p in VR_DIR.iterdir() if p.is_file()
    }

    pdf_path = VR_DIR / "data.pdf"
    tiff_path = VR_DIR / "data.tiff"
    xps_path = VR_DIR / "data.xps"

    print("[1/3] Generating PDF...")
    generate_pdf(pdf_path)

    print("[2/3] Generating TIFF...")
    convert_pdf_to_tiff(pdf_path, tiff_path)

    print("[3/3] Generating XPS...")
    convert_pdf_to_xps(pdf_path, xps_path)

    files_after = {p.name: calculate_sha256(p) for p in VR_DIR.iterdir() if p.is_file()}

    print()
    print("wrote under:", VR_DIR)
    keys = set(files_before.keys()) | set(files_after.keys())
    for name in sorted(keys):
        before = files_before.get(name)
        after = files_after.get(name)
        if before is None:
            print(f"  + {name} (new)")
            print(f"        SHA256: {after}")
        elif after is None:
            print(f"  - {name} (deleted)")
        elif before != after:
            print(f"  * {name} (changed)")
            print(f"        SHA256: {after}")
        else:
            print(f"    {name} (unchanged)")
            print(f"        SHA256: {after}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
