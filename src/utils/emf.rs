use crate::utils::com::ComInitializer;
use scopeguard::defer;
use std::mem;
use std::ptr;
use std::slice;
use thiserror::Error;
use windows::Win32::Graphics::Gdi::MODIFY_WORLD_TRANSFORM_MODE;
use windows::Win32::Graphics::Gdi::{CHECKJPEGFORMAT, CHECKPNGFORMAT, EMRSETWORLDTRANSFORM};
use windows::Win32::Graphics::Gdi::{EMRMODIFYWORLDTRANSFORM, QUERYESCSUPPORT};
use windows::Win32::Storage::Xps::ExtEscape;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{LPARAM, RECT},
        Graphics::{
            Gdi::{
                CloseEnhMetaFile, CreateEnhMetaFileW, DeleteEnhMetaFile, EnumEnhMetaFile,
                GetDeviceCaps, GetEnhMetaFileHeader, GetWorldTransform, ModifyWorldTransform,
                PlayEnhMetaFileRecord, SetWorldTransform, StretchDIBits, BITMAPINFO,
                BITMAPINFOHEADER, BI_JPEG, BI_PNG, BI_RGB, DIB_USAGE, EMRSTRETCHDIBITS,
                EMR_MODIFYWORLDTRANSFORM, EMR_SETLAYOUT, EMR_SETWORLDTRANSFORM, EMR_STRETCHDIBITS,
                ENHMETAHEADER, ENHMETARECORD, HANDLETABLE, HDC, HENHMETAFILE, HORZRES, HORZSIZE,
                MWT_IDENTITY, MWT_LEFTMULTIPLY, MWT_RIGHTMULTIPLY, ROP_CODE, VERTRES, VERTSIZE,
                XFORM,
            },
            Imaging::{
                CLSID_WICImagingFactory2, GUID_WICPixelFormat32bppBGRA, WICBitmapDitherTypeNone,
                WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand,
                D2D::IWICImagingFactory2,
            },
        },
        System::Com::{CoCreateInstance, CLSCTX_ALL},
    },
};

#[derive(Error, Debug)]
#[error("Failed to create EMF")]
pub struct EmfCreateFailedError;

#[derive(Error, Debug)]
#[error("Failed to play EMF")]
pub struct EmfPlaybackFailedError;

pub struct Emf {
    metafile: HENHMETAFILE,
}

impl Emf {
    /// Creates a new EMF by drawing on a metafile DC.
    /// `width` and `height` are in reference-device pixels.
    pub unsafe fn new(
        hdc: HDC,
        width: i32,
        height: i32,
        draw: impl FnOnce(HDC) -> bool,
    ) -> Result<Self, EmfCreateFailedError> {
        // EMF playback maps the frame back to pixels using szlDevice and
        // szlMillimeters (HORZRES/HORZSIZE), not LOGPIXELSX/Y. The rounded
        // millimeter device size need not agree with its nominal DPI.
        // Use that same mapping here, otherwise playback subtly stretches
        // the recorded pixels even when the destination has the same size.
        let frame = RECT {
            left: 0,
            top: 0,
            right: frame_extent(
                width,
                GetDeviceCaps(Some(hdc), HORZRES),
                GetDeviceCaps(Some(hdc), HORZSIZE),
            )?,
            bottom: frame_extent(
                height,
                GetDeviceCaps(Some(hdc), VERTRES),
                GetDeviceCaps(Some(hdc), VERTSIZE),
            )?,
        };

        let emf_hdc = CreateEnhMetaFileW(
            Some(hdc),
            PCWSTR::null(),
            Some(ptr::addr_of!(frame)),
            PCWSTR::null(),
        );
        if emf_hdc.is_invalid() {
            return Err(EmfCreateFailedError);
        }

        if !draw(emf_hdc) {
            let _ = CloseEnhMetaFile(emf_hdc);
            return Err(EmfCreateFailedError);
        }

        let metafile = CloseEnhMetaFile(emf_hdc);
        if metafile.is_invalid() {
            return Err(EmfCreateFailedError);
        }

        Ok(Self { metafile })
    }

