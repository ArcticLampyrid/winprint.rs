use image::DynamicImage;
use std::time::Duration;
use winprint::{
    printer::PrinterDevice,
    test_utils::file_device::{FilePrinterDevice, PwgRaster},
};

use super::loader;

pub fn print_pages<F>(print: F) -> Vec<DynamicImage>
where
    F: FnOnce(&PrinterDevice),
{
    let device = FilePrinterDevice::<PwgRaster>::new().expect("install virtual PWG printer");
    print(device.device());
    device
        .wait_for_idle(Duration::from_mins(10))
        .expect("PWG print job should finish writing its output");
    loader::load_pages_from_pwg(device.file_path()).expect("decode PWG raster output")
}
