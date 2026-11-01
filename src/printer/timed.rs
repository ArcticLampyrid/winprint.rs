use std::time::Duration;

/// A trait representing a printer that can limit how long a print job takes.
pub trait TimedPrinter {
    /// Limit how long a print job can take, or remove the limit with `None`.
    ///
    /// There is no limit by default.
    ///
    /// The limit counts from the start of the print call, so it includes reading the document.
    /// It applies while the job is submitted and, for a printer that waits for it, while the
    /// print system processes the job. A job that
    /// does not end in time fails with a timeout error and is canceled.
    ///
    /// A call into the print system is not interrupted, so a job can run past the limit until
    /// that call returns.
    fn set_timeout(&mut self, timeout: Option<Duration>);
}
