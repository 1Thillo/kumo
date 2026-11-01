//! Thin Direct3D 11 layer: device, buffers, shaders, render targets and swap chains.

use windows::core::{Interface, Result};
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

pub struct Gpu {
    pub dev: ID3D11Device,
    pub ctx: ID3D11DeviceContext,
    pub factory: IDXGIFactory2,
}

impl Gpu {
    pub fn new() -> Result<Gpu> {
        let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
        let create = |flags| {
            let (mut dev, mut ctx) = (None, None);
            unsafe { D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), flags, Some(&levels), D3D11_SDK_VERSION, Some(&mut dev), None, Some(&mut ctx))? };
            Ok::<_, windows::core::Error>((dev, ctx))
        };
        // video decoding needs VIDEO_SUPPORT; a few drivers (remote sessions) refuse it
        let (dev, ctx) = create(D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT).or_else(|_| create(D3D11_CREATE_DEVICE_BGRA_SUPPORT))?;
        let dev: ID3D11Device = dev.unwrap();
        // Media Foundation uses the device from its own threads
        if let Ok(mt) = dev.cast::<windows::Win32::Graphics::Direct3D10::ID3D10Multithread>() {
            unsafe {
                let _ = mt.SetMultithreadProtected(true);
            }
        }
        let dxgi: IDXGIDevice1 = dev.cast()?;
        let factory: IDXGIFactory2 = unsafe {
            dxgi.SetMaximumFrameLatency(1)?;
            dxgi.GetAdapter()?.GetParent()?
        };
        Ok(Gpu { dev, ctx: ctx.unwrap(), factory })
    }

    /// Immutable buffer with initial data.
    pub fn buffer(&self, data: &[u8], bind: D3D11_BIND_FLAG) -> Result<ID3D11Buffer> {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: data.len() as u32,
            Usage: D3D11_USAGE_IMMUTABLE,
            BindFlags: bind.0 as u32,
            ..Default::default()
        };
        let init = D3D11_SUBRESOURCE_DATA { pSysMem: data.as_ptr().cast(), ..Default::default() };
        let mut buf = None;
        unsafe { self.dev.CreateBuffer(&desc, Some(&init), Some(&mut buf))? };
        Ok(buf.unwrap())
    }

    /// CPU-writable constant buffer for `T`.
    pub fn constants<T>(&self) -> Result<ID3D11Buffer> {
        let size = (std::mem::size_of::<T>() as u32 + 15) & !15;
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: size,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            ..Default::default()
        };
        let mut buf = None;
        unsafe { self.dev.CreateBuffer(&desc, None, Some(&mut buf))? };
        Ok(buf.unwrap())
    }

    /// CPU-writable structured buffer of `count` elements of `T`, plus its shader view.
    pub fn structured<T>(&self, count: u32) -> Result<(ID3D11Buffer, ID3D11ShaderResourceView)> {
        let stride = std::mem::size_of::<T>() as u32;
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: stride * count,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32,
            StructureByteStride: stride,
        };
        let mut buf = None;
        unsafe { self.dev.CreateBuffer(&desc, None, Some(&mut buf))? };
        let buf = buf.unwrap();
        let view = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_UNKNOWN,
            ViewDimension: D3D11_SRV_DIMENSION_BUFFER,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Buffer: D3D11_BUFFER_SRV {
                    Anonymous1: D3D11_BUFFER_SRV_0 { FirstElement: 0 },
                    Anonymous2: D3D11_BUFFER_SRV_1 { NumElements: count },
                },
            },
        };
        let mut srv = None;
        unsafe { self.dev.CreateShaderResourceView(&buf, Some(&view), Some(&mut srv))? };
        Ok((buf, srv.unwrap()))
    }

    /// Replaces the whole content of a dynamic buffer.
    pub fn upload<T: Copy>(&self, buf: &ID3D11Buffer, data: &[T]) {
        if data.is_empty() {
            return;
        }
        unsafe {
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            if self.ctx.Map(buf, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut m)).is_ok() {
                std::ptr::copy_nonoverlapping(data.as_ptr(), m.pData.cast::<T>(), data.len());
                self.ctx.Unmap(buf, 0);
            }
        }
    }

    pub fn vertex_shader(&self, code: &[u8]) -> Result<ID3D11VertexShader> {
        let mut s = None;
        unsafe { self.dev.CreateVertexShader(code, None, Some(&mut s))? };
        Ok(s.unwrap())
    }

    pub fn pixel_shader(&self, code: &[u8]) -> Result<ID3D11PixelShader> {
        let mut s = None;
        unsafe { self.dev.CreatePixelShader(code, None, Some(&mut s))? };
        Ok(s.unwrap())
    }

    pub fn blend(&self, src: D3D11_BLEND, dst: D3D11_BLEND) -> Result<ID3D11BlendState> {
        let mut desc = D3D11_BLEND_DESC::default();
        desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            SrcBlend: src,
            DestBlend: dst,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_ONE,
            DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };
        let mut s = None;
        unsafe { self.dev.CreateBlendState(&desc, Some(&mut s))? };
        Ok(s.unwrap())
    }

    pub fn raster(&self, cull: D3D11_CULL_MODE) -> Result<ID3D11RasterizerState> {
        let desc = D3D11_RASTERIZER_DESC {
            FillMode: D3D11_FILL_SOLID,
            CullMode: cull,
            // scenes use the OpenGL / three.js convention: counter-clockwise faces are front faces
            FrontCounterClockwise: true.into(),
            DepthClipEnable: true.into(),
            ..Default::default()
        };
        let mut s = None;
        unsafe { self.dev.CreateRasterizerState(&desc, Some(&mut s))? };
        Ok(s.unwrap())
    }

    pub fn depth(&self, test: bool, write: bool) -> Result<ID3D11DepthStencilState> {
        let desc = D3D11_DEPTH_STENCIL_DESC {
            DepthEnable: test.into(),
            DepthWriteMask: if write { D3D11_DEPTH_WRITE_MASK_ALL } else { D3D11_DEPTH_WRITE_MASK_ZERO },
            DepthFunc: D3D11_COMPARISON_LESS_EQUAL,
            ..Default::default()
        };
        let mut s = None;
        unsafe { self.dev.CreateDepthStencilState(&desc, Some(&mut s))? };
        Ok(s.unwrap())
    }

    pub fn sampler(&self, filter: D3D11_FILTER, cmp: D3D11_COMPARISON_FUNC) -> Result<ID3D11SamplerState> {
        let desc = D3D11_SAMPLER_DESC {
            Filter: filter,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            ComparisonFunc: cmp,
            MaxLOD: f32::MAX,
            ..Default::default()
        };
        let mut s = None;
        unsafe { self.dev.CreateSamplerState(&desc, Some(&mut s))? };
        Ok(s.unwrap())
    }

    /// Lets the driver give back temporary allocations (called while paused).
    pub fn trim(&self) {
        unsafe {
            self.ctx.ClearState();
            self.ctx.Flush();
            if let Ok(d) = self.dev.cast::<IDXGIDevice3>() {
                d.Trim();
            }
        }
    }
}

