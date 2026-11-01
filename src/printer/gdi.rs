use crate::printer::deleted_from_queue;
use crate::printer::PrintOutcome;
use crate::printer::PrinterDevice;
use crate::printer::TimedPrinter;
use crate::ticket::PrintTicket;
use crate::ticket::ToDevModeError;
use crate::utils::wchar;
use crate::utils::Deadline;
use scopeguard::defer;
use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::mem;
use std::time::Duration;
use thiserror::Error;
use windows::core::{HRESULT, PCWSTR};
use windows::Win32::Foundation::GetLastError;
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, GetDeviceCaps, SetBrushOrgEx, SetStretchBltMode, StretchDIBits,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HALFTONE, HDC, LOGPIXELSX, LOGPIXELSY,
    PHYSICALOFFSETX, PHYSICALOFFSETY, SRCCOPY,
};
use windows::Win32::Storage::Xps::{AbortDoc, EndDoc, EndPage, StartDocW, StartPage, DOCINFOW};
use windows::Win32::UI::ColorSystem::{SetICMMode, ICM_DONE_OUTSIDEDC, ICM_OFF};

/// The largest image, in bytes, that [`GdiPrinter`] sends in one piece.
const MAX_IMAGE_BYTES: usize = 512 * 1024 * 1024;
/// The largest number of copies of a page that one job holds.
const MAX_COPIES: u32 = 999;

/// Offset of `dmFields` in a `DEVMODEW`.
const DM_FIELDS_AT: usize = 72;
/// Offset of `dmCopies` in a `DEVMODEW`.
const DM_COPIES_AT: usize = 86;
/// `DM_COPIES`: `dmCopies` is valid.
const DM_COPIES: u32 = 0x100;

#[derive(Error, Debug)]
/// Represents an error from [`GdiPrinter`].
pub enum GdiPrinterError {
    /// The image has no pixels, or its size does not match its width and height.
    #[error("The image is empty or its size does not match its width and height")]
    InvalidImage,
    /// The image is too large to be sent in one piece.
    #[error("The image is too large")]
    ImageTooLarge,
    /// The area to print into is empty or not finite.
    #[error("The area to print into is empty")]
    InvalidArea,
    /// A name contains a NUL character.
    #[error("A name contains a NUL character")]
    InvalidName,
    /// Print ticket error.
    #[error("Print Ticket Error")]
    PrintTicketError(#[source] ToDevModeError),
    /// The settings the print ticket was converted to are too short to hold a copy count.
    #[error("The printer settings are damaged")]
    InvalidDevMode,
    /// Failed to open printer.
    #[error("Failed to open printer")]
    FailedToOpenPrinter,
    /// The printer reported no resolution.
    #[error("The printer reported no resolution")]
    NoResolution,
    /// SetICMMode failed.
    #[error("SetICMMode failed")]
    SetColorModeFailed,
    /// StartDoc failed.
    #[error("StartDocW failed (returned {0})")]
    StartDocFailed(i32),
    /// StartPage failed.
    #[error("StartPage failed for page {0} (returned {1})")]
    StartPageFailed(u32, i32),
    /// StretchDIBits failed.
    #[error("StretchDIBits failed for page {0}")]
    DrawFailed(u32),
    /// EndPage failed.
    #[error("EndPage failed for page {0} (returned {1})")]
    EndPageFailed(u32, i32),
    /// EndDoc failed.
    #[error("EndDoc failed (returned {0})")]
    EndDocFailed(i32),
    /// The job did not end before its timeout, after the given time.
    #[error("The print job did not end within {0:?}")]
    TimedOut(Duration),
}

/// An 8-bit RGB image, rows from top to bottom, `width * 3` bytes each.
#[derive(Clone, Copy, Debug)]
pub struct RgbImage<'a> {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// The pixels as red, green and blue bytes.
    pub data: &'a [u8],
}

/// A rectangle on the sheet, in points (1/72 inch) from the top-left corner of the paper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetRect {
    /// The distance from the left edge of the paper.
    pub x: f64,
    /// The distance from the top edge of the paper.
    pub y: f64,
    /// The width.
    pub width: f64,
    /// The height.
    pub height: f64,
}

/// Who converts the colors of an image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorPolicy {
    /// The driver applies its color settings to the image.
    #[default]
    DriverManaged,
    /// The caller has already converted the pixels into the color space of the printer, so the
    /// device context must not convert them again.
    CallerManaged,
}

