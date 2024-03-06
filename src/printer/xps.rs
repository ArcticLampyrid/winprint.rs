use crate::printer::FilePrinter;
use crate::printer::FilePrinterError;
use crate::printer::PrinterInfo;
use crate::utils::wchar;
use scopeguard::defer;
use std::path::Path;
use std::ptr;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::CloseHandle,
        Storage::Xps::{Printing::*, *},
        System::{Com::*, Threading::*},
    },
};

pub struct XpsPrinter {
    printer: PrinterInfo,
}

impl XpsPrinter {
    pub fn new(printer: PrinterInfo) -> Self {
        Self { printer }
    }
}

impl FilePrinter for XpsPrinter {
    fn print(&self, path: &Path) -> std::result::Result<(), FilePrinterError> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            defer! {
                CoUninitialize();
            }
            let event = CreateEventW(None, true, false, None)
                .map_err(|_| FilePrinterError::FailedToCreateJob)?;
            defer! {
                let _ = CloseHandle(event);
            }
            let xps_factory: IXpsOMObjectFactory =
                CoCreateInstance(&XpsOMObjectFactory, None, CLSCTX_ALL)
                    .map_err(|_| FilePrinterError::FailedToOpenPrinter)?;
            let mut document_stream_container = Option::<IXpsPrintJobStream>::None;
            StartXpsPrintJob(
                PCWSTR(wchar::to_wide_chars(self.printer.os_name()).as_ptr()),
                PCWSTR(wchar::to_wide_chars(path.file_name().unwrap_or(path.as_ref())).as_ptr()),
                None,
                None,
                event,
                &[],
                ptr::null_mut(),
                ptr::addr_of_mut!(document_stream_container),
                ptr::null_mut(),
            )
            .map_err(|_| FilePrinterError::FailedToCreateJob)?;
            let document_stream =
                document_stream_container.ok_or(FilePrinterError::FailedToCreateJob)?;
            let xps_package = xps_factory
                .CreatePackageFromFile(PCWSTR(wchar::to_wide_chars(path).as_ptr()), false)
                .map_err(|_| FilePrinterError::FailedToWriteDocument)?;
            xps_package
                .WriteToStream(&document_stream, false)
                .map_err(|_| FilePrinterError::FailedToWriteDocument)?;
            document_stream
                .Close()
                .map_err(|_| FilePrinterError::FailedToWriteDocument)?;
            WaitForSingleObject(event, INFINITE);
        }
        Ok(())
    }
}
