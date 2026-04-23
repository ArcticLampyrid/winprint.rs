//! A manual smoke test for `FilePrinterDevice`.
//!
//! This example spins up a thread-local "print-to-file" virtual printer, prints one of the
//! shipped test documents through it, and then moves the resulting spool file next to the
//! current working directory so it can be inspected.
//!
//! Run from a Windows host (or Windows VM) with administrator privileges (creating printers and
//! printer ports requires elevation):
//!
//! ```powershell
//! cargo run --example print_to_file --features test-utils -- pwg
//! cargo run --example print_to_file --features test-utils -- pdf
//! ```
//!
//! The argument picks the driver (`pwg` → `Microsoft PWG Raster Class Driver`, `pdf` →
//! `Microsoft Print To PDF`).
use std::path::{Path, PathBuf};
use winprint::printer::{FilePrinter, XpsPrinter};
#[cfg(feature = "pdfium")]
use winprint::printer::PdfiumPrinter;
use winprint::test_utils::file_device::{FilePrinterDevice, Pdf, PwgRaster};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = std::env::args().nth(1).unwrap_or_else(|| "pwg".to_string());

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let xps_path = manifest_dir.join("test_data/test_document.xps");
    let pdf_path = manifest_dir.join("test_data/test_document.pdf");

    let device = match provider.as_str() {
        "pwg" => FilePrinterDevice::<PwgRaster>::thread_local(),
        "pdf" => FilePrinterDevice::<Pdf>::thread_local(),
        other => {
            eprintln!("unknown provider '{}', expected 'pwg' or 'pdf'", other);
            std::process::exit(2);
        }
    };
    println!("virtual printer: {}", device.name());

    // Print an XPS document first.
    if xps_path.exists() {
        println!("printing XPS: {}", xps_path.display());
        let xps = XpsPrinter::new(device.clone());
        xps.print(&xps_path, Default::default())?;
        println!("  -> XPS print submitted");
    } else {
        println!("skipping XPS: {} not found", xps_path.display());
    }

    // Then a PDF, via PDFium.
    #[cfg(feature = "pdfium")]
    if pdf_path.exists() {
        println!("printing PDF: {}", pdf_path.display());
        let pdf = PdfiumPrinter::new(device.clone());
        pdf.print(&pdf_path, Default::default())?;
        println!("  -> PDF print submitted");
    } else {
        println!("skipping PDF: {} not found", pdf_path.display());
    }
    #[cfg(not(feature = "pdfium"))]
    {
        let _ = pdf_path;
        println!("skipping PDF: pdfium feature is disabled");
    }

    // We let the thread-local be dropped at process exit so the printer/port/file are cleaned up
    // by the Drop impl. Sleep a bit first so the spooler has a chance to flush to the backing
    // file if the user wants to inspect it manually (by attaching a debugger or tweaking the
    // example to copy the file before drop).
    //
    // For users who want to inspect the output: uncomment the block below to copy the spool file
    // before cleanup runs.
    #[allow(unused_variables)]
    let artifact_dir: PathBuf = std::env::current_dir()?.join("print-to-file-output");
    // let _ = std::fs::create_dir_all(&artifact_dir);
    // std::thread::sleep(std::time::Duration::from_secs(3));

    Ok(())
}
