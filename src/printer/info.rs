use crate::utils::wchar;
use scopeguard::defer;
use std::alloc::{alloc, dealloc, Layout};
use std::ffi::{OsStr, OsString};
use std::mem;
use thiserror::Error;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Printing::*;
#[derive(Clone, Debug)]
pub struct PrinterInfo {
    name: String,
    os_server: OsString,
    os_name: OsString,
    os_attributes: u32,
}

#[derive(Error, Debug)]
pub enum EnumPrinterError {
    #[error("Failed to enum printer: {0}")]
    FailedToEnumPrinter(windows::core::Error),
}

impl PrinterInfo {
    pub fn all() -> Result<Vec<Self>, EnumPrinterError> {
        let mut bytes_needed = 0;
        let mut count_returned = 0;
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let name = PCWSTR::null();
        let level = 4;
        unsafe {
            let _ = EnumPrintersW(
                flags,
                name,
                level,
                None,
                &mut bytes_needed,
                &mut count_returned,
            );
            let buffer_layout = Layout::from_size_align_unchecked(
                bytes_needed as usize,
                mem::align_of::<PRINTER_INFO_4W>(),
            );
            let buffer = alloc(buffer_layout);
            defer! {
                dealloc(buffer, buffer_layout);
            }
            EnumPrintersW(
                flags,
                name,
                level,
                Some(std::slice::from_raw_parts_mut(
                    buffer,
                    bytes_needed as usize,
                )),
                &mut bytes_needed,
                &mut count_returned,
            )
            .map_err(EnumPrinterError::FailedToEnumPrinter)?;
            let mut result = Vec::<PrinterInfo>::with_capacity(count_returned as usize);
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
            Ok(result)
        }
    }

    /// Get a reference to the printer info's name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Get the printer info's os attributes.
    pub fn os_attributes(&self) -> u32 {
        self.os_attributes
    }

    /// Get a reference to the printer info's os server.
    pub fn os_server(&self) -> &OsStr {
        self.os_server.as_ref()
    }

    /// Get a reference to the printer info's os name.
    pub fn os_name(&self) -> &OsStr {
        self.os_name.as_ref()
    }

    pub fn is_local(&self) -> bool {
        self.os_attributes & PRINTER_ATTRIBUTE_LOCAL != 0
    }

    pub fn is_remote(&self) -> bool {
        self.os_attributes & PRINTER_ATTRIBUTE_NETWORK != 0
    }
}
