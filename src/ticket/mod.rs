mod builder;
/// Document object model representation of print schema.
pub mod document;
mod media_size_tuple;
mod page_media_size;
mod predefined_media_name;
mod print_capabilities;
mod print_ticket;

pub use builder::*;
pub use media_size_tuple::*;
pub use page_media_size::*;
pub use predefined_media_name::*;
pub use print_capabilities::*;
pub use print_ticket::*;

/// The default print ticket XML.
pub const DEFAULT_PRINT_TICKET_XML: &str = r#"<psf:PrintTicket xmlns:psf="http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" version="1" xmlns:psk="http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords"></psf:PrintTicket>"#;
