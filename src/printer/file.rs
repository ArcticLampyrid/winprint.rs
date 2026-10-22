use crate::printer::{PrintOutcome, PrintProgress};
use std::ops::ControlFlow;
use std::path::Path;

/// A trait representing a printer that can print files.
pub trait FilePrinter {
    /// The options type for the printer.
    type Options;
    /// The error type for the printer.
    type Error;

    /// Print the file with the given options, reporting progress to `on_progress`.
    ///
    /// Returning [`ControlFlow::Break`] from `on_progress` cancels the job. The job may still
    /// complete if it has already been submitted, so check the returned [`PrintOutcome`].
    fn print_with_progress(
        &self,
        path: &Path,
        options: Self::Options,
        on_progress: &mut dyn FnMut(&PrintProgress) -> ControlFlow<()>,
    ) -> Result<PrintOutcome, Self::Error>;

    /// Print the file with the given options.
    fn print(&self, path: &Path, options: Self::Options) -> Result<PrintOutcome, Self::Error> {
        self.print_with_progress(path, options, &mut |_| ControlFlow::Continue(()))
    }
}
