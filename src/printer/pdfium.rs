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
        CreateDCW, CreateEnhMetaFileW, CloseEnhMetaFile, DeleteDC, DeleteEnhMetaFile,
        GetDeviceCaps, PlayEnhMetaFile, SetBrushOrgEx, SetGraphicsMode, SetStretchBltMode,
        SetViewportOrgEx, GET_DEVICE_CAPS_INDEX, GM_ADVANCED, HALFTONE, LOGPIXELSX, LOGPIXELSY,
        PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH,
    },
    Win32::Foundation::RECT,
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
    /// StartDoc failed.
    #[error("StartDocW failed (returned {0})")]
    StartDocFailed(i32),
    /// StartPage failed.
    #[error("StartPage failed (returned {0})")]
    StartPageFailed(i32),
    /// EndPage failed.
    #[error("EndPage failed (returned {0})")]
    EndPageFailed(i32),
    /// EndDoc failed.
    #[error("EndDoc failed (returned {0})")]
    EndDocFailed(i32),
    /// EMF creation failed.
    #[error("CreateEnhMetaFile failed for page {0}")]
    EmfCreateFailed(i32),
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
            // Use "WINSPOOL" as driver name, matching Chromium's approach.
            // See: chromium/printing/printing_context_win.cc InitializeSettings()
            let driver_name = wchar::to_wide_chars("WINSPOOL");
            let hdc_print = CreateDCW(
                PCWSTR(driver_name.as_ptr()),
                PCWSTR(wchar::to_wide_chars(self.printer.os_name()).as_ptr()),
                None,
                Some(dev_mode.as_ptr() as *const _),
            );
            if hdc_print.is_invalid() {
                return Err(PdfiumPrinterError::FailedToOpenPrinter);
            }
            defer! {
                let _ = DeleteDC(hdc_print);
            }
            // Initialize DC the same way Chromium does (skia::InitializeDC):
            // See: chromium/skia/ext/skia_utils_win.cc InitializeDC()
            SetGraphicsMode(hdc_print, GM_ADVANCED);
            SetStretchBltMode(hdc_print, HALFTONE);
            let _ = SetBrushOrgEx(hdc_print, 0, 0, None);
            let mut doc_name = wchar::to_wide_chars(path.file_name().unwrap_or(path.as_ref()));
            let doc_info = DOCINFOW {
                cbSize: mem::size_of::<DOCINFOW>() as i32,
                fwType: 0,
                lpszDocName: PCWSTR(doc_name.as_mut_ptr()),
                lpszOutput: PCWSTR::null(),
                lpszDatatype: PCWSTR::null(),
            };

            // Windows printing pipeline as Chromium:
            //
            // 1. Render each page as EMF (metafile)
            // 2. StartPage → metafile->SafePlayback(context_) (= PlayEnhMetaFile) → EndPage
            //
            // See chromium/printing/printing_context_win.cc:422:
            // bool played_back = page.metafile()->SafePlayback(context_);
            //
            // See chromium/printing/emf_win.cc:122:
            // bool Emf::Playback(HDC hdc, const RECT* rect) const {
            //     return PlayEnhMetaFile(hdc, emf_, rect) != 0;
            // }
            let start_doc_ret = StartDocW(hdc_print, &doc_info);
            if start_doc_ret <= 0 {
                return Err(PdfiumPrinterError::StartDocFailed(start_doc_ret));
            }

            {
                let _pdfium_guard = PdfiumGuard::guard();
                let mut file = File::open(path).map_err(PdfiumPrinterError::FileIOError)?;
                let mut file_delegation = PdfiumCustomDocument::new(&mut file)
                    .map_err(PdfiumPrinterError::FileIOError)?;
                let document = FPDF_LoadCustomDocument(file_delegation.as_mut(), ptr::null());
                if document.is_null() {
                    EndDoc(hdc_print);
                    return Err(PdfiumPrinterError::FileIOError(
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("PDFium load failed: error={}", FPDF_GetLastError()),
                        ),
                    ));
                }
                defer! {
                    FPDF_CloseDocument(document);
                }
                let page_count = FPDF_GetPageCount(document);

                let get_attr =
                    |kind: GET_DEVICE_CAPS_INDEX| -> i32 { GetDeviceCaps(hdc_print, kind) };
                let dpi_x = get_attr(LOGPIXELSX);
                let dpi_y = get_attr(LOGPIXELSY);
                let physical_width = get_attr(PHYSICALWIDTH);
                let physical_height = get_attr(PHYSICALHEIGHT);
                let org_x = -get_attr(PHYSICALOFFSETX);
                let org_y = -get_attr(PHYSICALOFFSETY);

                for page_index in 0..page_count {
                    let page = FPDF_LoadPage(document, page_index);
                    defer! {
                        FPDF_ClosePage(page);
                    }

                    let page_width = FPDF_GetPageWidth(page) * dpi_x as f64 / 72.0;
                    let page_height = FPDF_GetPageHeight(page) * dpi_y as f64 / 72.0;
                    let scale = f64::min(
                        physical_width as f64 / page_width,
                        physical_height as f64 / page_height,
                    );
                    let w = (page_width * scale) as i32;
                    let h = (page_height * scale) as i32;

                    Self::render_page_emf(
                        hdc_print, page, page_index,
                        w, h, org_x, org_y, dpi_x, dpi_y,
                    )?;
                }
            }

            let end_doc_ret = EndDoc(hdc_print);
            if end_doc_ret <= 0 {
                return Err(PdfiumPrinterError::EndDocFailed(end_doc_ret));
            }
        }
        Ok(())
    }
}

