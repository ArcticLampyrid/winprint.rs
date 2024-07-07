use std::mem::ManuallyDrop;
use windows::core::w;
use windows::core::BSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::{
    DISP_E_BADPARAMCOUNT, DISP_E_EXCEPTION, DISP_E_MEMBERNOTFOUND, DISP_E_TYPEMISMATCH,
    DISP_E_UNKNOWNNAME,
};
use windows::Win32::Storage::Xps::Printing::PrintDocumentPackageCompletion_Canceled;
use windows::Win32::Storage::Xps::Printing::PrintDocumentPackageCompletion_Completed;
use windows::Win32::Storage::Xps::Printing::PrintDocumentPackageCompletion_Failed;
use windows::Win32::System::Com::DISPATCH_METHOD;
use windows::Win32::System::Ole::DISPID_UNKNOWN;
use windows::Win32::System::Threading::CreateEventW;
use windows::Win32::System::Threading::SetEvent;
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::Win32::System::Threading::INFINITE;
use windows::Win32::System::Variant::VT_BYREF;
use windows::{
    core::implement,
    Win32::{
        Foundation::E_NOTIMPL,
        Storage::Xps::Printing::{
            IPrintDocumentPackageStatusEvent, IPrintDocumentPackageStatusEvent_Impl,
            PrintDocumentPackageStatus,
        },
        System::Com::{
            IDispatch, IDispatch_Impl, ITypeInfo, DISPATCH_FLAGS, DISPPARAMS, EXCEPINFO,
        },
    },
};

#[implement(IPrintDocumentPackageStatusEvent, IDispatch)]
pub struct PrintCompletionSource {
    event: HANDLE,
}

impl PrintCompletionSource {
    pub fn new() -> windows::core::Result<Self> {
        let event = unsafe { CreateEventW(None, true, false, None)? };
        Ok(Self { event })
    }
    pub fn waiter(&self) -> PrintCompletionWaiter {
        PrintCompletionWaiter { event: self.event }
    }
}

pub struct PrintCompletionWaiter {
    event: HANDLE,
}

impl PrintCompletionWaiter {
    pub fn wait(&self) {
        unsafe {
            let _ = WaitForSingleObject(self.event, INFINITE);
        }
    }
}

impl Drop for PrintCompletionSource {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.event);
        }
    }
}

impl IPrintDocumentPackageStatusEvent_Impl for PrintCompletionSource_Impl {
    fn PackageStatusUpdated(
        &self,
        packagestatus: *const PrintDocumentPackageStatus,
    ) -> windows::core::Result<()> {
        unsafe {
            if packagestatus.is_null() {
                return Err(windows::core::Error::from_hresult(E_INVALIDARG));
            }
            let status = &*packagestatus;
            match status.Completion {
                #[allow(non_upper_case_globals)]
                PrintDocumentPackageCompletion_Canceled
                | PrintDocumentPackageCompletion_Completed
                | PrintDocumentPackageCompletion_Failed => SetEvent(self.event)?,
                _ => {}
            }
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
        pvarresult: *mut windows::core::VARIANT,
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
            let vt = unsafe { arg.as_raw().Anonymous.Anonymous.vt };
            if vt & VT_BYREF.0 == 0 {
                if !puargerr.is_null() {
                    unsafe {
                        *puargerr = 0;
                    }
                }
                return Err(windows::core::Error::from_hresult(DISP_E_TYPEMISMATCH));
            }
            let status = unsafe { arg.as_raw().Anonymous.Anonymous.Anonymous.byref }
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
