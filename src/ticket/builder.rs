use super::{PrintTicket, DEFAULT_PRINT_TICKET_XML};
use crate::{
    printer::PrinterInfo,
    utils::{stream::copy_com_stream_to_string, wchar},
};
use thiserror::Error;
use windows::{
    core::{BSTR, PCWSTR},
    Win32::{
        Graphics::Printing::PrintTicket::{
            kPTJobScope, PTCloseProvider, PTMergeAndValidatePrintTicket, PTOpenProvider,
        },
        Storage::Xps::HPTPROVIDER,
        UI::Shell::SHCreateMemStream,
    },
};

pub struct PrintTicketBuilder {
    xml: String,
    provider: HPTPROVIDER,
}

#[derive(Error, Debug)]
pub enum PrintTicketBuilderError {
    #[error("Failed to open print ticket provider: {0}")]
    OpenProviderFailed(windows::core::Error),
    #[error("Stream not allocated")]
    StreamNotAllocated,
    #[error("Failed to merge print tickets: {0}")]
    MergePrintTicketsFailed(String, windows::core::Error),
    #[error("Failed to decode print ticket: {0}")]
    DecodePrintTicketFailed(windows::core::Error),
}

impl PrintTicketBuilder {
    pub fn new(info: &PrinterInfo) -> Result<Self, PrintTicketBuilderError> {
        let provider = unsafe {
            PTOpenProvider(PCWSTR(wchar::to_wide_chars(info.os_name()).as_ptr()), 1)
                .map_err(PrintTicketBuilderError::OpenProviderFailed)?
        };
        Ok(Self {
            xml: DEFAULT_PRINT_TICKET_XML.to_string(),
            provider,
        })
    }

    pub fn merge(&mut self, xml: &str) -> Result<(), PrintTicketBuilderError> {
        unsafe {
            let base = SHCreateMemStream(Some(self.xml.as_ref()))
                .ok_or(PrintTicketBuilderError::StreamNotAllocated)?;
            let delta = SHCreateMemStream(Some(xml.as_ref()))
                .ok_or(PrintTicketBuilderError::StreamNotAllocated)?;
            let result =
                SHCreateMemStream(None).ok_or(PrintTicketBuilderError::StreamNotAllocated)?;
            let mut error_message = BSTR::default();
            PTMergeAndValidatePrintTicket(
                self.provider,
                &base,
                &delta,
                kPTJobScope,
                Some(&result),
                Some(&mut error_message),
            )
            .map_err(|win32_error| {
                PrintTicketBuilderError::MergePrintTicketsFailed(
                    error_message.to_string(),
                    win32_error,
                )
            })?;
            copy_com_stream_to_string(&mut self.xml, &result)
                .map_err(PrintTicketBuilderError::DecodePrintTicketFailed)?;
        }
        Ok(())
    }

    pub fn build(mut self) -> Result<PrintTicket, PrintTicketBuilderError> {
        let xml = std::mem::replace(&mut self.xml, String::with_capacity(0));
        Ok(PrintTicket { xml })
    }
}

impl Drop for PrintTicketBuilder {
    fn drop(&mut self) {
        unsafe {
            let _ = PTCloseProvider(self.provider);
        }
    }
}