impl PdfiumPrinter {
    /// Print page via EMF
    unsafe fn render_page_emf(
        hdc_print: windows::Win32::Graphics::Gdi::HDC,
        page: FPDF_PAGE,
        page_index: i32,
        w: i32, h: i32,
        org_x: i32, org_y: i32,
        dpi_x: i32, dpi_y: i32,
    ) -> Result<(), PdfiumPrinterError> {
        // EMF の座標系は 0.01mm 単位
        let emf_rect = RECT {
            left: 0,
            top: 0,
            right: (w as f64 * 2540.0 / dpi_x as f64) as i32,
            bottom: (h as f64 * 2540.0 / dpi_y as f64) as i32,
        };
        let hdc_emf = CreateEnhMetaFileW(hdc_print, None, Some(&emf_rect), None);
        if hdc_emf.is_invalid() {
            EndDoc(hdc_print);
            return Err(PdfiumPrinterError::EmfCreateFailed(page_index));
        }

        SetGraphicsMode(hdc_emf, GM_ADVANCED);
        FPDF_RenderPage(hdc_emf, page, 0, 0, w, h, 0, FPDF_PRINTING);

        let hemf = CloseEnhMetaFile(hdc_emf);
        if hemf.0.is_null() {
            EndDoc(hdc_print);
            return Err(PdfiumPrinterError::EmfCreateFailed(page_index));
        }
        defer! {
            let _ = DeleteEnhMetaFile(hemf);
        }

        let start_page_ret = StartPage(hdc_print);
        if start_page_ret <= 0 {
            EndDoc(hdc_print);
            return Err(PdfiumPrinterError::StartPageFailed(start_page_ret));
        }

        let _ = SetViewportOrgEx(hdc_print, org_x, org_y, None);
        let play_rect = RECT { left: 0, top: 0, right: w, bottom: h };
        let _ = PlayEnhMetaFile(hdc_print, hemf, &play_rect);

        let end_page_ret = EndPage(hdc_print);
        if end_page_ret <= 0 {
            EndDoc(hdc_print);
            return Err(PdfiumPrinterError::EndPageFailed(end_page_ret));
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
