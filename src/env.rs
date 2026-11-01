//! Time of day: sky keyframes and the paths of sun and moon.

use crate::scene::{Sky, SkyKey};
use glam::Vec3;

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> Vec3 {
    Vec3::from(a).lerp(Vec3::from(b), t)
}

pub struct Env {
    pub zen: Vec3,
    pub hor: Vec3,
    pub gnd: Vec3,
    pub key_c: Vec3,
    pub hemi_s: Vec3,
    pub hemi_g: Vec3,
    pub key: f32,
    pub hemi_i: f32,
    pub glow: f32,
    pub stars: f32,
    pub exp: f32,
}

/// Smoothstep-interpolated look for `hour` (keys wrap around midnight).
pub fn sample(sky: &Sky, hour: f32) -> Env {
    let hour = hour.rem_euclid(24.0);
    let keys = &sky.keys;
    let wrap = SkyKey { h: 24.0, ..keys[0] };
    let at = |i: usize| if i < keys.len() { &keys[i] } else { &wrap };
    let mut k = 0;
    while k + 1 < keys.len() && at(k + 1).h <= hour {
        k += 1;
    }
    let (a, b) = (at(k), at(k + 1));
    let mut t = ((hour - a.h) / (b.h - a.h)).clamp(0.0, 1.0);
    t = t * t * (3.0 - 2.0 * t);
    let f = |x: f32, y: f32| x + (y - x) * t;
    Env {
        zen: lerp3(a.zen, b.zen, t),
        hor: lerp3(a.hor, b.hor, t),
        gnd: lerp3(a.gnd, b.gnd, t),
        key_c: lerp3(a.key_c, b.key_c, t),
        hemi_s: lerp3(a.hemi_s, b.hemi_s, t),
        hemi_g: lerp3(a.hemi_g, b.hemi_g, t),
        key: f(a.key, b.key),
        hemi_i: f(a.hemi_i, b.hemi_i),
        glow: f(a.glow, b.glow),
        stars: f(a.stars, b.stars),
        exp: f(a.exp, b.exp),
    }
}

/// Angle of the sun on its arc: 0 at sunrise, PI at sunset, the night half takes the rest.
fn sun_angle(sky: &Sky, hour: f32) -> f32 {
    let (rise, set) = (sky.sun.rise, sky.sun.set);
    let h = hour.rem_euclid(24.0);
    if h >= rise && h <= set {
        (h - rise) / (set - rise) * std::f32::consts::PI
    } else {
        let night = 24.0 - (set - rise);
        std::f32::consts::PI + ((h - set).rem_euclid(24.0) / night) * std::f32::consts::PI
    }
}

fn body_dir(angle: f32, tilt: f32, yaw: f32) -> Vec3 {
    let v = Vec3::new(angle.cos(), angle.sin() * tilt.sin(), angle.sin() * tilt.cos());
    // rotate around +Y (three.js applyAxisAngle)
    let (s, c) = yaw.sin_cos();
    Vec3::new(c * v.x + s * v.z, v.y, -s * v.x + c * v.z).normalize()
}

pub fn sun_dir(sky: &Sky, hour: f32) -> Vec3 {
    body_dir(sun_angle(sky, hour), sky.sun.tilt, sky.sun.yaw)
}

pub fn moon_dir(sky: &Sky, hour: f32) -> Vec3 {
    body_dir(sun_angle(sky, hour) + std::f32::consts::PI, sky.moon.tilt, sky.moon.yaw)
}

pub fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sky() -> Sky {
        crate::scene::parse(crate::library::PAGODA_SCENE).unwrap().manifest.sky
    }

    #[test]
    fn sun_is_up_by_day_and_down_at_night() {
        let s = sky();
        assert!(sun_dir(&s, 12.5).y > 0.5);
        assert!(sun_dir(&s, 0.5).y < 0.0);
        assert!(moon_dir(&s, 0.5).y > 0.0);
    }

    #[test]
    fn every_hour_samples_without_nan() {
        let s = sky();
        for i in 0..=480 {
            let h = i as f32 * 0.05 - 0.5; // also before 0 and past 24
            let e = sample(&s, h);
            assert!(e.zen.is_finite() && e.exp.is_finite() && e.key.is_finite(), "hour {h}");
        }
    }
}
