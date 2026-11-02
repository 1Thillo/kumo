//! Pictures through ImageIO and video stills through AVFoundation: decoding for image wallpapers
//! and JPEG thumbnails for the library. Both ship with macOS.

use objc2::rc::Retained;
use objc2_av_foundation::{AVAssetImageGenerator, AVURLAsset};
use objc2_core_foundation::{kCFBooleanTrue, CFDictionary, CFNumber, CFRetained, CFString, CFType, CFURL, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo};
use objc2_core_media::CMTime;
use objc2_foundation::{NSString, NSURL};
use objc2_image_io::{kCGImageSourceCreateThumbnailFromImageAlways, kCGImageSourceCreateThumbnailWithTransform, kCGImageSourceThumbnailMaxPixelSize, CGImageDestination, CGImageSource};
use std::path::Path;

const THUMB_W: f64 = 480.0;
const THUMB_H: f64 = 270.0;
/// A video's first second is often black.
const STILL_AT_SECONDS: i64 = 1;

fn cf_url(path: &Path) -> Result<CFRetained<CFURL>, String> {
    CFURL::from_file_path(path).ok_or_else(|| format!("{}: not a file path", path.display()))
}

fn source(path: &Path) -> Result<CFRetained<CGImageSource>, String> {
    let url = cf_url(path)?;
    unsafe { CGImageSource::with_url(&url, None) }.ok_or_else(|| format!("{}: cannot read this picture", path.display()))
}

pub fn ns_url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

/// Pixel size of an image file without decoding it (a CGImage only decodes once it is drawn).
pub fn image_size(path: &Path) -> Result<(u32, u32), String> {
    let source = source(path)?;
    let image = unsafe { source.image_at_index(0, None) }.ok_or("no picture in this file")?;
    Ok((CGImage::width(Some(&image)) as u32, CGImage::height(Some(&image)) as u32))
}

/// Decodes a picture, its longer side at most `max_side` pixels: a 24-megapixel photo should not
/// sit in memory behind a screen with a fraction of that.
pub fn load_picture(path: &Path, max_side: u32) -> Result<CFRetained<CGImage>, String> {
    let yes: &CFType = unsafe { kCFBooleanTrue }.ok_or("no CFBoolean")?.as_ref();
    let side = CFNumber::new_i32(max_side as i32);
    let keys: [&CFString; 3] = unsafe { [kCGImageSourceCreateThumbnailFromImageAlways, kCGImageSourceCreateThumbnailWithTransform, kCGImageSourceThumbnailMaxPixelSize] };
    let options = CFDictionary::<CFString, CFType>::from_slices(&keys, &[yes, yes, side.as_ref()]);
    let source = source(path)?;
    unsafe { source.thumbnail_at_index(0, Some(options.as_opaque())) }.ok_or_else(|| format!("{}: cannot decode this picture", path.display()))
}

/// Writes a 480x270 JPEG thumbnail, cropped to fill.
fn write_thumbnail(image: &CGImage, dst: &Path) -> Result<(), String> {
    let (w, h) = (CGImage::width(Some(image)) as f64, CGImage::height(Some(image)) as f64);
    let k = (THUMB_W / w).max(THUMB_H / h);
    let (dw, dh) = (w * k, h * k);
    let rect = CGRect::new(CGPoint::new((THUMB_W - dw) / 2.0, (THUMB_H - dh) / 2.0), CGSize::new(dw, dh));
    let space = CGColorSpace::new_device_rgb().ok_or("no colour space")?;
    let context = unsafe { CGBitmapContextCreate(std::ptr::null_mut(), THUMB_W as usize, THUMB_H as usize, 8, 0, Some(&space), CGImageAlphaInfo::NoneSkipLast.0) }.ok_or("no bitmap context")?;
    CGContext::draw_image(Some(&context), rect, Some(image));
    let thumb = CGBitmapContextCreateImage(Some(&context)).ok_or("no thumbnail")?;
    let jpeg = CFString::from_static_str("public.jpeg");
    let url = cf_url(dst)?;
    let out = unsafe { CGImageDestination::with_url(&url, &jpeg, 1, None) }.ok_or("cannot write the thumbnail")?;
    unsafe { out.add_image(&thumb, None) };
    if unsafe { out.finalize() } {
        Ok(())
    } else {
        Err("cannot write the thumbnail".into())
    }
}

pub fn thumbnail_from_file(src: &Path, dst: &Path) -> Result<(), String> {
    let image = load_picture(src, (THUMB_W * 2.0) as u32)?;
    write_thumbnail(&image, dst)
}

/// Same name as on Windows, where the Explorer's thumbnails do this: a still from the video.
pub fn thumbnail_via_shell(src: &Path, dst: &Path) -> Result<(), String> {
    let asset = unsafe { AVURLAsset::URLAssetWithURL_options(&ns_url(src), None) };
    let generator = unsafe { AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset) };
    unsafe {
        generator.setAppliesPreferredTrackTransform(true);
        generator.setMaximumSize(CGSize::new(THUMB_W * 2.0, THUMB_H * 2.0));
    }
    let at = unsafe { CMTime::new(STILL_AT_SECONDS, 1) };
    #[allow(deprecated)]
    let still = unsafe { generator.copyCGImageAtTime_actualTime_error(at, std::ptr::null_mut()) }.map_err(|e| e.localizedDescription().to_string())?;
    write_thumbnail(&still, dst)
}
