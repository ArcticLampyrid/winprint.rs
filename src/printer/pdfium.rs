use crate::bindings::pdfium::*;
use crate::printer::FilePrinter;
use crate::printer::FilePrinterError;
use crate::printer::PrinterInfo;
use crate::utils::pdfium::PdfiumGuard;
use crate::utils::wchar;
use crate::{
    bindings::Windows::Win32::{
        Foundation::PWSTR,
        Graphics::Gdi::{
            CreateDCW, DeleteDC, GetDeviceCaps, SetViewportOrgEx, GET_DEVICE_CAPS_INDEX, HDC,
            LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY,
            PHYSICALWIDTH,
        },
        Storage::Xps::*,
    },
    utils::pdfium::PdfiumCustomDocument,
};
use scopeguard::defer;
use std::path::Path;
use std::ptr;
use std::{fs::File, mem};
use windows::Handle;
pub struct PdfiumPrinter {
    printer: PrinterInfo,
}

impl PdfiumPrinter {
    pub fn new(printer: PrinterInfo) -> Self {
        Self { printer }
    }
}

impl FilePrinter for PdfiumPrinter {
    fn print(&self, path: &Path) -> std::result::Result<(), FilePrinterError> {
        unsafe {
            let created_hdc_print = CreateDCW(
                None,
                PWSTR(wchar::to_wide_chars(self.printer.os_name()).as_mut_ptr()),
                None,
                ptr::null(),
            )
            .ok()
            .map_err(|_| FilePrinterError::FailedToOpenPrinter)?;
            defer! {
                DeleteDC(created_hdc_print);
            }
            let hdc_print = HDC(created_hdc_print.0);
            let mut doc_name = wchar::to_wide_chars(path.file_name().unwrap_or(path.as_ref()));
            let doc_info = DOCINFOW {
                cbSize: mem::size_of::<DOCINFOW>() as i32,
                fwType: 0,
                lpszDocName: PWSTR(doc_name.as_mut_ptr()),
                lpszOutput: PWSTR::default(),
                lpszDatatype: PWSTR::default(),
            };
            StartDocW(hdc_print, &doc_info);
            {
                let _pdfium_guard = PdfiumGuard::guard();
                let mut file = File::open(path).map_err(|_| FilePrinterError::FailedToCreateJob)?;
                let mut file_delegation = PdfiumCustomDocument::new(&mut file)
                    .map_err(|_| FilePrinterError::FailedToOpenPrinter)?;
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
                    let get_attr = |kind: u32| -> i32 {
                        GetDeviceCaps(hdc_print, GET_DEVICE_CAPS_INDEX(kind))
                    };
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
                    SetViewportOrgEx(hdc_print, org_x, org_y, ptr::null_mut());
                    FPDF_RenderPage(hdc_print, page, 0, 0, w as i32, h as i32, 0, FPDF_PRINTING);
                    EndPage(hdc_print);
                }
            }
            EndDoc(hdc_print);
        }
        Ok(())
    }
}
