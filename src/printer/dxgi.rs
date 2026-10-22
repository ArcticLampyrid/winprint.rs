use crate::printer::deleted_from_queue;
use crate::printer::PrintOutcome;
use crate::printer::PrinterDevice;
use crate::ticket::PrintTicket;
use crate::ticket::ToDevModeError;
use crate::utils::com::ComInitializer;
use crate::utils::print_completion_source::PrintCompletionSource;
use crate::utils::print_completion_source::PrintCompletionWaiter;
use crate::utils::wchar;
use std::cmp::max;
use std::ffi::OsStr;
use std::ops::ControlFlow;
use std::ptr;
use thiserror::Error;
use windows::core::Interface;
use windows::core::PCWSTR;
use windows::Win32::Foundation::E_UNEXPECTED;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_F;
use windows::Win32::Graphics::Direct2D::D2D1CreateFactory;
use windows::Win32::Graphics::Direct2D::ID2D1CommandList;
use windows::Win32::Graphics::Direct2D::ID2D1Device;
use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext;
use windows::Win32::Graphics::Direct2D::ID2D1Factory1;
use windows::Win32::Graphics::Direct2D::ID2D1PrintControl;
use windows::Win32::Graphics::Direct2D::D2D1_DEVICE_CONTEXT_OPTIONS_NONE;
use windows::Win32::Graphics::Direct2D::D2D1_FACTORY_OPTIONS;
use windows::Win32::Graphics::Direct2D::D2D1_FACTORY_TYPE_MULTI_THREADED;
use windows::Win32::Graphics::Direct2D::D2D1_PRINT_CONTROL_PROPERTIES;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::D3D11CreateDevice;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext;
use windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_BGRA_SUPPORT;
use windows::Win32::Graphics::Direct3D11::D3D11_SDK_VERSION;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::DEVMODEW;
use windows::Win32::Graphics::Imaging::CLSID_WICImagingFactory2;
use windows::Win32::Graphics::Imaging::D2D::IWICImagingFactory2;
use windows::Win32::Storage::Xps::Printing::IPrintDocumentPackageStatusEvent;
use windows::Win32::Storage::Xps::Printing::IPrintDocumentPackageTarget;
use windows::Win32::Storage::Xps::Printing::IPrintDocumentPackageTargetFactory;
use windows::Win32::Storage::Xps::Printing::PrintDocumentPackageTargetFactory;
use windows::Win32::System::Com::CoCreateInstance;
use windows::Win32::System::Com::IConnectionPointContainer;
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::Win32::UI::Shell::SHCreateMemStream;