    pub unsafe fn playback(
        &self,
        printer_hdc: HDC,
        rect: RECT,
    ) -> Result<(), EmfPlaybackFailedError> {
        let mut base_matrix = XFORM::default();
        if !GetWorldTransform(printer_hdc, &mut base_matrix).as_bool() {
            return Err(EmfPlaybackFailedError);
        }
        defer! {
            let _ = SetWorldTransform(printer_hdc, ptr::addr_of!(base_matrix));
        }

        let mut context = EnumerationContext { base_matrix };

        let mut header = ENHMETAHEADER::default();
        if GetEnhMetaFileHeader(
            self.metafile,
            mem::size_of::<ENHMETAHEADER>() as u32,
            Some(&mut header),
        ) != mem::size_of::<ENHMETAHEADER>() as u32
        {
            return Err(EmfPlaybackFailedError);
        }
        if EnumEnhMetaFile(
            Some(printer_hdc),
            self.metafile,
            Some(Self::playback_proc),
            Some((&mut context as *mut EnumerationContext).cast()),
            Some(ptr::addr_of!(rect)),
        )
        .as_bool()
        {
            return Ok(());
        }
        Err(EmfPlaybackFailedError)
    }

    unsafe extern "system" fn playback_proc(
        hdc: HDC,
        handle_table: *const HANDLETABLE,
        record: *const ENHMETARECORD,
        handle_count: i32,
        param: LPARAM,
    ) -> i32 {
        let context = &mut *(param.0 as *mut EnumerationContext);
        let handle_table_slice = if handle_table.is_null() || handle_count <= 0 {
            &[]
        } else {
            slice::from_raw_parts(handle_table, handle_count as usize)
        };
        match play_record(hdc, record, handle_table_slice, context) {
            true => 1,
            false => 0,
        }
    }
}

/// Convert pixels to .01-mm frame units using the EMF reference-device metrics.
fn frame_extent(
    pixels: i32,
    device_pixels: i32,
    device_mm: i32,
) -> Result<i32, EmfCreateFailedError> {
    if pixels <= 0 || device_pixels <= 0 || device_mm <= 0 {
        return Err(EmfCreateFailedError);
    }
    let extent = (pixels as f64 * device_mm as f64 * 100.0 / device_pixels as f64).round();
    if extent < 1.0 || extent > i32::MAX as f64 {
        return Err(EmfCreateFailedError);
    }
    Ok(extent as i32)
}

impl Drop for Emf {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteEnhMetaFile(Some(self.metafile));
        }
    }
}

struct EnumerationContext {
    base_matrix: XFORM,
}

