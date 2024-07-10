use crate::printer::FilePrinter;
use crate::printer::PrinterDevice;
use crate::ticket::PrintTicket;
use crate::ticket::ToDevModeError;
use crate::utils::print_completion_source::PrintCompletionSource;
use crate::utils::wchar;
use scopeguard::defer;
use std::cmp::max;
use std::path::Path;
use std::ptr;
use thiserror::Error;
use windows::core::Interface;
use windows::core::HSTRING;
use windows::core::PCWSTR;
use windows::Data::Pdf::PdfDocument;
use windows::Storage::StorageFile;
use windows::Win32::Foundation::E_UNEXPECTED;
use windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_F;
use windows::Win32::Graphics::Direct2D::D2D1CreateFactory;
use windows::Win32::Graphics::Direct2D::ID2D1Factory1;
use windows::Win32::Graphics::Direct2D::D2D1_DEVICE_CONTEXT_OPTIONS_NONE;
use windows::Win32::Graphics::Direct2D::D2D1_FACTORY_OPTIONS;
use windows::Win32::Graphics::Direct2D::D2D1_FACTORY_TYPE_SINGLE_THREADED;
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
use windows::Win32::Storage::Xps::Printing::IPrintDocumentPackageTargetFactory;
use windows::Win32::Storage::Xps::Printing::PrintDocumentPackageTargetFactory;
use windows::Win32::System::Com::CoCreateInstance;
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::CoUninitialize;
use windows::Win32::System::Com::IConnectionPointContainer;
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::Win32::System::Com::COINIT_MULTITHREADED;
use windows::Win32::System::WinRT::Pdf::PdfCreateRenderer;
use windows::Win32::System::WinRT::Pdf::PDF_RENDER_PARAMS;
use windows::Win32::UI::Shell::SHCreateMemStream;

