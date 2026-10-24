//! End-to-end color management for [`ImagePrinter`].

#![cfg(all(windows, feature = "test-utils"))]

mod common;
use winprint::printer::{ImagePrinter, PrinterDevice};

fn image_printer(device: PrinterDevice, wide_color_gamut: bool) -> ImagePrinter {
    let mut printer = ImagePrinter::new(device);
    printer.set_wide_color_gamut(wide_color_gamut);
    printer
}

mod color_management {
    crate::color_management_tests! {
        printers {
            wide_color_gamut_enabled: |device| crate::image_printer(device, true),
            wide_color_gamut_disabled: |device| crate::image_printer(device, false),
        }
        // All colors are inside 8-bit sRGB, so both settings must print them the same.
        basic {
            untagged_rgb_tiff: "rgb-untagged.tiff" => "expected.tiff",
            tagged_srgb_tiff: "srgb.tiff" => "expected.tiff",
            untagged_rgb_16bit_png: "rgb-16bit-untagged.png" => "expected.tiff",
            tagged_srgb_16bit_png: "srgb-16bit.png" => "expected.tiff",
            adobe_rgb_tiff: "adobe-rgb.tiff" => "expected.tiff",
            adobe_rgb_png: "adobe-rgb.png" => "expected.tiff",
            adobe_rgb_jpeg: "adobe-rgb.jpg" => "expected.tiff",
            cmyk_tiff: "cmyk.tiff" => "expected.tiff",
            cmyk_jpeg: "cmyk.jpg" => "expected.tiff",
            gray_gamma18_tiff: "gray-gamma18.tiff" => "expected-gray.tiff",
        }
    }
}
