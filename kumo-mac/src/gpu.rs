//! Thin wgpu layer: device, render targets and the shadow map. Metal on the Mac, Direct3D 12 when
//! the renderer is developed on Windows.

pub const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const SHADOW: wgpu::TextureFormat = wgpu::TextureFormat::Depth16Unorm;

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub fn new() -> Result<Gpu, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::PRIMARY;
        let instance = wgpu::Instance::new(desc);
        // on a Mac with two GPUs the wallpaper stays on the integrated one
        let options = wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::LowPower, force_fallback_adapter: false, compatible_surface: None, apply_limit_buckets: false };
        let adapter = pollster::block_on(instance.request_adapter(&options)).map_err(|e| e.to_string())?;
        let limits = wgpu::Limits { max_texture_dimension_2d: adapter.limits().max_texture_dimension_2d, ..wgpu::Limits::default() };
        let desc = wgpu::DeviceDescriptor { label: Some("kumo"), required_limits: limits, memory_hints: wgpu::MemoryHints::MemoryUsage, ..Default::default() };
        let (device, queue) = pollster::block_on(adapter.request_device(&desc)).map_err(|e| e.to_string())?;
        Ok(Gpu { instance, adapter, device, queue })
    }

    pub fn name(&self) -> String {
        let i = self.adapter.get_info();
        format!("{} ({:?})", i.name, i.backend)
    }
}

/// The bytes of plain `repr(C)` values, ready to upload.
pub fn bytes<T: Copy>(data: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr().cast(), std::mem::size_of_val(data)) }
}

/// Offscreen colour + depth that a scene view renders into before the post pass.
pub struct Target {
    pub w: u32,
    pub h: u32,
    pub color: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub depth: wgpu::TextureView,
}

impl Target {
    pub fn new(gpu: &Gpu, w: u32, h: u32) -> Target {
        let texture = |format, usage| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = texture(COLOR, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC);
        let depth = texture(DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT);
        Target { w, h, view: color.create_view(&Default::default()), depth: depth.create_view(&Default::default()), color }
    }

    /// Reads the colour target back as tightly packed RGBA rows (for screenshots).
    pub fn read_rgba(&self, gpu: &Gpu) -> Result<Vec<u8>, String> {
        let row = (self.w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (row * self.h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            self.color.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &staging, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None } },
            wgpu::Extent3d { width: self.w, height: self.h, depth_or_array_layers: 1 },
        );
        gpu.queue.submit([enc.finish()]);
        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        let mapped = slice.get_mapped_range().map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity((self.w * self.h * 4) as usize);
        for y in 0..self.h {
            out.extend_from_slice(&mapped[(y * row) as usize..][..(self.w * 4) as usize]);
        }
        Ok(out)
    }
}

pub struct ShadowMap {
    pub view: wgpu::TextureView,
}

impl ShadowMap {
    pub fn new(gpu: &Gpu, size: u32) -> ShadowMap {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        ShadowMap { view: texture.create_view(&Default::default()) }
    }
}