/// Offscreen colour + depth that a scene view renders into before the post pass.
pub struct Target {
    pub w: u32,
    pub h: u32,
    pub color: ID3D11Texture2D,
    pub rtv: ID3D11RenderTargetView,
    pub srv: ID3D11ShaderResourceView,
    pub dsv: ID3D11DepthStencilView,
}

impl Target {
    pub fn new(gpu: &Gpu, w: u32, h: u32) -> Result<Target> {
        let mut desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            ..Default::default()
        };
        let (mut color, mut depth, mut rtv, mut srv, mut dsv) = (None, None, None, None, None);
        unsafe {
            gpu.dev.CreateTexture2D(&desc, None, Some(&mut color))?;
            desc.Format = DXGI_FORMAT_D32_FLOAT;
            desc.BindFlags = D3D11_BIND_DEPTH_STENCIL.0 as u32;
            gpu.dev.CreateTexture2D(&desc, None, Some(&mut depth))?;
            let color = color.as_ref().unwrap();
            gpu.dev.CreateRenderTargetView(color, None, Some(&mut rtv))?;
            gpu.dev.CreateShaderResourceView(color, None, Some(&mut srv))?;
            gpu.dev.CreateDepthStencilView(depth.as_ref().unwrap(), None, Some(&mut dsv))?;
        }
        Ok(Target { w, h, color: color.unwrap(), rtv: rtv.unwrap(), srv: srv.unwrap(), dsv: dsv.unwrap() })
    }

    /// Reads the colour target back as tightly packed RGBA rows (for screenshots).
    pub fn read_rgba(&self, gpu: &Gpu) -> Result<Vec<u8>> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: self.w,
            Height: self.h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..Default::default()
        };
        let mut staging = None;
        let mut out = vec![0u8; (self.w * self.h * 4) as usize];
        unsafe {
            gpu.dev.CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging = staging.unwrap();
            gpu.ctx.CopyResource(&staging, &self.color);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            gpu.ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            for y in 0..self.h as usize {
                let row = std::slice::from_raw_parts((m.pData as *const u8).add(y * m.RowPitch as usize), self.w as usize * 4);
                out[y * self.w as usize * 4..][..row.len()].copy_from_slice(row);
            }
            gpu.ctx.Unmap(&staging, 0);
        }
        Ok(out)
    }
}

