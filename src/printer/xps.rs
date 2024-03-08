use crate::printer::FilePrinter;
use crate::printer::PrinterInfo;
use crate::utils::wchar;
use scopeguard::defer;
use std::path::Path;
use std::ptr;
use thiserror::Error;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::CloseHandle,
        Storage::Xps::{Printing::*, *},
        System::{Com::*, Threading::*},
    },
};

#[derive(Error, Debug)]
pub enum XpsPrinterError {
    #[error("Failed to create event: {0}")]
    FailedToCreateEvent(windows::core::Error),
    #[error("Failed to create object factory: {0}")]
    FailedToCreateObjectFactory(windows::core::Error),
    #[error("Failed to start job: {0}")]
    FailedToStartJob(windows::core::Error),
    #[error("Failed to write document: {0}")]
    FailedToWriteDocument(windows::core::Error),
    #[error("Stream is not available")]
    StreamNotAvailable,
}

pub struct XpsPrinter {
    printer: PrinterInfo,
}

impl XpsPrinter {
    pub fn new(printer: PrinterInfo) -> Self {
        Self { printer }
    }
}

impl FilePrinter for XpsPrinter {
    type Options = ();
    type Error = XpsPrinterError;
    fn print(&self, path: &Path, _options: ()) -> std::result::Result<(), XpsPrinterError> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            defer! {
                CoUninitialize();
            }
            let event = CreateEventW(None, true, false, None)
                .map_err(XpsPrinterError::FailedToCreateEvent)?;
            defer! {
                let _ = CloseHandle(event);
            }
            let xps_factory: IXpsOMObjectFactory =
                CoCreateInstance(&XpsOMObjectFactory, None, CLSCTX_ALL)
                    .map_err(XpsPrinterError::FailedToCreateObjectFactory)?;
            let mut document_stream = Option::<IXpsPrintJobStream>::None;
            StartXpsPrintJob(
                PCWSTR(wchar::to_wide_chars(self.printer.os_name()).as_ptr()),
                PCWSTR(wchar::to_wide_chars(path.file_name().unwrap_or(path.as_ref())).as_ptr()),
                None,
                None,
                event,
                &[],
                ptr::null_mut(),
                ptr::addr_of_mut!(document_stream),
                ptr::null_mut(),
            )
            .map_err(XpsPrinterError::FailedToStartJob)?;
            let document_stream = document_stream.ok_or(XpsPrinterError::StreamNotAvailable)?;
            let xps_package = xps_factory
                .CreatePackageFromFile(PCWSTR(wchar::to_wide_chars(path).as_ptr()), false)
                .map_err(XpsPrinterError::FailedToWriteDocument)?;
            xps_package
                .WriteToStream(&document_stream, false)
                .map_err(XpsPrinterError::FailedToWriteDocument)?;
            document_stream
                .Close()
                .map_err(XpsPrinterError::FailedToWriteDocument)?;
            WaitForSingleObject(event, INFINITE);
        }
        Ok(())
    }
}