/// Options for [`GdiPrinter::print_rgb`].
#[derive(Clone, Debug)]
pub struct GdiOptions {
    /// The printing preferences. Its copy count is not used: [`copies`](Self::copies) is.
    pub ticket: PrintTicket,
    /// The name of the document in the print queue.
    pub title: OsString,
    /// How many pages to print, each the same image. At least one, at most 999.
    ///
    /// Every copy is a page of this job, so a driver cannot multiply them again.
    pub copies: u32,
    /// Who converts the colors of the image.
    pub color_policy: ColorPolicy,
    /// Write the job to this file instead of the device, for drivers that can.
    pub output_file: Option<OsString>,
}

impl Default for GdiOptions {
    fn default() -> Self {
        Self {
            ticket: PrintTicket::default(),
            title: OsString::from("winprint"),
            copies: 1,
            color_policy: ColorPolicy::default(),
            output_file: None,
        }
    }
}

/// A printer that sends an RGB image through GDI, with the color management under the caller's
/// control.
///
/// Other printers of this crate draw through Direct2D or XPS, which convert colors on their own.
/// This one sends the pixels as they are, so an application that has already converted them into
/// the color space of the printer can keep them from being converted again, see [`ColorPolicy`].
pub struct GdiPrinter {
    printer: PrinterDevice,
    timeout: Option<Duration>,
}

impl GdiPrinter {
    /// Create a new [`GdiPrinter`] for the given printer device.
    pub fn new(printer: PrinterDevice) -> Self {
        Self {
            printer,
            timeout: None,
        }
    }

    /// Print `image` into `area` of the paper, `options.copies` times, one copy to a page.
    ///
    /// The image is stretched to the area. Returns [`PrintOutcome::Canceled`] if the job is
    /// deleted from the print queue.
    pub fn print_rgb(
        &self,
        image: &RgbImage<'_>,
        area: SheetRect,
        options: &GdiOptions,
    ) -> Result<PrintOutcome, GdiPrinterError> {
        let deadline = Deadline::after(self.timeout);
        let (width, height) = check_image(image)?;
        check_area(&area)?;
        let copies = options.copies.clamp(1, MAX_COPIES);
        let title = checked_wide(&options.title)?;
        let output = options
            .output_file
            .as_deref()
            .map(checked_wide)
            .transpose()?;
        let dib = Dib::new(image);
        let mut dev_mode = options
            .ticket
            .to_dev_mode(&self.printer)
            .map_err(GdiPrinterError::PrintTicketError)?;
        set_one_copy(&mut dev_mode)?;
        let color_mode = match options.color_policy {
            ColorPolicy::DriverManaged => ICM_OFF,
            // ICM_OFF would turn on old-style halftone correction. Pixels the caller converted
            // need both the conversion of the device context and that correction off.
            ColorPolicy::CallerManaged => ICM_DONE_OUTSIDEDC,
        };

        unsafe {
            // See `PdfiumPrinter` on the driver name.
            let hdc = CreateDCW(
                PCWSTR(PRINT_DRIVER.as_ptr()),
                PCWSTR(wchar::to_wide_chars(self.printer.os_name()).as_ptr()),
                None,
                Some(dev_mode.as_ptr() as *const _),
            );
            if hdc.is_invalid() {
                return Err(GdiPrinterError::FailedToOpenPrinter);
            }
            defer! {
                let _ = DeleteDC(hdc);
            }
            let target = Target::of(hdc, &area)?;
            let set_color_mode = || {
                if SetICMMode(hdc, color_mode) == 0 {
                    Err(GdiPrinterError::SetColorModeFailed)
                } else {
                    Ok(())
                }
            };
            set_color_mode()?;
            SetStretchBltMode(hdc, HALFTONE);
            // After setting the HALFTONE stretching mode, the brush origin must be set again.
            let _ = SetBrushOrgEx(hdc, 0, 0, None);

            let doc_info = DOCINFOW {
                cbSize: mem::size_of::<DOCINFOW>() as i32,
                fwType: 0,
                lpszDocName: PCWSTR(title.as_ptr()),
                lpszOutput: output
                    .as_ref()
                    .map_or(PCWSTR::null(), |output| PCWSTR(output.as_ptr())),
                lpszDatatype: PCWSTR::null(),
            };
            let started = StartDocW(hdc, &doc_info);
            if started <= 0 {
                return Err(GdiPrinterError::StartDocFailed(started));
            }
            let document_completed = Cell::new(false);
            defer! {
                if !document_completed.get() {
                    let _ = AbortDoc(hdc);
                }
            }

            for copy in 0..copies {
                if deadline.has_passed() {
                    return Err(GdiPrinterError::TimedOut(deadline.elapsed()));
                }
                let started = StartPage(hdc);
                if started <= 0 && job_deleted_from_queue() {
                    return Ok(PrintOutcome::Canceled);
                }
                if started <= 0 {
                    return Err(GdiPrinterError::StartPageFailed(copy, started));
                }
                // StartPage restores the state of the device context. Set the policy for every
                // page, before the pixels.
                set_color_mode()?;
                let drawn = StretchDIBits(
                    hdc,
                    target.x,
                    target.y,
                    target.width,
                    target.height,
                    0,
                    0,
                    width,
                    height,
                    Some(dib.bits.as_ptr() as *const _),
                    &dib.info,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
                let ended = EndPage(hdc);
                if (drawn <= 0 || ended <= 0) && job_deleted_from_queue() {
                    return Ok(PrintOutcome::Canceled);
                }
                if drawn <= 0 {
                    return Err(GdiPrinterError::DrawFailed(copy));
                }
                if ended <= 0 {
                    return Err(GdiPrinterError::EndPageFailed(copy, ended));
                }
            }

            let ended = EndDoc(hdc);
            if ended <= 0 && job_deleted_from_queue() {
                return Ok(PrintOutcome::Canceled);
            }
            if ended <= 0 {
                return Err(GdiPrinterError::EndDocFailed(ended));
            }
            document_completed.set(true);
        }
        Ok(PrintOutcome::Completed)
    }
}

impl TimedPrinter for GdiPrinter {
    fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }
}

