//! Draws a [`World`] into a [`Target`]: shadow map, opaque voxels, petals, sky, then the
//! transparent layers, and finally the post pass into a window's back buffer.

use crate::gfx::{viewport, Gpu, ShadowMap, Target};
use crate::scene::Scene;
use crate::world::{BoardGpu, Draw, ObjectCB, PetalGpu, ViewCB, World};
use windows::core::{Result, PCSTR};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

/// Bytecode compiled by build.rs.
macro_rules! shader {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/", $name, ".cso"))
    };
}

const MAX_BOARDS: u32 = 1024;

pub struct Renderer {
    vb: ID3D11Buffer,
    ib: ID3D11Buffer,
    layout: ID3D11InputLayout,
    vs_voxel: ID3D11VertexShader,
    vs_shadow: ID3D11VertexShader,
    vs_petal: ID3D11VertexShader,
    vs_full: ID3D11VertexShader,
    vs_surface: ID3D11VertexShader,
    vs_board: ID3D11VertexShader,
    ps_voxel: ID3D11PixelShader,
    ps_petal: ID3D11PixelShader,
    ps_sky: ID3D11PixelShader,
    ps_water: ID3D11PixelShader,
    ps_fall: ID3D11PixelShader,
    ps_point: ID3D11PixelShader,
    ps_glow: ID3D11PixelShader,
    cb_frame: ID3D11Buffer,
    cb_view: ID3D11Buffer,
    cb_object: ID3D11Buffer,
    petal_buf: ID3D11Buffer,
    petal_srv: ID3D11ShaderResourceView,
    board_buf: ID3D11Buffer,
    board_srv: ID3D11ShaderResourceView,
    rs_back: ID3D11RasterizerState,
    rs_none: ID3D11RasterizerState,
    rs_front: ID3D11RasterizerState,
    ds_write: ID3D11DepthStencilState,
    ds_read: ID3D11DepthStencilState,
    bs_alpha: ID3D11BlendState,
    bs_add: ID3D11BlendState,
    smp_shadow: ID3D11SamplerState,
    smp_linear: ID3D11SamplerState,
    shadow: Option<ShadowMap>,
    shadow_size: u32,
    water_mesh: crate::scene::Mesh,
    fall_mesh: crate::scene::Mesh,
}

