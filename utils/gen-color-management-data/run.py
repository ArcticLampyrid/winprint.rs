#!/usr/bin/env python3
"""Generate the color-management fixtures and ICC profiles with LittleCMS."""

import ctypes as C
import ctypes.util
import io
import itertools
import struct
import subprocess
import tempfile
import zipfile
import zlib
from pathlib import Path
from typing import NamedTuple

from PIL import Image, ImageCms, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "test_data" / "color_management"
PROFILES = DATA / "profiles"
BASIC = DATA / "basic"
CMYK_PROFILE = "cmyk-ideal-inks.icc"
# Ink coverage is (1 - sample)^INK_GAMMA in linear light, so that 8-bit samples
# quantize evenly, like sRGB. Black ink replaces this part of the gray component.
INK_GAMMA = 2.2
INK_GCR = 0.5
# CLUT grid points per ink: LittleCMS interpolates M, Y and K tetrahedrally,
# where the products of inks are not exact. 9 keeps conversions within 2/255.
INK_GRID = 9
# Exactly A4 at 50.8 DPI (JPEG's integer JFIF density rounds to 51).
# Large uniform tiles avoid resampling/JPEG edge artifacts.
SIZE = (420, 594)
GRID = (4, 6)
DPI = (50.8, 50.8)
# The same A4 page in PDF points and XPS device-independent pixels.
PAGE_POINTS = tuple(v * 72 / d for v, d in zip(SIZE, DPI))
PAGE_DIPS = tuple(v * 96 / d for v, d in zip(SIZE, DPI))
# Moderate colors, well inside sRGB and the common printable gamut.
COLORS = [
    (48, 48, 48),
    (96, 96, 96),
    (160, 160, 160),
    (224, 224, 224),
    (160, 80, 80),
    (80, 144, 96),
    (80, 96, 160),
    (176, 144, 80),
    (144, 80, 144),
    (80, 144, 144),
    (192, 128, 96),
    (112, 96, 144),
    (128, 64, 64),
    (64, 112, 80),
    (64, 80, 128),
    (144, 120, 64),
    (120, 64, 120),
    (64, 120, 120),
    (192, 160, 144),
    (144, 176, 152),
    (144, 152, 192),
    (192, 176, 136),
    (176, 144, 176),
    (136, 176, 176),
]
GRAYS = [(32 + i * 8,) * 3 for i in range(24)]


class XyY(C.Structure):
    _fields_ = [("x", C.c_double), ("y", C.c_double), ("Y", C.c_double)]


class Primaries(C.Structure):
    _fields_ = [("red", XyY), ("green", XyY), ("blue", XyY)]


class Space(NamedTuple):
    """A chart expressed in a color space, as an image and its ICC profile."""

    image: Image.Image
    # None for untagged samples, taken as sRGB.
    profile: bytes | None
    reference: Image.Image


class Document(NamedTuple):
    name: str
    data: bytes
    # The image fixture that the document must look like, or None if MuPDF
    # cannot tell: it ignores the ICC profiles of XPS ContextColor.
    image: str | None


class Fixture(NamedTuple):
    name: str
    space: Space
    # Store 16 bits per channel; only for RGB PNG.
    deep: bool = False


def fixed_profile(data):
    # ICC creation date and profile ID must not depend on the wall clock.
    data = bytearray(data)
    data[24:36] = struct.pack(">6H", 2023, 11, 14, 22, 13, 20)
    data[84:100] = bytes(16)
    return bytes(data)


