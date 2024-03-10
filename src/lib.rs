mod bindings;
pub mod printer;
pub mod ticket;
mod utils;
#[cfg(test)]
mod tests {
    use crate::printer::FilePrinter;
    use crate::printer::PdfiumPrinter;
    use crate::printer::PrinterInfo;
    use crate::printer::XpsPrinter;
    use crate::ticket::PrintCapabilities;
    use crate::ticket::PrintTicket;
    use crate::ticket::PrintTicketBuilder;
    use ctor::{ctor, dtor};
    use std::path::Path;

    const TEST_PRINTER_NAME: &str = "winprint-rs-null-printer";

    #[ctor]
    fn setup() {
        env_logger::init();
        std::process::Command::new("rundll32")
            .args(&[
                "printui.dll,PrintUIEntry",
                "/dl",
                "/n",
                "winprint-rs-null-printer",
                "/q",
            ])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();

        std::process::Command::new("rundll32")
            .args(&[
                "printui.dll,PrintUIEntry",
                "/if",
                "/b",
                "winprint-rs-null-printer",
                "/f",
                &format!("{}\\inf\\ntprint.inf", std::env::var("SystemRoot").unwrap()),
                "/r",
                "nul:",
                "/m",
                "Generic / Text Only",
                "/z",
            ])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }

    #[dtor]
    fn cleanup() {
        std::process::Command::new("rundll32")
            .args(&[
                "printui.dll,PrintUIEntry",
                "/dl",
                "/n",
                "winprint-rs-null-printer",
                "/q",
            ])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }

    fn get_test_printer() -> PrinterInfo {
        let printers = PrinterInfo::all().unwrap();
        printers
            .into_iter()
            .find(|x| x.name() == TEST_PRINTER_NAME)
            .unwrap()
    }

    #[test]
    fn print_xps_file() {
        let test_printer = get_test_printer();
        let xps = XpsPrinter::new(test_printer);
        xps.print(Path::new("D:\\1.xps"), Default::default())
            .unwrap();
    }

    #[test]
    fn print_pdf_file() {
        let test_printer = get_test_printer();
        let pdf = PdfiumPrinter::new(test_printer);
        pdf.print(Path::new("D:\\1.pdf"), Default::default())
            .unwrap();
    }

    #[test]
    fn get_print_capabilities_xml() {
        let test_printer = get_test_printer();
        PrintCapabilities::fetch_xml(&test_printer).unwrap();
    }

    #[test]
    fn print_ticket_builder_merge() {
        let test_printer = get_test_printer();
        let mut builder = PrintTicketBuilder::new(&test_printer).unwrap();
        let delta = r#"<psf:PrintTicket xmlns:psf="http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" version="1" xmlns:psk="http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords">
    <psf:Feature name="psk:PageMediaSize">
		<psf:Option name="psk:NorthAmericaTabloid">
			<psf:ScoredProperty name="psk:MediaSizeWidth">
				<psf:Value xsi:type="xsd:integer">279400</psf:Value>
			</psf:ScoredProperty>
			<psf:ScoredProperty name="psk:MediaSizeHeight">
				<psf:Value xsi:type="xsd:integer">431800</psf:Value>
			</psf:ScoredProperty>
			<psf:Property name="psk:DisplayName">
				<psf:Value xsi:type="xsd:string">Tabloid</psf:Value>
			</psf:Property>
		</psf:Option>
	</psf:Feature>
</psf:PrintTicket>"#;
        builder.merge(PrintTicket::from_xml(delta)).unwrap();
        println!("{}", builder.build().unwrap().into_xml());
    }
}
