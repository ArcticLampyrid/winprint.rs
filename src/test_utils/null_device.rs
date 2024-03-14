use crate::printer::PrinterDevice;
use sha2::{Digest, Sha256};
use std::cell::OnceCell;

thread_local! {
    static NULL_DEVICE: OnceCell<NullPrinterDevice> = OnceCell::new();
}
static DEVICE_ID_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct NullPrinterDevice {
    printer: PrinterDevice,
}

impl NullPrinterDevice {
    fn new() -> Self {
        let id = DEVICE_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let exe_path = std::env::current_exe().unwrap().canonicalize().unwrap();
        let exe_id =
            bs58::encode(Sha256::digest(exe_path.to_string_lossy().as_bytes())).into_string();

        let printer_name = format!("null-device-{}-{}", exe_id, id);

        let printer = PrinterDevice::all()
            .unwrap()
            .into_iter()
            .find(|p| p.name() == printer_name);
        if let Some(printer) = printer {
            return NullPrinterDevice { printer };
        }

        std::process::Command::new("rundll32")
            .args([
                "printui.dll,PrintUIEntry",
                "/if",
                "/b",
                printer_name.as_ref(),
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

        let printer = PrinterDevice::all()
            .unwrap()
            .into_iter()
            .find(|p| p.name() == printer_name)
            .unwrap();
        NullPrinterDevice { printer }
    }
}

impl Drop for NullPrinterDevice {
    fn drop(&mut self) {
        std::process::Command::new("rundll32")
            .args([
                "printui.dll,PrintUIEntry",
                "/dl",
                "/n",
                self.printer.name(),
                "/q",
            ])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }
}

pub fn thread_local() -> PrinterDevice {
    NULL_DEVICE.with(|f| f.get_or_init(NullPrinterDevice::new).printer.clone())
}