impl Renderer {
    pub fn new(gpu: &Gpu, scene: &Scene, shadow_size: u32) -> Result<Renderer> {
        let voxel_code = shader!("VSVoxel");
        let elements = [
            D3D11_INPUT_ELEMENT_DESC { SemanticName: PCSTR(b"POSITION\0".as_ptr()), Format: DXGI_FORMAT_R32G32B32_FLOAT, AlignedByteOffset: 0, ..Default::default() },
            D3D11_INPUT_ELEMENT_DESC { SemanticName: PCSTR(b"NORMAL\0".as_ptr()), Format: DXGI_FORMAT_R8G8B8A8_SNORM, AlignedByteOffset: 12, ..Default::default() },
            D3D11_INPUT_ELEMENT_DESC { SemanticName: PCSTR(b"COLOR\0".as_ptr()), Format: DXGI_FORMAT_R8G8B8A8_UNORM, AlignedByteOffset: 16, ..Default::default() },
        ];
        let mut layout = None;
        unsafe { gpu.dev.CreateInputLayout(&elements, voxel_code, Some(&mut layout))? };
        let m = &scene.manifest;
        let (petal_buf, petal_srv) = gpu.structured::<PetalGpu>(m.petals.max.max(1))?;
        let (board_buf, board_srv) = gpu.structured::<BoardGpu>(MAX_BOARDS)?;
        Ok(Renderer {
            vb: gpu.buffer(scene.vertices, D3D11_BIND_VERTEX_BUFFER)?,
            ib: gpu.buffer(scene.indices, D3D11_BIND_INDEX_BUFFER)?,
            layout: layout.unwrap(),
            vs_voxel: gpu.vertex_shader(voxel_code)?,
            vs_shadow: gpu.vertex_shader(shader!("VSShadow"))?,
            vs_petal: gpu.vertex_shader(shader!("VSPetal"))?,
            vs_full: gpu.vertex_shader(shader!("VSFull"))?,
            vs_surface: gpu.vertex_shader(shader!("VSSurface"))?,
            vs_board: gpu.vertex_shader(shader!("VSBoard"))?,
            ps_voxel: gpu.pixel_shader(shader!("PSVoxel"))?,
            ps_petal: gpu.pixel_shader(shader!("PSPetal"))?,
            ps_sky: gpu.pixel_shader(shader!("PSSky"))?,
            ps_water: gpu.pixel_shader(shader!("PSWater"))?,
            ps_fall: gpu.pixel_shader(shader!("PSFall"))?,
            ps_point: gpu.pixel_shader(shader!("PSPoint"))?,
            ps_glow: gpu.pixel_shader(shader!("PSGlow"))?,
            cb_frame: gpu.constants::<crate::world::FrameCB>()?,
            cb_view: gpu.constants::<ViewCB>()?,
            cb_object: gpu.constants::<ObjectCB>()?,
            petal_buf,
            petal_srv,
            board_buf,
            board_srv,
            rs_back: gpu.raster(D3D11_CULL_BACK)?,
            rs_none: gpu.raster(D3D11_CULL_NONE)?,
            rs_front: gpu.raster(D3D11_CULL_FRONT)?,
            ds_write: gpu.depth(true, true)?,
            ds_read: gpu.depth(true, false)?,
            bs_alpha: gpu.blend(D3D11_BLEND_SRC_ALPHA, D3D11_BLEND_INV_SRC_ALPHA)?,
            bs_add: gpu.blend(D3D11_BLEND_SRC_ALPHA, D3D11_BLEND_ONE)?,
            smp_shadow: gpu.sampler(D3D11_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT, D3D11_COMPARISON_LESS_EQUAL)?,
            smp_linear: gpu.sampler(D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_COMPARISON_NEVER)?,
            shadow: None,
            shadow_size,
            water_mesh: m.meshes[m.water.mesh],
            fall_mesh: m.meshes[m.waterfall.mesh],
        })
    }

    /// Frees the shadow map (it is rebuilt on the next frame).
    pub fn release(&mut self) {
        self.shadow = None;
    }

    fn object(&self, gpu: &Gpu, d: &Draw, fog: bool, offset: u32) {
        gpu.upload(&self.cb_object, &[ObjectCB {
            world: d.world.to_cols_array_2d(),
            emissive: [d.emissive.x, d.emissive.y, d.emissive.z, if d.receive { 1.0 } else { 0.0 }],
            flags: [if fog { 1.0 } else { 0.0 }, offset as f32, 0.0, 0.0],
        }]);
    }

    fn bind_common(&self, gpu: &Gpu) {
        let c = &gpu.ctx;
        let cbs = [Some(self.cb_frame.clone()), Some(self.cb_view.clone()), Some(self.cb_object.clone())];
        unsafe {
            c.VSSetConstantBuffers(0, Some(&cbs));
            c.PSSetConstantBuffers(0, Some(&cbs));
            c.IASetInputLayout(&self.layout);
            let stride = crate::scene::VERTEX_SIZE as u32;
            c.IASetVertexBuffers(0, 1, Some(&Some(self.vb.clone())), Some(&stride), Some(&0));
            c.IASetIndexBuffer(&self.ib, DXGI_FORMAT_R32_UINT, 0);
            c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            c.PSSetSamplers(0, Some(&[Some(self.smp_shadow.clone()), Some(self.smp_linear.clone())]));
        }
    }