unsafe fn play_record(
    hdc: HDC,
    record: *const ENHMETARECORD,
    handle_table_slice: &[HANDLETABLE],
    context: &mut EnumerationContext,
) -> bool {
    match (*record).iType {
        EMR_STRETCHDIBITS => {
            // EMRSTRETCHDIBITS is flexible in size, so we check the size with "<" operation.
            if (*record).nSize < (mem::size_of::<EMRSTRETCHDIBITS>()) as u32 {
                return false;
            }
            play_stretch_dibits_record(hdc, record as *const EMRSTRETCHDIBITS, handle_table_slice)
        }
        EMR_SETWORLDTRANSFORM => {
            if (*record).nSize != (mem::size_of::<EMRSETWORLDTRANSFORM>()) as u32 {
                return false;
            }
            let set_world_transform = &*(record as *const EMRSETWORLDTRANSFORM);
            SetWorldTransform(hdc, ptr::addr_of!(context.base_matrix)).as_bool()
                && ModifyWorldTransform(
                    hdc,
                    Some(ptr::addr_of!(set_world_transform.xform)),
                    MWT_LEFTMULTIPLY,
                )
                .as_bool()
        }
        EMR_MODIFYWORLDTRANSFORM => {
            if (*record).nSize != (mem::size_of::<EMRMODIFYWORLDTRANSFORM>()) as u32 {
                return false;
            }
            let modify_world_transform = &*(record as *const EMRMODIFYWORLDTRANSFORM);
            match modify_world_transform.iMode {
                MWT_IDENTITY => {
                    SetWorldTransform(hdc, ptr::addr_of!(context.base_matrix)).as_bool()
                }
                MWT_LEFTMULTIPLY => ModifyWorldTransform(
                    hdc,
                    Some(ptr::addr_of!(modify_world_transform.xform)),
                    MWT_LEFTMULTIPLY,
                )
                .as_bool(),
                MWT_RIGHTMULTIPLY => ModifyWorldTransform(
                    hdc,
                    Some(ptr::addr_of!(modify_world_transform.xform)),
                    MWT_RIGHTMULTIPLY,
                )
                .as_bool(),
                // 4 = MWT_SET
                MODIFY_WORLD_TRANSFORM_MODE(4) => {
                    SetWorldTransform(hdc, ptr::addr_of!(context.base_matrix)).as_bool()
                        && ModifyWorldTransform(
                            hdc,
                            Some(ptr::addr_of!(modify_world_transform.xform)),
                            MWT_LEFTMULTIPLY,
                        )
                        .as_bool()
                }
                _ => false,
            }
        }
        // Skip EMR_SETLAYOUT because some printer drivers choke on
        // layout changes when replaying EMF records.
        // Chromium also skips EMR_SETLAYOUT in their EMF playback implementation:
        // https://github.com/chromium/chromium/blob/056cc5e47e8dd311cf345d4d83a2729691ac2610/printing/emf_win.cc#L368-L371
        EMR_SETLAYOUT => true,
        _ => PlayEnhMetaFileRecord(hdc, handle_table_slice, record).as_bool(),
    }
}

unsafe fn play_stretch_dibits_record(
    hdc: HDC,
    sdib_record: *const EMRSTRETCHDIBITS,
    handle_table_slice: &[HANDLETABLE],
) -> bool {
    let record_size = (*sdib_record).emr.nSize as usize;
    let bmi_offset = (*sdib_record).offBmiSrc as usize;
    let bmi_len = (*sdib_record).cbBmiSrc as usize;
    let bmi_end = match bmi_offset.checked_add(bmi_len) {
        Some(end) => end,
        None => return false,
    };
    if bmi_len < mem::size_of::<BITMAPINFOHEADER>() || bmi_end > record_size {
        return false;
    }
    let bitmap_header = &*(sdib_record.byte_add(bmi_offset) as *const BITMAPINFOHEADER);
    let bits_offset = (*sdib_record).offBitsSrc as usize;
    let bits_len = bitmap_header.biSizeImage as usize;
    let bits_end = match bits_offset.checked_add(bits_len) {
        Some(end) => end,
        None => return false,
    };
    if bits_end > record_size {
        return false;
    }
    let bits = {
        if bits_len == 0 {
            &[]
        } else {
            slice::from_raw_parts(sdib_record.byte_add(bits_offset) as *const u8, bits_len)
        }
    };
    if (bitmap_header.biCompression == BI_JPEG.0
        && !dib_format_natively_supported(hdc, CHECKJPEGFORMAT, bits))
        || (bitmap_header.biCompression == BI_PNG.0
            && !dib_format_natively_supported(hdc, CHECKPNGFORMAT, bits))
    {
        // Decode compressed format in application and play with StretchDIBits, to avoid EMF playback failure.
        // Aligns with how Chromium plays EMF records with compressed DIBs:
        // https://github.com/chromium/chromium/blob/c6e8aae4573cec06352a4748ae902831f9a1334c/printing/emf_win.cc#L276
        return match DecodedBitmap::from_compressed(bits) {
            Ok(decoded) => {
                StretchDIBits(
                    hdc,
                    (*sdib_record).xDest,
                    (*sdib_record).yDest,
                    (*sdib_record).cxDest,
                    (*sdib_record).cyDest,
                    (*sdib_record).xSrc,
                    (*sdib_record).ySrc,
                    (*sdib_record).cxSrc,
                    (*sdib_record).cySrc,
                    Some(decoded.pixels.as_ptr().cast()),
                    ptr::addr_of!(decoded.bitmap_info),
                    DIB_USAGE((*sdib_record).iUsageSrc),
                    ROP_CODE((*sdib_record).dwRop),
                ) != 0
            }
            Err(_) => false,
        };
    }

    PlayEnhMetaFileRecord(hdc, handle_table_slice, sdib_record as *const ENHMETARECORD).as_bool()
}

