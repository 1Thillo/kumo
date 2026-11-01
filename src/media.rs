//! Still images through WIC (Windows Imaging Component): decoding for image wallpapers and
//! JPEG thumbnails for the library. WIC ships with Windows and handles JPEG, PNG, GIF, BMP,
//! TIFF and, with the store codecs, WebP/HEIC/AVIF.

use std::path::Path;
use windows::core::{Result, GUID, HSTRING};
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;

fn factory() -> Result<IWICImagingFactory> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
    }
}

fn open(f: &IWICImagingFactory, path: &Path) -> Result<IWICBitmapSource> {
    unsafe {
        let dec = f.CreateDecoderFromFilename(&HSTRING::from(path.as_os_str()), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)?;
        Ok(dec.GetFrame(0)?.into())
    }
}

fn size(src: &IWICBitmapSource) -> Result<(u32, u32)> {
    let (mut w, mut h) = (0, 0);
    unsafe { src.GetSize(&mut w, &mut h)? };
    Ok((w, h))
}

fn scaled(f: &IWICImagingFactory, src: &IWICBitmapSource, w: u32, h: u32) -> Result<IWICBitmapSource> {
    unsafe {
        let s = f.CreateBitmapScaler()?;
        s.Initialize(src, w, h, WICBitmapInterpolationModeHighQualityCubic)?;
        Ok(s.into())
    }
}

fn converted(f: &IWICImagingFactory, src: &IWICBitmapSource, fmt: &GUID) -> Result<IWICBitmapSource> {
    unsafe {
        let c = f.CreateFormatConverter()?;
        c.Initialize(src, fmt, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
        Ok(c.into())
    }
}

fn pixels(src: &IWICBitmapSource, w: u32, h: u32, bpp: u32) -> Result<Vec<u8>> {
    let stride = w * bpp;
    let mut buf = vec![0u8; (stride * h) as usize];
    unsafe { src.CopyPixels(std::ptr::null(), stride, &mut buf)? };
    Ok(buf)
}

/// Decodes an image as BGRA, shrunk (never enlarged) so it still covers `max_w` x `max_h`.
/// Big photos are scaled down right away so a 24-megapixel picture does not sit in memory.
pub fn load_bgra(path: &Path, max_w: u32, max_h: u32) -> Result<(Vec<u8>, u32, u32)> {
    let f = factory()?;
    let src = open(&f, path)?;
    let (w, h) = size(&src)?;
    let k = (max_w as f32 / w as f32).max(max_h as f32 / h as f32).min(1.0);
    let (tw, th) = (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1));
    let src = if k < 1.0 { scaled(&f, &src, tw, th)? } else { src };
    let src = converted(&f, &src, &GUID_WICPixelFormat32bppBGRA)?;
    Ok((pixels(&src, tw, th, 4)?, tw, th))
}

/// Pixel size of an image file without decoding it.
pub fn image_size(path: &Path) -> Result<(u32, u32)> {
    let f = factory()?;
    size(&open(&f, path)?)
}

/// Writes a 480x270 JPEG thumbnail (cropped to fill) from an image file.
pub fn thumbnail_from_file(src: &Path, dst: &Path) -> Result<()> {
    let (bgra, w, h) = load_bgra(src, 480, 270)?;
    thumbnail_from_bgra(&bgra, w, h, dst)
}

/// Writes a 480x270 JPEG thumbnail (cropped to fill) from BGRA pixels.
pub fn thumbnail_from_bgra(bgra: &[u8], w: u32, h: u32, dst: &Path) -> Result<()> {
    const TW: u32 = 480;
    const TH: u32 = 270;
    let f = factory()?;
    unsafe {
        let bmp = f.CreateBitmapFromMemory(w, h, &GUID_WICPixelFormat32bppBGRA, w * 4, bgra)?;
        // crop to 16:9, then scale
        let (cw, ch) = if w * TH > h * TW { (h * TW / TH, h) } else { (w, w * TH / TW) };
        let clip = f.CreateBitmapClipper()?;
        let rect = WICRect { X: ((w - cw) / 2) as i32, Y: ((h - ch) / 2) as i32, Width: cw.max(1) as i32, Height: ch.max(1) as i32 };
        clip.Initialize(&bmp, &rect)?;
        let src = scaled(&f, &clip.into(), TW, TH)?;
        let src = converted(&f, &src, &GUID_WICPixelFormat24bppBGR)?;
        let px = pixels(&src, TW, TH, 3)?;

        let stream = f.CreateStream()?;
        stream.InitializeFromFilename(&HSTRING::from(dst.as_os_str()), GENERIC_WRITE.0)?;
        let enc = f.CreateEncoder(&GUID_ContainerFormatJpeg, std::ptr::null())?;
        enc.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut frame: Option<IWICBitmapFrameEncode> = None;
        let mut props: Option<windows::Win32::System::Com::StructuredStorage::IPropertyBag2> = None;
        enc.CreateNewFrame(&mut frame, &mut props)?;
        let frame = frame.unwrap();
        frame.Initialize(props.as_ref())?;
        frame.SetSize(TW, TH)?;
        let mut fmt = GUID_WICPixelFormat24bppBGR;
        frame.SetPixelFormat(&mut fmt)?;
        frame.WritePixels(TH, TW * 3, &px)?;
        frame.Commit()?;
        enc.Commit()?;
    }
    Ok(())
}

use windows::Win32::Foundation::GENERIC_WRITE;

/// Thumbnail of any file the Explorer can preview (videos included) via the shell's thumbnail
/// cache, written as a 480x270 JPEG.
pub fn thumbnail_via_shell(src: &Path, dst: &Path) -> Result<()> {
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_RESIZETOFIT};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let item: IShellItemImageFactory = SHCreateItemFromParsingName(&HSTRING::from(src.as_os_str()), None)?;
        let hbm = item.GetImage(SIZE { cx: 960, cy: 540 }, SIIGBF_RESIZETOFIT | SIIGBF_BIGGERSIZEOK)?;
        let mut bm = BITMAP::default();
        GetObjectW(hbm, std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut _));
        let (w, h) = (bm.bmWidth.max(1) as u32, bm.bmHeight.unsigned_abs().max(1));
        let mut bi = BITMAPINFO::default();
        bi.bmiHeader = BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: w as i32, biHeight: -(h as i32), biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() };
        let mut px = vec![0u8; (w * h * 4) as usize];
        let dc = GetDC(None);
        let lines = GetDIBits(dc, hbm, 0, h, Some(px.as_mut_ptr().cast()), &mut bi, DIB_RGB_COLORS);
        ReleaseDC(None, dc);
        let _ = DeleteObject(hbm);
        if lines == 0 {
            return Err(windows::core::Error::from_win32());
        }
        for p in px.chunks_exact_mut(4) {
            p[3] = 255;
        }
        thumbnail_from_bgra(&px, w, h, dst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_thumbnail_from_pixels() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("test-out");
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("thumb.jpg");
        let _ = std::fs::remove_file(&out);
        let px = vec![128u8; 640 * 360 * 4];
        thumbnail_from_bgra(&px, 640, 360, &out).unwrap();
        assert!(std::fs::metadata(&out).unwrap().len() > 100);
        assert_eq!(image_size(&out).unwrap(), (480, 270));
        let (bgra, w, h) = load_bgra(&out, 240, 135).unwrap();
        assert_eq!((w, h), (240, 135));
        assert_eq!(bgra.len(), 240 * 135 * 4);
    }

    #[test]
    fn missing_files_are_errors_not_crashes() {
        assert!(image_size(Path::new("Z:\\does\\not\\exist.png")).is_err());
    }
}
