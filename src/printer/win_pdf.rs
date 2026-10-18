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
    /// Measured on a GPU machine: nothing here failed for plain images and fills up to A4 at
    /// 9600 dpi, but a blend mode other than Normal (for example `/BM /Multiply`) over a very
    /// large area does. The print control then refuses the layer from `AddPage` with
    /// `D2DERR_EXCEEDS_MAX_BITMAP_SIZE` (0x8899001D), and drawing the page in tiles of 8192,
    /// 2048 or 1024 pixels did not help: it ended in `E_FAIL` after minutes.
    ///
    /// Direct3D 11 guarantees textures only up to 16384 texels a side
    /// (<https://learn.microsoft.com/windows/win32/direct3d11/overviews-direct3d-11-resources-limits>)
    /// and Direct2D printing limits a bitmap to 65535 device units
    /// (<https://learn.microsoft.com/windows/win32/direct2d/d1163>). Neither was reached when
    /// pages were rendered whole up to A4 at 9600 dpi, but a driver or GPU may have its own limits.
    #[error("The page could not be rendered at {dpi} dpi ({width_px} x {height_px} px)")]
    RenderAtResolution {
        /// The requested resolution.
        dpi: u32,
        /// The page width in pixels at that resolution.
        width_px: u32,
        /// The page height in pixels at that resolution.
        height_px: u32,
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
}

impl WinPdfPrinter {
    /// Create a new [`WinPdfPrinter`] for the given printer device.
    pub fn new(printer: PrinterDevice) -> Self {
        Self { printer }
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
        self.print_at(path, options, None)
    }
}

/// The resolution Direct2D (and `Windows.Data.Pdf` with it) draws a page at: 96 per inch.
const DIP_PER_INCH: f32 = 96.0;

