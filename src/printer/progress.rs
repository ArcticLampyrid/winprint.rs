/// How far a print job has been submitted to the print system.
///
/// The unit of [`current`](Self::current) and [`total`](Self::total) is chosen by each printer
/// and may change between versions; only their ratio is meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrintProgress {
    current: u32,
    total: u32,
}

impl PrintProgress {
    pub(crate) fn new(current: u32, total: u32) -> Self {
        Self {
            current: current.min(total),
            total,
        }
    }
    /// The amount of work submitted so far, at most [`total`](Self::total).
    pub fn current(&self) -> u32 {
        self.current
    }
    /// The amount of work to submit.
    pub fn total(&self) -> u32 {
        self.total
    }
}
