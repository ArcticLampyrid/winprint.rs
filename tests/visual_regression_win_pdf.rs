//! End-to-end visual regression for [`WinPdfPrinter`].

#![cfg(all(windows, feature = "test-utils"))]

mod common;
use common::print;
use common::visual::VisualExpectation;
use winprint::{
    printer::{FilePrinter, PrintOutcome, WinPdfPrinter},
    ticket::{
        FeatureOptionPackWithPredefined, PredefinedMediaName, PrintCapabilities, PrintTicket,
    },
};

#[test]
fn win_pdf_visual_regression() {
    let _ = env_logger::try_init();
    let visual_regression_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_data")
        .join("visual_regression");
    // WinPdf preserves detail more faithfully than the EMF/XPS paths. Use a
    // tighter tolerance so downsampling the raster-pattern page to 96 dpi fails.
    let expectation = VisualExpectation::new(visual_regression_dir.join("data.tiff"));
    let actual_pages = print::print_pages(|device| {
        let origin = visual_regression_dir.join("data.pdf");
        let printer = WinPdfPrinter::new(device.clone());
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        let media_size = capabilities
            .page_media_sizes()
            .find(|x| x.as_predefined_name() == Some(PredefinedMediaName::ISOA4))
            .unwrap();
        let print_ticket: PrintTicket = media_size.into();
        let outcome = printer
            .print(origin.as_path(), print_ticket)
            .expect("failed to print the document");
        assert_eq!(outcome, PrintOutcome::Completed);
    });
    expectation.assert(&actual_pages);
}
