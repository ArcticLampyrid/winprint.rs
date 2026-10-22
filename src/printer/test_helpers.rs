use super::{FilePrinter, PrintOutcome, PrintProgress, PrinterDevice};
use crate::ticket::PrintTicket;
use std::fmt::Debug;
use std::ops::ControlFlow;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Print `path`, recording every progress report and letting `on_progress` decide whether to go on.
fn print_recording<P>(
    printer: &P,
    path: &Path,
    mut on_progress: impl FnMut(&PrintProgress) -> ControlFlow<()>,
) -> (Result<PrintOutcome, P::Error>, Vec<PrintProgress>)
where
    P: FilePrinter<Options = PrintTicket>,
{
    let mut reports = Vec::new();
    let outcome = printer.print_with_progress(path, Default::default(), &mut |progress| {
        reports.push(*progress);
        on_progress(progress)
    });
    (outcome, reports)
}

/// Pipe the jobs queued on `device` into the PowerShell `pipeline`, returning its output.
fn with_print_jobs(device: &PrinterDevice, pipeline: &str) -> String {
    let output = Command::new("powershell")
        .stdin(Stdio::null())
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Get-PrintJob -PrinterName '{}' | {pipeline}",
                device.name().replace('\'', "''")
            ),
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{pipeline} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// Remove every job queued on `device`, as a user would from the print queue.
fn remove_print_jobs(device: &PrinterDevice) {
    with_print_jobs(device, "Remove-PrintJob");
}

/// Pause `device`, so that jobs submitted to it stay in its queue.
fn pause_printer(device: &PrinterDevice) {
    let status = Command::new("powershell")
        .stdin(Stdio::null())
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Get-CimInstance -ClassName Win32_Printer -Filter \"Name='{}'\" | Invoke-CimMethod -MethodName Pause",
                device.name().replace('\'', "''")
            ),
        ])
        .status()
        .unwrap();
    assert!(status.success(), "pausing the printer failed: {status:?}");
}

/// Wait until no job is queued on `device`, failing after a while.
fn assert_queue_drains(device: &PrinterDevice) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let jobs = with_print_jobs(device, "Format-List -Property Id, JobStatus");
        if jobs.trim().is_empty() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "jobs are left in the queue:\n{jobs}"
        );
        thread::sleep(Duration::from_secs(1));
    }
}

/// Printing `path` reports progress from nothing up to everything, never going back, then
/// completes, even when the callback is slow.
pub fn assert_reports_progress<P>(printer: &P, path: &Path)
where
    P: FilePrinter<Options = PrintTicket>,
    P::Error: Debug,
{
    let mut reported = 0;
    let (outcome, reports) = print_recording(printer, path, |_| {
        reported += 1;
        // A slow callback lets the job get ahead of the reports.
        if reported == 2 {
            thread::sleep(Duration::from_secs(2));
        }
        ControlFlow::Continue(())
    });
    eprintln!("{reports:?} -> {outcome:?}");
    assert_eq!(outcome.unwrap(), PrintOutcome::Completed);
    let total = reports.first().expect("progress is reported").total();
    assert!(total > 0);
    assert!(reports.iter().all(|progress| progress.total() == total));
    assert_eq!(reports.first().unwrap().current(), 0);
    assert!(reports
        .windows(2)
        .all(|pair| pair[0].current() <= pair[1].current()));
    assert_eq!(reports.last().unwrap().current(), total);
}

/// Breaking from the first progress report cancels the job and removes it from the print queue.
pub fn assert_cancels_on_request<P>(printer: &P, device: &PrinterDevice, path: &Path)
where
    P: FilePrinter<Options = PrintTicket>,
    P::Error: Debug,
{
    // A paused printer keeps submitted jobs, so only a canceled job leaves the queue.
    pause_printer(device);
    let (outcome, reports) = print_recording(printer, path, |_| ControlFlow::Break(()));
    eprintln!("{reports:?} -> {outcome:?}");
    assert_eq!(outcome.unwrap(), PrintOutcome::Canceled);
    assert_eq!(reports.len(), 1);
    assert_queue_drains(device);
}

/// A job removed from the print queue while printing ends without an error.
pub fn assert_observes_external_cancel<P>(printer: &P, device: &PrinterDevice, path: &Path)
where
    P: FilePrinter<Options = PrintTicket>,
    P::Error: Debug,
{
    let mut removed = false;
    let (outcome, reports) = print_recording(printer, path, |progress| {
        if !removed && progress.current() > 0 {
            removed = true;
            remove_print_jobs(device);
        }
        ControlFlow::Continue(())
    });
    eprintln!("{reports:?} -> {outcome:?}");
    outcome.unwrap();
}
