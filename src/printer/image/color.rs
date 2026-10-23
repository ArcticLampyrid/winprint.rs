//! Color management for [`ImagePrinter`](super::ImagePrinter).
//!
//! A print job takes its colors in sRGB or in scRGB, and its print control reads every bitmap in
//! that color space, ignoring the color contexts of bitmaps. So each frame is converted into the
//! color space of the job. For sRGB in / sRGB out the conversion is a no-op.

use crate::printer::DxgiRenderContext;
use std::mem::ManuallyDrop;
use windows::core::{Interface, Result, GUID};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COMPOSITE_MODE_SOURCE_COPY, D2D1_PIXEL_FORMAT, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1ColorManagement, ID2D1Bitmap1, ID2D1ColorContext, ID2D1DeviceContext, ID2D1Effect,
    ID2D1Image, D2D1_BITMAP_OPTIONS, D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_BUFFER_PRECISION_32BPC_FLOAT, D2D1_COLORMANAGEMENT_PROP,
    D2D1_COLORMANAGEMENT_PROP_DESTINATION_COLOR_CONTEXT,
    D2D1_COLORMANAGEMENT_PROP_DESTINATION_RENDERING_INTENT, D2D1_COLORMANAGEMENT_PROP_QUALITY,
    D2D1_COLORMANAGEMENT_PROP_SOURCE_COLOR_CONTEXT,
    D2D1_COLORMANAGEMENT_PROP_SOURCE_RENDERING_INTENT, D2D1_COLORMANAGEMENT_QUALITY_BEST,
    D2D1_COLORMANAGEMENT_RENDERING_INTENT_RELATIVE_COLORIMETRIC, D2D1_COLOR_SPACE,
    D2D1_COLOR_SPACE_SCRGB, D2D1_COLOR_SPACE_SRGB, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
    D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_PROPERTY_TYPE_COLOR_CONTEXT,
    D2D1_PROPERTY_TYPE_ENUM, D2D1_RENDERING_CONTROLS,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT,
    DXGI_FORMAT_R16G16B16A16_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM,
};
use windows::Win32::Graphics::Imaging::D2D::IWICImagingFactory2;
use windows::Win32::Graphics::Imaging::{
    GUID_WICPixelFormat32bppCMYK, GUID_WICPixelFormat32bppPBGRA, GUID_WICPixelFormat64bppCMYK,
    GUID_WICPixelFormat64bppPRGBA, GUID_WICPixelFormat64bppPRGBAHalf, IWICBitmapFrameDecode,
    IWICPixelFormatInfo, IWICPixelFormatInfo2, WICBitmapDitherTypeNone,
    WICBitmapPaletteTypeMedianCut, WICPixelFormatNumericRepresentationFixed,
    WICPixelFormatNumericRepresentationFloat,
};

/// Read the color context of the samples of `frame` with Direct2D.
///
/// Without an embedded one that Direct2D reads, floating and fixed point samples are in linear
/// scRGB, as WIC takes them, and the others are in sRGB.
pub fn color_context(
    render: &DxgiRenderContext,
    frame: &IWICBitmapFrameDecode,
) -> Result<ID2D1ColorContext> {
    unsafe {
        if let Some(context) = embedded_color_context(render, frame) {
            return Ok(context);
        }
        // Samples in an unknown format are taken as sRGB.
        let color_space = match is_linear(&render.wic_factory, &frame.GetPixelFormat()?) {
            Ok(true) => D2D1_COLOR_SPACE_SCRGB,
            _ => D2D1_COLOR_SPACE_SRGB,
        };
        render.d2d_context.CreateColorContext(color_space, None)
    }
}

/// Read the color context embedded in `frame`, if it has one that Direct2D reads.
///
/// Note: Some sRGB ICC profles are indentified as custom by Direct2D.
unsafe fn embedded_color_context(
    render: &DxgiRenderContext,
    frame: &IWICBitmapFrameDecode,
) -> Option<ID2D1ColorContext> {
    let wic_context = render.wic_factory.CreateColorContext().ok()?;
    let mut count = 0;
    frame
        .GetColorContexts(&mut [Some(wic_context.clone())], &mut count)
        .ok()?;
    if count == 0 {
        return None;
    }
    render
        .d2d_context
        .CreateColorContextFromWicColorContext(&wic_context)
        .ok()
}

/// Loads frames into bitmaps in the color space of a print job.
pub struct Converter {
    wic_factory: IWICImagingFactory2,
    d2d_context: ID2D1DeviceContext,
    color_space: D2D1_COLOR_SPACE,
}

impl Converter {
    /// Check if wide color gamut is available
    pub fn is_scrgb_available(render: &DxgiRenderContext) -> bool {
        unsafe {
            render
                .d2d_context
                .IsDxgiFormatSupported(DXGI_FORMAT_R16G16B16A16_FLOAT)
                .into()
        }
    }