def matrix_profiles():
    """Build our own Adobe RGB (1998)-compatible and gamma-1.8 gray profiles.

    No Adobe-owned profile binary is redistributed. These are ordinary v2
    matrix/TRC profiles using published primaries, D65 white and gamma 563/256.
    Gray uses D50 and gamma 1.8 (deliberately not sRGB's transfer function).
    """
    lcms = C.CDLL(ctypes.util.find_library("lcms2"))
    signatures = {
        "cmsBuildGamma": ([C.c_void_p, C.c_double], C.c_void_p),
        "cmsCreateRGBProfile": (
            [C.POINTER(XyY), C.POINTER(Primaries), C.POINTER(C.c_void_p)],
            C.c_void_p,
        ),
        "cmsCreateGrayProfile": ([C.POINTER(XyY), C.c_void_p], C.c_void_p),
        "cmsSetProfileVersion": ([C.c_void_p, C.c_double], None),
        "cmsSaveProfileToMem": (
            [C.c_void_p, C.c_void_p, C.POINTER(C.c_uint32)],
            C.c_int,
        ),
        "cmsCloseProfile": ([C.c_void_p], C.c_int),
        "cmsFreeToneCurve": ([C.c_void_p], None),
    }
    for name, (args, result) in signatures.items():
        function = getattr(lcms, name)
        function.argtypes, function.restype = args, result

    def serialize(profile):
        assert profile
        try:
            lcms.cmsSetProfileVersion(profile, 2.1)
            size = C.c_uint32()
            assert lcms.cmsSaveProfileToMem(profile, None, C.byref(size))
            data = C.create_string_buffer(size.value)
            assert lcms.cmsSaveProfileToMem(profile, data, C.byref(size))
            return fixed_profile(data.raw)
        finally:
            lcms.cmsCloseProfile(profile)

    adobe_gamma = lcms.cmsBuildGamma(None, 563 / 256)
    gray_gamma = lcms.cmsBuildGamma(None, 1.8)
    assert adobe_gamma and gray_gamma
    try:
        adobe = serialize(
            lcms.cmsCreateRGBProfile(
                C.byref(XyY(0.3127, 0.3290, 1)),
                C.byref(
                    Primaries(
                        XyY(0.64, 0.33, 1), XyY(0.21, 0.71, 1), XyY(0.15, 0.06, 1)
                    )
                ),
                (C.c_void_p * 3)(adobe_gamma, adobe_gamma, adobe_gamma),
            )
        )
        gray = serialize(
            lcms.cmsCreateGrayProfile(C.byref(XyY(0.3457, 0.3585, 1)), gray_gamma)
        )
        return adobe, gray
    finally:
        lcms.cmsFreeToneCurve(adobe_gamma)
        lcms.cmsFreeToneCurve(gray_gamma)


def s15_fixed16(*values):
    return b"".join(struct.pack(">i", round(v * 65536)) for v in values)


def icc_tags(profile):
    """The tag data of `profile`, by signature."""
    count = struct.unpack(">I", profile[128:132])[0]
    entries = (
        struct.unpack(">4sII", profile[132 + 12 * i : 144 + 12 * i])
        for i in range(count)
    )
    return {sig: profile[offset : offset + size] for sig, offset, size in entries}


def srgb_matrix(srgb):
    """The rows X, Y, Z of the matrix from linear sRGB to D50 XYZ in `srgb`."""
    columns = [
        struct.unpack(">3i", icc_tags(srgb)[sig][8:20])
        for sig in (b"rXYZ", b"gXYZ", b"bXYZ")
    ]
    return [[column[row] / 65536 for column in columns] for row in range(3)]


def ink_profile(srgb):
    """Build a v2 CMYK input profile of ideal inks, whose black is absolute black.

    Each ink absorbs its complement in linear sRGB, and black absorbs all three.
    The black point is therefore XYZ 0: black point compensation changes nothing,
    and every rendering intent gives the same colors. Real printing conditions
    have lighter blacks, which makes renderers that compensate for black differ.
    """
    matrix = srgb_matrix(srgb)
    nodes = [i / (INK_GRID - 1) for i in range(INK_GRID)]
    # A2B0 is lut16Type: ink curves, then a CLUT from inks to XYZ (0x8000 is 1.0).
    clut = [
        min(
            65535,
            round(32768 * sum(m * (1 - ink) * (1 - k) for m, ink in zip(row, inks))),
        )
        for *inks, k in itertools.product(nodes, repeat=4)
        for row in matrix
    ]
    entries = 4096
    curve = [
        round(65535 * (1 - (1 - i / (entries - 1)) ** INK_GAMMA))
        for i in range(entries)
    ]
    a2b0 = (
        b"mft2\0\0\0\0"
        + bytes([4, 3, INK_GRID, 0])
        + s15_fixed16(1, 0, 0, 0, 1, 0, 0, 0, 1)
        + struct.pack(">HH", entries, 2)
        + struct.pack(f">{4 * entries}H", *curve * 4)
        + struct.pack(f">{len(clut)}H", *clut)
        + struct.pack(">6H", *[0, 65535] * 3)
    )
    description = b"CMYK with ideal inks\0"
    tags = {
        b"desc": b"desc\0\0\0\0"
        + struct.pack(">I", len(description))
        + description
        + bytes(8 + 2 + 1 + 67),
        b"cprt": b"text\0\0\0\0No copyright, use freely\0",
        b"wtpt": b"XYZ \0\0\0\0" + s15_fixed16(*map(sum, matrix)),
        b"A2B0": a2b0,
    }
    tags = {sig: data + bytes(-len(data) % 4) for sig, data in tags.items()}
    offset = 128 + 4 + 12 * len(tags)
    table, data = b"", b""
    for sig, body in tags.items():
        table += sig + struct.pack(">II", offset + len(data), len(body))
        data += body
    header = (
        struct.pack(">I", offset + len(data))
        + bytes(4)
        + bytes([2, 0x10, 0, 0])
        + b"scnrCMYKXYZ "
        + bytes(12)
        + b"acsp"
        + bytes(24)
        # Relative colorimetric, although all intents are the same.
        + struct.pack(">I", 1)
        + s15_fixed16(0.9642, 1, 0.8249)
        + bytes(48)
    )
    return fixed_profile(header + struct.pack(">I", len(tags)) + table + data)