// See `PdfiumPrinter`.
const PRINT_DRIVER: [u16; 9] = [
    'W' as u16, 'I' as u16, 'N' as u16, 'S' as u16, 'P' as u16, 'O' as u16, 'O' as u16, 'L' as u16,
    0,
];

/// The image box in device pixels, from the origin of the printable area, where GDI draws.
struct Target {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Target {
    /// Where `area` is on the device context `hdc`.
    unsafe fn of(hdc: HDC, area: &SheetRect) -> Result<Self, GdiPrinterError> {
        let cap = |index| unsafe { GetDeviceCaps(Some(hdc), index) };
        let (dpi_x, dpi_y) = (cap(LOGPIXELSX), cap(LOGPIXELSY));
        if dpi_x <= 0 || dpi_y <= 0 {
            return Err(GdiPrinterError::NoResolution);
        }
        let (dx, dy) = (f64::from(dpi_x) / 72.0, f64::from(dpi_y) / 72.0);
        let (offset_x, offset_y) = (
            f64::from(cap(PHYSICALOFFSETX)),
            f64::from(cap(PHYSICALOFFSETY)),
        );
        Ok(Self {
            x: pixels(area.x * dx - offset_x),
            y: pixels(area.y * dy - offset_y),
            width: pixels(area.width * dx),
            height: pixels(area.height * dy),
        })
    }
}

/// A device pixel count, rounded and kept to a range GDI can address.
fn pixels(value: f64) -> i32 {
    value.round().clamp(-1e8, 1e8) as i32
}

/// A bottom-up 24-bit device independent bitmap of an RGB image.
struct Dib {
    info: BITMAPINFO,
    bits: Vec<u8>,
}

impl Dib {
    /// `image` must have passed [`check_image`].
    fn new(image: &RgbImage<'_>) -> Self {
        let (width, height) = (image.width as usize, image.height as usize);
        // Rows are padded to 4 bytes.
        let stride = (width * 3 + 3) & !3;
        let mut bits = vec![0u8; stride * height];
        for (y, source) in image.data.chunks_exact(width * 3).enumerate() {
            let start = (height - 1 - y) * stride;
            let row = &mut bits[start..start + width * 3];
            for (target, pixel) in row
                .as_chunks_mut::<3>()
                .0
                .iter_mut()
                .zip(source.as_chunks::<3>().0)
            {
                *target = [pixel[2], pixel[1], pixel[0]];
            }
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: image.width as i32,
                biHeight: image.height as i32,
                biPlanes: 1,
                biBitCount: 24,
                biCompression: BI_RGB.0,
                biSizeImage: bits.len() as u32,
                ..Default::default()
            },
            bmiColors: Default::default(),
        };
        Self { info, bits }
    }
}