#[derive(Error, Debug)]
/// Represents an error from [`WinPdfPrinter`].
pub enum WinPdfPrinterError {
    /// Print ticket error.
    #[error("Print Ticker Error")]
    PrintTicketError(#[source] ToDevModeError),
    /// Failed to create event.
    #[error("Failed to create event")]
    FailedToCreateEvent(#[source] windows::core::Error),
    /// Failed to initialize DirectX.
    #[error("Failed to initialize DirectX")]
    InitializeDirectX(#[source] windows::core::Error),
    /// Invalid path.
    #[error("Invalid path")]
    InvalidPath(#[source] std::io::Error),
    /// Failed to open the document.
    #[error("Failed to open the document")]
    FailedToOpenDocument(#[source] windows::core::Error),
    /// Failed to start job.
    #[error("Failed to start job")]
    FailedToStartJob(#[source] windows::core::Error),
    /// Render error.
    #[error("Render error")]
    RenderError(#[source] windows::core::Error),
    /// Stream not allocated.
    #[error("Stream not allocated")]
    StreamNotAllocated,
}

/// A printer that uses [`Windows.Data.Pdf`](https://learn.microsoft.com/en-us/uwp/api/windows.data.pdf?view=winrt-26100) to to print PDF documents.
pub struct WinPdfPrinter {
    printer: PrinterDevice,
}

impl WinPdfPrinter {
    /// Create a new [`WinPdfPrinter`] for the given printer device.
    pub fn new(printer: PrinterDevice) -> Self {
        Self { printer }
    }
}

const DEFAULT_DPI: i16 = 300;

impl FilePrinter for WinPdfPrinter {
    type Options = PrintTicket;
    type Error = WinPdfPrinterError;
    fn print(
        &self,
        path: &Path,
        options: PrintTicket,
    ) -> std::result::Result<(), WinPdfPrinterError> {
        unsafe {
            let dev_mode = options
                .to_dev_mode(&self.printer)
                .map_err(WinPdfPrinterError::PrintTicketError)?;
            let raster_dpi = {
                let dev_mode = &*(dev_mode.as_ptr() as *const DEVMODEW);
                let dpi_x = if (dev_mode.dmFields & windows::Win32::Graphics::Gdi::DM_PRINTQUALITY)
                    .0
                    != 0
                {
                    Some(dev_mode.Anonymous1.Anonymous1.dmPrintQuality)
                } else {
                    None
                };
                let dpi_y =
                    if (dev_mode.dmFields & windows::Win32::Graphics::Gdi::DM_YRESOLUTION).0 != 0 {
                        Some(dev_mode.dmYResolution)
                    } else {
                        None
                    };
                match (dpi_x, dpi_y) {
                    (Some(x), Some(y)) => max(x, y),
                    (Some(x), None) => x,
                    (None, Some(y)) => y,
                    (None, None) => DEFAULT_DPI,
                }
            };
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            defer! {
                CoUninitialize();
            }
            let mut d3d_device: Option<ID3D11Device> = None;
            let mut d3d_context: Option<ID3D11DeviceContext> = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(ptr::addr_of_mut!(d3d_device)),
                None,
                Some(ptr::addr_of_mut!(d3d_context)),
            )
            .map_err(WinPdfPrinterError::InitializeDirectX)?;
            let d3d_device = d3d_device.ok_or(WinPdfPrinterError::InitializeDirectX(
                windows::core::Error::from_hresult(E_UNEXPECTED),
            ))?;
            let dxgi_device = d3d_device
                .cast::<IDXGIDevice>()
                .map_err(WinPdfPrinterError::InitializeDirectX)?;

            let d2d_factory_options = D2D1_FACTORY_OPTIONS::default();
            let d2d_factory: ID2D1Factory1 = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                Some(&d2d_factory_options),
            )
            .map_err(WinPdfPrinterError::InitializeDirectX)?;

            let d2d_device = d2d_factory
                .CreateDevice(&dxgi_device)
                .map_err(WinPdfPrinterError::InitializeDirectX)?;
            let d2d_context = d2d_device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
                .map_err(WinPdfPrinterError::InitializeDirectX)?;

            let wic_factory: IWICImagingFactory2 =
                CoCreateInstance(&CLSID_WICImagingFactory2, None, CLSCTX_ALL)
                    .map_err(WinPdfPrinterError::InitializeDirectX)?;

            let absolute_path =
                std::path::absolute(path).map_err(WinPdfPrinterError::InvalidPath)?;
            let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(absolute_path.as_path()))
                .map_err(WinPdfPrinterError::FailedToOpenDocument)?
                .get()
                .map_err(WinPdfPrinterError::FailedToOpenDocument)?;
            let pdf_document = PdfDocument::LoadFromFileAsync(&file)
                .map_err(WinPdfPrinterError::FailedToOpenDocument)?
                .get()
                .map_err(WinPdfPrinterError::FailedToOpenDocument)?;

            let document_target_factory: IPrintDocumentPackageTargetFactory =
                CoCreateInstance(&PrintDocumentPackageTargetFactory, None, CLSCTX_ALL)
                    .map_err(WinPdfPrinterError::FailedToStartJob)?;
            let print_ticket_stream = SHCreateMemStream(Some(options.get_xml()))
                .ok_or(WinPdfPrinterError::StreamNotAllocated)?;
            let document_target = document_target_factory
                .CreateDocumentPackageTargetForPrintJob(
                    PCWSTR(wchar::to_wide_chars(self.printer.os_name()).as_ptr()),
                    PCWSTR(
                        wchar::to_wide_chars(path.file_name().unwrap_or(path.as_ref())).as_ptr(),
                    ),
                    None,
                    &print_ticket_stream,
                )
                .map_err(WinPdfPrinterError::FailedToStartJob)?;
            let print_control_options = D2D1_PRINT_CONTROL_PROPERTIES {
                rasterDPI: raster_dpi as f32,
                ..Default::default()
            };
            let print_control = d2d_device
                .CreatePrintControl(
                    &wic_factory,
                    &document_target,
                    Some(ptr::addr_of!(print_control_options)),
                )
                .map_err(WinPdfPrinterError::FailedToStartJob)?;

            let completion_source =
                PrintCompletionSource::new().map_err(WinPdfPrinterError::FailedToCreateEvent)?;
            let completion_waiter = completion_source.waiter();
            let completion_source: IPrintDocumentPackageStatusEvent = completion_source.into();
            let cpc: IConnectionPointContainer = document_target
                .cast()
                .map_err(WinPdfPrinterError::FailedToCreateEvent)?;
            cpc.FindConnectionPoint(&IPrintDocumentPackageStatusEvent::IID)
                .map_err(WinPdfPrinterError::FailedToCreateEvent)?
                .Advise(&completion_source)
                .map_err(WinPdfPrinterError::FailedToCreateEvent)?;

            let pdf_renderer =
                PdfCreateRenderer(&dxgi_device).map_err(WinPdfPrinterError::RenderError)?;
            let pdf_render_options = PDF_RENDER_PARAMS {
                IgnoreHighContrast: true.into(),
                ..Default::default()
            };
            let page_count = pdf_document
                .PageCount()
                .map_err(WinPdfPrinterError::RenderError)?;
            for i in 0..page_count {
                let pdf_page = pdf_document
                    .GetPage(i)
                    .map_err(WinPdfPrinterError::RenderError)?;

                let command_list = d2d_context
                    .CreateCommandList()
                    .map_err(WinPdfPrinterError::RenderError)?;
                d2d_context.SetTarget(&command_list);
                d2d_context.BeginDraw();
                pdf_renderer
                    .RenderPageToDeviceContext(&pdf_page, &d2d_context, Some(&pdf_render_options))
                    .map_err(WinPdfPrinterError::RenderError)?;
                d2d_context
                    .EndDraw(None, None)
                    .map_err(WinPdfPrinterError::RenderError)?;
                command_list
                    .Close()
                    .map_err(WinPdfPrinterError::RenderError)?;

                let pdf_size = pdf_page.Size().map_err(WinPdfPrinterError::RenderError)?;
                let dx_size = D2D_SIZE_F {
                    width: pdf_size.Width as f32,
                    height: pdf_size.Height as f32,
                };
                print_control
                    .AddPage(&command_list, dx_size, None, None, None)
                    .map_err(WinPdfPrinterError::RenderError)?;
            }
            print_control
                .Close()
                .map_err(WinPdfPrinterError::RenderError)?;

            completion_waiter.wait();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::WinPdfPrinter;
    use crate::{printer::FilePrinter, test_utils::null_device};
    use std::path::Path;

    #[test]
    fn print_simple_pdf_document() {
        let device = null_device::thread_local();
        let pdf = WinPdfPrinter::new(device);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.pdf");
        pdf.print(path.as_path(), Default::default()).unwrap();
    }
}