def ink_samples(color):
    """The CMYK samples of 8-bit sRGB `color` in the ideal ink profile."""
    rgb = [linear(v) for v in color]
    k = INK_GCR * (1 - max(rgb))
    inks = [1 - v / (1 - k) for v in rgb] + [k]
    return tuple(round(255 * (1 - (1 - v) ** (1 / INK_GAMMA))) for v in inks)


def load_profiles():
    """Write the generated profiles."""
    srgb = fixed_profile(
        ImageCms.ImageCmsProfile(ImageCms.createProfile("sRGB")).tobytes()
    )
    adobe, gray = matrix_profiles()
    cmyk = ink_profile(srgb)
    for name, profile in [
        ("srgb.icc", srgb),
        ("adobe-rgb-compatible.icc", adobe),
        ("gray-gamma18.icc", gray),
        (CMYK_PROFILE, cmyk),
    ]:
        (PROFILES / name).write_bytes(profile)
    return srgb, adobe, gray, cmyk


def chart(colors, mode="RGB"):
    image = Image.new(mode, SIZE)
    draw = ImageDraw.Draw(image)
    tile_width, tile_height = SIZE[0] // GRID[0], SIZE[1] // GRID[1]
    for i, color in enumerate(colors):
        x, y = i % GRID[0], i // GRID[0]
        draw.rectangle(
            (
                x * tile_width,
                y * tile_height,
                (x + 1) * tile_width - 1,
                (y + 1) * tile_height - 1,
            ),
            fill=color,
        )
    return image


def convert(
    image, source, destination, mode, intent=ImageCms.Intent.RELATIVE_COLORIMETRIC
):
    return ImageCms.profileToProfile(
        image,
        ImageCms.ImageCmsProfile(io.BytesIO(source)),
        ImageCms.ImageCmsProfile(io.BytesIO(destination)),
        outputMode=mode,
        renderingIntent=intent,
    )


def save(path, image, profile=None, deep=False):
    if deep:
        return save_deep_png(path, image, profile)
    # Do not accidentally inherit an ICC profile from profileToProfile().
    image = image.copy()
    image.info.clear()
    options = {"dpi": DPI}
    if profile is not None:
        options["icc_profile"] = profile
    if path.suffix == ".tiff":
        options["compression"] = "tiff_adobe_deflate"
    elif path.suffix == ".jpg":
        options.update(quality=100, subsampling=0)
    image.save(path, **options)


