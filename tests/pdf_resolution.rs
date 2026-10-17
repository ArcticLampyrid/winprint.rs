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
