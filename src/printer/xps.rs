use crate::printer::pipeline_outcome;
use crate::printer::FilePrinter;
use crate::printer::PrintOutcome;
use crate::printer::PrinterDevice;
use crate::ticket::PrintTicket;
use crate::utils::wchar;
use scopeguard::defer;
use std::mem::MaybeUninit;
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
    fn print(
        &self,
        path: &Path,
        options: PrintTicket,
    ) -> std::result::Result<PrintOutcome, XpsPrinterError> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            defer! {
                CoUninitialize();
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
                HANDLE::default(),
                completion_event,
                &[],
                ptr::addr_of_mut!(job),
                ptr::addr_of_mut!(document_stream),
                ptr::addr_of_mut!(print_ticket_stream),
            )
            .map_err(XpsPrinterError::FailedToStartJob)?;
            let job = job.ok_or(XpsPrinterError::StreamNotAvailable)?;
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
            xps_package
                .WriteToStream(&document_stream, false)
                .map_err(XpsPrinterError::FailedToWriteDocument)?;
            document_stream
                .Close()
                .map_err(XpsPrinterError::FailedToWriteDocument)?;
            wait_for_job(&job, completion_event)
        }
    }
}

/// Wait until `job` ends.
unsafe fn wait_for_job(
    job: &IXpsPrintJob,
    completion_event: HANDLE,
) -> Result<PrintOutcome, XpsPrinterError> {
    loop {
        let mut status = XPS_JOB_STATUS::default();
        job.GetJobStatus(&mut status)
            .map_err(XpsPrinterError::FailedToWaitForJob)?;
        let completion = PrintDocumentPackageCompletion(status.completion.0);
        if let Some(end) = pipeline_outcome(completion, status.jobStatus) {
            return end.map_err(XpsPrinterError::JobFailed);
        }
        if WaitForSingleObject(completion_event, INFINITE) == WAIT_FAILED {
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
        printer::{FilePrinter, PrintOutcome},
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
}
