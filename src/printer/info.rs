use crate::utils::wchar;
use crate::bindings::Windows::Win32::{Foundation::*, Graphics::Printing::*};
use std::ffi::{OsString, OsStr};
use std::ptr;
#[derive(Clone, Debug)]
pub struct PrinterInfo {
    name: String,
    os_server: OsString,
    os_name: OsString,
    os_attributes: u32,
}
impl PrinterInfo {
    pub fn all() -> Vec<Self> {
        let mut bytes_needed = 0;
        let mut count_returned = 0;
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let name = PWSTR::default();
        let level = 4;
        unsafe {
            EnumPrintersW(
                flags,
                name,
                level,
                ptr::null_mut(),
                0,
                &mut bytes_needed,
                &mut count_returned,
            )
        };
        let mut buffer = Vec::<u8>::with_capacity(bytes_needed as usize);
        unsafe {
            EnumPrintersW(
                flags,
                name,
                level,
                buffer.as_mut_ptr(),
                bytes_needed,
                &mut bytes_needed,
                &mut count_returned,
            )
        };
        unsafe { buffer.set_len(bytes_needed as usize) };
        let mut result = Vec::<PrinterInfo>::with_capacity(count_returned as usize);
        for i in 0..count_returned {
            let info = unsafe {
                std::ptr::read_unaligned(
                    (buffer.as_mut_ptr() as *const PRINTER_INFO_4W).offset(i as isize),
                )
            };
            let os_name = wchar::from_wide_ptr(info.pPrinterName.0);
            result.push(Self {
                name: os_name.to_string_lossy().into_owned(),
                os_server: wchar::from_wide_ptr(info.pServerName.0),
                os_attributes: info.Attributes,
                os_name
            })
        }
        result
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