def save_deep_png(path, image, profile):
    """Save 8-bit RGB `image` as 16-bit PNG, which Pillow cannot write.

    Each sample is scaled exactly (v * 257), so the colors stay the same.
    """
    assert image.mode == "RGB" and path.suffix == ".png"

    def chunk(kind, data):
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data))
        )

    width, height = image.size
    raw = image.tobytes()
    samples = struct.pack(f">{len(raw)}H", *(v * 257 for v in raw))
    stride = 6 * width
    scanlines = b"".join(
        b"\0" + samples[y * stride : (y + 1) * stride] for y in range(height)
    )
    # 50.8 DPI is exactly 2000 pixels per meter.
    pixels_per_meter = round(DPI[0] / 0.0254)
    chunks = [
        chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 16, 2, 0, 0, 0)),
        chunk(b"pHYs", struct.pack(">IIB", pixels_per_meter, pixels_per_meter, 1)),
    ]
    if profile is not None:
        chunks.append(chunk(b"iCCP", b"ICC\0\0" + zlib.compress(profile, 9)))
    chunks += [chunk(b"IDAT", zlib.compress(scanlines, 9)), chunk(b"IEND", b"")]
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + b"".join(chunks))


def tiles(page):
    """The (x, y, width, height) of each patch on `page`, from the top left."""
    width, height = page[0] / GRID[0], page[1] / GRID[1]
    for i in range(GRID[0] * GRID[1]):
        yield i % GRID[0] * width, i // GRID[0] * height, width, height


def samples(image):
    """The samples at the center of each patch, as tuples even for gray."""
    return [p if isinstance(p, tuple) else (p,) for p in centers(image)]


def numbers(values):
    return " ".join(f"{v:.4f}" for v in values)


def pdf_stream(entries, data):
    return b"<< %s /Length %d >>\nstream\n%s\nendstream" % (entries, len(data), data)


def pdf_add(objects, body):
    """Add an object after the catalog, pages, page and content, and refer to it."""
    objects.append(body)
    return b"%d 0 R" % (len(objects) + 4)


def pdf_color_space(space, objects):
    """Add the ICC profile of `space` to `objects`, and return a reference to it."""
    if space.profile is None:
        return b"/DeviceRGB"
    channels = len(space.image.getbands())
    alternate = {1: b"/DeviceGray", 3: b"/DeviceRGB", 4: b"/DeviceCMYK"}[channels]
    profile = pdf_stream(
        b"/N %d /Alternate %s /Filter /FlateDecode" % (channels, alternate),
        zlib.compress(space.profile, 9),
    )
    return b"[/ICCBased %s]" % pdf_add(objects, profile)


def pdf(resources, content, objects):
    """Serialize a one-page PDF, after the catalog, pages, page and content objects."""
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 %s] /Resources << %s >> "
        b"/Contents 4 0 R >>" % (numbers(PAGE_POINTS).encode(), resources),
        pdf_stream(b"/Filter /FlateDecode", zlib.compress(content, 9)),
        *objects,
    ]
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for number, body in enumerate(objects, 1):
        offsets.append(len(out))
        out += b"%d 0 obj\n%s\nendobj\n" % (number, body)
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objects) + 1)
    out += b"".join(b"%010d 00000 n \n" % offset for offset in offsets)
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (
        len(objects) + 1,
        xref,
    )
    return bytes(out)


def pdf_image(space, jpeg=False, deep=False):
    """A page covered by the chart image of `space`, as raw or JPEG samples."""
    objects = []
    color_space = pdf_color_space(space, objects)
    image = space.image
    entries = b"/Type /XObject /Subtype /Image /Width %d /Height %d /ColorSpace %s" % (
        *image.size,
        color_space,
    )
    if jpeg:
        # Leave the color space to the PDF, not an embedded profile.
        buffer = io.BytesIO()
        image.save(buffer, "JPEG", quality=100, subsampling=0)
        data = buffer.getvalue()
        entries += b" /BitsPerComponent 8 /Filter /DCTDecode"
        if image.mode == "CMYK":
            # Pillow follows Adobe in storing inverted CMYK JPEG samples.
            entries += b" /Decode [1 0 1 0 1 0 1 0]"
    else:
        raw = image.tobytes()
        if deep:
            raw = struct.pack(f">{len(raw)}H", *(v * 257 for v in raw))
        entries += b" /BitsPerComponent %d /Filter /FlateDecode" % (16 if deep else 8)
        data = zlib.compress(raw, 9)
    image_object = pdf_add(objects, pdf_stream(entries, data))
    content = b"q %s 0 0 %s 0 0 cm /Im0 Do Q" % tuple(
        f"{v:.4f}".encode() for v in PAGE_POINTS
    )
    return pdf(b"/XObject << /Im0 %s >>" % image_object, content, objects)


