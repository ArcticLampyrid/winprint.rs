//! Color comparison for color management tests.

use crate::common::{loader, print};
use image::{DynamicImage, RgbImage};
use std::{fmt::Debug, ops::Range, path::PathBuf};
use winprint::{
    printer::{FilePrinter, PrintOutcome, PrinterDevice},
    ticket::PrintTicket,
};

// Match the 4 x 6 full-page chart in utils/gen-color-management-data/run.py.
const COLUMNS: u32 = 4;
const ROWS: u32 = 6;
// Independent LittleCMS round trips differ by at most 4/255 in the fixtures.
// Allow two further levels for Windows CMM/driver rounding, not arbitrary hue shifts.
const MAX_CHANNEL_ERROR: f64 = 6.0;

/// A patch of the chart, by its position.
#[derive(Clone, Copy)]
struct Patch {
    row: u32,
    column: u32,
}

impl Patch {
    fn all() -> impl Iterator<Item = Self> {
        (0..ROWS).flat_map(|row| (0..COLUMNS).map(move |column| Self { row, column }))
    }

    /// The color at the center of the patch in `image`.
    fn center(self, image: &RgbImage) -> [u8; 3] {
        let x = (2 * self.column + 1) * image.width() / (2 * COLUMNS);
        let y = (2 * self.row + 1) * image.height() / (2 * ROWS);
        image.get_pixel(x, y).0
    }

    /// The colors in the central third of the patch in `image`. No JPEG/resampling
    /// boundaries, printer margins or subpixel page registration reach them.
    fn interior(self, image: &RgbImage) -> impl Iterator<Item = [u8; 3]> + '_ {
        let xs = central_third(self.column, COLUMNS, image.width());
        let ys = central_third(self.row, ROWS, image.height());
        ys.flat_map(move |y| xs.clone().map(move |x| image.get_pixel(x, y).0))
    }

    /// Compare the interior of the patch in `actual` with its color in `expected`, using
    /// mean absolute error per channel, so that opposing errors cannot cancel out.
    fn failure(self, actual: &RgbImage, expected: &RgbImage) -> Option<String> {
        let target = self.center(expected);
        let (count, sum, error) = self.interior(actual).fold(
            (0_u64, [0_u64; 3], [0_u64; 3]),
            |(count, sum, error), pixel| {
                (
                    count + 1,
                    std::array::from_fn(|c| sum[c] + u64::from(pixel[c])),
                    std::array::from_fn(|c| error[c] + u64::from(pixel[c].abs_diff(target[c]))),
                )
            },
        );
        let mean = sum.map(|v| v as f64 / count as f64);
        let error = error.map(|v| v as f64 / count as f64);
        error.iter().any(|&v| v > MAX_CHANNEL_ERROR).then(|| {
            format!(
                "patch {} (row {}, column {}): expected sRGB {target:?}, mean {mean:.1?}, \
                 mean absolute channel error {error:.1?} exceeds {MAX_CHANNEL_ERROR}",
                self.row * COLUMNS + self.column + 1,
                self.row + 1,
                self.column + 1,
            )
        })
    }
}

/// The central third of the `index`-th of `count` equal parts of `0..length`.
fn central_third(index: u32, count: u32, length: u32) -> Range<u32> {
    (3 * index + 1) * length / (3 * count)..(3 * index + 2) * length / (3 * count)
}

/// Expectation for a single color management test: one page with the colors of a chart.
pub struct ColorExpectation {
    /// Path of the expected sRGB chart, in TIFF format.
    pub expected_path: PathBuf,
}

impl ColorExpectation {
    pub const fn new(expected_tiff_path: PathBuf) -> Self {
        Self {
            expected_path: expected_tiff_path,
        }
    }

    /// Compare each patch independently; global SSIM can hide incorrect ICC handling.
    pub fn failures(&self, actual_pages: &[DynamicImage]) -> Vec<String> {
        let expected_pages =
            loader::load_pages_from_tiff(&self.expected_path).expect("load expected TIFF");
        let [expected] = &expected_pages[..] else {
            panic!("the expected chart must have exactly one page");
        };
        let [actual] = actual_pages else {
            return vec![format!("expected one page, got {}", actual_pages.len())];
        };
        let (expected, actual) = (expected.to_rgb8(), actual.to_rgb8());

        let (width, height) = actual.dimensions();
        if width < COLUMNS * 12 || height < ROWS * 12 {
            return vec![format!("page is too small: {width} x {height}")];
        }
        let aspect = f64::from(width) / f64::from(height);
        let expected_aspect = f64::from(expected.width()) / f64::from(expected.height());
        if (aspect / expected_aspect - 1.0).abs() > 0.01 {
            return vec![format!(
                "page has a different aspect ratio: {width} x {height}"
            )];
        }

        Patch::all()
            .filter_map(|patch| patch.failure(&actual, &expected))
            .collect()
    }

    pub fn assert(&self, actual_pages: &[DynamicImage]) {
        let failures = self.failures(actual_pages);
        assert!(
            failures.is_empty(),
            "color management failures:\n  - {}",
            failures.join("\n  - ")
        );
    }
}

/// Print `test_data/color_management/<scope>/<input>` with the printer that `new_printer`
/// creates, on color A4, and compare the page with `<expected>` there.
pub fn check<P>(
    scope: &str,
    input: &str,
    expected: &str,
    new_printer: impl FnOnce(PrinterDevice) -> P,
) where
    P: FilePrinter<Options = PrintTicket>,
    P::Error: Debug,
{
    let _ = env_logger::try_init();
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_data")
        .join("color_management")
        .join(scope);
    let expectation = ColorExpectation::new(data_dir.join(expected));
    let actual_pages = print::print_pages(|device| {
        let outcome = new_printer(device.clone())
            .print(&data_dir.join(input), print::a4_color_ticket(device))
            .expect("failed to print the color fixture");
        assert_eq!(outcome, PrintOutcome::Completed);
    });
    expectation.assert(&actual_pages);
}

/// Generate `<scope>::<printer>::<case>` tests, which [`check`] each case of a scope with
/// each printer, created by `<new_printer>`. Attributes of a case, such as `#[ignore]`, apply
/// to its tests:
///
/// ```ignore
/// color_management_tests! {
///     printers { <printer>: <new_printer>, ... }
///     <scope> { #[<attribute>] <case>: "<input>" => "<expected>", ... }
///     ...
/// }
/// ```
#[macro_export]
macro_rules! color_management_tests {
    (printers $printers:tt $($scope:ident $cases:tt)+) => {
        $(mod $scope {
            $crate::color_management_tests!(@printers stringify!($scope), $printers, $cases);
        })+
    };
    (
        @printers $scope:expr,
        { $($printer:ident: $new_printer:expr),+ $(,)? },
        $cases:tt
    ) => {
        $(mod $printer {
            $crate::color_management_tests!(@cases $scope, $new_printer, $cases);
        })+
    };
    (
        @cases $scope:expr,
        $new_printer:expr,
        { $($(#[$attr:meta])* $case:ident: $input:literal => $expected:literal),+ $(,)? }
    ) => {
        $(#[test]
        $(#[$attr])*
        fn $case() {
            $crate::common::color::check($scope, $input, $expected, $new_printer);
        })+
    };
}
