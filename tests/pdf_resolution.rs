//! A PDF page is rendered at the resolution of the job, not at 96 dpi.

#![cfg(all(windows, feature = "test-utils"))]

mod common;
use common::print;
use winprint::{
    printer::{FilePrinter, WinPdfPrinter},
    ticket::{
        FeatureOptionPackWithPredefined, PredefinedMediaName, PrintCapabilities, PrintTicket,
    },
};

/// `stripes.pdf` places a 600 x 600 pixel image of two-pixel stripes on a 2 x 2 inch square, 300 dpi
/// of image. Drawn at 96 dpi (`Windows.Data.Pdf`'s default) the stripes blur to grey; drawn at the
/// 300 dpi of the virtual printer they keep their contrast.
#[test]
fn an_image_keeps_its_detail_at_the_job_resolution() {
    let _ = env_logger::try_init();
    let pdf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_data")
        .join("stripes.pdf");
    let pages = print::print_pages(|device| {
        let capabilities = PrintCapabilities::fetch(device).unwrap();
        let media = capabilities
            .page_media_sizes()
            .find(|x| x.as_predefined_name() == Some(PredefinedMediaName::ISOA4))
            .unwrap();
        let ticket: PrintTicket = media.into();
        WinPdfPrinter::new(device.clone())
            .print(&pdf, ticket)
            .expect("failed to print the document");
    });
    let page = pages[0].to_luma8();
    // The square is at x 100..244 pt, y 500..644 pt from the bottom of a 595.28 x 841.89 pt page.
    let px_per_pt = page.width() as f64 / 595.28;
    let (x0, x1) = (
        (100.0 * px_per_pt) as u32 + 4,
        (244.0 * px_per_pt) as u32 - 4,
    );
    let top = (841.89 - 644.0) * px_per_pt;
    let bottom = (841.89 - 500.0) * px_per_pt;
    let (y0, y1) = (top as u32 + 4, bottom as u32 - 4);
    let values: Vec<f64> = (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .map(|(x, y)| f64::from(page.get_pixel(x, y)[0]))
        .collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let deviation =
        (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt();
    // About 90 at 300 dpi, about 33 when the page is drawn at 96 dpi.
    assert!(deviation > 70.0, "the stripes are blurred ({deviation:.1})");
}

/// Tiles are what a page is drawn in when Direct2D refuses its layer whole; they must print like
/// the whole page. `blended.pdf` has two overlapping multiply-blended squares (a layer for the
/// print control to composite) next to plain fills.
#[test]
fn a_page_drawn_in_tiles_prints_like_the_whole_page() {
    let _ = env_logger::try_init();
    let pdf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_data")
        .join("blended.pdf");
    let ticket = |device: &winprint::printer::PrinterDevice| -> PrintTicket {
        let capabilities = PrintCapabilities::fetch(device).unwrap();
        let media = capabilities
            .page_media_sizes()
            .find(|x| x.as_predefined_name() == Some(PredefinedMediaName::ISOA4))
            .unwrap();
        let ticket: PrintTicket = media.into();
        ticket
    };
    let whole = print::print_pages(|device| {
        WinPdfPrinter::new(device.clone())
            .print(&pdf, ticket(device))
            .expect("the whole page");
    });
    let mut tile_count = 0;
    let tiled = print::print_pages(|device| {
        tile_count = WinPdfPrinter::new(device.clone())
            .print_in_tiles(&pdf, ticket(device), 512)
            .expect("the page in tiles");
    });
    assert!(tile_count > 1, "the page was not tiled ({tile_count})");
    assert_eq!((whole.len(), tiled.len()), (1, 1));
    let score = common::visual::compare_single_page(&tiled[0], &whole[0]);
    assert!(score > 0.99, "tiles differ from the whole page ({score})");
}
