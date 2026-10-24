use image::DynamicImage;
use std::time::Duration;
use winprint::{
    printer::PrinterDevice,
    test_utils::file_device::{FilePrinterDevice, PwgRaster},
    ticket::{
        FeatureOptionPackWithPredefined, PredefinedMediaName, PredefinedPageOutputColor,
        PrintCapabilities, PrintTicket, PrintTicketBuilder,
    },
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

/// A print ticket for color pages on A4.
pub fn a4_color_ticket(device: &PrinterDevice) -> PrintTicket {
    let capabilities = PrintCapabilities::fetch(device).expect("fetch printer capabilities");
    let media = capabilities
        .page_media_sizes()
        .find(|option| option.as_predefined_name() == Some(PredefinedMediaName::ISOA4))
        .expect("the printer must support A4");
    let color = capabilities
        .page_output_colors()
        .find(|option| option.as_predefined_name() == Some(PredefinedPageOutputColor::Color))
        .expect("the printer must support color");
    let mut ticket = PrintTicketBuilder::new(device).expect("create print ticket");
    ticket.merge(media).expect("select A4");
    ticket.merge(color).expect("select color output");
    ticket.build().expect("build print ticket")
}
