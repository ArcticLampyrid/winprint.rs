use super::DEFAULT_PRINT_TICKET_XML;
use crate::{printer::PrinterInfo, utils::wchar};
use scopeguard::defer;
use std::ptr;
use thiserror::Error;
use windows::{
    core::{BSTR, PCWSTR},
    Win32::{
        Foundation::{HANDLE, HWND},
        Graphics::{
            Gdi::{DM_IN_BUFFER, DM_OUT_BUFFER},
            Printing::{
                ClosePrinter, DocumentPropertiesW, OpenPrinterW,
                PrintTicket::{
                    kPTJobScope, kUserDefaultDevmode, PTCloseProvider,
                    PTConvertPrintTicketToDevMode, PTOpenProvider, PTReleaseMemory,
                },
            },
        },
        UI::{Shell::SHCreateMemStream, WindowsAndMessaging::IDOK},
    },
};

#[derive(Clone, Debug)]
pub struct PrintTicket {
    pub(crate) xml: String,
}

impl Default for PrintTicket {
    fn default() -> Self {
        Self {
            xml: DEFAULT_PRINT_TICKET_XML.to_string(),
        }
    }
}

#[derive(Error, Debug)]
pub enum ToDevModeError {
    #[error("Failed to open print ticket provider: {0}")]
    OpenProviderFailed(windows::core::Error),
    #[error("Stream not allocated")]
    StreamNotAllocated,
    #[error("Failed to convert print ticket to dev mode: {0}")]
    ConvertPrintTicketToDevModeFailed(String, windows::core::Error),
    #[error("Failed to open printer: {0}")]
    FailedToOpenPrinter(windows::core::Error),
    #[error("Failed to correct dev mode via DocumentProperties")]
    FailedToCorrectDevMode,
}

impl PrintTicket {
    pub fn into_xml(self) -> String {
        self.xml
    }

    pub fn get_xml(&self) -> &str {
        &self.xml
    }

    pub fn to_dev_mode(&self, info: &PrinterInfo) -> Result<Vec<u8>, ToDevModeError> {
        unsafe {
            let provider = PTOpenProvider(PCWSTR(wchar::to_wide_chars(info.os_name()).as_ptr()), 1)
                .map_err(ToDevModeError::OpenProviderFailed)?;
            defer! {
                let _ = PTCloseProvider(provider);
            }

            let xml = self.get_xml();
            let stream =
                SHCreateMemStream(Some(xml.as_ref())).ok_or(ToDevModeError::StreamNotAllocated)?;

            let mut dev_mode_size = 0;
            let mut dev_mode_data = std::ptr::null_mut();
            let mut error_message = BSTR::default();
            PTConvertPrintTicketToDevMode(
                provider,
                &stream,
                kUserDefaultDevmode,
                kPTJobScope,
                ptr::addr_of_mut!(dev_mode_size),
                ptr::addr_of_mut!(dev_mode_data),
                Some(&mut error_message),
            )
            .map_err(|win32_error| {
                ToDevModeError::ConvertPrintTicketToDevModeFailed(
                    error_message.to_string(),
                    win32_error,
                )
            })?;
            defer! {
                let _ = PTReleaseMemory(dev_mode_data as *mut _);
            }

            let printer_name = wchar::to_wide_chars(info.os_name());
            let printer_handle = {
                let mut printer_handle = HANDLE::default();
                OpenPrinterW(
                    PCWSTR(printer_name.as_ptr()),
                    ptr::addr_of_mut!(printer_handle),
                    None,
                )
                .map_err(ToDevModeError::FailedToOpenPrinter)?;
                printer_handle
            };
            defer! {
                let _ = ClosePrinter(printer_handle);
            }

            let mut buffer_size = DocumentPropertiesW(
                HWND::default(),
                printer_handle,
                PCWSTR(printer_name.as_ptr()),
                None,
                None,
                0,
            );

            // Workaround for buggy printer drivers
            // See also: https://chromium.googlesource.com/chromium/src/+/refs/tags/124.0.6347.1/printing/backend/win_helper.cc#586
            buffer_size = buffer_size * 2 + 8192;

            let mut buffer = vec![0u8; buffer_size as usize];
            if DocumentPropertiesW(
                HWND::default(),
                printer_handle,
                PCWSTR(printer_name.as_ptr()),
                Some(buffer.as_mut_ptr() as *mut _),
                Some(dev_mode_data as *mut _),
                (DM_IN_BUFFER | DM_OUT_BUFFER).0,
            ) != IDOK.0
            {
                return Err(ToDevModeError::FailedToCorrectDevMode);
            }

            Ok(buffer)
        }
    }
}
