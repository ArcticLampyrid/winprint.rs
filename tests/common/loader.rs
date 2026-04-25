use futures::{executor::block_on, io::BufReader as FutBufReader, AsyncReadExt};
use image::{DynamicImage, GrayImage, RgbImage, RgbaImage};
use print_raster::{
    model::cups::{CupsColorOrder, CupsColorSpace},
    reader::{cups::unified::CupsRasterUnifiedReader, RasterPageReader, RasterReader},
};
use std::path::Path;
use std::{
    fs::File,
    io::{self, Read},
    pin::pin,
};
use tiff::decoder::{Decoder as TiffDecoder, DecodingResult as TiffDecodingResult};
use tiff::ColorType;

/// Read every page of `path` as a PWG raster file.
pub fn load_pages_from_pwg(path: &Path) -> anyhow::Result<Vec<DynamicImage>> {
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;

    block_on(async move {
        let reader = FutBufReader::new(futures::io::Cursor::new(&bytes[..]));
        let reader = pin!(reader);
        let reader = CupsRasterUnifiedReader::new(reader)
            .await
            .map_err(|e| anyhow::anyhow!("open CUPS/PWG raster: {e:?}"))?;

        let mut pages: Vec<DynamicImage> = Vec::new();
        let mut next = reader
            .next_page()
            .await
            .map_err(|e| anyhow::anyhow!("read first page: {e:?}"))?;
        while let Some(mut page) = next {
            let header = page.header().clone();
            if header.v1.color_order != CupsColorOrder::Chunky {
                return Err(anyhow::anyhow!(
                    "unsupported color order: {:?}",
                    header.v1.color_order
                ));
            }

            let mut data = Vec::<u8>::new();
            page.content_mut()
                .read_to_end(&mut data)
                .await
                .map_err(io::Error::other)?;

            let w = header.v1.width;
            let h = header.v1.height;
            let image = match (header.v1.color_space, header.v1.bits_per_pixel) {
                (CupsColorSpace::sRGB, 24) | (CupsColorSpace::RGB, 24) => DynamicImage::ImageRgb8(
                    RgbImage::from_raw(w, h, data)
                        .ok_or_else(|| anyhow::anyhow!("invalid RGB pixel data"))?,
                ),
                (CupsColorSpace::sGray, 8) | (CupsColorSpace::Gray, 8) => DynamicImage::ImageLuma8(
                    GrayImage::from_raw(w, h, data)
                        .ok_or_else(|| anyhow::anyhow!("invalid Gray pixel data",))?,
                ),
                (cs, bpp) => {
                    return Err(anyhow::anyhow!(
                        "unsupported pixel data format: {:?} with {} bits per pixel",
                        cs,
                        bpp
                    ))
                }
            };

            pages.push(image);

            next = page
                .next_page()
                .await
                .map_err(|e| anyhow::anyhow!("advance page: {e:?}"))?;
        }

        Ok(pages)
    })
}

/// Load a multi-page TIFF.
pub fn load_pages_from_tiff(path: &Path) -> anyhow::Result<Vec<DynamicImage>> {
    let file = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut decoder = TiffDecoder::new(file)?;
    let mut pages = Vec::new();
    loop {
        let (w, h) = decoder.dimensions()?;
        let color = decoder.colortype()?;
        let raw = decoder.read_image()?;
        let image: DynamicImage = match (color, raw) {
            (ColorType::RGB(8), TiffDecodingResult::U8(data)) => {
                DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, data).ok_or_else(|| {
                    anyhow::anyhow!("TIFF page {} dimensions / buffer mismatch", pages.len())
                })?)
            }
            (ColorType::RGBA(8), TiffDecodingResult::U8(data)) => {
                DynamicImage::ImageRgba8(RgbaImage::from_raw(w, h, data).ok_or_else(|| {
                    anyhow::anyhow!("TIFF page {} dimensions / buffer mismatch", pages.len())
                })?)
            }
            (ColorType::Gray(8), TiffDecodingResult::U8(data)) => {
                DynamicImage::ImageLuma8(GrayImage::from_raw(w, h, data).ok_or_else(|| {
                    anyhow::anyhow!("TIFF page {} dimensions / buffer mismatch", pages.len())
                })?)
            }
            (ct, _) => return Err(anyhow::anyhow!("unsupported TIFF color type: {ct:?}")),
        };
        pages.push(image);
        if !decoder.more_images() {
            break;
        }
        decoder.next_image()?;
    }
    Ok(pages)
}