/// The size of a valid image as the GDI types it, or why the image is not valid.
fn check_image(image: &RgbImage<'_>) -> Result<(i32, i32), GdiPrinterError> {
    let row = (image.width as usize)
        .checked_mul(3)
        .ok_or(GdiPrinterError::ImageTooLarge)?;
    let total = row
        .checked_mul(image.height as usize)
        .ok_or(GdiPrinterError::ImageTooLarge)?;
    if image.width == 0 || image.height == 0 || total != image.data.len() {
        return Err(GdiPrinterError::InvalidImage);
    }
    // A bitmap padded to 4 bytes per row must stay below what its header can hold.
    let padded = (row + 3) & !3;
    match (
        i32::try_from(image.width),
        i32::try_from(image.height),
        padded.checked_mul(image.height as usize),
    ) {
        (Ok(width), Ok(height), Some(size))
            if total <= MAX_IMAGE_BYTES && size <= i32::MAX as usize =>
        {
            Ok((width, height))
        }
        _ => Err(GdiPrinterError::ImageTooLarge),
    }
}

fn check_area(area: &SheetRect) -> Result<(), GdiPrinterError> {
    let finite = [area.x, area.y, area.width, area.height]
        .iter()
        .all(|value| value.is_finite());
    if finite && area.width > 0.0 && area.height > 0.0 {
        Ok(())
    } else {
        Err(GdiPrinterError::InvalidArea)
    }
}

/// A NUL-terminated wide string of `name`, which must not contain a NUL itself.
fn checked_wide(name: &OsStr) -> Result<Vec<u16>, GdiPrinterError> {
    let wide = wchar::to_wide_chars(name);
    if wide[..wide.len() - 1].contains(&0) {
        return Err(GdiPrinterError::InvalidName);
    }
    Ok(wide)
}

