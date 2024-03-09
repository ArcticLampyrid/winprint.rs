use crate::{
    printer::PrinterInfo,
    utils::{stream::read_com_stream, wchar},
};
use scopeguard::defer;
use std::fmt::Debug;
use thiserror::Error;
use windows::{
    core::{BSTR, PCWSTR},
    Win32::{
        Graphics::Printing::PrintTicket::{
            PTCloseProvider, PTGetPrintCapabilities, PTOpenProvider,
        },
        UI::Shell::SHCreateMemStream,
    },
};

pub const NS_PRINT_SCHEMA: &str =
    "http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework";

#[derive(Error, Debug)]
pub enum FetchPrintCapabilitiesError {
    #[error("Failed to open print ticket provider: {0}")]
    OpenProviderFailed(windows::core::Error),
    #[error("Stream not allocated")]
    StreamNotAllocated,
    #[error("Cannot get print capabilities: {0}")]
    CannotGetPrintCapabilities(String, windows::core::Error),
    #[error("Failed to read stream: {0}")]
    ReadStreamFailed(windows::core::Error),
}

#[derive(Clone, Debug)]
pub struct PrintCapabilities {}

impl PrintCapabilities {
    pub fn fetch_xml(info: &PrinterInfo) -> Result<Vec<u8>, FetchPrintCapabilitiesError> {
        unsafe {
            let provider = PTOpenProvider(PCWSTR(wchar::to_wide_chars(info.os_name()).as_ptr()), 1)
                .map_err(FetchPrintCapabilitiesError::OpenProviderFailed)?;
            defer! {
                let _ = PTCloseProvider(provider);
            }
            let stream =
                SHCreateMemStream(None).ok_or(FetchPrintCapabilitiesError::StreamNotAllocated)?;

            let mut error_message = BSTR::default();
            PTGetPrintCapabilities(provider, None, &stream, Some(&mut error_message)).map_err(
                |win32_error| {
                    FetchPrintCapabilitiesError::CannotGetPrintCapabilities(
                        error_message.to_string(),
                        win32_error,
                    )
                },
            )?;

            let data =
                read_com_stream(&stream).map_err(FetchPrintCapabilitiesError::ReadStreamFailed)?;

            Ok(data)
        }
    }
}
