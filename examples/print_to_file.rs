//! A manual smoke test for [`winprint::test_utils::file_device::FilePrinterDevice`].
//!
//! Creates a virtual "print-to-file" printer backed by the requested driver, prints one or two
//! of the shipped test documents through it, and then tears the printer down. The backing file
//! is also deleted on drop — pass `--keep` to copy the produced spool output next to the
//! current working directory before cleanup.
//!
//! Requires administrator privileges on the host (adding printers and printer ports is
//! privileged). Run from a Windows host or the bundled Windows VM:
//!
//! ```powershell
//! cargo run --example print_to_file --features test-utils -- pwg
//! cargo run --example print_to_file --features test-utils -- pdf --keep
//! ```
use std::path::Path;
use winprint::printer::{FilePrinter, PrinterDevice, XpsPrinter};
#[cfg(feature = "pdfium")]
use winprint::printer::PdfiumPrinter;
use winprint::test_utils::file_device::{FilePrinterDevice, FilePrinterProvider, Pdf, PwgRaster};

fn run<T: FilePrinterProvider>(keep: bool) -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let xps_path = manifest_dir.join("test_data/test_document.xps");
    let pdf_path = manifest_dir.join("test_data/test_document.pdf");

    let dev = FilePrinterDevice::<T>::new()?;
    println!("virtual printer: {}", dev.device().name());
    println!("backing file:    {}", dev.file_path().display());

    print_xps(dev.device(), &xps_path)?;
    print_pdf(dev.device(), &pdf_path)?;

    if keep {
        let out = std::env::current_dir()?
            .join(format!("print-to-file-{}.out", dev.device().name()));
        match std::fs::copy(dev.file_path(), &out) {
            Ok(n) => println!("copied {} bytes -> {}", n, out.display()),
            Err(e) => println!("copy failed ({}); the spooler may not have flushed yet", e),
        }
    }

    // dev is dropped here — printer, port and backing file are removed.
    Ok(())
}

fn print_xps(device: &PrinterDevice, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        println!("printing XPS: {}", path.display());
        let xps = XpsPrinter::new(device.clone());
        xps.print(path, Default::default())?;
        println!("  -> XPS print submitted");
    } else {
        println!("skipping XPS: {} not found", path.display());
    }
    Ok(())
}

#[cfg(feature = "pdfium")]
fn print_pdf(device: &PrinterDevice, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        println!("printing PDF: {}", path.display());
        let pdf = PdfiumPrinter::new(device.clone());
        pdf.print(path, Default::default())?;
        println!("  -> PDF print submitted");
    } else {
        println!("skipping PDF: {} not found", path.display());
    }
    Ok(())
}

#[cfg(not(feature = "pdfium"))]
fn print_pdf(_device: &PrinterDevice, _path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("skipping PDF: pdfium feature is disabled");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let provider = args.next().unwrap_or_else(|| "pwg".to_string());
    let mut keep = false;
    for a in args {
        match a.as_str() {
            "--keep" => keep = true,
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }

    match provider.as_str() {
        "pwg" => run::<PwgRaster>(keep),
        "pdf" => run::<Pdf>(keep),
        other => {
            eprintln!("unknown provider '{other}', expected 'pwg' or 'pdf'");
            std::process::exit(2);
        }
    }
}
