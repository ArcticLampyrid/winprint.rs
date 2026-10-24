//! Validate the color fixtures and comparison using image operations only.
//! These tests do not install a printer or submit any print jobs.

#![cfg(all(windows, feature = "test-utils"))]

mod common;

mod color_management {
    mod basic {
        use crate::common::{color::ColorExpectation, loader::load_pages_from_tiff};
        use image::{imageops::FilterType, DynamicImage, Rgb, RgbImage};
        use std::path::PathBuf;

        fn data_path(name: &str) -> PathBuf {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("test_data/color_management/basic")
                .join(name)
        }

        /// Load the raw samples of `name`, deliberately without applying its ICC profile.
        fn raw_pages(name: &str) -> Vec<DynamicImage> {
            load_pages_from_tiff(&data_path(name)).expect("load color fixture")
        }

        fn fixture(expected: &str) -> (ColorExpectation, RgbImage) {
            let pages = raw_pages(expected);
            assert_eq!(pages.len(), 1, "the expected chart must have one page");
            let page = pages[0].to_rgb8();
            assert_eq!(
                page.dimensions(),
                (420, 594),
                "the expected chart must be A4"
            );
            (ColorExpectation::new(data_path(expected)), page)
        }

        fn assert_rejected(expectation: &ColorExpectation, pages: &[DynamicImage]) {
            assert!(
                !expectation.failures(pages).is_empty(),
                "the color comparison accepted a broken conversion"
            );
        }

        #[test]
        fn unchanged_and_printer_sized_pages_pass() {
            for expected in ["expected.tiff", "expected-gray.tiff"] {
                let (expectation, page) = fixture(expected);
                let page = DynamicImage::ImageRgb8(page);
                expectation.assert(&[page.resize_exact(2480, 3508, FilterType::Triangle)]);
                expectation.assert(&[page]);
            }
        }

        #[test]
        fn missing_extra_and_blank_pages_fail() {
            let (expectation, page) = fixture("expected.tiff");
            let page = DynamicImage::ImageRgb8(page);
            assert_rejected(&expectation, &[]);
            assert_rejected(&expectation, &[page.clone(), page]);
            let blank = RgbImage::from_pixel(420, 594, Rgb([255; 3]));
            assert_rejected(&expectation, &[blank.into()]);
        }

        #[test]
        fn ignoring_adobe_rgb_profile_fails() {
            let (expectation, _) = fixture("expected.tiff");
            assert_rejected(&expectation, &raw_pages("adobe-rgb.tiff"));
        }

        #[test]
        fn ignoring_gray_transfer_function_fails() {
            let (expectation, _) = fixture("expected-gray.tiff");
            assert_rejected(&expectation, &raw_pages("gray-gamma18.tiff"));
        }

        #[test]
        fn grayscale_output_fails() {
            let (expectation, page) = fixture("expected.tiff");
            let grayscale = DynamicImage::ImageRgb8(page).into_luma8();
            assert_rejected(&expectation, &[grayscale.into()]);
        }

        #[test]
        fn linear_samples_taken_as_srgb_fail() {
            let (expectation, mut page) = fixture("expected.tiff");
            for pixel in page.pixels_mut() {
                pixel.0 = pixel.0.map(|v| {
                    let encoded = f64::from(v) / 255.0;
                    let linear = if encoded <= 0.04045 {
                        encoded / 12.92
                    } else {
                        ((encoded + 0.055) / 1.055).powf(2.4)
                    };
                    (linear * 255.0).round() as u8
                });
            }
            assert_rejected(&expectation, &[page.into()]);
        }

        #[test]
        fn one_bad_patch_is_not_hidden_by_the_rest_of_the_page() {
            let (expectation, mut page) = fixture("expected.tiff");
            for y in 0..99 {
                for x in 0..105 {
                    page.get_pixel_mut(x, y).0[0] += 12;
                }
            }
            let failures = expectation.failures(&[page.into()]);
            assert_eq!(failures.len(), 1, "{failures:?}");
            assert!(failures[0].starts_with("patch 1 "), "{failures:?}");
        }
    }
}