    /// Create a converter into `color_space` on the devices of `render`.
    pub fn new(render: &DxgiRenderContext, color_space: D2D1_COLOR_SPACE) -> Result<Self> {
        unsafe {
            let d2d_context = render
                .d2d_device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            // The best quality of the color management effect needs floating point buffers. They
            // keep colors outside [0, 1]. Tiles bound the buffers to a part of the frame, as a
            // whole frame would take gigabytes for a large image.
            d2d_context.SetRenderingControls(&D2D1_RENDERING_CONTROLS {
                bufferPrecision: D2D1_BUFFER_PRECISION_32BPC_FLOAT,
                tileSize: D2D_SIZE_U {
                    width: 1024,
                    height: 1024,
                },
            });
            Ok(Self {
                wic_factory: render.wic_factory.clone(),
                d2d_context,
                color_space,
            })
        }
    }

    /// Load `frame`, whose samples are in `source`, into a bitmap in the color space of the job.
    pub fn bitmap(
        &self,
        frame: &IWICBitmapFrameDecode,
        source: &ID2D1ColorContext,
    ) -> Result<ID2D1Bitmap1> {
        unsafe {
            let pixel_format = frame.GetPixelFormat()?;
            if source.GetColorSpace() == D2D1_COLOR_SPACE_SRGB
                && self.color_space == D2D1_COLOR_SPACE_SRGB
            {
                // sRGB --> sRGB is a no-op, so load the samples as they are, without the color management effect.
                // Unidentified samples are taken as sRGB, so they are loaded as 32-bit premultiplied BGRA.
                let deep = is_deep(&self.wic_factory, &pixel_format).unwrap_or(false);
                if deep
                    && self
                        .d2d_context
                        .IsDxgiFormatSupported(DXGI_FORMAT_R16G16B16A16_UNORM)
                        .into()
                {
                    return self.load(
                        frame,
                        &GUID_WICPixelFormat64bppPRGBA,
                        DXGI_FORMAT_R16G16B16A16_UNORM,
                    );
                } else {
                    return self.load(
                        frame,
                        &GUID_WICPixelFormat32bppPBGRA,
                        DXGI_FORMAT_B8G8R8A8_UNORM,
                    );
                }
            }
            self.profiled_samples(frame, &pixel_format)
                .and_then(|samples| self.convert(&samples, source, DXGI_FORMAT_R16G16B16A16_FLOAT))
        }
    }

    /// Load the samples of a frame with a color context, as the color management effect reads
    /// them.
    ///
    /// CMYK samples go unconverted into the RGBA channels, as the effect reads them for a CMYK
    /// profile. Floating and fixed point samples go into half floats, which keep them outside
    /// \[0, 1\].
    unsafe fn profiled_samples(
        &self,
        frame: &IWICBitmapFrameDecode,
        pixel_format: &GUID,
    ) -> Result<ID2D1Bitmap1> {
        match pixel_format {
            format if *format == GUID_WICPixelFormat32bppCMYK => {
                self.copy(frame, DXGI_FORMAT_R8G8B8A8_UNORM, 4)
            }
            format if *format == GUID_WICPixelFormat64bppCMYK => {
                self.copy(frame, DXGI_FORMAT_R16G16B16A16_UNORM, 8)
            }
            format if is_linear(&self.wic_factory, format)? => self.load(
                frame,
                &GUID_WICPixelFormat64bppPRGBAHalf,
                DXGI_FORMAT_R16G16B16A16_FLOAT,
            ),
            _ => self.load(
                frame,
                &GUID_WICPixelFormat64bppPRGBA,
                DXGI_FORMAT_R16G16B16A16_UNORM,
            ),
        }
    }

    /// Load the samples of `frame` into a bitmap of `format`, converted with WIC regardless of
    /// its color profile.
    unsafe fn load(
        &self,
        frame: &IWICBitmapFrameDecode,
        wic_format: &GUID,
        dxgi_format: DXGI_FORMAT,
    ) -> Result<ID2D1Bitmap1> {
        let format_converter = self.wic_factory.CreateFormatConverter()?;
        format_converter.Initialize(
            frame,
            wic_format,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeMedianCut,
        )?;
        self.d2d_context.CreateBitmapFromWicBitmap(
            &format_converter,
            Some(&properties(dxgi_format, D2D1_BITMAP_OPTIONS_NONE)),
        )
    }

    /// Copy the samples of `frame`, of `bytes_per_pixel`, unconverted into a bitmap of `format`.
    unsafe fn copy(
        &self,
        frame: &IWICBitmapFrameDecode,
        dxgi_format: DXGI_FORMAT,
        bytes_per_pixel: u32,
    ) -> Result<ID2D1Bitmap1> {
        let mut size = D2D_SIZE_U::default();
        frame.GetSize(&mut size.width, &mut size.height)?;
        let stride = size.width * bytes_per_pixel;
        let mut samples = vec![0; stride as usize * size.height as usize];
        frame.CopyPixels(std::ptr::null(), stride, &mut samples)?;
        self.d2d_context.CreateBitmap(
            size,
            Some(samples.as_ptr().cast()),
            stride,
            &properties(dxgi_format, D2D1_BITMAP_OPTIONS_NONE),
        )
    }

