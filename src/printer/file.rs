use std::path::Path;

pub trait FilePrinter {
    type Options;
    type Error;
    fn print(&self, path: &Path, options: Self::Options) -> std::result::Result<(), Self::Error>;
}
