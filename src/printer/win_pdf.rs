use super::DxgiPrintContext;
use super::DxgiPrintContextError;
use crate::printer::FilePrinter;
use crate::printer::PrinterDevice;
use crate::ticket::PrintTicket;
use std::path::Path;
use thiserror::Error;
use windows::core::HSTRING;
use windows::Data::Pdf::PdfDocument;
use windows::Storage::StorageFile;
use windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_F;
use windows::Win32::System::WinRT::Pdf::PdfCreateRenderer;
use windows::Win32::System::WinRT::Pdf::PDF_RENDER_PARAMS;
use windows_numerics::Matrix3x2;

#[derive(Error, Debug)]
/// Represents an error from [`WinPdfPrinter`].
pub enum WinPdfPrinterError {
    /// DXGI print context error.
    #[error("DXGI print context error")]
    DxgiPrintContextError(#[from] DxgiPrintContextError),
    /// Invalid path.
    #[error("Invalid path")]
    InvalidPath(#[source] std::io::Error),
    /// Failed to open the document.
    #[error("Failed to open the document")]
    FailedToOpenDocument(#[source] windows::core::Error),
    /// The page does not fit the requested resolution.
    #[error("The page ({dips} DIPs) cannot be rendered at {scale} times its size")]
    PageSize {
        /// The side of the page in device-independent pixels.
        dips: f32,
        /// The factor of the requested resolution over 96 dpi.
        scale: f32,
    },
    /// A page could not be rendered or recorded at the requested resolution.
    ///
    /// Direct3D 11 guarantees textures only up to 16384 texels a side
    /// (<https://learn.microsoft.com/windows/win32/direct3d11/overviews-direct3d-11-resources-limits>)
    /// and Direct2D printing limits a bitmap to 65535 device units
    /// (<https://learn.microsoft.com/windows/win32/direct2d/d1163>). Neither was reached when
    /// pages were rendered whole up to A4 at 9600 dpi, but a driver or GPU may have its own limits.
    #[error("The page could not be rendered at {render_dpi:?} dpi ({render_size_px:?} pixels)")]
    RenderAtResolution {
        /// The render resolution.
        render_dpi: (u32, u32),
        /// The render size in pixels at that resolution.
        render_size_px: (u32, u32),
        /// The failing call's error.
        #[source]
        source: windows::core::Error,
    },
    /// Render error.
    #[error("Render error")]
    RenderError(#[source] windows::core::Error),
}

/// A printer that uses [`Windows.Data.Pdf`](https://learn.microsoft.com/en-us/uwp/api/windows.data.pdf?view=winrt-26100) to to print PDF documents.
pub struct WinPdfPrinter {
    printer: PrinterDevice,
    override_render_dpi: Option<(u32, u32)>,
}

impl WinPdfPrinter {
    /// Create a new [`WinPdfPrinter`] for the given printer device.
    pub fn new(printer: PrinterDevice) -> Self {
        Self {
            printer,
            override_render_dpi: None,
        }
    }
}

const DIP_PER_INCH: f32 = 96.0;

impl WinPdfPrinter {
    /// Override the resolution`Windows.Data.Pdf` draws a page at.
    pub fn override_render_dpi(&mut self, x: u32, y: u32) {
        self.override_render_dpi = Some((x, y));
    }
}

impl FilePrinter for WinPdfPrinter {
    type Options = PrintTicket;
    type Error = WinPdfPrinterError;
    fn print(
        &self,
        path: &Path,
        options: PrintTicket,
    ) -> std::result::Result<(), WinPdfPrinterError> {
        let context = DxgiPrintContext::new(
            &self.printer,
            &options,
            path.file_name().unwrap_or(path.as_ref()),
        )?;
        let dxgi_device = &context.dxgi_device;
        let print_control = &context.print_control;
        let d2d_context = &context.d2d_context;
        let render_dpi = self.override_render_dpi.unwrap_or(context.dpi);
        let scale = (
            render_dpi.0 as f32 / DIP_PER_INCH,
            render_dpi.1 as f32 / DIP_PER_INCH,
        );
        unsafe {
            let absolute_path =
                std::path::absolute(path).map_err(WinPdfPrinterError::InvalidPath)?;
            let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(absolute_path.as_path()))
                .map_err(WinPdfPrinterError::FailedToOpenDocument)?
                .join()
                .map_err(WinPdfPrinterError::FailedToOpenDocument)?;
            let pdf_document = PdfDocument::LoadFromFileAsync(&file)
                .map_err(WinPdfPrinterError::FailedToOpenDocument)?
                .join()
                .map_err(WinPdfPrinterError::FailedToOpenDocument)?;

            let pdf_renderer =
                PdfCreateRenderer(dxgi_device).map_err(WinPdfPrinterError::RenderError)?;
            let page_count = pdf_document
                .PageCount()
                .map_err(WinPdfPrinterError::RenderError)?;
            for i in 0..page_count {
                let pdf_page = pdf_document
                    .GetPage(i)
                    .map_err(WinPdfPrinterError::RenderError)?;

                let pdf_size = pdf_page.Size().map_err(WinPdfPrinterError::RenderError)?;
                let render_size_px = (
                    scaled(pdf_size.Width, scale.0)?,
                    scaled(pdf_size.Height, scale.1)?,
                );
                let at_resolution = |source| WinPdfPrinterError::RenderAtResolution {
                    render_dpi,
                    render_size_px,
                    source,
                };
                let pdf_render_options = PDF_RENDER_PARAMS {
                    DestinationWidth: render_size_px.0,
                    DestinationHeight: render_size_px.1,
                    IgnoreHighContrast: true.into(),
                    ..Default::default()
                };
                let command_list = d2d_context
                    .CreateCommandList()
                    .map_err(WinPdfPrinterError::RenderError)?;
                d2d_context.SetTarget(&command_list);
                d2d_context.BeginDraw();
                d2d_context.SetTransform(&Matrix3x2::scale(
                    pdf_size.Width / pdf_render_options.DestinationWidth as f32,
                    pdf_size.Height / pdf_render_options.DestinationHeight as f32,
                ));
                let rendered = pdf_renderer.RenderPageToDeviceContext(
                    &pdf_page,
                    d2d_context,
                    Some(&pdf_render_options),
                );
                d2d_context.SetTransform(&Matrix3x2::identity());
                rendered.map_err(at_resolution)?;
                d2d_context.EndDraw(None, None).map_err(at_resolution)?;
                command_list
                    .Close()
                    .map_err(WinPdfPrinterError::RenderError)?;

                let dx_size = D2D_SIZE_F {
                    width: pdf_size.Width as f32,
                    height: pdf_size.Height as f32,
                };
                print_control
                    .AddPage(&command_list, dx_size, None, None, None)
                    .map_err(at_resolution)?;
            }
        }
        context.close_and_wait()?;
        Ok(())
    }
}

/// A page side in pixels at `scale` times its size in DIPs.
fn scaled(dips: f32, scale: f32) -> Result<u32, WinPdfPrinterError> {
    let pixels = (dips * scale).ceil();
    if pixels >= 1.0 && pixels < u32::MAX as f32 {
        Ok(pixels as u32)
    } else {
        Err(WinPdfPrinterError::PageSize { dips, scale })
    }
}

#[cfg(test)]
mod tests {
    use super::{scaled, WinPdfPrinter};
    use crate::{printer::FilePrinter, test_utils::null_device};
    use std::path::Path;

    #[test]
    fn print_simple_pdf_document() {
        let device = null_device::thread_local();
        let pdf = WinPdfPrinter::new(device);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.pdf");
        pdf.print(path.as_path(), Default::default()).unwrap();
    }

    /// `Windows.Data.Pdf` rasterizes Type 3 glyphs on WinRT thread pool workers, all drawing
    /// into the same device context. With a single threaded Direct2D factory the workers race
    /// on the context's glyph realization cache and crash in `d2d1.dll` (issue #2).
    #[test]
    fn print_pdf_document_with_multiple_type_3_fonts() {
        let device = null_device::thread_local();
        let pdf = WinPdfPrinter::new(device);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/type3_glyphs.pdf");
        pdf.print(path.as_path(), Default::default()).unwrap();
    }

    /// A4 at 1600 dpi is 13229 x 18709 px, taller than the 16384 texels Direct3D 11 allows a
    /// texture. The page is still rendered whole, in one pass: `RenderPageToDeviceContext` records
    /// into a command list and never allocates a page-sized texture.
    #[test]
    fn a_page_larger_than_a_texture_prints_at_a_high_resolution() {
        let device = null_device::thread_local();
        let mut pdf = WinPdfPrinter::new(device);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.pdf");
        pdf.override_render_dpi(1600, 1600);
        pdf.print(path.as_path(), Default::default()).unwrap();
    }

    #[test]
    fn a_page_side_is_scaled_and_never_overflows() {
        assert_eq!(scaled(793.7, 1600.0 / 96.0).unwrap(), 13229);
        assert_eq!(scaled(1122.5, 1.0).unwrap(), 1123);
        assert!(scaled(0.0, 1.0).is_err());
        assert!(scaled(f32::NAN, 1.0).is_err());
        assert!(scaled(1e12, 100.0).is_err());
    }
}