def pdf_vector(space):
    """A page of rectangles filled with the patch colors of `space`."""
    objects = []
    color_space = pdf_color_space(space, objects)
    content = b"/CS0 cs\n" + b"".join(
        b"%s sc %s re f\n"
        % (
            numbers(v / 255 for v in color).encode(),
            numbers((x, PAGE_POINTS[1] - y - h, w, h)).encode(),
        )
        for color, (x, y, w, h) in zip(samples(space.image), tiles(PAGE_POINTS))
    )
    return pdf(b"/ColorSpace << /CS0 %s >>" % color_space, content, objects)


XPS_NAMESPACE = "http://schemas.microsoft.com/xps/2005/06"
XPS_CONTENT_TYPES = {
    "rels": "application/vnd.openxmlformats-package.relationships+xml",
    "fdseq": "application/vnd.ms-package.xps-fixeddocumentsequence+xml",
    "fdoc": "application/vnd.ms-package.xps-fixeddocument+xml",
    "fpage": "application/vnd.ms-package.xps-fixedpage+xml",
    "icc": "application/vnd.ms-color.iccprofile",
    "jpg": "image/jpeg",
    "png": "image/png",
    "tiff": "image/tiff",
}


def xps_relationships(relationships):
    items = "".join(
        f'<Relationship Id="R{i}" Type="{kind}" Target="{target}"/>'
        for i, (kind, target) in enumerate(relationships)
    )
    return (
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/'
        f'relationships">{items}</Relationships>'
    )


def xps(page, resources):
    """Serialize a one-page XPS package with the markup and resources of the page.

    `resources` maps part names under /Resources/ to their content.
    """
    page = (
        f'<FixedPage xmlns="{XPS_NAMESPACE}" Width="{PAGE_DIPS[0]:.4f}" '
        f'Height="{PAGE_DIPS[1]:.4f}" xml:lang="und">{page}</FixedPage>'
    )
    defaults = "".join(
        f'<Default Extension="{extension}" ContentType="{kind}"/>'
        for extension, kind in XPS_CONTENT_TYPES.items()
    )
    parts = {
        "[Content_Types].xml": '<Types xmlns="http://schemas.openxmlformats.org/'
        f'package/2006/content-types">{defaults}</Types>',
        "_rels/.rels": xps_relationships(
            [
                (
                    "http://schemas.microsoft.com/xps/2005/06/fixedrepresentation",
                    "/FixedDocumentSequence.fdseq",
                )
            ]
        ),
        "FixedDocumentSequence.fdseq": f'<FixedDocumentSequence xmlns="{XPS_NAMESPACE}">'
        '<DocumentReference Source="/Documents/1/FixedDocument.fdoc"/>'
        "</FixedDocumentSequence>",
        "Documents/1/FixedDocument.fdoc": f'<FixedDocument xmlns="{XPS_NAMESPACE}">'
        '<PageContent Source="/Documents/1/Pages/1.fpage"/></FixedDocument>',
        "Documents/1/Pages/1.fpage": page,
        "Documents/1/Pages/_rels/1.fpage.rels": xps_relationships(
            (
                "http://schemas.microsoft.com/xps/2005/06/required-resource",
                f"/Resources/{name}",
            )
            for name in resources
        ),
        **{f"Resources/{name}": data for name, data in resources.items()},
    }
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as package:
        for name, data in parts.items():
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            package.writestr(info, data)
    return buffer.getvalue()


def xps_rectangle(x, y, width, height):
    return (
        f"M {x:.4f},{y:.4f} L {x + width:.4f},{y:.4f} "
        f"{x + width:.4f},{y + height:.4f} {x:.4f},{y + height:.4f} Z"
    )


def xps_image(path):
    """A page covered by the image at `path`, at the size given by its DPI."""
    with Image.open(path) as image:
        viewbox = [v * 96 / d for v, d in zip(image.size, image.info["dpi"])]
    page = (
        f'<Path Data="{xps_rectangle(0, 0, *PAGE_DIPS)}"><Path.Fill>'
        f'<ImageBrush ImageSource="/Resources/{path.name}" '
        f'Viewbox="0,0,{viewbox[0]:.4f},{viewbox[1]:.4f}" ViewboxUnits="Absolute" '
        f'Viewport="0,0,{PAGE_DIPS[0]:.4f},{PAGE_DIPS[1]:.4f}" ViewportUnits="Absolute"/>'
        "</Path.Fill></Path>"
    )
    return xps(page, {path.name: path.read_bytes()})