#[derive(Error, Debug)]
/// Represents an error occurred while printing via DXGI.
pub enum DxgiPrintContextError {
    /// Print ticket error.
    #[error("Print Ticker Error")]
    PrintTicketError(#[source] ToDevModeError),
    /// Failed to create event.
    #[error("Failed to create event")]
    FailedToCreateEvent(#[source] windows::core::Error),
    /// Failed to initialize DirectX.
    #[error("Failed to initialize DirectX")]
    InitializeDirectX(#[source] windows::core::Error),
    /// Failed to start job.
    #[error("Failed to start job")]
    FailedToStartJob(#[source] windows::core::Error),
    /// Stream not allocated.
    #[error("Stream not allocated")]
    StreamNotAllocated,
    /// Failed to close print control.
    #[error("Failed to close print control")]
    FailedToClosePrintControl(#[source] windows::core::Error),
    /// The print pipeline failed the job.
    #[error("The print job failed")]
    JobFailed(#[source] windows::core::Error),
}

const DEFAULT_DPI: u32 = 300;

/// Represents a print context during printing via DXGI.
#[allow(missing_docs)]
pub struct DxgiPrintContext {
    pub d3d_device: ID3D11Device,
    pub d3d_context: ID3D11DeviceContext,
    pub dxgi_device: IDXGIDevice,
    pub d2d_factory: ID2D1Factory1,
    pub d2d_device: ID2D1Device,
    pub d2d_context: ID2D1DeviceContext,
    pub wic_factory: IWICImagingFactory2,
    pub document_target_factory: IPrintDocumentPackageTargetFactory,
    pub document_target: IPrintDocumentPackageTarget,
    pub print_control: ID2D1PrintControl,
    completion_waiter: PrintCompletionWaiter,
    /// Whether the job has ended, so that dropping the context leaves it alone.
    job_ended: bool,
    pub dpi: (u32, u32),
    // The last field will be dropped last, according to the drop order.
    _com_initializer: ComInitializer,
}

impl DxgiPrintContext {
    /// Create a new print context, used for printing via DXGI.
    pub fn new(
        device: &PrinterDevice,
        options: &PrintTicket,
        job_name: &OsStr,
    ) -> Result<Self, DxgiPrintContextError> {
        // Initialize COM before calling any COM functions.
        let _com_initializer = ComInitializer::new();
        unsafe {
            let dev_mode = options
                .to_dev_mode(device)
                .map_err(DxgiPrintContextError::PrintTicketError)?;
            let dpi = {
                let dev_mode = &*(dev_mode.as_ptr() as *const DEVMODEW);
                let dpi_x = if (dev_mode.dmFields & windows::Win32::Graphics::Gdi::DM_PRINTQUALITY)
                    .0
                    != 0
                {
                    Some(dev_mode.Anonymous1.Anonymous1.dmPrintQuality).filter(|x| *x > 0)
                } else {
                    None
                };
                let dpi_y =
                    if (dev_mode.dmFields & windows::Win32::Graphics::Gdi::DM_YRESOLUTION).0 != 0 {
                        Some(dev_mode.dmYResolution).filter(|y| *y > 0)
                    } else {
                        None
                    };
                match (dpi_x, dpi_y) {
                    (Some(x), Some(y)) => (x as u32, y as u32),
                    (Some(x), None) => (x as u32, x as u32),
                    (None, Some(y)) => (y as u32, y as u32),
                    (None, None) => (DEFAULT_DPI, DEFAULT_DPI),
                }
            };
            let mut d3d_device: Option<ID3D11Device> = None;
            let mut d3d_context: Option<ID3D11DeviceContext> = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(ptr::addr_of_mut!(d3d_device)),
                None,
                Some(ptr::addr_of_mut!(d3d_context)),
            )
            .map_err(DxgiPrintContextError::InitializeDirectX)?;
            let d3d_device = d3d_device.ok_or(DxgiPrintContextError::InitializeDirectX(
                windows::core::Error::from_hresult(E_UNEXPECTED),
            ))?;
            let d3d_context = d3d_context.ok_or(DxgiPrintContextError::InitializeDirectX(
                windows::core::Error::from_hresult(E_UNEXPECTED),
            ))?;
            let dxgi_device = d3d_device
                .cast::<IDXGIDevice>()
                .map_err(DxgiPrintContextError::InitializeDirectX)?;

            let d2d_factory_options = D2D1_FACTORY_OPTIONS::default();
            // Windows.Data.Pdf rasterizes Type 3 glyphs on WinRT thread pool
            // workers, and every worker draws into this device context. A
            // single threaded factory leaves the context's glyph realization
            // cache unsynchronized, so concurrent workers corrupt it and
            // crash in d2d1.dll. A multi threaded factory makes Direct2D
            // serialize access to that state. See issue #2.
            let d2d_factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_MULTI_THREADED, Some(&d2d_factory_options))
                    .map_err(DxgiPrintContextError::InitializeDirectX)?;

            let d2d_device = d2d_factory
                .CreateDevice(&dxgi_device)
                .map_err(DxgiPrintContextError::InitializeDirectX)?;
            let d2d_context = d2d_device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
                .map_err(DxgiPrintContextError::InitializeDirectX)?;
            d2d_context.SetDpi(dpi.0 as f32, dpi.1 as f32);

            let wic_factory: IWICImagingFactory2 =
                CoCreateInstance(&CLSID_WICImagingFactory2, None, CLSCTX_ALL)
                    .map_err(DxgiPrintContextError::InitializeDirectX)?;

            let document_target_factory: IPrintDocumentPackageTargetFactory =
                CoCreateInstance(&PrintDocumentPackageTargetFactory, None, CLSCTX_ALL)
                    .map_err(DxgiPrintContextError::FailedToStartJob)?;
            let print_ticket_stream = SHCreateMemStream(Some(options.get_xml()))
                .ok_or(DxgiPrintContextError::StreamNotAllocated)?;
            let document_target = document_target_factory
                .CreateDocumentPackageTargetForPrintJob(
                    PCWSTR(wchar::to_wide_chars(device.os_name()).as_ptr()),
                    PCWSTR(wchar::to_wide_chars(job_name).as_ptr()),
                    None,
                    &print_ticket_stream,
                )
                .map_err(DxgiPrintContextError::FailedToStartJob)?;
            // The job exists from here on: cancel it on any early return (or panic) below.
            let job_guard = scopeguard::guard(document_target.clone(), |target| {
                let _ = target.Cancel();
            });
            let print_control_options = D2D1_PRINT_CONTROL_PROPERTIES {
                rasterDPI: max(dpi.0, dpi.1) as f32,
                ..Default::default()
            };
            let print_control = d2d_device
                .CreatePrintControl(
                    &wic_factory,
                    &document_target,
                    Some(ptr::addr_of!(print_control_options)),
                )
                .map_err(DxgiPrintContextError::FailedToStartJob)?;

            let completion_source = PrintCompletionSource::new();
            let completion_waiter = completion_source.waiter();
            let completion_source: IPrintDocumentPackageStatusEvent = completion_source.into();
            let cpc: IConnectionPointContainer = document_target
                .cast()
                .map_err(DxgiPrintContextError::FailedToCreateEvent)?;
            cpc.FindConnectionPoint(&IPrintDocumentPackageStatusEvent::IID)
                .map_err(DxgiPrintContextError::FailedToCreateEvent)?
                .Advise(&completion_source)
                .map_err(DxgiPrintContextError::FailedToCreateEvent)?;

            // The finished context owns the job from here on.
            let _ = scopeguard::ScopeGuard::into_inner(job_guard);
            Ok(Self {
                d3d_device,
                d3d_context,
                dxgi_device,
                d2d_factory,
                d2d_device,
                d2d_context,
                wic_factory,
                document_target_factory,
                document_target,
                print_control,
                completion_waiter,
                job_ended: false,
                dpi,
                _com_initializer,
            })
        }
    }

    /// Add a page to the job, breaking when the job has been deleted from the print queue.
    pub fn add_page(
        &self,
        page: &ID2D1CommandList,
        size: D2D_SIZE_F,
    ) -> windows::core::Result<ControlFlow<PrintOutcome>> {
        match unsafe { self.print_control.AddPage(page, size, None, None, None) } {
            Err(error) if deleted_from_queue(error.code()) => {
                Ok(ControlFlow::Break(PrintOutcome::Canceled))
            }
            added => added.map(ControlFlow::Continue),
        }
    }

    /// Decide whether to go on submitting pages.
    ///
    /// Cancels the job if `requested` breaks, as a progress callback does to cancel it.
    /// Otherwise, checks whether the job has already ended, for example canceled outside this
    /// process. Breaks with the outcome once the job is over.
    pub fn checkpoint(
        &self,
        requested: ControlFlow<()>,
    ) -> Result<ControlFlow<PrintOutcome>, DxgiPrintContextError> {
        if requested.is_break() {
            unsafe {
                let _ = self.document_target.Cancel();
            }
            return Ok(ControlFlow::Break(PrintOutcome::Canceled));
        }
        self.completion_waiter
            .poll()
            .map_or(Ok(ControlFlow::Continue(())), |end| {
                end.map(ControlFlow::Break)
                    .map_err(DxgiPrintContextError::JobFailed)
            })
    }

    /// Close the print context and wait until the print job has been submitted.
    pub fn close_and_wait(mut self) -> Result<PrintOutcome, DxgiPrintContextError> {
        let closed = unsafe { self.print_control.Close() };
        // A job that has ended, such as one canceled outside this process, may fail to close.
        let end = match (closed, self.completion_waiter.poll()) {
            (Err(error), None) if deleted_from_queue(error.code()) => Ok(PrintOutcome::Canceled),
            (Err(error), None) => {
                return Err(DxgiPrintContextError::FailedToClosePrintControl(error))
            }
            _ => self
                .completion_waiter
                .wait()
                .map_err(DxgiPrintContextError::JobFailed),
        };
        self.job_ended = true;
        end
    }
}

impl Drop for DxgiPrintContext {
    /// Cancel the job unless it has ended.
    ///
    /// Releasing an unfinished job submits it, so an error or a panic while adding pages would
    /// otherwise print the pages added so far.
    fn drop(&mut self) {
        if !self.job_ended {
            unsafe {
                let _ = self.document_target.Cancel();
            }
        }
    }
}