fn dib_format_natively_supported(dc: HDC, escape: u32, bits: &[u8]) -> bool {
    let mut supported: [u8; 4] = [0, 0, 0, 0];
    unsafe {
        if ExtEscape(
            dc,
            QUERYESCSUPPORT as _,
            Some(escape.to_ne_bytes().as_slice()),
            None,
        ) > 0
        {
            ExtEscape(dc, escape as _, Some(bits), Some(supported.as_mut_slice()));
        }
    }
    supported != [0, 0, 0, 0]
}

struct DecodedBitmap {
    bitmap_info: BITMAPINFO,
    pixels: Vec<u8>,
}

impl DecodedBitmap {
    unsafe fn from_compressed(bytes: &[u8]) -> windows::core::Result<Self> {
        let _com = ComInitializer::new();
        let factory: IWICImagingFactory2 =
            CoCreateInstance(&CLSID_WICImagingFactory2, None, CLSCTX_ALL)?;
        let stream = factory.CreateStream()?;
        stream.InitializeFromMemory(bytes)?;
        let decoder = factory.CreateDecoderFromStream(
            &stream,
            ptr::null(),
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = decoder.GetFrame(0)?;
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeMedianCut,
        )?;

        let mut width = 0u32;
        let mut height = 0u32;
        converter.GetSize(&mut width, &mut height)?;
        let stride = width.saturating_mul(4);
        let mut pixels = vec![0u8; stride.saturating_mul(height) as usize];
        converter.CopyPixels(ptr::null(), stride, &mut pixels)?;

        Ok(Self {
            bitmap_info: BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: pixels.len() as u32,
                    ..Default::default()
                },
                ..Default::default()
            },
            pixels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::frame_extent;

    #[test]
    fn emf_frame_round_trips_reference_device_pixels() {
        // The PWG driver's printable area at nominal 300 dpi: its rounded
        // millimeter dimensions do not describe exactly 300 pixels per inch.
        for (pixels, device_pixels, device_mm, expected_frame) in
            [(2480, 2180, 185, 21046), (3507, 3208, 272, 29735)]
        {
            let frame = frame_extent(pixels, device_pixels, device_mm).unwrap();
            assert_eq!(frame, expected_frame);
            let replayed_pixels = frame as f64 * device_pixels as f64 / (device_mm as f64 * 100.0);
            assert!((replayed_pixels - pixels as f64).abs() < 0.1);

            // The old nominal-DPI conversion causes a multi-pixel drift.
            let nominal_frame = (pixels as f64 * 2540.0 / 300.0).round();
            let replayed_pixels = nominal_frame * device_pixels as f64 / (device_mm as f64 * 100.0);
            assert!((replayed_pixels - pixels as f64).abs() > 4.0);
        }
    }

    #[test]
    fn emf_frame_rejects_invalid_or_overflowing_dimensions() {
        for (pixels, device_pixels, device_mm) in [
            (0, 2180, 185),
            (-1, 2180, 185),
            (2480, 0, 185),
            (2480, 2180, -1),
            (1, i32::MAX, 1),
            (i32::MAX, 1, i32::MAX),
        ] {
            assert!(frame_extent(pixels, device_pixels, device_mm).is_err());
        }
    }
}
