use crate::printer::{pipeline_outcome, PrintOutcome};
use std::mem::ManuallyDrop;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use windows::core::w;
use windows::core::BSTR;
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Foundation::{
    DISP_E_BADPARAMCOUNT, DISP_E_EXCEPTION, DISP_E_MEMBERNOTFOUND, DISP_E_TYPEMISMATCH,
    DISP_E_UNKNOWNNAME,
};
use windows::Win32::System::Com::DISPATCH_METHOD;
use windows::Win32::System::Ole::DISPID_UNKNOWN;
use windows::Win32::System::Variant::{VARIANT, VT_BYREF};
use windows::Win32::{
    Foundation::E_NOTIMPL,
    Storage::Xps::Printing::{
        IPrintDocumentPackageStatusEvent, IPrintDocumentPackageStatusEvent_Impl,
        PrintDocumentPackageStatus,
    },
    System::Com::{IDispatch, IDispatch_Impl, ITypeInfo, DISPATCH_FLAGS, DISPPARAMS, EXCEPINFO},
};
use windows_implement::implement;

type JobEnd = Result<PrintOutcome, windows::core::Error>;

/// The end of a job, set once by the print pipeline from one of its threads.
#[derive(Default)]
struct JobEndSlot {
    end: Mutex<Option<JobEnd>>,
    ended: Condvar,
}

#[implement(IPrintDocumentPackageStatusEvent, IDispatch)]
pub struct PrintCompletionSource {
    slot: Arc<JobEndSlot>,
}

impl PrintCompletionSource {
    pub fn new() -> Self {
        Self {
            slot: Default::default(),
        }
    }
    pub fn waiter(&self) -> PrintCompletionWaiter {
        PrintCompletionWaiter {
            slot: self.slot.clone(),
        }
    }
}

pub struct PrintCompletionWaiter {
    slot: Arc<JobEndSlot>,
}

impl PrintCompletionWaiter {
    /// How the job ended, or `None` while it runs.
    pub fn poll(&self) -> Option<JobEnd> {
        self.slot
            .end
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
    /// Block until the job ends.
    pub fn wait(&self) -> JobEnd {
        let end = self.slot.end.lock().unwrap_or_else(PoisonError::into_inner);
        self.slot
            .ended
            .wait_while(end, |end| end.is_none())
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .expect("the job has ended")
    }
}

impl IPrintDocumentPackageStatusEvent_Impl for PrintCompletionSource_Impl {
    fn PackageStatusUpdated(
        &self,
        packagestatus: *const PrintDocumentPackageStatus,
    ) -> windows::core::Result<()> {
        let status = unsafe { packagestatus.as_ref() }
            .ok_or_else(|| windows::core::Error::from_hresult(E_INVALIDARG))?;
        if let Some(job_end) = pipeline_outcome(status.Completion, status.PackageStatus) {
            self.slot
                .end
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get_or_insert(job_end);
            self.slot.ended.notify_all();
        }
        Ok(())
    }
}

impl IDispatch_Impl for PrintCompletionSource_Impl {
    fn GetTypeInfoCount(&self) -> windows::core::Result<u32> {
        Ok(0)
    }
    fn GetTypeInfo(&self, _itinfo: u32, _lcid: u32) -> windows::core::Result<ITypeInfo> {
        Err(windows::core::Error::from_hresult(E_NOTIMPL))
    }
    fn GetIDsOfNames(
        &self,
        _riid: *const windows::core::GUID,
        rgsznames: *const windows::core::PCWSTR,
        cnames: u32,
        _lcid: u32,
        rgdispid: *mut i32,
    ) -> windows::core::Result<()> {
        let names = unsafe { std::slice::from_raw_parts(rgsznames, cnames as usize) };
        let dispid = unsafe { std::slice::from_raw_parts_mut(rgdispid, cnames as usize) };
        let mut unknown_found = false;
        for (name, dispid) in names.iter().zip(dispid.iter_mut()) {
            if *name == w!("PackageStatusUpdated") {
                *dispid = 1;
            } else {
                *dispid = DISPID_UNKNOWN;
                unknown_found = true;
            }
        }
        if unknown_found {
            Err(windows::core::Error::from_hresult(DISP_E_UNKNOWNNAME))
        } else {
            Ok(())
        }
    }
    fn Invoke(
        &self,
        dispidmember: i32,
        _riid: *const windows::core::GUID,
        _lcid: u32,
        wflags: DISPATCH_FLAGS,
        pdispparams: *const DISPPARAMS,
        pvarresult: *mut VARIANT,
        pexcepinfo: *mut EXCEPINFO,
        puargerr: *mut u32,
    ) -> windows::core::Result<()> {
        if dispidmember == 1 && wflags == DISPATCH_METHOD {
            if !pvarresult.is_null() {
                unsafe {
                    (*pvarresult) = Default::default();
                }
            }
            let params = unsafe { &*pdispparams };
            if params.cArgs != 1 {
                return Err(windows::core::Error::from_hresult(DISP_E_BADPARAMCOUNT));
            }
            let arg = unsafe { &*params.rgvarg };
            let vt = unsafe { arg.Anonymous.Anonymous.vt };
            if vt.0 & VT_BYREF.0 == 0 {
                if !puargerr.is_null() {
                    unsafe {
                        *puargerr = 0;
                    }
                }
                return Err(windows::core::Error::from_hresult(DISP_E_TYPEMISMATCH));
            }
            let status = unsafe { arg.Anonymous.Anonymous.Anonymous.byref }
                as *const PrintDocumentPackageStatus;
            if let Err(e) = self.PackageStatusUpdated(status) {
                if !pexcepinfo.is_null() {
                    unsafe {
                        (*pexcepinfo).wCode = 0;
                        (*pexcepinfo).bstrSource = ManuallyDrop::new(BSTR::default());
                        (*pexcepinfo).bstrDescription = ManuallyDrop::new(BSTR::default());
                        (*pexcepinfo).bstrHelpFile = ManuallyDrop::new(BSTR::default());
                        (*pexcepinfo).dwHelpContext = 0;
                        (*pexcepinfo).scode = e.code().0;
                    }
                }
                Err(windows::core::Error::from_hresult(DISP_E_EXCEPTION))
            } else {
                Ok(())
            }
        } else {
            Err(windows::core::Error::from_hresult(DISP_E_MEMBERNOTFOUND))
        }
    }
}