    /// Uploads per-frame data and refreshes the shadow map if the light moved.
    pub fn prepare(&mut self, gpu: &Gpu, world: &mut World) -> Result<()> {
        gpu.upload(&self.cb_frame, &[world.frame]);
        gpu.upload(&self.petal_buf, &world.petal_gpu);
        let n = world.boards.len().min(MAX_BOARDS as usize);
        gpu.upload(&self.board_buf, &world.boards[..n]);
        if self.shadow.is_none() {
            self.shadow = Some(ShadowMap::new(gpu, self.shadow_size)?);
            world.shadow_dirty = true;
        }
        if !world.shadow_dirty {
            return Ok(());
        }
        world.shadow_dirty = false;
        let sm = self.shadow.as_ref().unwrap();
        let c = &gpu.ctx;
        self.bind_common(gpu);
        unsafe {
            c.PSSetShaderResources(0, Some(&[None]));
            c.OMSetRenderTargets(None, &sm.dsv);
            c.ClearDepthStencilView(&sm.dsv, D3D11_CLEAR_DEPTH.0 as u32, 1.0, 0);
            c.RSSetViewports(Some(&[viewport(sm.size, sm.size)]));
            c.RSSetState(&self.rs_front);
            c.OMSetDepthStencilState(&self.ds_write, 0);
            c.OMSetBlendState(None, None, 0xffffffff);
            c.VSSetShader(&self.vs_shadow, None);
            c.PSSetShader(None, None);
            for d in world.draws.iter().filter(|d| d.cast) {
                self.object(gpu, d, false, 0);
                c.DrawIndexed(d.mesh.index_count, d.mesh.first_index, d.mesh.first_vertex as i32);
            }
        }
        Ok(())
    }

    pub fn render(&self, gpu: &Gpu, world: &World, view: &ViewCB, t: &Target) {
        let c = &gpu.ctx;
        gpu.upload(&self.cb_view, &[*view]);
        self.bind_common(gpu);
        unsafe {
            c.OMSetRenderTargets(Some(&[Some(t.rtv.clone())]), &t.dsv);
            c.ClearDepthStencilView(&t.dsv, D3D11_CLEAR_DEPTH.0 as u32, 1.0, 0);
            c.RSSetViewports(Some(&[viewport(t.w, t.h)]));
            let shadow_srv = self.shadow.as_ref().map(|s| s.srv.clone());
            c.PSSetShaderResources(0, Some(&[shadow_srv]));
            c.OMSetBlendState(None, None, 0xffffffff);

            // opaque voxel objects
            c.RSSetState(&self.rs_back);
            c.OMSetDepthStencilState(&self.ds_write, 0);
            c.VSSetShader(&self.vs_voxel, None);
            c.PSSetShader(&self.ps_voxel, None);
            for d in &world.draws {
                self.object(gpu, d, true, 0);
                c.DrawIndexed(d.mesh.index_count, d.mesh.first_index, d.mesh.first_vertex as i32);
            }

            // petals
            let np = world.petal_gpu.len() as u32;
            if np > 0 {
                c.RSSetState(&self.rs_none);
                c.VSSetShaderResources(1, Some(&[Some(self.petal_srv.clone())]));
                c.VSSetShader(&self.vs_petal, None);
                c.PSSetShader(&self.ps_petal, None);
                c.DrawInstanced(6, np, 0, 0);
            }

            // sky last among the opaques: only fills pixels nothing else covered
            c.RSSetState(&self.rs_none);
            c.OMSetDepthStencilState(&self.ds_read, 0);
            c.VSSetShader(&self.vs_full, None);
            c.PSSetShader(&self.ps_sky, None);
            c.Draw(3, 0);

            // pond, waterfall
            let ident = Draw { mesh: self.water_mesh, world: glam::Mat4::IDENTITY, emissive: glam::Vec3::ZERO, receive: false, cast: false };
            self.object(gpu, &ident, false, 0);
            c.OMSetBlendState(&self.bs_alpha, None, 0xffffffff);
            c.VSSetShader(&self.vs_surface, None);
            c.PSSetShader(&self.ps_water, None);
            c.DrawIndexed(self.water_mesh.index_count, self.water_mesh.first_index, self.water_mesh.first_vertex as i32);
            c.PSSetShader(&self.ps_fall, None);
            c.DrawIndexed(self.fall_mesh.index_count, self.fall_mesh.first_index, self.fall_mesh.first_vertex as i32);

            // spray (alpha), fireflies and glow halos (additive)
            c.VSSetShaderResources(1, Some(&[Some(self.board_srv.clone())]));
            c.VSSetShader(&self.vs_board, None);
            let passes = [(&self.ps_point, &self.bs_alpha), (&self.ps_point, &self.bs_add), (&self.ps_glow, &self.bs_add)];
            for (i, (ps, bs)) in passes.iter().enumerate() {
                let (start, count) = world.board_ranges[i];
                let count = count.min(MAX_BOARDS.saturating_sub(start));
                if count == 0 {
                    continue;
                }
                self.object(gpu, &ident, false, start);
                c.OMSetBlendState(*bs, None, 0xffffffff);
                c.PSSetShader(*ps, None);
                c.DrawInstanced(6, count, 0, 0);
            }
            c.PSSetShaderResources(0, Some(&[None]));
        }
    }

}

