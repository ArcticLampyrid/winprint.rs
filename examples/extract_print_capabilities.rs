use std::io::Write;

use winprint::{printer::PrinterDevice, ticket::PrintCapabilities};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::io::stdout().write_all(b"Extract print capabilities to current directory? (Y/n): ")?;
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "y" && !input.trim().is_empty() {
        return Ok(());
    }

    let devices = PrinterDevice::all()?;
    for device in devices {
        let capabilities = PrintCapabilities::fetch_xml(&device)?;
        std::fs::write(format!("{}.capabilities.xml", device.name()), capabilities)?;
    }
    Ok(())
}
