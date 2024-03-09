mod builder;
pub use builder::*;
mod print_capabilities;
pub use print_capabilities::*;
mod print_ticket;
pub use print_ticket::*;
pub const DEFAULT_PRINT_TICKET_XML: &str = r#"<psf:PrintTicket xmlns:psf="http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" version="1" xmlns:psk="http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords"></psf:PrintTicket>"#;