impl WinPdfPrinter {
    /// Print `path` rendering at `dpi`, else at the job's resolution (the DEVMODE of the ticket).
    ///
    /// `Windows.Data.Pdf` draws the images of a page at the destination size, 96 dpi by default
    /// whatever the images are, so a page is rendered `dpi / 96` times larger and the transform
    /// scales it back onto the page.
    fn print_at(
        &self,
        path: &Path,
        options: PrintTicket,
        dpi: Option<u32>,
    ) -> std::result::Result<(), WinPdfPrinterError> {
        let context = DxgiPrintContext::new(
            &self.printer,
            &options,
            path.file_name().unwrap_or(path.as_ref()),
        )?;
        let dxgi_device = &context.dxgi_device;
        let print_control = &context.print_control;
        let d2d_context = &context.d2d_context;
        let scale = dpi.unwrap_or_else(|| job_dpi(context.raster_dpi)) as f32 / DIP_PER_INCH;
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
                let (width_px, height_px) = (
                    scaled(pdf_size.Width, scale)?,
                    scaled(pdf_size.Height, scale)?,
                );
                let at_resolution = |source| WinPdfPrinterError::RenderAtResolution {
                    dpi: dpi.unwrap_or_else(|| job_dpi(context.raster_dpi)),
                    width_px,
                    height_px,
                    source,
                };
                let pdf_render_options = PDF_RENDER_PARAMS {
                    DestinationWidth: width_px,
                    DestinationHeight: height_px,
                    IgnoreHighContrast: true.into(),
                    ..Default::default()
                };
                let command_list = d2d_context
                    .CreateCommandList()
                    .map_err(WinPdfPrinterError::RenderError)?;
                d2d_context.SetTarget(&command_list);
                d2d_context.BeginDraw();
                // The destination is whole pixels, so scale back by the page's own size over it:
                // the bitmap then covers the page exactly (1 / scale would overshoot by a pixel).
                d2d_context.SetTransform(&Matrix3x2 {
                    M11: pdf_size.Width / width_px as f32,
                    M22: pdf_size.Height / height_px as f32,
                    ..Matrix3x2::identity()
                });
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

/// The resolution of the job: the DEVMODE's, unless it holds one of the negative `DMRES_*` presets
/// (draft, low, medium, high), which are not a number of dots per inch; then 300.
fn job_dpi(raster_dpi: i16) -> u32 {
    u32::try_from(raster_dpi)
        .ok()
        .filter(|dpi| *dpi > 0)
        .unwrap_or(300)
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
    use super::{scaled, WinPdfPrinter, WinPdfPrinterError};
    use crate::{printer::FilePrinter, test_utils::null_device};
    use std::path::Path;

    #[test]
    fn print_simple_pdf_document() {
        let device = null_device::thread_local();
        let pdf = WinPdfPrinter::new(device);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.pdf");
        pdf.print(path.as_path(), Default::default()).unwrap();
    }

    /// A4 at 1600 dpi is 13229 x 18709 px, taller than the 16384 texels Direct3D 11 allows a
    /// texture. The page is still rendered whole, in one pass: `RenderPageToDeviceContext` records
    /// into a command list and never allocates a page-sized texture.
    #[test]
    fn a_page_larger_than_a_texture_prints_at_a_high_resolution() {
        let device = null_device::thread_local();
        let pdf = WinPdfPrinter::new(device);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/test_document.pdf");
        pdf.print_at(path.as_path(), Default::default(), Some(1600))
            .unwrap();
    }

    /// The smallest PDF that draws a gray image of `w` x `h` pixels, 8 bits, in a `draw_w` x
    /// `draw_h` pt rectangle on a page of `media` pt, then `extra` content under `resources`.
    fn pdf(
        media: (u32, u32),
        image: Option<(u32, u32, u32, u32)>,
        resources: &str,
        extra: &str,
    ) -> Vec<u8> {
        let mut content = String::new();
        let mut image_object = Vec::new();
        if let Some((w, h, draw_w, draw_h)) = image {
            content += &format!("q {draw_w} 0 0 {draw_h} 20 20 cm /Im Do Q ");
            let mut object = format!(
                "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8 /Length {} >>\nstream\n",
                w * h
            )
            .into_bytes();
            object.extend((0..w * h).map(|i| if (i / w / 2) % 2 == 0 { 0u8 } else { 255 }));
            object.extend(b"\nendstream");
            image_object = object;
        }
        content += extra;
        let mut objects: Vec<Vec<u8>> = vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << /XObject << /Im 5 0 R >> {resources} >> >>",
                media.0, media.1
            )
            .into_bytes(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).into_bytes(),
        ];
        objects.push(if image_object.is_empty() {
            b"<< >>".to_vec()
        } else {
            image_object
        });
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, object) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend(format!("{} 0 obj\n", i + 1).into_bytes());
            out.extend(object);
            out.extend(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).into_bytes());
        for offset in offsets {
            out.extend(format!("{offset:010} 00000 n \n").into_bytes());
        }
        out.extend(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .into_bytes(),
        );
        out
    }

    /// Print `bytes` to the null device at `dpi`, from a file that is removed afterwards.
    fn print_bytes(name: &str, bytes: &[u8], dpi: Option<u32>) -> Result<(), WinPdfPrinterError> {
        let path =
            std::env::temp_dir().join(format!("winprint-limits-{}-{name}.pdf", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        let result = WinPdfPrinter::new(null_device::thread_local()).print_at(
            &path,
            Default::default(),
            dpi,
        );
        let _ = std::fs::remove_file(&path);
        result
    }

    /// The limits that make a page-sized texture impossible: Direct3D 11 guarantees 16384 texels a
    /// side (feature level 11,
    /// <https://learn.microsoft.com/windows/win32/direct3d11/overviews-direct3d-11-resources-limits>)
    /// and nothing above it. A page rendered at a high resolution is larger than that, so the
    /// renderer must not go through one texture; the tests below print such pages.
    #[test]
    fn direct3d_refuses_a_texture_of_a_large_page() {
        use windows::Win32::Foundation::HMODULE;
        use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
        use windows::Win32::Graphics::Direct3D11::*;
        use windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
        };
        let mut device = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )
            .unwrap();
        }
        let device = device.unwrap();
        let texture = |width, height| {
            let description = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
                ..Default::default()
            };
            let mut texture = None;
            unsafe { device.CreateTexture2D(&description, None, Some(&mut texture)) }
                .map(|()| texture.is_some())
        };
        assert_eq!(texture(2480, 3507), Ok(true), "A4 at 300 dpi");
        assert_eq!(texture(16384, 16384), Ok(true), "the limit itself");
        assert!(texture(16385, 100).is_err(), "one texel over");
        assert!(texture(13229, 18709).is_err(), "A4 at 1600 dpi");
    }

    /// A page far past the Direct3D texture limit and the 65535 units Direct2D prints a bitmap in
    /// (<https://learn.microsoft.com/windows/win32/direct2d/d1163>) is still drawn whole: the
    /// resolution only scales the destination, and no page-sized bitmap is made.
    #[test]
    fn pages_beyond_the_texture_and_print_bitmap_limits_print() {
        // Whole pages: nothing here needs a page-sized bitmap.
        // A4 with a 300 dpi image: 79371 x 112252 px at 9600 dpi.
        let a4 = pdf((595, 842), Some((600, 600, 144, 144)), "", "");
        print_bytes("a4", &a4, Some(9600)).expect("A4 at 9600 dpi");
        // An image more than 65535 px wide, drawn 550 pt wide.
        let wide = pdf((595, 842), Some((70_000, 40, 550, 20)), "", "");
        print_bytes("wide-default", &wide, None).expect("a 70000 px wide image");
        print_bytes("wide-1600", &wide, Some(1600)).expect("a wide image at 1600 dpi");
    }

    const BLEND: &str = "/ExtGState << /G1 << /ca 0.5 /BM /Multiply >> >>";

    /// Blended content that fits one layer is drawn whole.
    #[test]
    fn a_blended_page_that_fits_is_drawn_whole() {
        // Overlapping 240 pt squares: 1000 px at 300 dpi, a second or two in the print control.
        let blended = pdf(
            (595, 842),
            None,
            BLEND,
            "1 0 0 rg 20 20 240 240 re f /G1 gs 0 0 1 rg 140 140 240 240 re f",
        );
        print_bytes("blend-fits", &blended, Some(300)).unwrap();
    }

    #[test]
    fn the_devmode_presets_are_not_a_resolution() {
        use super::job_dpi;
        assert_eq!((job_dpi(600), job_dpi(1200), job_dpi(75)), (600, 1200, 75));
        // DMRES_DRAFT, LOW, MEDIUM, HIGH and a zero field
        assert_eq!([-1, -2, -3, -4, 0].map(job_dpi), [300; 5]);
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
