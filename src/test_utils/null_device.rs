use super::powershell::{quote, run_sequentially};
use crate::printer::PrinterDevice;
use std::{cell::OnceCell, sync::OnceLock};
use uuid::Uuid;

thread_local! {
    static NULL_DEVICE: OnceCell<NullPrinterDevice> = const { OnceCell::new() };
}
static PORT_AND_DRIVER_INSTALLER: OnceLock<()> = OnceLock::new();

struct NullPrinterDevice {
    printer: PrinterDevice,
}

impl NullPrinterDevice {
    fn new() -> Self {
        PORT_AND_DRIVER_INSTALLER.get_or_init(|| {
            run_sequentially(
                "install null printer driver and port",
                r#"
if (-not (Get-PrinterDriver -Name 'Generic / Text Only' -ErrorAction SilentlyContinue)) {
    Add-PrinterDriver -Name 'Generic / Text Only'
}
if (-not (Get-PrinterPort -Name 'nul:' -ErrorAction SilentlyContinue)) {
    Add-PrinterPort -Name 'nul:'
}
"#,
            )
            .expect("install null printer driver and port");
        });

        // A same executable can be run in multiple processes.
        // Never use process local counters to generate unique names,
        // because they can collide across processes.
        let printer_name = format!(
            "null-device-{}",
            bs58::encode(Uuid::new_v4().as_bytes()).into_string()
        );
        run_sequentially(
            "add null printer",
            &format!(
                "Add-Printer -Name {} -PortName 'nul:' -DriverName 'Generic / Text Only'",
                quote(&printer_name)
            ),
        )
        .expect("add null printer");

        let printer =
            PrinterDevice::by_os_name(&printer_name).expect("find newly created null printer");
        NullPrinterDevice { printer }
    }
}

impl Drop for NullPrinterDevice {
    fn drop(&mut self) {
        // Destructors also run while unwinding; report cleanup failures without panicking.
        if let Err(error) = run_sequentially(
            "remove null printer",
            &format!("Remove-Printer -Name {}", quote(self.printer.name())),
        ) {
            eprintln!("{error}");
        }
    }
}

/// Get a thread-local null printer device.
/// This device is automatically managed and to be deleted when the thread ends.
pub fn thread_local() -> PrinterDevice {
    NULL_DEVICE.with(|f| f.get_or_init(NullPrinterDevice::new).printer.clone())
}