def xps_vector(colors, resources=None):
    """A page of rectangles filled with `colors`, in XPS color syntax."""
    page = "".join(
        f'<Path Data="{xps_rectangle(*tile)}" Fill="{color}"/>'
        for color, tile in zip(colors, tiles(PAGE_DIPS))
    )
    return xps(page, resources or {})


def xps_context_vector(space, profile_name):
    """Rectangles in `space`, referring to its ICC profile in ContextColor syntax."""
    colors = (
        f"ContextColor /Resources/{profile_name} 1,"
        + ",".join(f"{v / 255:.4f}" for v in color)
        for color in samples(space.image)
    )
    return xps_vector(colors, {profile_name: space.profile})


def linear(v):
    """Decode an 8-bit sRGB sample to linear light, as scRGB."""
    v /= 255
    return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4


def render(path):
    """Render the file at `path` in sRGB with MuPDF, and sample each patch."""
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "page.png"
        subprocess.run(
            ["mutool", "draw", "-q", "-r", "72", "-c", "rgb", "-o", output, path],
            check=True,
            capture_output=True,
        )
        with Image.open(output) as page:
            page = page.convert("RGB")
    return [
        page.getpixel((int(x + w / 2), int(y + h / 2)))
        for x, y, w, h in tiles(page.size)
    ]


def verify_document(path, document):
    """Check that MuPDF renders the document at `path` like its image fixture.

    The fixture is checked against the reference with LittleCMS. Comparing with
    MuPDF's own rendering of it rules out MuPDF's ICC handling in CMYK JPEG.
    """
    if document.image is None:
        print(f"{document.name}: not checked, same samples as the PDF paths")
        return
    error = max(
        abs(a - b)
        for p, q in zip(render(path), render(BASIC / document.image))
        for a, b in zip(p, q)
    )
    # Two levels accommodate the loss of PDF JPEG images, and PDF/XPS number rounding.
    assert error <= 2, (document.name, error)
    print(f"{document.name}: MuPDF max channel error from {document.image} = {error}")


