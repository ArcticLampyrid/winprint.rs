use super::{
    document::{writer::XmlDocumentRoot, PrintTicketDocument},
    DEFAULT_PRINT_TICKET_XML,
};
use crate::{
    printer::PrinterDevice,
    utils::{stream::read_com_stream, wchar},
};
use scopeguard::defer;
use std::ptr;
use thiserror::Error;
use windows::{
    core::{BSTR, PCWSTR},
    Win32::{
        Graphics::{
            Gdi::{DEVMODEA, DEVMODEW, DM_IN_BUFFER, DM_OUT_BUFFER},
            Printing::{
                ClosePrinter, DocumentPropertiesW, OpenPrinterW,
                PrintTicket::{
                    kPTJobScope, kUserDefaultDevmode, PTCloseProvider,
                    PTConvertDevModeToPrintTicket, PTConvertPrintTicketToDevMode, PTOpenProvider,
                    PTReleaseMemory,
                },
                PRINTER_HANDLE,
            },
        },
        UI::{Shell::SHCreateMemStream, WindowsAndMessaging::IDOK},
    },
};

#[derive(Clone, Debug)]
/// Represents a print ticket.
pub struct PrintTicket {
    pub(crate) xml: Vec<u8>,
}

impl Default for PrintTicket {
    fn default() -> Self {
        Self {
            xml: DEFAULT_PRINT_TICKET_XML.into(),
        }
    }
}

