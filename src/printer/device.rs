use crate::utils::wchar;
use scopeguard::defer;
use std::alloc::{alloc, dealloc, Layout};
use std::ffi::{OsStr, OsString};
use std::mem;
use std::ptr;
use thiserror::Error;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER, ERROR_INVALID_PRINTER_NAME,
};
use windows::Win32::Graphics::Printing::*;
#[derive(Clone, Debug)]
/// Represents a printer device.
pub struct PrinterDevice {
    name: String,
    os_server: OsString,
    os_name: OsString,
    os_attributes: u32,
}

#[derive(Error, Debug)]
/// Represents an error occurred while enumerating printer devices.
pub enum EnumDeviceError {
    /// Failed to enum printer device.
    #[error("Failed to enum printer device")]
    FailedToEnumPrinterDevice(#[source] windows::core::Error),
}

#[derive(Error, Debug)]
/// Represents an error occurred while finding a printer device by its OS name.
pub enum FindDeviceError {
    /// There is no printer device with the given name.
    #[error("Printer not found")]
    NotFound,
    /// Failed to open printer.
    #[error("Failed to open printer")]
    FailedToOpenPrinter(#[source] windows::core::Error),
    /// Failed to read printer info.
    #[error("Failed to read printer info")]
    FailedToReadPrinterInfo(#[source] windows::core::Error),
}

#[derive(Error, Debug)]
/// Represents an error occurred while finding the user's default printer device.
pub enum DefaultDeviceError {
    /// Failed to get the name of the user's default printer.
    #[error("Failed to get the name of the user's default printer")]
    FailedToGetDefaultPrinter(#[source] windows::core::Error),
    /// Failed to find the user's default printer by its name.
    #[error("Failed to find the user's default printer")]
    FailedToFindPrinter(#[source] FindDeviceError),
}

impl PrinterDevice {
    /// Fetch all printer devices.
    pub fn all() -> Result<Vec<Self>, EnumDeviceError> {
        let mut bytes_needed: u32 = 0;
        let mut count_returned: u32 = 0;
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let name = PCWSTR::null();
        let level = 4;
        unsafe {
            let mut buffer: *mut u8 = ptr::null_mut();
            let mut buffer_size: usize = 0;

            loop {
                let slice = if buffer_size > 0 {
                    Some(std::slice::from_raw_parts_mut(buffer, buffer_size))
                } else {
                    None
                };
                let result = EnumPrintersW(
                    flags,
                    name,
                    level,
                    slice,
                    &mut bytes_needed,
                    &mut count_returned,
                );
                match result {
                    Ok(()) => break,
                    Err(e) if e.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() => {
                        let new_size = bytes_needed as usize;
                        let align = mem::align_of::<PRINTER_INFO_4W>();
                        if buffer_size > 0 {
                            dealloc(
                                buffer,
                                Layout::from_size_align_unchecked(buffer_size, align),
                            );
                        }
                        buffer = alloc(Layout::from_size_align_unchecked(new_size, align));
                        buffer_size = new_size;
                    }
                    Err(e) => {
                        if buffer_size > 0 {
                            dealloc(
                                buffer,
                                Layout::from_size_align_unchecked(
                                    buffer_size,
                                    mem::align_of::<PRINTER_INFO_4W>(),
                                ),
                            );
                        }
                        return Err(EnumDeviceError::FailedToEnumPrinterDevice(e));
                    }
                }
            }

            let mut result = Vec::<PrinterDevice>::with_capacity(count_returned as usize);
            for i in 0..count_returned {
                let info = &*(buffer as *const PRINTER_INFO_4W).offset(i as isize);
                let os_name = wchar::from_wide_ptr(info.pPrinterName.0);
                result.push(Self {
                    name: os_name.to_string_lossy().into_owned(),
                    os_server: wchar::from_wide_ptr(info.pServerName.0),
                    os_attributes: info.Attributes,
                    os_name,
                });
            }

            if buffer_size > 0 {
                dealloc(
                    buffer,
                    Layout::from_size_align_unchecked(
                        buffer_size,
                        mem::align_of::<PRINTER_INFO_4W>(),
                    ),
                );
            }

            Ok(result)
        }
    }

    /// Fetch the printer device with the given OS name, as accepted by [`OpenPrinter`].
    ///
    /// Fails with [`FindDeviceError::NotFound`] if there is no printer device with the given OS
    /// name.
    ///
    /// [`OpenPrinter`]: https://learn.microsoft.com/en-us/windows/win32/printdocs/openprinter
    pub fn by_os_name(name: impl AsRef<OsStr>) -> Result<Self, FindDeviceError> {
        let printer_name = wchar::to_wide_chars(name);
        let level = 4;
        let mut printer_handle = PRINTER_HANDLE::default();
        unsafe {
            OpenPrinterW(
                PCWSTR(printer_name.as_ptr()),
                ptr::addr_of_mut!(printer_handle),
                None,
            )
            .map_err(|e| {
                if e.code() == ERROR_INVALID_PRINTER_NAME.to_hresult() {
                    FindDeviceError::NotFound
                } else {
                    FindDeviceError::FailedToOpenPrinter(e)
                }
            })?;
            defer! {
                let _ = ClosePrinter(printer_handle);
            }

            // `u64` elements keep the buffer aligned for `PRINTER_INFO_4W`. It starts with room
            // for the record, so a success always leaves one in the buffer.
            const _: () = assert!(mem::align_of::<PRINTER_INFO_4W>() <= mem::align_of::<u64>());
            let mut buffer =
                vec![0u64; mem::size_of::<PRINTER_INFO_4W>().div_ceil(mem::size_of::<u64>())];
            loop {
                let buffer_size = buffer.len() * mem::size_of::<u64>();
                let slice = std::slice::from_raw_parts_mut(buffer.as_mut_ptr().cast(), buffer_size);
                let mut bytes_needed: u32 = 0;
                match GetPrinterW(printer_handle, level, Some(slice), &mut bytes_needed) {
                    Ok(()) => break,
                    Err(e)
                        if e.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult()
                            && bytes_needed as usize > buffer_size =>
                    {
                        buffer.resize((bytes_needed as usize).div_ceil(mem::size_of::<u64>()), 0);
                    }
                    Err(e) => return Err(FindDeviceError::FailedToReadPrinterInfo(e)),
                }
            }

            let info = &*buffer.as_ptr().cast::<PRINTER_INFO_4W>();
            let os_name = wchar::from_wide_ptr(info.pPrinterName.0);
            Ok(Self {
                name: os_name.to_string_lossy().into_owned(),
                os_server: wchar::from_wide_ptr(info.pServerName.0),
                os_attributes: info.Attributes,
                os_name,
            })
        }
    }

    /// Fetch the user's default printer device.
    ///
    /// Returns `Ok(None)` if the user has no default printer, or if it no longer exists.
    pub fn user_default() -> Result<Option<Self>, DefaultDeviceError> {
        let Some(os_name) = default_printer_name()? else {
            return Ok(None);
        };
        match Self::by_os_name(os_name) {
            Ok(device) => Ok(Some(device)),
            // Windows may keep a removed printer as the default.
            Err(FindDeviceError::NotFound) => Ok(None),
            Err(e) => Err(DefaultDeviceError::FailedToFindPrinter(e)),
        }
    }

    /// Get a reference to the printer device's name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Get the printer device's os attributes.
    pub fn os_attributes(&self) -> u32 {
        self.os_attributes
    }

    /// Get a reference to the printer device's os server.
    pub fn os_server(&self) -> &OsStr {
        self.os_server.as_ref()
    }

    /// Get a reference to the printer device's os name.
    pub fn os_name(&self) -> &OsStr {
        self.os_name.as_ref()
    }

    /// Check if the printer device is local.
    pub fn is_local(&self) -> bool {
        self.os_attributes & PRINTER_ATTRIBUTE_LOCAL != 0
    }

    /// Check if the printer device is remote.
    pub fn is_remote(&self) -> bool {
        self.os_attributes & PRINTER_ATTRIBUTE_NETWORK != 0
    }
}

/// Get the name of the user's default printer, if there is one.
fn default_printer_name() -> Result<Option<OsString>, DefaultDeviceError> {
    let mut buffer: Vec<u16> = Vec::new();
    loop {
        // `GetDefaultPrinter` counts characters, including the terminating null.
        let mut chars_needed = buffer.len() as u32;
        let buffer_ptr = if buffer.is_empty() {
            None
        } else {
            Some(PWSTR(buffer.as_mut_ptr()))
        };
        let result = unsafe { GetDefaultPrinterW(buffer_ptr, ptr::addr_of_mut!(chars_needed)) };
        match result.ok() {
            Ok(()) => return Ok(Some(wchar::from_wide_ptr(buffer.as_ptr()))),
            Err(e) if e.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() => {
                buffer.resize(chars_needed as usize, 0);
            }
            // The user has no default printer, which is not an error.
            Err(e) if e.code() == ERROR_FILE_NOT_FOUND.to_hresult() => return Ok(None),
            Err(e) => return Err(DefaultDeviceError::FailedToGetDefaultPrinter(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FindDeviceError, PrinterDevice};
    use crate::test_utils::null_device;

    #[test]
    fn fetch_printer_device() {
        let _devices = PrinterDevice::all().unwrap();
    }

    #[test]
    fn find_device_by_os_name() {
        let device = null_device::thread_local();
        let found = PrinterDevice::by_os_name(device.os_name()).unwrap();
        assert_eq!(found.name(), device.name());
        assert_eq!(found.os_name(), device.os_name());
        assert_eq!(found.os_server(), device.os_server());
        assert_eq!(found.os_attributes(), device.os_attributes());
    }

    #[test]
    fn find_missing_device() {
        let name = format!("winprint-missing-device-{}", std::process::id());
        let result = PrinterDevice::by_os_name(&name);
        assert!(
            matches!(result, Err(FindDeviceError::NotFound)),
            "unexpected result: {result:?}"
        );
    }

    #[test]
    fn find_user_default_device() {
        // Other tests add and remove printers concurrently, any of which Windows may make the
        // default, so the default may vanish at any time and must not be looked up again.
        if let Some(device) = PrinterDevice::user_default().unwrap() {
            assert!(!device.name().is_empty());
        }
    }

    #[test]
    fn test_printer_should_be_local() {
        let device = null_device::thread_local();
        assert!(device.is_local());
        assert_eq!(device.is_remote(), false);
        assert_eq!(device.os_server(), "");
        // PRINTER_ATTRIBUTE_LOCAL == 0x00000040
        assert!(device.os_attributes() & 0x00000040 != 0);
    }
}
