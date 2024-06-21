use crate::bindings::pdfium::*;
use crate::printer::FilePrinter;
use crate::printer::PrinterDevice;
use crate::ticket::PrintTicket;
use crate::ticket::ToDevModeError;
use crate::utils::pdfium::PdfiumCustomDocument;
use crate::utils::pdfium::PdfiumGuard;
use crate::utils::wchar;
use scopeguard::defer;
use std::path::Path;
use std::ptr;
use std::{fs::File, mem};
use thiserror::Error;
use windows::{
    core::PCWSTR,
    Win32::Graphics::Gdi::{
        CreateDCW, DeleteDC, GetDeviceCaps, SetViewportOrgEx, GET_DEVICE_CAPS_INDEX, LOGPIXELSX,
        LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH,
    },
    Win32::Storage::Xps::*,
};

#[derive(Error, Debug)]
/// Represents an error from [`PdfiumPrinter`].
pub enum PdfiumPrinterError {
    /// Failed to open printer.
    #[error("Failed to open printer")]
    FailedToOpenPrinter,
    /// File I/O error.
    #[error("File I/O error")]
    FileIOError(#[source] std::io::Error),
    /// Print ticket error.
    #[error("Print Ticker Error")]
    PrintTicketError(#[source] ToDevModeError),
}

/// A printer that uses Pdfium to print PDF documents.
pub struct PdfiumPrinter {
    printer: PrinterDevice,
}

impl PdfiumPrinter {
    /// Create a new [`PdfiumPrinter`] for the given printer device.
    pub fn new(printer: PrinterDevice) -> Self {
        Self { printer }
    }
}

impl FilePrinter for PdfiumPrinter {
    type Options = PrintTicket;
    type Error = PdfiumPrinterError;
    fn print(
        &self,
        path: &Path,
        options: PrintTicket,
    ) -> std::result::Result<(), PdfiumPrinterError> {
        unsafe {
            let dev_mode = options
                .to_dev_mode(&self.printer)
                .map_err(PdfiumPrinterError::PrintTicketError)?;
            let hdc_print = CreateDCW(
                None,
                PCWSTR(wchar::to_wide_chars(self.printer.os_name()).as_ptr()),
                None,
                Some(dev_mode.as_ptr() as *const _),
            );
            if hdc_print.is_invalid() {
                return Err(PdfiumPrinterError::FailedToOpenPrinter);
            }
            defer! {
                DeleteDC(hdc_print);
            }
            let mut doc_name = wchar::to_wide_chars(path.file_name().unwrap_or(path.as_ref()));
            let doc_info = DOCINFOW {
                cbSize: mem::size_of::<DOCINFOW>() as i32,
                fwType: 0,
                lpszDocName: PCWSTR(doc_name.as_mut_ptr()),
                lpszOutput: PCWSTR::null(),
                lpszDatatype: PCWSTR::null(),
            };
            StartDocW(hdc_print, &doc_info);
            {
                let _pdfium_guard = PdfiumGuard::guard();
                let mut file = File::open(path).map_err(PdfiumPrinterError::FileIOError)?;
                let mut file_delegation = PdfiumCustomDocument::new(&mut file)
                    .map_err(PdfiumPrinterError::FileIOError)?;
                let document = FPDF_LoadCustomDocument(file_delegation.as_mut(), ptr::null());
                defer! {
                    FPDF_CloseDocument(document);
                }
                let page_count = FPDF_GetPageCount(document);
                for page_index in 0..page_count {
                    let page = FPDF_LoadPage(document, page_index);
                    defer! {
                        FPDF_ClosePage(page);
                    }
                    StartPage(hdc_print);
                    let get_attr =
                        |kind: GET_DEVICE_CAPS_INDEX| -> i32 { GetDeviceCaps(hdc_print, kind) };
                    let dpi_x = get_attr(LOGPIXELSX);
                    let dpi_y = get_attr(LOGPIXELSY);
                    let page_width = FPDF_GetPageWidth(page) * dpi_x as f64 / 72.0;
                    let page_height = FPDF_GetPageHeight(page) * dpi_y as f64 / 72.0;
                    let physical_width = get_attr(PHYSICALWIDTH) as f64;
                    let physical_height = get_attr(PHYSICALHEIGHT) as f64;
                    let scale =
                        f64::min(physical_width / page_width, physical_height / page_height);
                    let w = page_width * scale;
                    let h = page_height * scale;
                    let org_x = -get_attr(PHYSICALOFFSETX);
                    let org_y = -get_attr(PHYSICALOFFSETY);
                    SetViewportOrgEx(hdc_print, org_x, org_y, None);
                    FPDF_RenderPage(hdc_print, page, 0, 0, w as i32, h as i32, 0, FPDF_PRINTING);
                    EndPage(hdc_print);
                }
            }
            EndDoc(hdc_print);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PdfiumPrinter;
    use crate::{printer::FilePrinter, test_utils::null_device};
    use std::path::Path;

    #[test]
    fn print_simple_pdf_document() {
        let device = null_device::thread_local();
        let pdf = PdfiumPrinter::new(device);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.pdf");
        pdf.print(path.as_path(), Default::default()).unwrap();
    }
}
