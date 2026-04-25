//! Visual similarity comparison for regression tests.

use crate::common::loader;
use image::{imageops::FilterType, DynamicImage, Rgb, RgbImage};
use std::path::PathBuf;

pub const DEFAULT_MIN_SCORE: f64 = 0.90;

/// Letterbox `img` into a `(container_w, container_h)` canvas, preserving
/// its aspect ratio and padding the rest with white.
fn fit_and_pad(img: &RgbImage, container_w: u32, container_h: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    let scale = (container_w as f64 / w as f64).min(container_h as f64 / h as f64);
    let new_w = ((w as f64 * scale).round() as u32).clamp(1, container_w);
    let new_h = ((h as f64 * scale).round() as u32).clamp(1, container_h);
    let resized = image::imageops::resize(img, new_w, new_h, FilterType::Lanczos3);
    if new_w == container_w && new_h == container_h {
        // Fast path: no letterboxing needed.
        return resized;
    }
    let mut canvas = RgbImage::from_pixel(container_w, container_h, Rgb([255, 255, 255]));
    let x = ((container_w - new_w) / 2) as i64;
    let y = ((container_h - new_h) / 2) as i64;
    image::imageops::overlay(&mut canvas, &resized, x, y);
    canvas
}

pub fn compare_single_page(actual: &DynamicImage, expected: &DynamicImage) -> f64 {
    let actual_rgb = actual.to_rgb8();
    let expected_rgb = expected.to_rgb8();
    let (w, h) = expected_rgb.dimensions();
    let resized_actual = fit_and_pad(&actual_rgb, w, h);
    let result = image_compare::rgb_hybrid_compare(&resized_actual, &expected_rgb)
        .expect("SSIM comparison failed");
    result.score
}

/// Expectation for a single visual regression test.
pub struct VisualExpectation {
    // Path of expected image, in TIFF format.
    pub expected_path: PathBuf,
    /// Minimum acceptable score per page.
    pub min_score: f64,
}

impl VisualExpectation {
    pub const fn new(expected_tiff_path: PathBuf) -> Self {
        Self {
            expected_path: expected_tiff_path,
            min_score: DEFAULT_MIN_SCORE,
        }
    }

    #[allow(dead_code)]
    pub const fn with_min_score(mut self, v: f64) -> Self {
        self.min_score = v;
        self
    }

    pub fn assert(&self, actual_pages: &[DynamicImage]) {
        let expected_pages =
            loader::load_pages_from_tiff(&self.expected_path).expect("load expected TIFF");

        assert_eq!(
            actual_pages.len(),
            expected_pages.len(),
            "page count mismatch: actual={}, expected={}",
            actual_pages.len(),
            expected_pages.len()
        );

        let mut failures = Vec::new();
        for (i, (actual_image, expected_image)) in
            actual_pages.iter().zip(expected_pages.iter()).enumerate()
        {
            let score = compare_single_page(actual_image, expected_image);
            if score < self.min_score {
                failures.push(format!("page {}, score: {:.2}", i + 1, score));
            }
        }

        assert!(
            failures.is_empty(),
            "visual regression failures:\n  - {}",
            failures.join("\n  - ")
        );
    }
}
