mod bindings;
pub mod printer;
pub mod ticket;
mod utils;
#[cfg(test)]
mod tests {
    use crate::printer::PrinterInfo;
    use crate::printer::XpsPrinter;
    use crate::printer::FilePrinter;
    use std::path::Path;
    #[test]
    fn it_works() {
        let printers = PrinterInfo::all().unwrap();
        println!("{:#?}", printers);
        let my_printer = printers.iter().find(|x| x.name() == "pdfFactory Pro").unwrap();
        let xps = XpsPrinter::new(my_printer.clone());
        xps.print(Path::new("D:\\1.xps"), Default::default())
            .unwrap();
    }
}
