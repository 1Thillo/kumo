//! Simulation: clock, camera, wind, petals, spray, fireflies and moving objects.
//! Everything here is plain CPU work (a few thousand floats per frame).

use crate::config::{Settings, TimeMode};
use crate::env;
use crate::scene::{Manifest, Mesh};
use glam::{Mat4, Vec3};

// ---------------------------------------------------------------- GPU-facing layouts (match shaders.hlsl)

pub type M4 = [[f32; 4]; 4];

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FrameCB {
    pub shadow_vp: M4,
    pub key_dir: [f32; 4],
    pub key_col: [f32; 4],
    pub hemi_sky: [f32; 4],
    pub hemi_ground: [f32; 4],
    pub fog_col: [f32; 4],
    pub fog_range: [f32; 4],
    pub shadow_cfg: [f32; 4],
    pub pl_pos: [[f32; 4]; 4],
    pub pl_col: [[f32; 4]; 4],
    pub sky_zen: [f32; 4],
    pub sky_hor: [f32; 4],
    pub sky_gnd: [f32; 4],
    pub sun_col: [f32; 4],
    pub sun_dir: [f32; 4],
    pub moon_dir: [f32; 4],
    pub water_sky: [f32; 4],
    pub water_deep: [f32; 4],
    pub water_shallow: [f32; 4],
    pub water_sun_dir: [f32; 4],
    pub water_sun_col: [f32; 4],
    pub water_glow: [f32; 4],
    pub fall_col: [f32; 4],
    pub petal_emissive: [f32; 4],
    pub petal_col: [[f32; 4]; 5],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ViewCB {
    pub view_proj: M4,
    pub inv_view_proj: M4,
    pub cam_pos: [f32; 4],
    pub cam_right: [f32; 4],
    pub cam_up: [f32; 4],
    pub view_info: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ObjectCB {
    pub world: M4,
    pub emissive: [f32; 4],
    pub flags: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PetalGpu {
    pub pos: [f32; 3],
    pub scale: f32,
    pub rot: [f32; 3],
    pub tint: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BoardGpu {
    pub pos: [f32; 3],
    pub size: f32,
    pub col: [f32; 4],
}

fn v4(v: Vec3, w: f32) -> [f32; 4] {
    [v.x, v.y, v.z, w]
}

// ---------------------------------------------------------------- simulation state

/// mulberry32, the same generator the scene authoring code uses.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(seed)
    }
    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        ((t ^ (t >> 14)) as f64 / 4294967296.0) as f32
    }
}

#[derive(Default, Clone, Copy)]
struct Petal {
    p: Vec3,
    r: Vec3,
    spin: Vec3,
    phase: f32,
    speed: f32,
    landed: f32,
}

#[derive(Default, Clone, Copy)]
struct Drop {
    d: f32,
    off: [f32; 2],
    v: f32,
    size: f32,
}

pub struct Draw {
    pub mesh: Mesh,
    pub world: Mat4,
    pub emissive: Vec3,
    pub receive: bool,
    pub cast: bool,
}

/// Animations run on a clock that wraps once a day of visible time: f32 seconds would get
/// coarse after a few days, and the shader only sees f32.
const CLOCK_WRAP: f64 = 86400.0;

pub struct World {
    pub m: Manifest,
    time: f64,
    pub hour: f32,
    virtual_hour: f32,
    cam_angle: f32,
    mouse: [f32; 2],
    rng: Rng,
    petals: Vec<Petal>,
    drops: Vec<Drop>,
    orbit_angles: Vec<f32>,
    last_shadow_dir: Vec3,
    last_shadow_time: f64,
    pub shadow_dirty: bool,
    pub frame: FrameCB,
    pub draws: Vec<Draw>,
    pub petal_gpu: Vec<PetalGpu>,
    pub boards: Vec<BoardGpu>,
    /// instance ranges inside `boards`: spray, fireflies, glows
    pub board_ranges: [(u32, u32); 3],
}

impl World {
    pub fn new(m: Manifest) -> World {
        let mut w = World {
            time: 0.0,
            hour: 12.0,
            virtual_hour: local_hour(),
            cam_angle: m.camera.angle,
            mouse: [0.0; 2],
            rng: Rng::new(99),
            petals: vec![Petal::default(); m.petals.max as usize],
            drops: vec![Drop::default(); 160],
            orbit_angles: m.objects.iter().map(|o| o.orbit.as_ref().map_or(0.0, |o| o.angle)).collect(),
            last_shadow_dir: Vec3::NEG_Y,
            last_shadow_time: -99.0,
            shadow_dirty: true,
            frame: FrameCB::default(),
            draws: Vec::new(),
            petal_gpu: Vec::new(),
            boards: Vec::new(),
            board_ranges: [(0, 0); 3],
            m,
        };
        for i in 0..w.petals.len() {
            w.respawn_petal(i, true);
        }
        for i in 0..w.drops.len() {
            w.reset_drop(i);
            w.drops[i].d = w.rng.next() * 40.0;
            w.drops[i].size = 0.5 + w.rng.next() * 1.4;
        }
        w
    }

    pub fn reset_clock(&mut self) {
        self.virtual_hour = local_hour();
    }

    fn respawn_petal(&mut self, i: usize, initial: bool) {
        let spawns = &self.m.petals.spawns;
        let s = spawns[((self.rng.next() * spawns.len() as f32) as usize).min(spawns.len() - 1)];
        let r = &mut self.rng;
        let mut p = Vec3::new(s[0] + (r.next() - 0.5), s[1] - if initial { r.next() * 10.0 } else { 0.0 }, s[2] + (r.next() - 0.5));
        if initial {
            p.x += (r.next() - 0.5) * 14.0;
            p.z += (r.next() - 0.5) * 14.0;
        }
        let rot = Vec3::new(r.next() * 6.3, r.next() * 6.3, r.next() * 6.3);
        let spin = Vec3::new((r.next() - 0.5) * 5.0, (r.next() - 0.5) * 3.0, (r.next() - 0.5) * 5.0);
        let phase = r.next() * 100.0;
        let speed = 0.45 + r.next() * 0.45;
        self.petals[i] = Petal { p, r: rot, spin, phase, speed, landed: 0.0 };
    }

    fn reset_drop(&mut self, i: usize) {
        let r = &mut self.rng;
        let d = &mut self.drops[i];
        d.d = 0.0;
        d.off = [(r.next() - 0.5) * 1.2, r.next()];
        d.v = 7.0 + r.next() * 6.0;
    }

    fn ground_at(&self, x: f32, z: f32) -> f32 {
        let g = &self.m.ground;
        let (ix, iz) = (x.floor() as i32 + g.half, z.floor() as i32 + g.half);
        let n = g.half * 2;
        if ix < 0 || iz < 0 || ix >= n || iz >= n {
            return -999.0;
        }
        g.heights[(iz * n + ix) as usize]
    }

    fn current_hour(&mut self, dt: f32, s: &Settings) -> f32 {
        match s.time {
            TimeMode::Real => local_hour(),
            TimeMode::Fixed => s.hour,
            TimeMode::Lapse => {
                self.virtual_hour = (self.virtual_hour + dt * s.lapse / 3600.0).rem_euclid(24.0);
                self.virtual_hour
            }
        }
    }

    pub fn set_mouse(&mut self, x: f32, y: f32, dt: f32) {
        let k = (dt * 1.5).min(1.0);
        self.mouse[0] += (x - self.mouse[0]) * k;
        self.mouse[1] += (y - self.mouse[1]) * k;
    }

    pub fn update(&mut self, dt: f32, s: &Settings) {
        self.time += dt as f64;
        let t = (self.time % CLOCK_WRAP) as f32;
        self.hour = self.current_hour(dt, s);
        self.cam_angle += dt * self.m.camera.orbit_speed * s.rotate;
        let (light_level, glow) = self.apply_environment();

        // wind
        let wa = t * 0.05 + (t * 0.13).sin() * 0.8;
        let ws = 0.55 + 0.35 * (t * 0.21).sin() + 0.25 * (t * 0.57).sin();
        let wind = Vec3::new(wa.cos() * ws, 0.0, wa.sin() * ws * 0.6 + 0.25);
        self.frame.key_col[3] = 0.7 + ws * 0.5;

        self.update_petals(dt, t, wind, s.petals);
        self.update_objects(dt, t);
        self.update_boards(dt, t, light_level, glow);
    }

    fn apply_environment(&mut self) -> (f32, f32) {
        let m = &self.m;
        let e = env::sample(&m.sky, self.hour);
        let sun = env::sun_dir(&m.sky, self.hour);
        let moon = env::moon_dir(&m.sky, self.hour);
        let (dir, vis) = if sun.y > -0.03 { (sun, env::smooth(-0.03, 0.12, sun.y)) } else { (moon, env::smooth(-0.03, 0.15, moon.y)) };
        let mut key_dir = dir;
        if key_dir.y < 0.24 {
            key_dir.y = 0.24;
            key_dir = key_dir.normalize();
        }
        let key_i = e.key * vis;
        let light_level = (0.32 + e.hemi_i * 0.35 + key_i * 0.17).min(1.25);
        let lit = 0.35 + 0.65 * light_level.min(1.0);
        let t = (self.time % CLOCK_WRAP) as f32;

        // shadow camera follows the key light
        let sh = &m.shadow;
        let target = Vec3::from(sh.target);
        let light_view = Mat4::look_at_rh(target + key_dir * sh.distance, target, Vec3::Y);
        let light_proj = Mat4::orthographic_rh(-sh.extent, sh.extent, -sh.extent, sh.extent, sh.near, sh.far);
        if key_dir.angle_between(self.last_shadow_dir) > 0.003 || self.time - self.last_shadow_time > 30.0 {
            self.shadow_dirty = true;
            self.last_shadow_dir = key_dir;
            self.last_shadow_time = self.time;
        }

        let f = &mut self.frame;
        f.shadow_vp = (light_proj * light_view).to_cols_array_2d();
        f.key_dir = v4(key_dir, t);
        f.key_col = v4(e.key_c * key_i, f.key_col[3]);
        f.hemi_sky = v4(e.hemi_s * e.hemi_i, e.exp);
        f.hemi_ground = v4(e.hemi_g * e.hemi_i, e.glow * 2.4);
        f.fog_col = v4(e.gnd.lerp(e.hor, 0.35), 1.0 - e.glow * 0.55);
        f.fog_range = [m.fog.near, m.fog.far, 1.0 / sh.size as f32, sh.normal_bias];
        f.shadow_cfg = [sh.bias, 1.0, if e.glow > 0.001 { 1.0 } else { 0.0 }, 0.0];
        for (i, l) in m.lights.iter().take(4).enumerate() {
            f.pl_pos[i] = v4(Vec3::from(l.pos), l.distance);
            f.pl_col[i] = v4(Vec3::from(l.color) * l.intensity * e.glow, l.decay);
        }
        f.sky_zen = v4(e.zen, e.stars);
        f.sky_hor = v4(e.hor, env::smooth(-0.12, 0.04, sun.y));
        f.sky_gnd = v4(e.gnd, env::smooth(-0.04, 0.06, moon.y) * (0.3 + e.stars * 0.7));
        f.sun_col = v4(e.key_c, 0.0);
        f.sun_dir = v4(sun, 0.0);
        f.moon_dir = v4(moon, 0.0);
        f.water_sky = v4(e.hor.lerp(e.zen, 0.3), 1.6);
        f.water_deep = v4(Vec3::from(m.water.deep) * lit, 1.0);
        f.water_shallow = v4(Vec3::from(m.water.shallow) * lit, e.glow);
        f.water_sun_dir = v4(dir, light_level.min(1.1));
        f.water_sun_col = v4(e.key_c * vis * 1.4, 0.0);
        f.water_glow = v4(Vec3::from(m.water.glow), 0.0);
        f.fall_col = v4(Vec3::from(m.waterfall.color), 0.0);
        f.petal_emissive = v4(Vec3::from(m.petals.emissive), 0.0);
        for (i, c) in m.petals.colors.iter().take(5).enumerate() {
            f.petal_col[i] = v4(Vec3::from(*c), 0.0);
        }
        (light_level, e.glow)
    }

    fn update_petals(&mut self, dt: f32, t: f32, wind: Vec3, amount: f32) {
        let n = ((self.m.petals.count as f32 * amount).round() as usize).min(self.petals.len());
        self.petal_gpu.clear();
        for i in 0..n {
            let mut scale = 1.0;
            let mut q = self.petals[i];
            if q.landed > 0.0 {
                q.landed -= dt;
                scale = (q.landed / 1.2).min(1.0);
                self.petals[i] = q;
                if q.landed <= 0.0 {
                    self.respawn_petal(i, false);
                    q = self.petals[i];
                    scale = 0.0;
                }
            } else {
                let ph = q.phase;
                q.p.x += (wind.x * 1.1 + (t * 1.7 + ph * 3.0).sin() * 0.45) * dt;
                q.p.z += (wind.z * 1.1 + (t * 1.3 + ph * 2.0).cos() * 0.45) * dt;
                q.p.y -= (q.speed + (t * 2.6 + ph).sin() * 0.25) * dt;
                q.r += q.spin * dt;
                let gy = self.ground_at(q.p.x, q.p.z);
                self.petals[i] = q;
                if gy > -100.0 && q.p.y <= gy + 0.04 && q.p.y > gy - 1.5 {
                    q.p.y = gy + 0.04;
                    q.r.x = -std::f32::consts::FRAC_PI_2;
                    q.r.y = 0.0;
                    q.landed = 3.0 + self.rng.next() * 6.0;
                    self.petals[i] = q;
                } else if q.p.y < -70.0 {
                    self.respawn_petal(i, false);
                    q = self.petals[i];
                }
            }
            if scale > 0.0 {
                self.petal_gpu.push(PetalGpu { pos: q.p.into(), scale, rot: q.r.into(), tint: (i % 5) as f32 });
            }
        }
    }

    fn update_objects(&mut self, dt: f32, t: f32) {
        self.draws.clear();
        let m = &self.m;
        for (i, o) in m.objects.iter().enumerate() {
            let mut pos = Vec3::from(o.pos.unwrap_or([0.0; 3]));
            let mut rot = o.rot_y;
            if let Some(b) = &o.bob {
                pos.y += (t * b.speed + b.phase).sin() * b.amp;
                rot = (t * b.rot_speed + b.phase).sin() * b.rot_amp;
            }
            if let Some(orb) = &o.orbit {
                self.orbit_angles[i] += dt * orb.speed;
                let a = self.orbit_angles[i];
                pos = Vec3::new(a.cos() * orb.radius, orb.y + (t * 0.1 + orb.radius).sin() * orb.wobble, a.sin() * orb.radius);
            }
            let scale = Vec3::from(o.scale.unwrap_or([1.0; 3]));
            self.draws.push(Draw {
                mesh: m.meshes[o.mesh],
                world: Mat4::from_translation(pos) * Mat4::from_rotation_y(rot) * Mat4::from_scale(scale),
                emissive: Vec3::from(o.emissive),
                receive: o.receive,
                cast: o.cast,
            });
        }
        for f in &m.swimmers {
            let th = f.th + f.w * t + 0.35 * (t * 0.21 + f.ph).sin();
            let x = f.center[0] + f.ox + th.cos() * f.rx;
            let z = f.center[1] + f.oz + th.sin() * f.rz;
            let s = f.w.signum();
            let (vx, vz) = (-th.sin() * f.rx * s, th.cos() * f.rz * s);
            let rot = -vz.atan2(vx) + (t * 5.0 + f.ph).sin() * 0.18;
            self.draws.push(Draw {
                mesh: m.meshes[f.mesh],
                world: Mat4::from_translation(Vec3::new(x, f.y, z)) * Mat4::from_rotation_y(rot) * Mat4::from_scale(Vec3::from(f.scale)),
                emissive: Vec3::ZERO,
                receive: false,
                cast: false,
            });
        }
    }

    fn update_boards(&mut self, dt: f32, t: f32, light: f32, glow: f32) {
        self.boards.clear();
        // waterfall spray
        let wf = (self.m.waterfall.x, self.m.waterfall.y, self.m.waterfall.z0, self.m.waterfall.z1);
        let spray = Vec3::from(self.m.waterfall.spray);
        let fall_light = light.min(1.1);
        for i in 0..self.drops.len() {
            self.drops[i].d += self.drops[i].v * dt;
            if self.drops[i].d > 42.0 {
                self.reset_drop(i);
            }
            let q = self.drops[i];
            let d = q.d;
            let out = 1.7 * (1.0 - (-d * 0.22).exp()) + d * 0.02 + q.off[0] * (0.3 + d * 0.06);
            let pos = [wf.0 + out + 0.3, wf.1 - d, wf.2 - d * 0.03 + q.off[1] * (wf.3 - wf.2 + d * 0.06)];
            let a = (d * 0.5).min(1.0) * (1.0 - d / 42.0).max(0.0) * 0.5 * fall_light;
            self.boards.push(BoardGpu { pos, size: q.size, col: v4(spray, a) });
        }
        let n0 = self.boards.len() as u32;
        // fireflies (night only)
        let vis = env::smooth(0.45, 0.9, glow);
        if vis > 0.01 {
            let col = Vec3::from(self.m.fireflies.color);
            for it in &self.m.fireflies.items {
                let ph = it[3];
                let pos = [
                    it[0] + (t * 0.31 + ph).sin() * 1.6 + (t * 0.9 + ph * 2.0).sin() * 0.4,
                    it[1] + (t * 0.47 + ph * 1.3).sin() * 0.8,
                    it[2] + (t * 0.27 + ph).cos() * 1.6,
                ];
                let blink = (t * (0.9 + ph.rem_euclid(1.0)) + ph).sin().max(0.0);
                self.boards.push(BoardGpu { pos, size: it[4], col: v4(col, vis * blink.powi(3) * 1.4) });
            }
        }
        let n1 = self.boards.len() as u32;
        // lantern and window halos
        if glow > 0.001 {
            for g in &self.m.glows {
                let pos = match (g.follow, g.local) {
                    (Some(o), Some(l)) => self.draws[o].world.transform_point3(Vec3::from(l)).into(),
                    _ => g.pos,
                };
                self.boards.push(BoardGpu { pos, size: g.size, col: v4(Vec3::from(g.color), glow * g.strength * 0.85) });
            }
        }
        let n2 = self.boards.len() as u32;
        self.board_ranges = [(0, n0), (n0, n1 - n0), (n1, n2 - n1)];
    }

    /// Camera for an output with the given aspect ratio.
    pub fn view(&self, aspect: f32, parallax: bool) -> ViewCB {
        let c = &self.m.camera;
        let t = (self.time % CLOCK_WRAP) as f32;
        let fov = c.fov.to_radians();
        let vf = fov / 2.0;
        let hf = (vf.tan() * aspect).atan();
        let fit = (c.fit[0] / vf.tan()).max(c.fit[1] / hf.tan());
        let (mx, my) = if parallax { (self.mouse[0], self.mouse[1]) } else { (0.0, 0.0) };
        let a = self.cam_angle + mx * 0.16;
        let e = c.elev + (t * 0.05).sin() * 0.03 - my * 0.05;
        let d = fit * (1.0 + (t * 0.07).sin() * 0.02);
        let target = Vec3::from(c.target);
        let eye = target + Vec3::new(a.sin() * e.cos() * d, e.sin() * d, a.cos() * e.cos() * d);
        let look = target + Vec3::new(0.0, (t * 0.09).sin() * 0.6, 0.0);
        let view = Mat4::look_at_rh(eye, look, Vec3::Y);
        let proj = Mat4::perspective_rh(fov, aspect, c.near, c.far);
        let vp = proj * view;
        ViewCB {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            cam_pos: v4(eye, 0.0),
            cam_right: v4(view.row(0).truncate(), 0.0),
            cam_up: v4(view.row(1).truncate(), 0.0),
            view_info: [0.0; 4],
        }
    }
}

#[cfg(windows)]
pub fn local_hour() -> f32 {
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    st.wHour as f32 + st.wMinute as f32 / 60.0 + (st.wSecond as f32 + st.wMilliseconds as f32 / 1000.0) / 3600.0
}

#[cfg(not(windows))]
pub fn local_hour() -> f32 {
    unsafe {
        let mut now: libc::timeval = std::mem::zeroed();
        libc::gettimeofday(&mut now, std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now.tv_sec, &mut tm);
        tm.tm_hour as f32 + tm.tm_min as f32 / 60.0 + (tm.tm_sec as f32 + now.tv_usec as f32 / 1e6) / 3600.0
    }
}
