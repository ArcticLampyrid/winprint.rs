use windows::core::HRESULT;
use windows::Win32::Foundation::{ERROR_PRINT_CANCELLED, E_FAIL};
use windows::Win32::Storage::Xps::Printing::{
    PrintDocumentPackageCompletion, PrintDocumentPackageCompletion_Canceled,
    PrintDocumentPackageCompletion_Completed, PrintDocumentPackageCompletion_Failed,
};

/// How a print job ended.
///
/// A job is completed once everything has been submitted to the print system. What the system
/// does with it afterwards, such as sending it to the device, is not observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintOutcome {
    /// Everything has been submitted to the print system.
    Completed,
    /// The job was canceled, either by the progress callback or outside this process.
    Canceled,
}

/// Whether `code` reports that the job was deleted from the print queue, which is how a job
/// canceled outside this process fails the calls submitting it.
pub(crate) fn deleted_from_queue(code: HRESULT) -> bool {
    code == HRESULT::from_win32(ERROR_PRINT_CANCELLED.0)
}

/// How a job ended, as the print pipeline reports it with `completion` and `status`, or `None`
/// while it runs.
///
/// `PrintDocumentPackageStatus` is the generic version of `XPS_JOB_STATUS`, and their completion
/// values match, so XPS print jobs report through this too.
#[allow(non_upper_case_globals)]
pub(crate) fn pipeline_outcome(
    completion: PrintDocumentPackageCompletion,
    status: HRESULT,
) -> Option<Result<PrintOutcome, windows::core::Error>> {
    match completion {
        PrintDocumentPackageCompletion_Completed => Some(Ok(PrintOutcome::Completed)),
        PrintDocumentPackageCompletion_Canceled => Some(Ok(PrintOutcome::Canceled)),
        PrintDocumentPackageCompletion_Failed if deleted_from_queue(status) => {
            Some(Ok(PrintOutcome::Canceled))
        }
        PrintDocumentPackageCompletion_Failed => Some(Err(windows::core::Error::from_hresult(
            Some(status)
                .filter(|status| status.is_err())
                .unwrap_or(E_FAIL),
        ))),
        _ => None,
    }
}
