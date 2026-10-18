//! Pseudo-random pixels survive printing: an image the renderer cannot compress, smooth or guess is
//! compared with the raster the printer receives, pixel for pixel. Unlike a pattern, noise tells a
//! page drawn at the job's resolution (no pixel off by more than 8) from one drawn at 96 dpi (93%
//! of the pixels off).

#![cfg(all(windows, feature = "test-utils"))]

mod common;
use common::print;
use image::GrayImage;
use winprint::{
    printer::{FilePrinter, WinPdfPrinter},
    ticket::{
        FeatureOptionPackWithPredefined, PredefinedMediaName, PrintCapabilities, PrintTicket,
    },
};

const SIDE: u32 = 600;
/// 300 dpi in points.
const PT_PER_PX: f64 = 72.0 / 300.0;

/// xorshift64*: the same bytes on every run and platform.
fn noise(seed: u64) -> GrayImage {
    let mut state = seed;
    let mut next = move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        (state.wrapping_mul(0x2545F4914F6CDD1D) >> 56) as u8
    };
    GrayImage::from_fn(SIDE, SIDE, |_, _| image::Luma([next()]))
}

/// A page of exactly 2480 x 3510 px at 300 dpi with the noise at pixel (100, 100), one image pixel
/// for one device pixel.
fn pdf(image: &GrayImage) -> Vec<u8> {
    let (w, h) = (2480.0 * PT_PER_PX, 3510.0 * PT_PER_PX);
    let (x, top, size) = (
        100.0 * PT_PER_PX,
        100.0 * PT_PER_PX,
        f64::from(SIDE) * PT_PER_PX,
    );
    let content = format!(
        "q {size:.4} 0 0 {size:.4} {x:.4} {:.4} cm /Im Do Q",
        h - top - size
    );
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.4} {h:.4}] /Contents 4 0 R /Resources << /XObject << /Im 5 0 R >> >> >>"
        )
        .into_bytes(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).into_bytes(),
    ];
    let mut object = format!(
        "<< /Type /XObject /Subtype /Image /Width {SIDE} /Height {SIDE} /ColorSpace /DeviceGray /BitsPerComponent 8 /Length {} >>\nstream\n",
        image.as_raw().len()
    )
    .into_bytes();
    object.extend(image.as_raw());
    object.extend(b"\nendstream");
    objects.push(object);
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).into_bytes());
        out.extend(object);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).into_bytes());
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").into_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .into_bytes(),
    );
    out
}

fn ticket(device: &winprint::printer::PrinterDevice) -> PrintTicket {
    let capabilities = PrintCapabilities::fetch(device).unwrap();
    let media = capabilities
        .page_media_sizes()
        .find(|x| x.as_predefined_name() == Some(PredefinedMediaName::ISOA4))
        .unwrap();
    media.into()
}

/// Mean absolute difference between `source` and the printed page at the best offset within +-3 px
/// of (100, 100), and that offset.
fn compare(printed: &image::DynamicImage, source: &GrayImage) -> (f64, (i32, i32)) {
    let (mean, at, _) = compare_counting(printed, source);
    (mean, at)
}

/// [`compare`], and how many pixels differ by more than 8 at that offset.
fn compare_counting(printed: &image::DynamicImage, source: &GrayImage) -> (f64, (i32, i32), u32) {
    let page = printed.to_luma8();
    let mut best = (f64::MAX, (0, 0), 0u32);
    for dy in -3..=3 {
        for dx in -3..=3 {
            let mut sum = 0u64;
            let mut far = 0u32;
            for y in 0..SIDE {
                for x in 0..SIDE {
                    let p = page
                        .get_pixel((100 + x as i32 + dx) as u32, (100 + y as i32 + dy) as u32)[0];
                    let d = p.abs_diff(source.get_pixel(x, y)[0]);
                    sum += u64::from(d);
                    far += u32::from(d > 8);
                }
            }
            let mean = sum as f64 / f64::from(SIDE * SIDE);
            if mean < best.0 {
                best = (mean, (dx, dy), far);
            }
        }
    }
    best
}

#[test]
fn noise_prints_back_pixel_for_pixel() {
    let _ = env_logger::try_init();
    let source = noise(0x5EED);
    let path = std::env::temp_dir().join(format!("winprint-noise-{}.pdf", std::process::id()));
    std::fs::write(&path, pdf(&source)).unwrap();
    let whole = print::print_pages(|device| {
        WinPdfPrinter::new(device.clone())
            .print(&path, ticket(device))
            .unwrap();
    });
    let _ = std::fs::remove_file(&path);
    let (whole_diff, whole_at, whole_far) = compare_counting(&whole[0], &source);
    assert_eq!(whole[0].width(), 2480, "the page is A4 at 300 dpi");
    assert_eq!(
        whole_at,
        (0, 0),
        "one image pixel is one device pixel, at the same place"
    );
    // About 85 for the wrong picture, 61 when the page is drawn at 96 dpi; 0.015 measured.
    assert!(
        whole_diff < 0.5 && whole_far == 0,
        "{whole_diff:.3}, {whole_far} pixels off"
    );
    let (other, _) = compare(&whole[0], &noise(0xBEEF));
    assert!(other > 50.0, "another seed matched ({other:.1})");
}