// ---------------------------------------------------------------- post pass

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PostCB {
    info: [f32; 4],
    uv: [f32; 4],
}

/// How a source fills a screen, see [`crate::config::Fit`].
pub fn fit_uv(fit: crate::config::Fit, sw: u32, sh: u32, ow: u32, oh: u32) -> [f32; 4] {
    use crate::config::Fit;
    let (src, out) = (sw as f32 / sh as f32, ow as f32 / oh as f32);
    let (sx, sy) = match fit {
        Fit::Stretch => (1.0, 1.0),
        Fit::Cover if src > out => (out / src, 1.0),
        Fit::Cover => (1.0, src / out),
        Fit::Contain if src > out => (1.0, src / out),
        Fit::Contain => (out / src, 1.0),
    };
    [sx, sy, (1.0 - sx) / 2.0, (1.0 - sy) / 2.0]
}

/// The last step of every frame: puts a rendered scene view, a picture or a video frame onto
/// a window's back buffer (scaled, optionally with FXAA and vignette).
pub struct Post {
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    cb: ID3D11Buffer,
    smp: ID3D11SamplerState,
    rs: ID3D11RasterizerState,
    ds: ID3D11DepthStencilState,
}

impl Post {
    pub fn new(gpu: &Gpu) -> Result<Post> {
        Ok(Post {
            vs: gpu.vertex_shader(shader!("VSFull"))?,
            ps: gpu.pixel_shader(shader!("PSPost"))?,
            cb: gpu.constants::<PostCB>()?,
            smp: gpu.sampler(D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_COMPARISON_NEVER)?,
            rs: gpu.raster(D3D11_CULL_NONE)?,
            ds: gpu.depth(false, false)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, gpu: &Gpu, src: &ID3D11ShaderResourceView, (sw, sh): (u32, u32), rtv: &ID3D11RenderTargetView, (w, h): (u32, u32), uv: [f32; 4], fxaa: bool, vignette: f32) {
        let c = &gpu.ctx;
        gpu.upload(&self.cb, &[PostCB { info: [1.0 / sw as f32, 1.0 / sh as f32, if fxaa { 1.0 } else { 0.0 }, vignette], uv }]);
        unsafe {
            c.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            c.RSSetViewports(Some(&[viewport(w, h)]));
            c.RSSetState(&self.rs);
            c.OMSetDepthStencilState(&self.ds, 0);
            c.OMSetBlendState(None, None, 0xffffffff);
            c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            c.IASetInputLayout(None);
            c.VSSetShader(&self.vs, None);
            c.PSSetShader(&self.ps, None);
            c.PSSetConstantBuffers(3, Some(&[Some(self.cb.clone())]));
            c.PSSetSamplers(1, Some(&[Some(self.smp.clone())]));
            c.PSSetShaderResources(2, Some(&[Some(src.clone())]));
            c.Draw(3, 0);
            c.PSSetShaderResources(2, Some(&[None]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Fit;

    #[test]
    fn same_shape_needs_no_crop() {
        assert_eq!(fit_uv(Fit::Cover, 1920, 1080, 2560, 1440), [1.0, 1.0, 0.0, 0.0]);
        assert_eq!(fit_uv(Fit::Contain, 1920, 1080, 2560, 1440), [1.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn cover_crops_and_contain_adds_bars() {
        // 2:1 picture on a 1:1 screen
        let c = fit_uv(Fit::Cover, 200, 100, 100, 100);
        assert_eq!((c[0], c[1]), (0.5, 1.0));
        assert_eq!(c[2], 0.25);
        let c = fit_uv(Fit::Contain, 200, 100, 100, 100);
        assert_eq!((c[0], c[1]), (1.0, 2.0));
        assert_eq!(c[3], -0.5);
        assert_eq!(fit_uv(Fit::Stretch, 200, 100, 100, 100), [1.0, 1.0, 0.0, 0.0]);
    }
}
