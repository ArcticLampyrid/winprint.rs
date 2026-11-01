use std::time::{Duration, Instant};

/// The time by which a print job must end, if any.
///
/// See [`TimedPrinter`](crate::printer::TimedPrinter).
#[derive(Clone, Copy, Debug)]
pub struct Deadline {
    start: Instant,
    end: Option<Instant>,
}

impl Deadline {
    /// The deadline `timeout` from now, or no deadline if `timeout` is `None`.
    ///
    /// A timeout too long to represent sets no deadline.
    pub fn after(timeout: Option<Duration>) -> Self {
        let start = Instant::now();
        Self {
            start,
            end: timeout.and_then(|timeout| start.checked_add(timeout)),
        }
    }
    /// The time since the deadline was set.
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
    /// The time left until the deadline, or `None` if there is no deadline.
    pub fn remaining(&self) -> Option<Duration> {
        self.end
            .map(|end| end.saturating_duration_since(Instant::now()))
    }
    /// Whether the deadline has passed.
    pub fn has_passed(&self) -> bool {
        self.remaining().is_some_and(|left| left.is_zero())
    }
}

#[cfg(test)]
mod tests {
    use super::Deadline;
    use std::time::Duration;

    #[test]
    fn no_timeout_never_passes() {
        assert!(!Deadline::after(None).has_passed());
        assert!(!Deadline::after(Some(Duration::MAX)).has_passed());
        assert_eq!(Deadline::after(None).remaining(), None);
    }

    #[test]
    fn a_zero_timeout_has_passed_at_once() {
        assert!(Deadline::after(Some(Duration::ZERO)).has_passed());
    }

    #[test]
    fn a_long_timeout_has_not_passed_yet() {
        let deadline = Deadline::after(Some(Duration::from_secs(3600)));
        assert!(!deadline.has_passed());
        assert!(deadline.remaining().unwrap() > Duration::from_secs(3500));
    }

    #[test]
    fn elapsed_counts_from_the_start() {
        let deadline = Deadline::after(None);
        std::thread::sleep(Duration::from_millis(20));
        assert!(deadline.elapsed() >= Duration::from_millis(20));
    }
}