def centers(image):
    return [
        image.getpixel(
            (
                (2 * (i % GRID[0]) + 1) * SIZE[0] // (2 * GRID[0]),
                (2 * (i // GRID[0]) + 1) * SIZE[1] // (2 * GRID[1]),
            )
        )
        for i in range(GRID[0] * GRID[1])
    ]


def verify(path, fixture, srgb):
    """Reopen `path` and convert it back to sRGB independently of Windows."""
    with Image.open(path) as stored:
        assert stored.info.get("icc_profile") == fixture.space.profile, fixture.name
        # Pillow reads 16-bit RGB as its high bytes, which is exact for v * 257.
        actual = convert(stored, fixture.space.profile or srgb, srgb, "RGB")
        # The fixtures must not depend on the intent. With a v4 sRGB, LittleCMS
        # also compensates for black in perceptual.
        perceptual = convert(
            stored,
            fixture.space.profile or srgb,
            srgb,
            "RGB",
            ImageCms.Intent.PERCEPTUAL,
        )
    patches = centers(actual)
    assert centers(perceptual) == patches, f"{fixture.name} depends on the intent"
    # Basic scope: no channel reaches clipping, all colors are inside 8-bit sRGB.
    assert all(8 < c < 247 for pixel in patches for c in pixel), fixture.name
    error = max(
        abs(a - b)
        for p, q in zip(patches, centers(fixture.space.reference))
        for a, b in zip(p, q)
    )
    assert error <= 4, (fixture.name, error)
    print(f"{fixture.name}: independent sRGB round-trip max channel error = {error}")


def xps_name(fixture):
    path = Path(fixture.name)
    return f"{path.stem}-{'jpeg' if path.suffix == '.jpg' else path.suffix[1:]}.xps"


def generate_basic(srgb, adobe, gray, cmyk):
    BASIC.mkdir(parents=True, exist_ok=True)
    reference = chart(COLORS)
    gray_reference = chart(GRAYS)
    save(BASIC / "expected.tiff", reference)
    save(BASIC / "expected-gray.tiff", gray_reference)

    untagged_space = Space(reference, None, reference)
    srgb_space = Space(reference, srgb, reference)
    adobe_space = Space(convert(reference, srgb, adobe, "RGB"), adobe, reference)
    cmyk_space = Space(chart(map(ink_samples, COLORS), "CMYK"), cmyk, reference)
    gray_space = Space(convert(gray_reference, srgb, gray, "L"), gray, gray_reference)
    fixtures = [
        Fixture("rgb-untagged.tiff", untagged_space),
        Fixture("srgb.tiff", srgb_space),
        Fixture("rgb-16bit-untagged.png", untagged_space, deep=True),
        Fixture("srgb-16bit.png", srgb_space, deep=True),
        Fixture("adobe-rgb.tiff", adobe_space),
        Fixture("adobe-rgb.png", adobe_space),
        Fixture("adobe-rgb.jpg", adobe_space),
        Fixture("cmyk.tiff", cmyk_space),
        Fixture("cmyk.jpg", cmyk_space),
        Fixture("gray-gamma18.tiff", gray_space),
    ]
    for fixture in fixtures:
        path = BASIC / fixture.name
        save(path, fixture.space.image, fixture.space.profile, fixture.deep)
        verify(path, fixture, srgb)

    documents = [
        # The color expressions of the images, as PDF images and paths.
        Document(
            "rgb-untagged-image.pdf", pdf_image(untagged_space), "rgb-untagged.tiff"
        ),
        Document("srgb-image.pdf", pdf_image(srgb_space), "srgb.tiff"),
        Document(
            "rgb-16bit-untagged-image.pdf",
            pdf_image(untagged_space, deep=True),
            "rgb-untagged.tiff",
        ),
        Document("srgb-16bit-image.pdf", pdf_image(srgb_space, deep=True), "srgb.tiff"),
        Document("adobe-rgb-image.pdf", pdf_image(adobe_space), "adobe-rgb.tiff"),
        Document(
            "adobe-rgb-jpeg.pdf", pdf_image(adobe_space, jpeg=True), "adobe-rgb.tiff"
        ),
        Document("cmyk-image.pdf", pdf_image(cmyk_space), "cmyk.tiff"),
        Document("cmyk-jpeg.pdf", pdf_image(cmyk_space, jpeg=True), "cmyk.tiff"),
        Document("gray-gamma18-image.pdf", pdf_image(gray_space), "gray-gamma18.tiff"),
        Document(
            "rgb-untagged-vector.pdf", pdf_vector(untagged_space), "rgb-untagged.tiff"
        ),
        Document("adobe-rgb-vector.pdf", pdf_vector(adobe_space), "adobe-rgb.tiff"),
        Document("cmyk-vector.pdf", pdf_vector(cmyk_space), "cmyk.tiff"),
        Document(
            "gray-gamma18-vector.pdf", pdf_vector(gray_space), "gray-gamma18.tiff"
        ),
        # The very same image files in XPS, then XPS paths in each color syntax.
        *(
            Document(xps_name(fixture), xps_image(BASIC / fixture.name), fixture.name)
            for fixture in fixtures
        ),
        Document(
            "srgb-vector.xps",
            xps_vector(
                "#" + "".join(f"{v:02X}" for v in color) for color in samples(reference)
            ),
            "rgb-untagged.tiff",
        ),
        Document(
            "scrgb-vector.xps",
            xps_vector(
                "sc#1," + ",".join(f"{linear(v):.4f}" for v in color)
                for color in samples(reference)
            ),
            "rgb-untagged.tiff",
        ),
        # No gray: Windows rejects one-channel ContextColor as invalid markup.
        Document(
            "adobe-rgb-vector.xps",
            xps_context_vector(adobe_space, "adobe-rgb-compatible.icc"),
            None,
        ),
        Document("cmyk-vector.xps", xps_context_vector(cmyk_space, CMYK_PROFILE), None),
    ]
    for document in documents:
        path = BASIC / document.name
        path.write_bytes(document.data)
        verify_document(path, document)


def main():
    generate_basic(*load_profiles())


if __name__ == "__main__":
    main()
