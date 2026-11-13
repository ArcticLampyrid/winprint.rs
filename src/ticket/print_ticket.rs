use super::{
    document::{writer::XmlDocumentRoot, PrintTicketDocument},
    DEFAULT_PRINT_TICKET_XML,
};
use crate::{
    printer::PrinterDevice,
    utils::{stream::read_com_stream, wchar},
};
use scopeguard::defer;
use std::{
    mem::{offset_of, size_of},
    ptr,
};
use thiserror::Error;
use windows::{
    core::{BSTR, PCWSTR},
    Win32::{
        Foundation::E_INVALIDARG,
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
    /// extra data. Bytes after the `DEVMODEW` are ignored, so the data from
    /// [`PrintTicket::to_dev_mode`] is accepted without trimming.
    ///
    /// # Errors
    ///
    /// Returns [`FromDevModeError::ConvertDevModeToPrintTicketFailed`] with
    /// `E_INVALIDARG` if `dev_mode` is shorter than its `dmSize` and
    /// `dmDriverExtra` members declare, or if `dmSize` does not cover the
    /// members up to `dmFields`. The message of that error is
    /// `Incomplete DEVMODE data`, unless the build sets
    /// `--cfg windows_slim_errors`.
    pub fn from_dev_mode(
        device: &PrinterDevice,
        dev_mode: &[u8],
    ) -> Result<Self, FromDevModeError> {
        let dev_mode_size = dev_mode_size(dev_mode).ok_or_else(|| {
            FromDevModeError::ConvertDevModeToPrintTicketFailed(windows::core::Error::new(
                E_INVALIDARG,
                INCOMPLETE_DEV_MODE_MESSAGE,
            ))
        })?;
        // The provider reads `DEVMODEW` members, and a byte slice has no
        // alignment. The zero padding up to a whole `DEVMODEW` keeps the
        // members of a shorter structure version inside the buffer.
        let mut aligned_dev_mode =
            vec![DEVMODEW::default(); dev_mode_size.div_ceil(size_of::<DEVMODEW>())];
        unsafe {
            ptr::copy_nonoverlapping(
                dev_mode.as_ptr(),
                aligned_dev_mode.as_mut_ptr().cast::<u8>(),
                dev_mode_size,
            );

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
                dev_mode_size as u32,
                aligned_dev_mode.as_ptr() as *const DEVMODEA,
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

/// The message of the error for DEVMODE data that fails the size check.
const INCOMPLETE_DEV_MODE_MESSAGE: &str = "Incomplete DEVMODE data";

/// The `dmSize` that covers the `DEVMODEW` members up to `dmFields`.
const MIN_DEV_MODE_PUBLIC_SIZE: usize = offset_of!(DEVMODEW, dmFields) + size_of::<u32>();

/// Get the size of the `DEVMODEW` at the start of `dev_mode`, including its
/// driver extra data.
///
/// Returns `None` if `dmSize` is too small or `dev_mode` is shorter than
/// `dmSize + dmDriverExtra`.
fn dev_mode_size(dev_mode: &[u8]) -> Option<usize> {
    let read_u16 = |offset: usize| {
        dev_mode
            .get(offset..offset + size_of::<u16>())
            .map(|bytes| usize::from(u16::from_ne_bytes([bytes[0], bytes[1]])))
    };
    let public_size = read_u16(offset_of!(DEVMODEW, dmSize))?;
    let size = public_size + read_u16(offset_of!(DEVMODEW, dmDriverExtra))?;
    (public_size >= MIN_DEV_MODE_PUBLIC_SIZE && size <= dev_mode.len()).then_some(size)
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

        // Some drivers report a size smaller than `dmSize + dmDriverExtra`.
        // See also: https://chromium.googlesource.com/chromium/src/+/refs/tags/124.0.6347.1/printing/backend/win_helper.cc#586
        let mut dev_mode = vec![0u8; dev_mode_size as usize * 2 + 8192];
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
    use super::{
        default_dev_mode, dev_mode_size, FromDevModeError, PrintTicket, INCOMPLETE_DEV_MODE_MESSAGE,
    };
    use crate::test_utils::null_device;
    use std::mem::{offset_of, size_of};
    use windows::Win32::{Foundation::E_INVALIDARG, Graphics::Gdi::DEVMODEW};

    const LANDSCAPE_TICKET: &str = r#"<psf:PrintTicket xmlns:psf="http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework" version="1" xmlns:psk="http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords">
    <psf:Feature name="psk:PageOrientation">
        <psf:Option name="psk:Landscape" />
    </psf:Feature>
</psf:PrintTicket>"#;

    /// Build `len` zero bytes that declare the given `dmSize` and `dmDriverExtra`.
    fn dev_mode_bytes(public_size: u16, driver_extra: u16, len: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; len];
        [
            (offset_of!(DEVMODEW, dmSize), public_size),
            (offset_of!(DEVMODEW, dmDriverExtra), driver_extra),
        ]
        .into_iter()
        .filter(|(offset, _)| offset + size_of::<u16>() <= len)
        .for_each(|(offset, value)| {
            bytes[offset..offset + size_of::<u16>()].copy_from_slice(&value.to_ne_bytes())
        });
        bytes
    }

    #[test]
    fn dev_mode_size_includes_the_driver_extra_data() {
        assert_eq!(dev_mode_size(&dev_mode_bytes(220, 0, 220)), Some(220));
        assert_eq!(dev_mode_size(&dev_mode_bytes(220, 100, 320)), Some(320));
        assert_eq!(dev_mode_size(&dev_mode_bytes(220, 100, 9000)), Some(320));
    }

    #[test]
    fn dev_mode_size_is_none_without_the_size_members() {
        assert_eq!(dev_mode_size(&[]), None);
        assert_eq!(dev_mode_size(&dev_mode_bytes(220, 0, 71)), None);
    }

    #[test]
    fn dev_mode_size_is_none_for_truncated_data() {
        assert_eq!(dev_mode_size(&dev_mode_bytes(220, 0, 219)), None);
        assert_eq!(dev_mode_size(&dev_mode_bytes(220, 100, 319)), None);
    }

    #[test]
    fn dev_mode_size_is_none_for_a_too_small_dm_size() {
        assert_eq!(dev_mode_size(&dev_mode_bytes(75, 0, 220)), None);
        assert_eq!(dev_mode_size(&dev_mode_bytes(0, 220, 220)), None);
        assert_eq!(dev_mode_size(&dev_mode_bytes(76, 0, 220)), Some(76));
    }

    #[test]
    fn from_dev_mode_accepts_unaligned_data_from_to_dev_mode() {
        let device = null_device::thread_local();
        let dev_mode = PrintTicket::from_xml(LANDSCAPE_TICKET)
            .to_dev_mode(&device)
            .unwrap();
        let unaligned = [&[0u8][..], &dev_mode].concat();
        let ticket = PrintTicket::from_dev_mode(&device, &unaligned[1..]).unwrap();
        let xml = String::from_utf8_lossy(ticket.get_xml());
        assert!(
            xml.contains("psk:Landscape"),
            "the ticket should keep the orientation: {xml}"
        );
    }

    #[test]
    fn from_dev_mode_refuses_truncated_data() {
        let device = null_device::thread_local();
        let dev_mode = default_dev_mode(&device).unwrap();
        let size = dev_mode_size(&dev_mode).unwrap();
        let error = PrintTicket::from_dev_mode(&device, &dev_mode[..size - 1]).unwrap_err();
        // A build with `--cfg windows_slim_errors` drops the message.
        let expected = windows::core::Error::new(E_INVALIDARG, INCOMPLETE_DEV_MODE_MESSAGE);
        assert!(
            matches!(
                &error,
                FromDevModeError::ConvertDevModeToPrintTicketFailed(e)
                    if e.code() == E_INVALIDARG && e.message() == expected.message()
            ),
            "truncated data should fail the size check: {error:?}"
        );
    }

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
