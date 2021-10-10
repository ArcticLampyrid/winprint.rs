use crate::bindings::pdfium::*;
use lazy_static::lazy_static;
use std::sync::Mutex;
use std::sync::MutexGuard;

pub struct PdfiumGuard {
    inited: bool,
}
lazy_static! {
    static ref PDFIUM_GUARD: Mutex<PdfiumGuard> = Mutex::new(PdfiumGuard::new());
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
        PDFIUM_GUARD.lock().unwrap()
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
