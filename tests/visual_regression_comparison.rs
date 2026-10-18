//! Validate the visual fixtures and comparison using image operations only.
//! These tests do not install a printer or submit any print jobs.

#![cfg(all(windows, feature = "test-utils"))]

mod common;

use common::{loader::load_pages_from_tiff, visual::VisualExpectation};
use image::{imageops::FilterType, DynamicImage};

fn fixture() -> (VisualExpectation, Vec<DynamicImage>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_data/visual_regression/data.tiff");
    let pages = load_pages_from_tiff(&path).expect("load visual fixtures");
    assert_eq!(
        pages.len(),
        5,
        "fixtures must include the raster-detail page"
    );
    (VisualExpectation::new(path), pages)
}

#[test]
fn unchanged_pages_pass_visual_comparison() {
    let (expectation, pages) = fixture();
    expectation.assert(&pages);
}

#[test]
#[should_panic(expected = "page 5, score:")]
fn downsampling_to_96_dpi_fails_visual_comparison() {
    // Match BASELINE_DPI in utils/gen-visual-regression-data/run.py.
    const BASELINE_DPI: u32 = 300;
    const DEGRADED_DPI: u32 = 96;

    let (expectation, pages) = fixture();
    let downsampled: Vec<_> = pages
        .iter()
        .map(|page| {
            page.resize_exact(
                (page.width() * DEGRADED_DPI + BASELINE_DPI / 2) / BASELINE_DPI,
                (page.height() * DEGRADED_DPI + BASELINE_DPI / 2) / BASELINE_DPI,
                FilterType::Lanczos3,
            )
        })
        .collect();

    // The comparison scales these pages back to the baseline size. It must
    // reject the lost raster detail, not merely the different pixel dimensions.
    expectation.assert(&downsampled);
}
