//! End-to-end visual regression for [`XpsPrinter`].

#![cfg(all(windows, feature = "test-utils"))]

mod common;
use common::print;
use common::visual::VisualExpectation;
use std::path::Path;
use winprint::{
    printer::{FilePrinter, XpsPrinter},
    ticket::{
        FeatureOptionPackWithPredefined, PredefinedMediaName, PrintCapabilities, PrintTicket,
    },
};

#[test]
fn xps_visual_regression() {
    let _ = env_logger::try_init();
    let visual_regression_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_data")
        .join("visual_regression");
    let expectation = VisualExpectation::new(visual_regression_dir.join("data.tiff"));
    let actual_pages = print::print_pages(|device| {
        let origin = visual_regression_dir.join("data.xps");
        let printer = XpsPrinter::new(device.clone());
        let capabilities = PrintCapabilities::fetch(&device).unwrap();
        let media_size = capabilities
            .page_media_sizes()
            .find(|x| x.as_predefined_name() == Some(PredefinedMediaName::ISOA4))
            .unwrap();
        let print_ticket: PrintTicket = media_size.into();
        printer
            .print(origin.as_path(), print_ticket)
            .expect("failed to print the document");
    });
    expectation.assert(&actual_pages);
}
