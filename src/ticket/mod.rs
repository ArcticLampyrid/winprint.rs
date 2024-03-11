mod builder;
mod print_capabilities;
mod print_schema;
mod print_schema_parser;
mod print_schema_serializer;
mod print_ticket;

pub use builder::*;
pub use print_capabilities::*;
pub use print_schema::*;
pub use print_schema_parser::*;
pub use print_schema_serializer::*;
pub use print_ticket::*;

pub const DEFAULT_PRINT_TICKET_XML: &str = r#"<psf:PrintTicket xmlns:psf="http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" version="1" xmlns:psk="http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords"></psf:PrintTicket>"#;

pub const NS_PSF: &str =
    "http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework";
pub const NS_PSK: &str =
    "http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords";
pub const NS_XSD: &str = "http://www.w3.org/2001/XMLSchema";
pub const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
