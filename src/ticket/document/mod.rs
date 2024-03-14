mod print_schema;
pub mod reader;
pub mod writer;

pub use print_schema::*;

pub const NS_PSF: &str =
    "http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework";
pub const NS_PSK: &str =
    "http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords";
pub const NS_XSD: &str = "http://www.w3.org/2001/XMLSchema";
pub const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
