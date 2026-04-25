use image::DynamicImage;
use std::{path::Path, time::Duration};
use winprint::{
    printer::PrinterDevice,
    test_utils::file_device::{FilePrinterDevice, PwgRaster},
};

use super::loader;

pub fn print_pages<F>(print: F) -> Vec<DynamicImage>
where
    F: FnOnce(&PrinterDevice),
{
    let device = FilePrinterDevice::<PwgRaster>::new().expect("install virtual PWG printer");
    print(device.device());
    wait_for_stable_file(device.file_path(), Duration::from_mins(10))
        .expect("PWG port file should settle after print");
    let actual_pages =
        loader::load_pages_from_pwg(device.file_path()).expect("decode PWG raster output");
    return actual_pages;
}

/// Poll until `path` exists, is non-empty, and its size stops changing for
/// a few consecutive checks (i.e. the writer is done). Returns
/// `ErrorKind::TimedOut` if `timeout` elapses first.
fn wait_for_stable_file(path: &Path, timeout: Duration) -> std::io::Result<()> {
    const STABLE_POLLS_REQUIRED: u32 = 3;
    const POLL_INTERVAL: Duration = Duration::from_secs(1);

    let deadline = std::time::Instant::now() + timeout;
    let mut last_size: Option<u64> = None;
    let mut stable = 0u32;
    loop {
        match std::fs::metadata(path) {
            Ok(meta) if meta.len() > 0 => {
                if Some(meta.len()) == last_size {
                    stable += 1;
                    if stable >= STABLE_POLLS_REQUIRED {
                        return Ok(());
                    }
                } else {
                    stable = 0;
                    last_size = Some(meta.len());
                }
            }
            Ok(_) => {
                // Exists but still empty: keep the stability counter reset
                // and wait for the first bytes to arrive.
                stable = 0;
                last_size = None;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                stable = 0;
                last_size = None;
            }
            Err(e) => return Err(e),
        }
        if std::time::Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "timed out waiting for spooler to flush {} (last size = {:?})",
                    path.display(),
                    last_size
                ),
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}
