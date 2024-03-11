mod bindings;
pub mod printer;
pub mod ticket;
mod utils;
#[cfg(test)]
mod tests {
    use crate::printer::PrinterInfo;
    use ctor::{ctor, dtor};

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

    pub fn get_test_printer() -> PrinterInfo {
        let printers = PrinterInfo::all().unwrap();
        printers
            .into_iter()
            .find(|x| x.name() == TEST_PRINTER_NAME)
            .unwrap()
    }
}
