use crate::bindings::pdfium::*;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;
use std::sync::PoisonError;

pub struct PdfiumGuard {
    inited: bool,
}

impl PdfiumGuard {
    fn new() -> PdfiumGuard {
        PdfiumGuard { inited: false }
    }
    pub fn init(&mut self) {
        if !self.inited {
            unsafe {
                FPDF_InitLibrary();
            }
            self.inited = true;
        }
    }
    #[allow(unused)]
    pub fn init_with_config(&mut self, config: &FPDF_LIBRARY_CONFIG) {
        if !self.inited {
            unsafe {
                FPDF_InitLibraryWithConfig(config);
            }
            self.inited = true;
        }
    }
    pub fn get() -> MutexGuard<'static, PdfiumGuard> {
        static PDFIUM_GUARD: OnceLock<Mutex<PdfiumGuard>> = OnceLock::new();
        PDFIUM_GUARD
            .get_or_init(|| Mutex::new(PdfiumGuard::new()))
            .lock()
            // A panic while printing, such as one in a progress callback, unwinds through guards
            // that close what it opened, so the library stays usable.
            .unwrap_or_else(PoisonError::into_inner)
    }
    pub fn guard() -> MutexGuard<'static, PdfiumGuard> {
        let mut x = Self::get();
        x.init();
        x
    }
}
impl Drop for PdfiumGuard {
    fn drop(&mut self) {
        if self.inited {
            unsafe {
                FPDF_DestroyLibrary();
            }
        }
    }
}
