use crate::printer::pipeline_outcome;
use crate::printer::FilePrinter;
use crate::printer::PrintOutcome;
use crate::printer::PrintProgress;
use crate::printer::PrinterDevice;
use crate::ticket::PrintTicket;
use crate::utils::wchar;
use scopeguard::defer;
use std::mem::MaybeUninit;
use std::ops::ControlFlow;
use std::path::Path;
use std::ptr;
use thiserror::Error;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_FAILED},
        Storage::Xps::{Printing::*, *},
        System::{Com::*, Threading::*},
    },
};

#[derive(Error, Debug)]
/// Represents an error from [`XpsPrinter`].
pub enum XpsPrinterError {
    /// Failed to create event.
    #[error("Failed to create event")]
    FailedToCreateEvent(#[source] windows::core::Error),
    /// Failed to create object factory.
    #[error("Failed to create object factory")]
    FailedToCreateObjectFactory(#[source] windows::core::Error),
    /// Failed to start job.
    #[error("Failed to start job")]
    FailedToStartJob(#[source] windows::core::Error),
    /// Failed to apply print ticket.
    #[error("Failed to apply print ticket")]
    FailedToApplyPrintTicket(#[source] windows::core::Error),
    /// Failed to write document.
    #[error("Failed to write document")]
    FailedToWriteDocument(#[source] windows::core::Error),
    /// Stream is not available.
    #[error("Stream is not available")]
    StreamNotAvailable,
    /// Failed to wait for the job.
    #[error("Failed to wait for the job")]
    FailedToWaitForJob(#[source] windows::core::Error),
    /// The print pipeline failed the job.
    #[error("The print job failed")]
    JobFailed(#[source] windows::core::Error),
}

/// A printer to print XPS documents.
pub struct XpsPrinter {
    printer: PrinterDevice,
}

impl XpsPrinter {
    /// Create a new [`XpsPrinter`] for the given printer device.
    pub fn new(printer: PrinterDevice) -> Self {
        Self { printer }
    }
}

impl FilePrinter for XpsPrinter {
    type Options = PrintTicket;
    type Error = XpsPrinterError;
    fn print_with_progress(
        &self,
        path: &Path,
        options: PrintTicket,
        on_progress: &mut dyn FnMut(&PrintProgress) -> ControlFlow<()>,
    ) -> std::result::Result<PrintOutcome, XpsPrinterError> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            defer! {
                CoUninitialize();
            }
            let progress_event = CreateEventW(None, false, false, None)
                .map_err(XpsPrinterError::FailedToCreateEvent)?;
            defer! {
                let _ = CloseHandle(progress_event);
            }
            let completion_event = CreateEventW(None, true, false, None)
                .map_err(XpsPrinterError::FailedToCreateEvent)?;
            defer! {
                let _ = CloseHandle(completion_event);
            }
            let xps_factory: IXpsOMObjectFactory =
                CoCreateInstance(&XpsOMObjectFactory, None, CLSCTX_ALL)
                    .map_err(XpsPrinterError::FailedToCreateObjectFactory)?;
            let mut job = Option::<IXpsPrintJob>::None;
            let mut document_stream = Option::<IXpsPrintJobStream>::None;
            let mut print_ticket_stream = Option::<IXpsPrintJobStream>::None;
            StartXpsPrintJob(
                PCWSTR(wchar::to_wide_chars(self.printer.os_name()).as_ptr()),
                PCWSTR(wchar::to_wide_chars(path.file_name().unwrap_or(path.as_ref())).as_ptr()),
                None,
                progress_event,
                completion_event,
                &[],
                ptr::addr_of_mut!(job),
                ptr::addr_of_mut!(document_stream),
                ptr::addr_of_mut!(print_ticket_stream),
            )
            .map_err(XpsPrinterError::FailedToStartJob)?;
            // Releasing an unfinished job submits it, so cancel it on any early return (or panic)
            // until it ends; otherwise, the part written so far would print.
            let job = scopeguard::guard(job.ok_or(XpsPrinterError::StreamNotAvailable)?, |job| {
                let _ = job.Cancel();
            });
            let print_ticket_stream =
                print_ticket_stream.ok_or(XpsPrinterError::StreamNotAvailable)?;
            let print_ticket = options.get_xml();
            let mut print_ticket_written = MaybeUninit::<u32>::uninit();
            print_ticket_stream
                .Write(
                    print_ticket.as_ptr() as *const _,
                    print_ticket.len() as u32,
                    Some(print_ticket_written.as_mut_ptr()),
                )
                .ok()
                .map_err(XpsPrinterError::FailedToApplyPrintTicket)?;
            print_ticket_stream
                .Close()
                .map_err(XpsPrinterError::FailedToApplyPrintTicket)?;
            let document_stream = document_stream.ok_or(XpsPrinterError::StreamNotAvailable)?;
            let xps_package = xps_factory
                .CreatePackageFromFile(PCWSTR(wchar::to_wide_chars(path).as_ptr()), false)
                .map_err(XpsPrinterError::FailedToWriteDocument)?;
            let page_count =
                page_count(&xps_package).map_err(XpsPrinterError::FailedToWriteDocument)?;
            if on_progress(&PrintProgress::new(0, page_count)).is_break() {
                return Ok(PrintOutcome::Canceled);
            }
            xps_package
                .WriteToStream(&document_stream, false)
                .map_err(XpsPrinterError::FailedToWriteDocument)?;
            document_stream
                .Close()
                .map_err(XpsPrinterError::FailedToWriteDocument)?;
            let outcome = wait_for_job(
                &job,
                [progress_event, completion_event],
                page_count,
                on_progress,
            )?;
            let _ = scopeguard::ScopeGuard::into_inner(job);
            Ok(outcome)
        }
    }
}

/// The number of pages in all documents of `package`.
unsafe fn page_count(package: &IXpsOMPackage) -> windows::core::Result<u32> {
    let documents = package.GetDocumentSequence()?.GetDocuments()?;
    (0..documents.GetCount()?).try_fold(0, |pages, index| {
        Ok(pages + documents.GetAt(index)?.GetPageReferences()?.GetCount()?)
    })
}

/// Wait until `job` ends, reporting the pages it has processed out of `page_count` to
/// `on_progress` until it asks to cancel.
///
/// The job may still complete after a cancel request, so the outcome comes from the job.
unsafe fn wait_for_job(
    job: &IXpsPrintJob,
    events: [HANDLE; 2],
    page_count: u32,
    on_progress: &mut dyn FnMut(&PrintProgress) -> ControlFlow<()>,
) -> Result<PrintOutcome, XpsPrinterError> {
    let mut cancel_requested = false;
    loop {
        let mut status = XPS_JOB_STATUS::default();
        job.GetJobStatus(&mut status)
            .map_err(XpsPrinterError::FailedToWaitForJob)?;
        let completion = PrintDocumentPackageCompletion(status.completion.0);
        match pipeline_outcome(completion, status.jobStatus) {
            Some(Ok(PrintOutcome::Completed)) => {
                // Progress events coalesce, so the last pages may only show up as the job
                // completes. Report them, though it is too late to cancel.
                if !cancel_requested {
                    let _ = on_progress(&PrintProgress::new(page_count, page_count));
                }
                return Ok(PrintOutcome::Completed);
            }
            Some(end) => return end.map_err(XpsPrinterError::JobFailed),
            None => {}
        }
        let processed = PrintProgress::new(status.currentPageTotal as u32, page_count);
        if !cancel_requested && on_progress(&processed).is_break() {
            cancel_requested = true;
            let _ = job.Cancel();
        }
        if WaitForMultipleObjects(&events, false, INFINITE) == WAIT_FAILED {
            return Err(XpsPrinterError::FailedToWaitForJob(
                windows::core::Error::from_thread(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::XpsPrinter;
    use crate::{
        printer::{test_helpers, FilePrinter, PrintOutcome},
        test_utils::null_device,
    };
    use std::path::Path;

    #[test]
    fn print_simple_xps_document() {
        let device = null_device::thread_local();
        let xps = XpsPrinter::new(device);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.xps");
        assert_eq!(
            xps.print(path.as_path(), Default::default()).unwrap(),
            PrintOutcome::Completed
        );
    }

    #[test]
    fn reports_progress() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.xps");
        test_helpers::assert_reports_progress(&XpsPrinter::new(null_device::thread_local()), &path);
    }

    #[test]
    fn cancels_on_request() {
        let device = null_device::thread_local();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.xps");
        test_helpers::assert_cancels_on_request(&XpsPrinter::new(device.clone()), &device, &path);
    }

    #[test]
    fn observes_external_cancel() {
        let device = null_device::thread_local();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.xps");
        test_helpers::assert_observes_external_cancel(
            &XpsPrinter::new(device.clone()),
            &device,
            &path,
        );
    }

    #[test]
    fn failure_leaves_no_job() {
        let device = null_device::thread_local();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.xps");
        test_helpers::assert_failure_leaves_no_job(
            &XpsPrinter::new(device.clone()),
            &device,
            &path,
        );
    }
}
