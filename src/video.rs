//! Video wallpapers: hardware decoding with Media Foundation's source reader.
//!
//! The reader works on demand: a frame is only decoded when the frame loop asks for one, so a
//! covered or paused video costs no CPU or GPU at all. Decoded NV12 frames stay in the decoder's
//! surfaces and are converted straight into each screen's back buffer by the GPU's video
//! processor, which saves the extra BGRA copy the post pass would need (about 25 MB at 1080p).
//! Audio is never selected, so it is neither parsed nor decoded.

use crate::config::Fit;
use crate::gfx::{Gpu, Swap};
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};
use windows::core::{w, Error, Interface, Result, GUID, HSTRING, PROPVARIANT};
use windows::Win32::Foundation::{E_FAIL, FALSE, RECT, TRUE};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_RATIONAL;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::LibraryLoader::{LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};

const STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

/// Media Foundation plus the device manager that lets decoders use the engine's D3D device.
/// Created when the first video is loaded, so scenes and pictures never pay for it. MFShutdown is
/// left to the end of the process on purpose: it unloads the decoders, and any of their threads
/// that is still finishing then crashes the engine.
pub struct Mf {
    manager: IMFDXGIDeviceManager,
}

impl Mf {
    pub fn new(gpu: &Gpu) -> Result<Mf> {
        unsafe {
            // delay-loaded: on Windows N editions the first call would bring the whole engine down
            for dll in [w!("mfplat.dll"), w!("mfreadwrite.dll")] {
                LoadLibraryExW(dll, None, LOAD_LIBRARY_SEARCH_SYSTEM32).map_err(|_| Error::new(E_FAIL, "Windows has no Media Foundation here (N editions need the Media Feature Pack)"))?;
            }
            MFStartup(MF_VERSION, MFSTARTUP_LITE)?;
            let mut token = 0u32;
            let mut manager = None;
            MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
            let manager = manager.unwrap();
            manager.ResetDevice(&gpu.dev, token)?;
            Ok(Mf { manager })
        }
    }
}

/// The GPU's colour converter and scaler for one output size.
struct Processor {
    enumerator: ID3D11VideoProcessorEnumerator,
    processor: ID3D11VideoProcessor,
    inputs: HashMap<(usize, u32), ID3D11VideoProcessorInputView>,
    outputs: HashMap<usize, ID3D11VideoProcessorOutputView>,
}

struct Open {
    reader: IMFSourceReader,
    vdev: ID3D11VideoDevice,
    vctx: ID3D11VideoContext,
    processors: HashMap<(u32, u32), Processor>,
    /// the frame on screen, kept for screens that come back into view
    sample: Option<IMFSample>,
    /// timestamp of `sample` in seconds
    shown: f64,
}

pub struct Video {
    path: PathBuf,
    /// the picture part of the decoder's frames, which can be larger (1920x1088 for 1080p)
    pub size: (u32, u32),
    coded: (u32, u32),
    fps: f64,
    color: u32,
    open: Option<Open>,
    /// playback position in seconds
    clock: f64,
}

fn visible_size(mt: &IMFMediaType, coded: (u32, u32)) -> (u32, u32) {
    let mut area = MFVideoArea::default();
    let bytes = unsafe { std::slice::from_raw_parts_mut(&mut area as *mut MFVideoArea as *mut u8, std::mem::size_of::<MFVideoArea>()) };
    match unsafe { mt.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, bytes, None) } {
        Ok(()) if area.Area.cx > 0 && area.Area.cy > 0 => ((area.Area.cx as u32).min(coded.0), (area.Area.cy as u32).min(coded.1)),
        _ => coded,
    }
}

fn open_reader(mf: &Mf, path: &Path) -> Result<(IMFSourceReader, (u32, u32), (u32, u32), f64, u32)> {
    unsafe {
        let mut attrs = None;
        MFCreateAttributes(&mut attrs, 2)?;
        let attrs = attrs.unwrap();
        attrs.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &mf.manager)?;
        attrs.SetUINT32(&MF_LOW_LATENCY, 1)?; // no reorder buffering: fewer decoder surfaces
        let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attrs)?;
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, FALSE)?;
        reader.SetStreamSelection(STREAM, TRUE)?;
        let mt = MFCreateMediaType()?;
        mt.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        mt.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        if reader.SetCurrentMediaType(STREAM, None, &mt).is_err() {
            // 10-bit sources only come out as P010
            mt.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_P010)?;
            reader.SetCurrentMediaType(STREAM, None, &mt).map_err(|_| Error::new(E_FAIL, "Windows has no decoder for this video format"))?;
        }
        let cur = reader.GetCurrentMediaType(STREAM)?;
        let fs = cur.GetUINT64(&MF_MT_FRAME_SIZE)?;
        let coded = ((fs >> 32) as u32, fs as u32);
        let size = visible_size(&cur, coded);
        let fps = cur.GetUINT64(&MF_MT_FRAME_RATE).map(|v| (v >> 32) as f64 / (v as u32).max(1) as f64).unwrap_or(30.0);
        // D3D11_VIDEO_PROCESSOR_COLOR_SPACE bits: YCbCr_Matrix (2) = BT.709, Nominal_Range (4..5)
        let bt709 = cur.GetUINT32(&MF_MT_YUV_MATRIX).map(|m| m != 2).unwrap_or(size.1 >= 720);
        let full = cur.GetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE).map(|r| r == 1).unwrap_or(false);
        let color = (bt709 as u32) << 2 | if full { 2 << 4 } else { 1 << 4 };
        Ok((reader, size, coded, fps, color))
    }
}