    /// Convert `samples` with colors in `source` into a bitmap of `format` in the color space of
    /// the job, with the color management effect.
    unsafe fn convert(
        &self,
        samples: &ID2D1Bitmap1,
        source: &ID2D1ColorContext,
        dxgi_format: DXGI_FORMAT,
    ) -> Result<ID2D1Bitmap1> {
        let effect = self.d2d_context.CreateEffect(&CLSID_D2D1ColorManagement)?;
        effect.SetInput(0, samples, true);
        let destination = self
            .d2d_context
            .CreateColorContext(self.color_space, None)?;
        set_color_context(
            &effect,
            D2D1_COLORMANAGEMENT_PROP_SOURCE_COLOR_CONTEXT,
            source,
        )?;
        set_color_context(
            &effect,
            D2D1_COLORMANAGEMENT_PROP_DESTINATION_COLOR_CONTEXT,
            &destination,
        )?;
        // The best quality keeps colors outside [0, 1]. Relative colorimetric intents convert
        // colors as they are, leaving gamut mapping to the printer driver, while perceptual ones
        // lift black in scRGB.
        set_enum(
            &effect,
            D2D1_COLORMANAGEMENT_PROP_QUALITY,
            D2D1_COLORMANAGEMENT_QUALITY_BEST.0,
        )?;
        set_enum(
            &effect,
            D2D1_COLORMANAGEMENT_PROP_SOURCE_RENDERING_INTENT,
            D2D1_COLORMANAGEMENT_RENDERING_INTENT_RELATIVE_COLORIMETRIC.0,
        )?;
        set_enum(
            &effect,
            D2D1_COLORMANAGEMENT_PROP_DESTINATION_RENDERING_INTENT,
            D2D1_COLORMANAGEMENT_RENDERING_INTENT_RELATIVE_COLORIMETRIC.0,
        )?;
        // Fail before drawing, so that an error never leaves the context drawing.
        let output = effect.GetOutput()?;
        let target = self.d2d_context.CreateBitmap(
            samples.GetPixelSize(),
            None,
            0,
            &properties(dxgi_format, D2D1_BITMAP_OPTIONS_TARGET),
        )?;
        self.d2d_context.SetTarget(&target);
        self.d2d_context.BeginDraw();
        self.d2d_context.DrawImage(
            &output,
            None,
            None,
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
            D2D1_COMPOSITE_MODE_SOURCE_COPY,
        );
        let drawn = self.d2d_context.EndDraw(None, None);
        self.d2d_context.SetTarget(None::<&ID2D1Image>);
        drawn.map(|()| target)
    }
}

/// Whether some channel of samples in `format` has more than 8 bits.
unsafe fn is_deep(wic_factory: &IWICImagingFactory2, format: &GUID) -> Result<bool> {
    let info: IWICPixelFormatInfo = wic_factory.CreateComponentInfo(format)?.cast()?;
    let mask_size = info.GetBitsPerPixel()?.div_ceil(8) as usize;
    let channel_bits = |channel| -> Result<u32> {
        let mut mask = vec![0; mask_size];
        info.GetChannelMask(channel, &mut mask, &mut 0)?;
        Ok(mask.iter().map(|byte| byte.count_ones()).sum())
    };
    (0..info.GetChannelCount()?)
        .map(channel_bits)
        .try_fold(false, |deep, bits| Ok(deep || bits? > 8))
}

/// Whether samples in `format` are in floating or fixed point.
unsafe fn is_linear(wic_factory: &IWICImagingFactory2, format: &GUID) -> Result<bool> {
    let info: IWICPixelFormatInfo2 = wic_factory.CreateComponentInfo(format)?.cast()?;
    Ok([
        WICPixelFormatNumericRepresentationFloat,
        WICPixelFormatNumericRepresentationFixed,
    ]
    .contains(&info.GetNumericRepresentation()?))
}

/// Properties of a bitmap with one pixel per DIP, so that effects map pixels one to one.
fn properties(format: DXGI_FORMAT, options: D2D1_BITMAP_OPTIONS) -> D2D1_BITMAP_PROPERTIES1 {
    D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: options,
        colorContext: ManuallyDrop::new(None),
    }
}

unsafe fn set_color_context(
    effect: &ID2D1Effect,
    property: D2D1_COLORMANAGEMENT_PROP,
    context: &ID2D1ColorContext,
) -> Result<()> {
    effect.SetValue(
        property.0 as u32,
        D2D1_PROPERTY_TYPE_COLOR_CONTEXT,
        &(context.as_raw() as usize).to_ne_bytes(),
    )
}

unsafe fn set_enum(
    effect: &ID2D1Effect,
    property: D2D1_COLORMANAGEMENT_PROP,
    value: i32,
) -> Result<()> {
    effect.SetValue(
        property.0 as u32,
        D2D1_PROPERTY_TYPE_ENUM,
        &value.to_ne_bytes(),
    )
}
