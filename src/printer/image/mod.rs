mod color;

use super::DxgiPrintContextError;
use super::DxgiRenderContext;
use crate::printer::FilePrinter;
use crate::printer::PrintOutcome;
use crate::printer::PrintProgress;
use crate::printer::PrinterDevice;
use crate::ticket::PrintTicket;
use crate::utils::com::ComInitializer;
use crate::utils::wchar;
use color::Converter;
use std::ops::ControlFlow;
use std::path::Path;
use thiserror::Error;
use windows::core::PCWSTR;
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_F;
use windows::Win32::Graphics::Direct2D::ID2D1ColorContext;
use windows::Win32::Graphics::Direct2D::ID2D1CommandList;
use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext;
use windows::Win32::Graphics::Direct2D::D2D1_COLOR_SPACE_SCRGB;
use windows::Win32::Graphics::Direct2D::D2D1_COLOR_SPACE_SRGB;
use windows::Win32::Graphics::Direct2D::D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC;
use windows::Win32::Graphics::Imaging::IWICBitmapFrameDecode;
use windows::Win32::Graphics::Imaging::WICDecodeMetadataCacheOnDemand;

#[derive(Error, Debug)]
/// Represents an error from [`ImagePrinter`].
pub enum ImagePrinterError {
    /// DXGI print context error.
    #[error("DXGI print context error")]
    DxgiPrintContextError(#[from] DxgiPrintContextError),
    /// Invalid path.
    #[error("Invalid path")]
    InvalidPath(#[source] std::io::Error),
    /// Failed to open the document.
    #[error("Failed to open the document")]
    FailedToOpenDocument(#[source] windows::core::Error),
    /// Render error.
    #[error("Render error")]
    RenderError(#[source] windows::core::Error),
}

/// A printer that prints images. Multiple frames in a single image file will be printed as separate pages.
///
/// Images are color managed with their embedded color profiles. See
/// [`ImagePrinter::set_wide_color_gamut`] for how colors outside sRGB are printed.
pub struct ImagePrinter {
    printer: PrinterDevice,
    wide_color_gamut: bool,
}

impl ImagePrinter {
    /// Create a new [`ImagePrinter`] for the given printer device.
    pub fn new(printer: PrinterDevice) -> Self {
        Self {
            printer,
            wide_color_gamut: true,
        }
    }

    /// Set whether to keep colors outside the sRGB gamut. Enabled by default.
    ///
    /// When enabled, an image may be printed in scRGB, so that the printer driver
    /// receives its full gamut.
    ///
    /// When disabled, every image is converted into sRGB, clipping the colors outside it.
    pub fn set_wide_color_gamut(&mut self, enabled: bool) {
        self.wide_color_gamut = enabled;
    }
}

/// An image file, whose frames are printed as pages.
struct Image {
    frames: Vec<Frame>,
    // Keeps COM initialized for the frames, which outlive the print context. The last field will
    // be dropped last, according to the drop order.
    _com_initializer: ComInitializer,
}

impl Image {
    /// Open the image at `path`, reading its frames.
    fn open(render: &DxgiRenderContext, path: &Path) -> Result<Self, ImagePrinterError> {
        let _com_initializer = ComInitializer::new();
        let absolute_path = std::path::absolute(path).map_err(ImagePrinterError::InvalidPath)?;
        unsafe {
            let decoder = render
                .wic_factory
                .CreateDecoderFromFilename(
                    PCWSTR(wchar::to_wide_chars(absolute_path.as_os_str()).as_ptr()),
                    None,
                    GENERIC_READ,
                    WICDecodeMetadataCacheOnDemand,
                )
                .map_err(ImagePrinterError::FailedToOpenDocument)?;
            let frames = (0..decoder
                .GetFrameCount()
                .map_err(ImagePrinterError::RenderError)?)
                .map(|i| {
                    decoder
                        .GetFrame(i)
                        .and_then(|decode| Frame::new(render, decode))
                })
                .collect::<windows::core::Result<_>>()
                .map_err(ImagePrinterError::RenderError)?;
            Ok(Self {
                frames,
                _com_initializer,
            })
        }
    }
}

/// A frame of an image, printed as a page.
struct Frame {
    decode: IWICBitmapFrameDecode,
    /// The color context of the samples.
    color_context: ID2D1ColorContext,
}

impl Frame {
    fn new(
        render: &DxgiRenderContext,
        decode: IWICBitmapFrameDecode,
    ) -> windows::core::Result<Self> {
        Ok(Self {
            color_context: color::color_context(render, &decode)?,
            decode,
        })
    }

    /// The size of the page, from the size and the resolution of the frame.
    fn page_size(&self) -> windows::core::Result<D2D_SIZE_F> {
        let mut image_width = 0;
        let mut image_height = 0;
        let mut image_dpi_x = 0.0;
        let mut image_dpi_y = 0.0;
        unsafe {
            self.decode.GetSize(&mut image_width, &mut image_height)?;
            self.decode
                .GetResolution(&mut image_dpi_x, &mut image_dpi_y)?;
        }
        Ok(D2D_SIZE_F {
            width: (image_width as f64 * 96.0 / image_dpi_x) as f32,
            height: (image_height as f64 * 96.0 / image_dpi_y) as f32,
        })
    }

    /// Draw the frame, loaded by `converter`, over a page of its size.
    fn draw(
        &self,
        d2d_context: &ID2D1DeviceContext,
        converter: &Converter,
    ) -> windows::core::Result<(ID2D1CommandList, D2D_SIZE_F)> {
        let size = self.page_size()?;
        let bitmap = converter.bitmap(&self.decode, &self.color_context)?;
        unsafe {
            let page = d2d_context.CreateCommandList()?;
            d2d_context.SetTarget(&page);
            d2d_context.BeginDraw();
            d2d_context.DrawBitmap(
                &bitmap,
                Some(&D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: size.width,
                    bottom: size.height,
                }),
                1.0,
                D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
                None,
                None,
            );
            d2d_context.EndDraw(None, None)?;
            page.Close()?;
            Ok((page, size))
        }
    }
}

impl FilePrinter for ImagePrinter {
    type Options = PrintTicket;
    type Error = ImagePrinterError;
    fn print_with_progress(
        &self,
        path: &Path,
        options: PrintTicket,
        on_progress: &mut dyn FnMut(&PrintProgress) -> ControlFlow<()>,
    ) -> std::result::Result<PrintOutcome, ImagePrinterError> {
        let render = DxgiRenderContext::new()?;
        // The frames decide the color space of the job, so read them before starting it.
        let image = Image::open(&render, path)?;
        // sRGB keeps the job smaller, unless some frame needs a wide color gamut.
        let color_space = if self.wide_color_gamut
            && Converter::is_scrgb_available(&render)
            && image.frames.iter().any(
                |frame| unsafe { frame.color_context.GetColorSpace() } != D2D1_COLOR_SPACE_SRGB,
            ) {
            D2D1_COLOR_SPACE_SCRGB
        } else {
            D2D1_COLOR_SPACE_SRGB
        };
        let job_name = path.file_name().unwrap_or(path.as_ref());
        let context = render.start_job(&self.printer, &options, job_name, color_space)?;
        let converter =
            Converter::new(&context, color_space).map_err(ImagePrinterError::RenderError)?;
        let page_count = u32::try_from(image.frames.len()).unwrap_or(u32::MAX);
        if let ControlFlow::Break(outcome) =
            context.checkpoint(on_progress(&PrintProgress::new(0, page_count)))?
        {
            return Ok(outcome);
        }
        for (i, frame) in image.frames.iter().enumerate() {
            let (page, size) = frame
                .draw(&context.d2d_context, &converter)
                .map_err(ImagePrinterError::RenderError)?;
            if let ControlFlow::Break(outcome) = context
                .add_page(&page, size)
                .map_err(ImagePrinterError::RenderError)?
            {
                return Ok(outcome);
            }
            let pages_added = u32::try_from(i + 1).unwrap_or(u32::MAX);
            if let ControlFlow::Break(outcome) =
                context.checkpoint(on_progress(&PrintProgress::new(pages_added, page_count)))?
            {
                return Ok(outcome);
            }
        }
        Ok(context.close_and_wait()?)
    }
}

#[cfg(test)]
mod tests {
    use super::ImagePrinter;
    use crate::{
        printer::{test_helpers, FilePrinter, PrintOutcome},
        test_utils::null_device,
    };
    use std::path::Path;

    #[test]
    fn print_simple_tiff_document() {
        let device = null_device::thread_local();
        let image = ImagePrinter::new(device);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.tiff");
        assert_eq!(
            image.print(path.as_path(), Default::default()).unwrap(),
            PrintOutcome::Completed
        );
    }

    #[test]
    fn reports_progress() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.tiff");
        test_helpers::assert_reports_progress(
            &ImagePrinter::new(null_device::thread_local()),
            &path,
        );
    }

    #[test]
    fn cancels_on_request() {
        let device = null_device::thread_local();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.tiff");
        test_helpers::assert_cancels_on_request(&ImagePrinter::new(device.clone()), &device, &path);
    }

    #[test]
    fn observes_external_cancel() {
        let device = null_device::thread_local();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.tiff");
        test_helpers::assert_observes_external_cancel(
            &ImagePrinter::new(device.clone()),
            &device,
            &path,
        );
    }

    #[test]
    fn failure_leaves_no_job() {
        let device = null_device::thread_local();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.tiff");
        test_helpers::assert_failure_leaves_no_job(
            &ImagePrinter::new(device.clone()),
            &device,
            &path,
        );
    }
}
