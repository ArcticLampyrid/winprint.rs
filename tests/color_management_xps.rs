//! End-to-end color management for [`XpsPrinter`].

#![cfg(all(windows, feature = "test-utils"))]

mod common;
use winprint::printer::XpsPrinter;

mod color_management {
    crate::color_management_tests! {
        printers {
            xps: crate::XpsPrinter::new,
        }
        // The basic images in XPS, then the colors of their charts in XPS paths.
        basic {
            untagged_rgb_tiff: "rgb-untagged-tiff.xps" => "expected.tiff",
            tagged_srgb_tiff: "srgb-tiff.xps" => "expected.tiff",
            untagged_rgb_16bit_png: "rgb-16bit-untagged-png.xps" => "expected.tiff",
            tagged_srgb_16bit_png: "srgb-16bit-png.xps" => "expected.tiff",
            adobe_rgb_tiff: "adobe-rgb-tiff.xps" => "expected.tiff",
            adobe_rgb_png: "adobe-rgb-png.xps" => "expected.tiff",
            adobe_rgb_jpeg: "adobe-rgb-jpeg.xps" => "expected.tiff",
            cmyk_tiff: "cmyk-tiff.xps" => "expected.tiff",
            cmyk_jpeg: "cmyk-jpeg.xps" => "expected.tiff",
            gray_gamma18_tiff: "gray-gamma18-tiff.xps" => "expected-gray.tiff",
            srgb_vector: "srgb-vector.xps" => "expected.tiff",
            scrgb_vector: "scrgb-vector.xps" => "expected.tiff",
            adobe_rgb_vector: "adobe-rgb-vector.xps" => "expected.tiff",
            cmyk_vector: "cmyk-vector.xps" => "expected.tiff",
        }
    }
}