pub struct ShadowMap {
    pub size: u32,
    pub dsv: ID3D11DepthStencilView,
    pub srv: ID3D11ShaderResourceView,
}

impl ShadowMap {
    pub fn new(gpu: &Gpu, size: u32) -> Result<ShadowMap> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: size,
            Height: size,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R16_TYPELESS,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_DEPTH_STENCIL.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            ..Default::default()
        };
        let dsv_desc = D3D11_DEPTH_STENCIL_VIEW_DESC {
            Format: DXGI_FORMAT_D16_UNORM,
            ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
            Flags: 0,
            Anonymous: D3D11_DEPTH_STENCIL_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_DSV { MipSlice: 0 } },
        };
        let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R16_UNORM,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1 } },
        };
        let (mut tex, mut dsv, mut srv) = (None, None, None);
        unsafe {
            gpu.dev.CreateTexture2D(&desc, None, Some(&mut tex))?;
            let tex = tex.unwrap();
            gpu.dev.CreateDepthStencilView(&tex, Some(&dsv_desc), Some(&mut dsv))?;
            gpu.dev.CreateShaderResourceView(&tex, Some(&srv_desc), Some(&mut srv))?;
        }
        Ok(ShadowMap { size, dsv: dsv.unwrap(), srv: srv.unwrap() })
    }
}

/// A window's swap chain (flip model, two buffers, no MSAA, no depth).
pub struct Swap {
    pub chain: IDXGISwapChain1,
    pub back: ID3D11Texture2D,
    pub rtv: ID3D11RenderTargetView,
    pub w: u32,
    pub h: u32,
    pub flip: bool,
}

impl Swap {
    pub fn new(gpu: &Gpu, hwnd: HWND, w: u32, h: u32) -> Result<Swap> {
        let mut desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: w,
            Height: h,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: 0,
            Stereo: false.into(),
        };
        let chain = unsafe { gpu.factory.CreateSwapChainForHwnd(&gpu.dev, hwnd, &desc, None, None) }.or_else(|_| {
            // older systems / unusual parents: fall back to the classic blt model
            desc.SwapEffect = DXGI_SWAP_EFFECT_DISCARD;
            desc.BufferCount = 1;
            unsafe { gpu.factory.CreateSwapChainForHwnd(&gpu.dev, hwnd, &desc, None, None) }
        })?;
        unsafe { gpu.factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER | DXGI_MWA_NO_WINDOW_CHANGES)? };
        let back: ID3D11Texture2D = unsafe { chain.GetBuffer(0)? };
        let mut rtv = None;
        unsafe { gpu.dev.CreateRenderTargetView(&back, None, Some(&mut rtv))? };
        Ok(Swap { chain, back, rtv: rtv.unwrap(), w, h, flip: desc.SwapEffect == DXGI_SWAP_EFFECT_FLIP_DISCARD })
    }

    pub fn present(&self) {
        unsafe {
            let _ = self.chain.Present(0, DXGI_PRESENT(0));
        }
    }
}

/// An immutable BGRA texture (pictures); the CPU copy can be dropped right after.
pub fn texture_bgra(gpu: &Gpu, px: &[u8], w: u32, h: u32) -> Result<ID3D11ShaderResourceView> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_IMMUTABLE,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        ..Default::default()
    };
    let init = D3D11_SUBRESOURCE_DATA { pSysMem: px.as_ptr().cast(), SysMemPitch: w * 4, SysMemSlicePitch: 0 };
    let (mut tex, mut srv) = (None, None);
    unsafe {
        gpu.dev.CreateTexture2D(&desc, Some(&init), Some(&mut tex))?;
        gpu.dev.CreateShaderResourceView(tex.as_ref().unwrap(), None, Some(&mut srv))?;
    }
    Ok(srv.unwrap())
}

pub fn viewport(w: u32, h: u32) -> D3D11_VIEWPORT {
    D3D11_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: w as f32, Height: h as f32, MinDepth: 0.0, MaxDepth: 1.0 }
}
