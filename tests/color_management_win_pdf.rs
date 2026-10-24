//! End-to-end color management for [`WinPdfPrinter`].

#![cfg(all(windows, feature = "test-utils"))]

mod common;
use winprint::printer::WinPdfPrinter;

mod color_management {
    crate::color_management_tests! {
        printers {
            win_pdf: crate::WinPdfPrinter::new,
        }
        // The colors of the basic images, in PDF images and paths.
        basic {
            untagged_rgb_image: "rgb-untagged-image.pdf" => "expected.tiff",
            tagged_srgb_image: "srgb-image.pdf" => "expected.tiff",
            untagged_rgb_16bit_image: "rgb-16bit-untagged-image.pdf" => "expected.tiff",
            // #[ignore = "Windows.Data.Pdf swaps red and blue in 16-bit ICCBased RGB images"]
            // tagged_srgb_16bit_image: "srgb-16bit-image.pdf" => "expected.tiff",
            adobe_rgb_image: "adobe-rgb-image.pdf" => "expected.tiff",
            adobe_rgb_jpeg: "adobe-rgb-jpeg.pdf" => "expected.tiff",
            cmyk_image: "cmyk-image.pdf" => "expected.tiff",
            cmyk_jpeg: "cmyk-jpeg.pdf" => "expected.tiff",
            gray_gamma18_image: "gray-gamma18-image.pdf" => "expected-gray.tiff",
            untagged_rgb_vector: "rgb-untagged-vector.pdf" => "expected.tiff",
            adobe_rgb_vector: "adobe-rgb-vector.pdf" => "expected.tiff",
            cmyk_vector: "cmyk-vector.pdf" => "expected.tiff",
            gray_gamma18_vector: "gray-gamma18-vector.pdf" => "expected-gray.tiff",
        }
    }
}