impl Video {
    pub fn new(mf: &Mf, gpu: &Gpu, path: &Path) -> Result<Video> {
        let mut v = Video { path: path.to_path_buf(), size: (0, 0), coded: (0, 0), fps: 30.0, color: 0, open: None, clock: 0.0 };
        v.advance(mf, gpu, 0.0, 1.0)?;
        // frames only reach the screen as GPU textures: a stream that fell back to software
        // decoding has to fail here, where the engine can still show something else
        let Some(sample) = v.open.as_ref().and_then(|o| o.sample.as_ref()) else { return Err(Error::new(E_FAIL, "the video has no frames")) };
        unsafe { sample.GetBufferByIndex(0)?.cast::<IMFDXGIBuffer>() }.map_err(|_| Error::new(E_FAIL, "the graphics card cannot decode this video format"))?;
        Ok(v)
    }

    fn ensure_open(&mut self, mf: &Mf, gpu: &Gpu) -> Result<()> {
        if self.open.is_some() {
            return Ok(());
        }
        let (reader, size, coded, fps, color) = open_reader(mf, &self.path)?;
        (self.size, self.coded, self.fps, self.color) = (size, coded, fps, color);
        if self.clock > 0.0 {
            unsafe { reader.SetCurrentPosition(&GUID::zeroed(), &PROPVARIANT::from((self.clock * 1e7) as i64))? };
        }
        self.open = Some(Open { reader, vdev: gpu.dev.cast()?, vctx: gpu.ctx.cast()?, processors: HashMap::new(), sample: None, shown: -1.0 });
        Ok(())
    }

    /// Frees the decoder and its surfaces; `advance` reopens at the same position.
    pub fn release(&mut self) {
        self.open = None;
    }

    /// The swap chains are about to go: views of their buffers would keep them alive.
    pub fn outputs_changed(&mut self) {
        if let Some(o) = &mut self.open {
            o.processors.clear();
        }
    }

