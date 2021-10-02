use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FilePrinterError {
    #[error("Failed to open printer")]
    FailedToOpenPrinter,
    #[error("Failed to create job")]
    FailedToCreateJob,
    #[error("Failed to write document")]
    FailedToWriteDocument,
}
pub trait FilePrinter {
    fn print(&self, path: &Path) -> std::result::Result<(), FilePrinterError>;
}