#[derive(Error, Debug)]
/// Represents an error occurred while converting print ticket to dev mode.
pub enum ToDevModeError {
    /// Failed to open print ticket provider.
    #[error("Failed to open print ticket provider")]
    OpenProviderFailed(#[source] windows::core::Error),
    /// Stream not allocated.
    #[error("Stream not allocated")]
    StreamNotAllocated,
    /// Failed to convert print ticket to dev mode.
    #[error("Failed to convert print ticket to dev mode")]
    ConvertPrintTicketToDevModeFailed(String, #[source] windows::core::Error),
    /// Failed to open printer.
    #[error("Failed to open printer")]
    FailedToOpenPrinter(#[source] windows::core::Error),
    /// Failed to correct dev mode via [`DocumentProperties`].
    ///
    /// [`DocumentProperties`]: https://learn.microsoft.com/en-us/windows/win32/printdocs/documentproperties
    #[error("Failed to correct dev mode via DocumentProperties")]
    FailedToCorrectDevMode,
}

#[derive(Error, Debug)]
/// Represents an error occurred while converting DEVMODE to a print ticket.
pub enum FromDevModeError {
    /// Failed to open printer.
    #[error("Failed to open printer")]
    FailedToOpenPrinter(#[source] windows::core::Error),
    /// Failed to read the printer's default DEVMODE.
    #[error("Failed to read the printer's default DEVMODE")]
    FailedToReadDevMode,
    /// Failed to open print ticket provider.
    #[error("Failed to open print ticket provider")]
    OpenProviderFailed(#[source] windows::core::Error),
    /// Stream not allocated.
    #[error("Stream not allocated")]
    StreamNotAllocated,
    /// Failed to convert DEVMODE to a print ticket.
    #[error("Failed to convert DEVMODE to a print ticket")]
    ConvertDevModeToPrintTicketFailed(#[source] windows::core::Error),
    /// Failed to read the print ticket.
    #[error("Failed to read the print ticket")]
    ReadPrintTicketFailed(#[source] windows::core::Error),
    /// The provider returned an empty print ticket.
    #[error("The provider returned an empty print ticket")]
    EmptyPrintTicket,
}

impl PrintTicket {
    /// Create a new print ticket from the given XML.
    pub fn from_xml(xml: impl Into<Vec<u8>>) -> Self {
        Self { xml: xml.into() }
    }

    /// Into the XML of the print ticket.
    pub fn into_xml(self) -> Vec<u8> {
        self.xml
    }

    /// Get the XML of the print ticket.
    pub fn get_xml(&self) -> &[u8] {
        &self.xml
    }

    /// Create a print ticket from the printer's default DEVMODE.
    ///
    /// The resulting ticket contains the driver's default features and
    /// namespace declarations.
    pub fn default_for(device: &PrinterDevice) -> Result<Self, FromDevModeError> {
        let dev_mode = default_dev_mode(device)?;
        Self::from_dev_mode(device, &dev_mode)
    }

    /// Create a print ticket from DEVMODE data.
    ///
    /// `dev_mode` must contain a complete `DEVMODEW`, including its driver
    /// extra data.
    pub fn from_dev_mode(
        device: &PrinterDevice,
        dev_mode: &[u8],
    ) -> Result<Self, FromDevModeError> {
        unsafe {
            let provider =
                PTOpenProvider(PCWSTR(wchar::to_wide_chars(device.os_name()).as_ptr()), 1)
                    .map_err(FromDevModeError::OpenProviderFailed)?;
            defer! {
                let _ = PTCloseProvider(provider);
            }

            let stream = SHCreateMemStream(None).ok_or(FromDevModeError::StreamNotAllocated)?;
            // `prntvpt.h` takes a `PDEVMODE`; the Windows metadata spells it
            // `DEVMODEA`, although the buffer uses the Unicode DEVMODEW layout.
            PTConvertDevModeToPrintTicket(
                provider,
                dev_mode.len() as u32,
                dev_mode.as_ptr() as *const DEVMODEA,
                kPTJobScope,
                &stream,
            )
            .map_err(FromDevModeError::ConvertDevModeToPrintTicketFailed)?;

            let xml = read_com_stream(&stream).map_err(FromDevModeError::ReadPrintTicketFailed)?;
            if xml.is_empty() {
                return Err(FromDevModeError::EmptyPrintTicket);
            }
            Ok(Self { xml })
        }
    }

    /// Convert the print ticket to [`DEVMODE`] data.
    ///
    /// [`DEVMODE`]: https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-devmodew
    pub fn to_dev_mode(&self, device: &PrinterDevice) -> Result<Vec<u8>, ToDevModeError> {
        unsafe {
            let provider =
                PTOpenProvider(PCWSTR(wchar::to_wide_chars(device.os_name()).as_ptr()), 1)
                    .map_err(ToDevModeError::OpenProviderFailed)?;
            defer! {
                let _ = PTCloseProvider(provider);
            }

            let stream = SHCreateMemStream(Some(self.get_xml()))
                .ok_or(ToDevModeError::StreamNotAllocated)?;

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

            let printer_name = wchar::to_wide_chars(device.os_name());
            let printer_handle = {
                let mut printer_handle = PRINTER_HANDLE::default();
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
                None,
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
                None,
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

fn default_dev_mode(device: &PrinterDevice) -> Result<Vec<u8>, FromDevModeError> {
    unsafe {
        let printer_name = wchar::to_wide_chars(device.os_name());
        let mut printer_handle = PRINTER_HANDLE::default();
        OpenPrinterW(
            PCWSTR(printer_name.as_ptr()),
            ptr::addr_of_mut!(printer_handle),
            None,
        )
        .map_err(FromDevModeError::FailedToOpenPrinter)?;
        defer! {
            let _ = ClosePrinter(printer_handle);
        }

        let dev_mode_size = DocumentPropertiesW(
            None,
            printer_handle,
            PCWSTR(printer_name.as_ptr()),
            None,
            None,
            0,
        );
        if dev_mode_size <= 0 {
            return Err(FromDevModeError::FailedToReadDevMode);
        }

        let mut dev_mode = vec![0u8; dev_mode_size as usize];
        if DocumentPropertiesW(
            None,
            printer_handle,
            PCWSTR(printer_name.as_ptr()),
            Some(dev_mode.as_mut_ptr() as *mut DEVMODEW),
            None,
            DM_OUT_BUFFER.0,
        ) != IDOK.0
        {
            return Err(FromDevModeError::FailedToReadDevMode);
        }

        Ok(dev_mode)
    }
}

impl From<PrintTicketDocument> for PrintTicket {
    fn from(value: PrintTicketDocument) -> Self {
        Self {
            xml: value.to_xml(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{default_dev_mode, PrintTicket};
    use crate::test_utils::null_device;

    #[test]
    fn default_for_returns_the_driver_default_ticket() {
        let device = null_device::thread_local();
        let ticket = PrintTicket::default_for(&device).unwrap();
        let xml = String::from_utf8_lossy(ticket.get_xml());
        assert!(
            xml.contains("<psf:Feature "),
            "the default ticket should contain at least one feature: {xml}"
        );
    }

    #[test]
    fn from_dev_mode_returns_a_print_ticket() {
        let device = null_device::thread_local();
        let dev_mode = default_dev_mode(&device).unwrap();
        let ticket = PrintTicket::from_dev_mode(&device, &dev_mode).unwrap();
        assert!(!ticket.get_xml().is_empty());
    }
}