    /// Moves the clock on and decodes up to the current frame. Returns true if the picture
    /// changed. Frames are paced by their timestamps, so a 24 fps video in a 30 fps loop only
    /// presents 24 times a second, and a 60 fps video skips frames instead of falling behind.
    pub fn advance(&mut self, mf: &Mf, gpu: &Gpu, dt: f32, speed: f32) -> Result<bool> {
        self.ensure_open(mf, gpu)?;
        self.clock += (dt * speed) as f64;
        let frame = 1.0 / self.fps.max(1.0);
        let o = self.open.as_mut().unwrap();
        let mut changed = false;
        for _ in 0..6 {
            if o.sample.is_some() && o.shown + frame > self.clock {
                break;
            }
            let (mut idx, mut flags, mut ts) = (0u32, 0u32, 0i64);
            let mut sample = None;
            unsafe { o.reader.ReadSample(STREAM, 0, Some(&mut idx), Some(&mut flags), Some(&mut ts), Some(&mut sample))? };
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                unsafe { o.reader.SetCurrentPosition(&GUID::zeroed(), &PROPVARIANT::from(0i64))? };
                self.clock = 0.0;
                o.shown = -1.0;
                continue;
            }
            if sample.is_some() {
                o.sample = sample;
                o.shown = ts as f64 / 1e7;
                changed = true;
            }
        }
        Ok(changed)
    }

    /// Converts the current frame into a screen's back buffer.
    pub fn draw(&mut self, swap: &Swap, fit: Fit) -> Result<()> {
        let (vw, vh) = self.size;
        let (cw, ch) = self.coded;
        let color = self.color;
        let fps = self.fps;
        let Some(o) = self.open.as_mut() else { return Ok(()) };
        let Some(sample) = &o.sample else { return Ok(()) };
        let (w, h) = (swap.w, swap.h);
        let (src, dst) = fit_rects(fit, (vw, vh), (w, h));
        unsafe {
            let dxgi: IMFDXGIBuffer = sample.GetBufferByIndex(0)?.cast()?;
            let mut p = std::ptr::null_mut();
            dxgi.GetResource(&ID3D11Texture2D::IID, &mut p)?;
            let tex = ID3D11Texture2D::from_raw(p);
            let slice = dxgi.GetSubresourceIndex()?;

            if !o.processors.contains_key(&(w, h)) {
                let rate = DXGI_RATIONAL { Numerator: (fps * 1000.0) as u32, Denominator: 1000 };
                let desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
                    InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                    InputFrameRate: rate,
                    InputWidth: cw,
                    InputHeight: ch,
                    OutputFrameRate: rate,
                    OutputWidth: w,
                    OutputHeight: h,
                    Usage: D3D11_VIDEO_USAGE_OPTIMAL_SPEED,
                };
                let enumerator = o.vdev.CreateVideoProcessorEnumerator(&desc)?;
                let processor = o.vdev.CreateVideoProcessor(&enumerator, 0)?;
                let cs = D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: color };
                o.vctx.VideoProcessorSetStreamColorSpace(&processor, 0, &cs);
                o.vctx.VideoProcessorSetStreamFrameFormat(&processor, 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
                o.vctx.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, FALSE);
                o.processors.insert((w, h), Processor { enumerator, processor, inputs: HashMap::new(), outputs: HashMap::new() });
            }
            let vp = o.processors.get_mut(&(w, h)).unwrap();

            let key = (tex.as_raw() as usize, slice);
            if !vp.inputs.contains_key(&key) {
                let d = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
                    FourCC: 0,
                    ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPIV { MipSlice: 0, ArraySlice: slice } },
                };
                let mut v = None;
                o.vdev.CreateVideoProcessorInputView(&tex, &vp.enumerator, &d, Some(&mut v))?;
                vp.inputs.insert(key, v.unwrap());
            }
            let out_key = swap.back.as_raw() as usize;
            if !vp.outputs.contains_key(&out_key) {
                let d = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                    ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 } },
                };
                let mut v = None;
                o.vdev.CreateVideoProcessorOutputView(&swap.back, &vp.enumerator, &d, Some(&mut v))?;
                vp.outputs.insert(out_key, v.unwrap());
            }
            let full = RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 };
            o.vctx.VideoProcessorSetStreamSourceRect(&vp.processor, 0, TRUE, Some(&src));
            o.vctx.VideoProcessorSetStreamDestRect(&vp.processor, 0, TRUE, Some(&dst));
            o.vctx.VideoProcessorSetOutputTargetRect(&vp.processor, TRUE, Some(&full));
            let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
                Enable: TRUE,
                pInputSurface: ManuallyDrop::new(Some(vp.inputs[&key].clone())),
                ..std::mem::zeroed()
            };
            let r = o.vctx.VideoProcessorBlt(&vp.processor, &vp.outputs[&out_key], 0, std::slice::from_ref(&stream));
            ManuallyDrop::drop(&mut stream.pInputSurface);
            r
        }
    }
}

/// Source rectangle in the video and destination rectangle on the screen for a fit mode.
fn fit_rects(fit: Fit, (vw, vh): (u32, u32), (w, h): (u32, u32)) -> (RECT, RECT) {
    let rect = |x: f32, y: f32, rw: f32, rh: f32| RECT { left: x as i32, top: y as i32, right: (x + rw) as i32, bottom: (y + rh) as i32 };
    let (v, s) = (vw as f32 / vh as f32, w as f32 / h as f32);
    let (fw, fh, sw, sh) = (vw as f32, vh as f32, w as f32, h as f32);
    let all_src = rect(0.0, 0.0, fw, fh);
    let all_dst = rect(0.0, 0.0, sw, sh);
    match fit {
        Fit::Stretch => (all_src, all_dst),
        Fit::Cover if v > s => (rect((fw - fh * s) / 2.0, 0.0, fh * s, fh), all_dst),
        Fit::Cover => (rect(0.0, (fh - fw / s) / 2.0, fw, fw / s), all_dst),
        Fit::Contain if v > s => (all_src, rect(0.0, (sh - sw / v) / 2.0, sw, sw / v)),
        Fit::Contain => (all_src, rect((sw - sh * v) / 2.0, 0.0, sh * v, sh)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuple(r: RECT) -> (i32, i32, i32, i32) {
        (r.left, r.top, r.right, r.bottom)
    }

    #[test]
    fn cover_crops_the_video() {
        let (src, dst) = fit_rects(Fit::Cover, (1920, 1080), (1080, 1080));
        assert_eq!(tuple(src), (420, 0, 1500, 1080));
        assert_eq!(tuple(dst), (0, 0, 1080, 1080));
    }

    #[test]
    fn contain_centres_the_video() {
        let (src, dst) = fit_rects(Fit::Contain, (1920, 1080), (1080, 1080));
        assert_eq!(tuple(src), (0, 0, 1920, 1080));
        assert_eq!(tuple(dst), (0, 236, 1080, 843));
    }
}