/// Make the settings print one copy of each page, whatever the ticket said.
fn set_one_copy(dev_mode: &mut [u8]) -> Result<(), GdiPrinterError> {
    let fields: [u8; 4] = dev_mode
        .get(DM_FIELDS_AT..DM_FIELDS_AT + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(GdiPrinterError::InvalidDevMode)?;
    let fields = u32::from_le_bytes(fields) | DM_COPIES;
    dev_mode[DM_FIELDS_AT..DM_FIELDS_AT + 4].copy_from_slice(&fields.to_le_bytes());
    dev_mode
        .get_mut(DM_COPIES_AT..DM_COPIES_AT + 2)
        .ok_or(GdiPrinterError::InvalidDevMode)?
        .copy_from_slice(&1i16.to_le_bytes());
    Ok(())
}

/// Whether the last failed GDI print call failed because the job was deleted from the print queue.
fn job_deleted_from_queue() -> bool {
    deleted_from_queue(HRESULT::from_win32(unsafe { GetLastError() }.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::file_device::{FilePrinterDevice, Pdf};
    use windows::Win32::Graphics::Gdi::DEVMODEW;

    #[test]
    fn the_dib_is_bottom_up_bgr_with_rows_padded_to_four_bytes() {
        // 2x2: red, green / blue, white.
        let data = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        let dib = Dib::new(&RgbImage {
            width: 2,
            height: 2,
            data: &data,
        });
        assert_eq!(dib.info.bmiHeader.biHeight, 2);
        assert_eq!(dib.bits.len(), 16);
        // The first row stored is the bottom one: blue, white.
        assert_eq!(dib.bits[0..6], [255, 0, 0, 255, 255, 255]);
        assert_eq!(dib.bits[8..14], [0, 0, 255, 0, 255, 0]);
    }

    #[test]
    fn images_are_checked_against_their_size() {
        let image = |width, height, data: &'static [u8]| RgbImage {
            width,
            height,
            data,
        };
        assert!(check_image(&image(1, 1, &[0; 3])).is_ok());
        assert!(matches!(
            check_image(&image(0, 1, &[])),
            Err(GdiPrinterError::InvalidImage)
        ));
        assert!(matches!(
            check_image(&image(2, 1, &[0; 3])),
            Err(GdiPrinterError::InvalidImage)
        ));
        assert!(matches!(
            check_image(&image(u32::MAX, u32::MAX, &[])),
            Err(GdiPrinterError::ImageTooLarge)
        ));
    }

    #[test]
    fn areas_must_be_finite_and_not_empty() {
        let area = |x, width| SheetRect {
            x,
            y: 0.0,
            width,
            height: 1.0,
        };
        assert!(check_area(&area(0.0, 1.0)).is_ok());
        assert!(check_area(&area(f64::NAN, 1.0)).is_err());
        assert!(check_area(&area(0.0, 0.0)).is_err());
        assert!(check_area(&area(0.0, f64::INFINITY)).is_err());
    }

    #[test]
    fn the_offsets_of_the_copy_fields_are_those_of_devmodew() {
        let mut mode = DEVMODEW::default();
        mode.Anonymous1.Anonymous1.dmCopies = 0x0102;
        let bytes = unsafe {
            std::slice::from_raw_parts(
                &mode as *const DEVMODEW as *const u8,
                mem::size_of::<DEVMODEW>(),
            )
        };
        assert_eq!(bytes[DM_COPIES_AT..DM_COPIES_AT + 2], [0x02, 0x01]);
        let mut bytes = bytes.to_vec();
        set_one_copy(&mut bytes).unwrap();
        assert_eq!(bytes[DM_COPIES_AT..DM_COPIES_AT + 2], [1, 0]);
        assert_eq!(
            u32::from_le_bytes(bytes[DM_FIELDS_AT..DM_FIELDS_AT + 4].try_into().unwrap())
                & DM_COPIES,
            DM_COPIES
        );
        assert!(set_one_copy(&mut [0u8; 8]).is_err());
    }

    fn pdf_device() -> FilePrinterDevice<Pdf> {
        FilePrinterDevice::<Pdf>::new().expect("install virtual PDF printer")
    }

    fn red_image() -> Vec<u8> {
        [255u8, 0, 0].repeat(40 * 30)
    }

    const AREA: SheetRect = SheetRect {
        x: 72.0,
        y: 72.0,
        width: 144.0,
        height: 108.0,
    };

    /// The number of pages of a PDF written by Microsoft Print To PDF.
    fn page_count(pdf: &[u8]) -> usize {
        pdf.windows(11)
            .filter(|w| *w == b"/Type /Page")
            .count()
            .saturating_sub(1)
    }

    #[test]
    fn prints_every_copy_as_a_page_of_one_job() {
        let device = pdf_device();
        let data = red_image();
        let image = RgbImage {
            width: 40,
            height: 30,
            data: &data,
        };
        let options = GdiOptions {
            copies: 3,
            color_policy: ColorPolicy::CallerManaged,
            ..Default::default()
        };
        let outcome = GdiPrinter::new(device.device().clone())
            .print_rgb(&image, AREA, &options)
            .unwrap();
        assert_eq!(outcome, PrintOutcome::Completed);
        device.wait_for_idle(Duration::from_secs(60)).unwrap();
        let pdf = std::fs::read(device.file_path()).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert_eq!(page_count(&pdf), 3);
    }

    #[test]
    fn honors_timeout() {
        let device = pdf_device();
        let data = red_image();
        let image = RgbImage {
            width: 40,
            height: 30,
            data: &data,
        };
        let mut printer = GdiPrinter::new(device.device().clone());
        printer.set_timeout(Some(Duration::from_secs(3600)));
        assert_eq!(
            printer
                .print_rgb(&image, AREA, &GdiOptions::default())
                .unwrap(),
            PrintOutcome::Completed
        );
        printer.set_timeout(Some(Duration::ZERO));
        let error = printer
            .print_rgb(&image, AREA, &GdiOptions::default())
            .unwrap_err();
        assert!(matches!(error, GdiPrinterError::TimedOut(_)), "{error:?}");
    }

    #[test]
    fn rejects_what_it_cannot_print_before_opening_the_printer() {
        let device = pdf_device();
        let printer = GdiPrinter::new(device.device().clone());
        let empty = RgbImage {
            width: 0,
            height: 0,
            data: &[],
        };
        assert!(matches!(
            printer.print_rgb(&empty, AREA, &GdiOptions::default()),
            Err(GdiPrinterError::InvalidImage)
        ));
        let named = GdiOptions {
            title: OsString::from("a b"),
            ..Default::default()
        };
        let data = red_image();
        let image = RgbImage {
            width: 40,
            height: 30,
            data: &data,
        };
        assert!(matches!(
            printer.print_rgb(&image, AREA, &named),
            Err(GdiPrinterError::InvalidName)
        ));
    }
}
