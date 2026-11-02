// Kumo scene shaders in WGSL: a line-by-line port of src/shaders.hlsl.
//
// Lighting, tone mapping and fog deliberately follow three.js (MeshLambertMaterial,
// NeutralToneMapping, linear fog mixed in after the sRGB transfer) so a baked scene looks the
// same as in its browser preview. Colours arrive in linear space; every fragment shader writes
// sRGB-encoded values into a plain UNORM target, exactly like a WebGL canvas.

const PI = 3.14159265;

struct Frame {
  shadow_vp: mat4x4f,
  key_dir: vec4f,       // xyz: direction towards the key light, w: time (s)
  key_col: vec4f,       // rgb: key colour * intensity, w: wind strength
  hemi_sky: vec4f,      // rgb: hemisphere sky colour * intensity, w: exposure
  hemi_ground: vec4f,   // rgb, w: glow amount for emissive voxels
  fog_col: vec4f,       // rgb, w: diffuse factor for glowing voxels
  fog_range: vec4f,     // x near, y far, z 1/shadow map size, w normal bias
  shadow_cfg: vec4f,    // x depth bias, y shadows enabled, z point lights active
  pl_pos: array<vec4f, 4>, // xyz position, w cutoff distance
  pl_col: array<vec4f, 4>, // rgb colour * intensity, w decay
  sky_zen: vec4f,       // rgb, w star visibility
  sky_hor: vec4f,       // rgb, w sun visibility
  sky_gnd: vec4f,       // rgb, w moon visibility
  sun_col: vec4f,
  sun_dir: vec4f,
  moon_dir: vec4f,
  water_sky: vec4f,     // rgb, w specular strength
  water_deep: vec4f,    // rgb, w light level
  water_shallow: vec4f, // rgb, w glow
  water_sun_dir: vec4f, // xyz, w waterfall light
  water_sun_col: vec4f,
  water_glow: vec4f,
  fall_col: vec4f,
  petal_emissive: vec4f,
  petal_col: array<vec4f, 5>,
}

struct View {
  view_proj: mat4x4f,
  inv_view_proj: mat4x4f,
  cam_pos: vec4f,
  cam_right: vec4f,
  cam_up: vec4f,
  view_info: vec4f,
}

struct Object {
  world: mat4x4f,
  emissive: vec4f,      // rgb, w: receives shadows
  flags: vec4f,         // x: fog, y: first billboard
}

struct Petal { pos: vec3f, scale: f32, rot: vec3f, tint: f32 }
struct Board { pos: vec3f, size: f32, col: vec4f }

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<uniform> view: View;
@group(0) @binding(2) var<uniform> object: Object;

@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_smp: sampler_comparison;
@group(1) @binding(2) var<storage, read> petals: array<Petal>;
@group(1) @binding(3) var<storage, read> boards: array<Board>;

const QUAD = array<vec2f, 6>(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, 1.0));

fn quad(vid: u32) -> vec2f {
  var q = QUAD;
  return q[vid];
}

// ---------------------------------------------------------------- helpers

fn ss(a: f32, b: f32, x: f32) -> f32 {
  let t = saturate((x - a) / (b - a));
  return t * t * (3.0 - 2.0 * t);
}

fn to_linear(c: vec3f) -> vec3f {
  return select(pow((c + 0.055) / 1.055, vec3f(2.4)), c / 12.92, c <= vec3f(0.04045));
}

fn to_srgb(c_in: vec3f) -> vec3f {
  let c = max(c_in, vec3f(0.0));
  return select(pow(c, vec3f(0.41666)) * 1.055 - 0.055, c * 12.92, c <= vec3f(0.0031308));
}

fn neutral(color_in: vec3f) -> vec3f {
  let start = 0.8 - 0.04;
  let desat = 0.15;
  var color = color_in * frame.hemi_sky.w;
  let x = min(color.r, min(color.g, color.b));
  let offset = select(0.04, x - 6.25 * x * x, x < 0.08);
  color -= vec3f(offset);
  let peak = max(color.r, max(color.g, color.b));
  if (peak < start) { return color; }
  let d = 1.0 - start;
  let new_peak = 1.0 - d * d / (peak + d - start);
  color *= new_peak / peak;
  let g = 1.0 - 1.0 / (desat * (peak - new_peak) + 1.0);
  return mix(color, vec3f(new_peak), g);
}

fn finish(c: vec3f, depth: f32, fog: f32) -> vec3f {
  var o = to_srgb(neutral(c));
  if (fog > 0.5) { o = mix(o, frame.fog_col.rgb, ss(frame.fog_range.x, frame.fog_range.y, depth)); }
  return o;
}

fn shadow_at(wpos: vec3f, n: vec3f) -> f32 {
  let c = frame.shadow_vp * vec4f(wpos + n * frame.fog_range.w, 1.0);
  let p = c.xyz / c.w;
  let uv = vec2f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
  let z = p.z + frame.shadow_cfg.x;
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0 || z > 1.0) { return 1.0; }
  let t = frame.fog_range.z;
  var s = textureSampleCompareLevel(shadow_map, shadow_smp, uv, z) * 2.0;
  s += textureSampleCompareLevel(shadow_map, shadow_smp, uv + vec2f(-t, -t), z);
  s += textureSampleCompareLevel(shadow_map, shadow_smp, uv + vec2f(t, -t), z);
  s += textureSampleCompareLevel(shadow_map, shadow_smp, uv + vec2f(-t, t), z);
  s += textureSampleCompareLevel(shadow_map, shadow_smp, uv + vec2f(t, t), z);
  return s / 6.0;
}

// Lambert diffuse (hemisphere + key light with shadow + point lights), three.js units.
fn lambert(albedo: vec3f, n: vec3f, wpos: vec3f, receive: bool) -> vec3f {
  var irr = mix(frame.hemi_ground.rgb, frame.hemi_sky.rgb, 0.5 * n.y + 0.5);
  let ndl = saturate(dot(n, frame.key_dir.xyz));
  if (ndl > 0.0) {
    var s = 1.0;
    if (receive && frame.shadow_cfg.y > 0.5) { s = shadow_at(wpos, n); }
    irr += ndl * frame.key_col.rgb * s;
  }
  if (frame.shadow_cfg.z > 0.5) {
    for (var k = 0; k < 4; k++) {
      let l = frame.pl_pos[k].xyz - wpos;
      let d = length(l);
      let att = 1.0 / max(pow(d, frame.pl_col[k].w), 0.01);
      let cut = saturate(1.0 - pow(d / frame.pl_pos[k].w, 4.0));
      irr += saturate(dot(n, l / d)) * frame.pl_col[k].rgb * att * cut * cut;
    }
  }
  return irr * albedo / PI;
}

// ---------------------------------------------------------------- voxel meshes

struct VoxelIn {
  @location(0) pos: vec3f,
  @location(1) nrm: vec4f,
  @location(2) col: vec4f,
}

struct VoxelOut {
  @builtin(position) pos: vec4f,
  @location(0) wpos: vec3f,
  @location(1) nrm: vec3f,
  @location(2) col: vec3f,
  @location(3) misc: vec2f, // x: glow, y: view depth
}

@vertex
fn vs_voxel(i: VoxelIn) -> VoxelOut {
  var p = i.pos;
  let sway = i.col.a;
  if (sway > 0.0) {
    let t = frame.key_dir.w;
    let q = p;
    let gust = 0.65 + 0.35 * sin(t * 0.31 + q.x * 0.015 + q.z * 0.01);
    let ph = t * 1.15 + q.x * 0.13 + q.z * 0.11;
    let s = sway * frame.key_col.w * gust;
    p.x += (sin(ph) * 0.15 + sin(t * 2.8 + q.y * 0.5 + q.z * 0.3) * 0.05) * s;
    p.z += (cos(ph * 0.9 + 1.3) * 0.11 + cos(t * 3.2 + q.x * 0.45) * 0.04) * s;
    p.y += sin(t * 2.1 + q.x * 0.6 + q.z * 0.4) * 0.03 * s;
  }
  var o: VoxelOut;
  let w = object.world * vec4f(p, 1.0);
  o.wpos = w.xyz;
  o.nrm = normalize(mat3x3f(object.world[0].xyz, object.world[1].xyz, object.world[2].xyz) * i.nrm.xyz);
  o.col = to_linear(i.col.rgb);
  o.pos = view.view_proj * w;
  o.misc = vec2f(select(0.0, 1.0, i.nrm.w > 0.004), o.pos.w);
  return o;
}

@fragment
fn fs_voxel(i: VoxelOut) -> @location(0) vec4f {
  var albedo = i.col;
  var emis = object.emissive.rgb;
  if (i.misc.x > 0.5) {
    emis += albedo * frame.hemi_ground.w;
    albedo *= frame.fog_col.w;
  }
  let c = lambert(albedo, normalize(i.nrm), i.wpos, object.emissive.w > 0.5) + emis;
  return vec4f(finish(c, i.misc.y, object.flags.x), 1.0);
}

@vertex
fn vs_shadow(i: VoxelIn) -> @builtin(position) vec4f {
  return frame.shadow_vp * (object.world * vec4f(i.pos, 1.0));
}

// ---------------------------------------------------------------- sakura petals (instanced)

struct PetalOut {
  @builtin(position) pos: vec4f,
  @location(0) wpos: vec3f,
  @location(1) nrm: vec3f,
  @location(2) col: vec3f,
  @location(3) depth: f32,
}

@vertex
fn vs_petal(@builtin(vertex_index) vid: u32, @builtin(instance_index) iid: u32) -> PetalOut {
  let p = petals[iid];
  let c = quad(vid) * vec2f(0.18, 0.14);
  // three.js Euler order 'XYZ': R = Rx * Ry * Rz
  let a = cos(p.rot.x);
  let b = sin(p.rot.x);
  let cy = cos(p.rot.y);
  let d = sin(p.rot.y);
  let e = cos(p.rot.z);
  let f = sin(p.rot.z);
  let local = vec3f(c, 0.0);
  let turned = vec3f(
    dot(vec3f(cy * e, -cy * f, d), local),
    dot(vec3f(a * f + b * e * d, a * e - b * f * d, -b * cy), local),
    dot(vec3f(b * f - a * e * d, b * e + a * f * d, a * cy), local),
  );
  var o: PetalOut;
  o.wpos = p.pos + turned * p.scale;
  o.nrm = vec3f(d, -b * cy, a * cy);
  o.col = frame.petal_col[u32(p.tint)].rgb;
  o.pos = view.view_proj * vec4f(o.wpos, 1.0);
  o.depth = o.pos.w;
  return o;
}

@fragment
fn fs_petal(i: PetalOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
  let n = normalize(i.nrm) * select(-1.0, 1.0, front);
  let c = lambert(i.col, n, i.wpos, false) + frame.petal_emissive.rgb;
  return vec4f(finish(c, i.depth, 1.0), 1.0);
}

// ---------------------------------------------------------------- full-screen passes

struct FsOut {
  @builtin(position) pos: vec4f,
  @location(0) ndc: vec2f,
}

@vertex
fn vs_full(@builtin(vertex_index) vid: u32) -> FsOut {
  let p = vec2f(f32((vid << 1u) & 2u), f32(vid & 2u));
  var o: FsOut;
  o.ndc = p * 2.0 - 1.0;
  o.pos = vec4f(o.ndc, 1.0, 1.0);
  return o;
}

fn hash3(p_in: vec3f) -> f32 {
  var p = fract(p_in * 0.3183099 + 0.1);
  p *= 17.0;
  return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}

@fragment
fn fs_sky(i: FsOut) -> @location(0) vec4f {
  let far = view.inv_view_proj * vec4f(i.ndc, 1.0, 1.0);
  let d = normalize(far.xyz / far.w - view.cam_pos.xyz);
  let h = d.y;
  var col = mix(frame.sky_hor.rgb, frame.sky_zen.rgb, pow(ss(-0.02, 0.85, h), 0.6));
  col = mix(col, frame.sky_gnd.rgb, ss(0.0, -0.3, h));

  let sun_vis = frame.sky_hor.w;
  let sd = max(dot(d, frame.sun_dir.xyz), 0.0);
  col += frame.sun_col.rgb * (pow(sd, 6.0) * 0.22 + pow(sd, 48.0) * 0.45) * sun_vis;
  col += frame.sun_col.rgb * ss(0.99935, 0.9996, sd) * 3.0 * sun_vis;
  col += frame.sun_col.rgb * sd * sd * exp(-abs(h) * 9.0) * 0.18 * sun_vis;

  let moon_vis = frame.sky_gnd.w;
  let md = dot(d, frame.moon_dir.xyz);
  let disc = ss(0.99955, 0.9997, md);
  let off = normalize(frame.moon_dir.xyz + vec3f(0.012, 0.006, 0.0));
  let shade = ss(0.99955, 0.9997, dot(d, off));
  col += vec3f(0.95, 0.97, 1.0) * disc * (1.0 - shade * 0.85) * 1.8 * moon_vis;
  col += vec3f(0.45, 0.55, 0.9) * pow(max(md, 0.0), 300.0) * 0.35 * moon_vis;

  let stars = frame.sky_zen.w;
  if (stars > 0.001 && h > -0.7) {
    let p = d * 230.0;
    let id = floor(p);
    let s = hash3(id);
    if (s > 0.972) {
      let c = id + 0.5 + (vec3f(hash3(id + 3.1), hash3(id + 7.7), hash3(id + 1.9)) - 0.5) * 0.6;
      let tw = 0.55 + 0.45 * sin(frame.key_dir.w * (0.8 + s * 3.0) + s * 91.0);
      let b = ss(0.22, 0.0, length(p - c)) * tw * (s - 0.972) / 0.028;
      col += mix(vec3f(0.8, 0.85, 1.0), vec3f(1.0, 0.9, 0.75), hash3(id + 5.0)) * b * 2.0 * stars * mix(0.25, 1.0, ss(-0.55, 0.1, h));
    }
  }
  return vec4f(to_srgb(neutral(col)), 1.0);
}

// ---------------------------------------------------------------- water & waterfall

struct SurfOut {
  @builtin(position) pos: vec4f,
  @location(0) wpos: vec3f,
  @location(1) uv: vec2f,
}

@vertex
fn vs_surface(i: VoxelIn) -> SurfOut {
  var o: SurfOut;
  o.wpos = (object.world * vec4f(i.pos, 1.0)).xyz;
  o.pos = view.view_proj * vec4f(o.wpos, 1.0);
  o.uv = i.col.rg;
  return o;
}

fn wave(p: vec2f, t: f32) -> f32 {
  return sin(p.x * 1.7 + t * 1.3) * 0.5 + sin(p.y * 2.3 - t * 1.1) * 0.5
       + sin((p.x - p.y) * 3.1 + t * 2.0) * 0.25 + sin((p.x * 0.7 + p.y * 1.3) * 5.3 - t * 2.6) * 0.12;
}

@fragment
fn fs_water(i: SurfOut) -> @location(0) vec4f {
  let t = frame.key_dir.w;
  let p = i.wpos.xz;
  let e = 0.05;
  let h = wave(p, t);
  let n = normalize(vec3f(-(wave(p + vec2f(e, 0.0), t) - h) / e * 0.07, 1.0, -(wave(p + vec2f(0.0, e), t) - h) / e * 0.07));
  let v = normalize(view.cam_pos.xyz - i.wpos);
  let fres = pow(1.0 - max(dot(n, v), 0.0), 3.0);
  let light = frame.water_deep.w;
  let base = mix(frame.water_shallow.rgb, frame.water_deep.rgb, 0.5 + 0.5 * sin(p.x * 0.31) * sin(p.y * 0.37)) * light;
  var col = mix(base, frame.water_sky.rgb, 0.22 + 0.55 * fres);
  let r = reflect(-v, n);
  col += frame.water_sun_col.rgb * pow(max(dot(r, frame.water_sun_dir.xyz), 0.0), 140.0) * frame.water_sky.w;
  let c = ss(0.7, 1.0, sin(p.x * 2.6 + h * 2.0 + t * 0.8) * sin(p.y * 2.2 - h * 1.5 + t * 0.6));
  col += vec3f(c * 0.08 * light);
  col += frame.water_glow.rgb * frame.water_shallow.w * (0.12 + 0.25 * c);
  return vec4f(to_srgb(neutral(col)), clamp(0.66 + fres * 0.28, 0.0, 0.94));
}

// sine-free hash: stays stable for large inputs (long uptimes)
fn hash2(p: vec2f) -> f32 {
  var p3 = fract(vec3f(p.x, p.y, p.x) * 0.1031);
  p3 += vec3f(dot(p3, p3.yzx + 33.33));
  return fract((p3.x + p3.y) * p3.z);
}

fn vnoise(p: vec2f) -> f32 {
  let i = floor(p);
  var f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash2(i), hash2(i + vec2f(1.0, 0.0)), f.x), mix(hash2(i + vec2f(0.0, 1.0)), hash2(i + vec2f(1.0, 1.0)), f.x), f.y);
}

@fragment
fn fs_fall(i: SurfOut) -> @location(0) vec4f {
  let t = frame.key_dir.w;
  let v = i.uv.y;
  let s = vnoise(vec2f(i.uv.x * 6.0, v * 26.0 + t * 4.2)) * 0.6 + vnoise(vec2f(i.uv.x * 14.0, v * 60.0 + t * 7.0)) * 0.4;
  let edge = ss(0.0, 0.2, i.uv.x) * ss(1.0, 0.8, i.uv.x);
  let fade = ss(0.0, 0.6, v);
  let col = mix(frame.fall_col.rgb, vec3f(1.0), ss(0.42, 0.85, s)) * frame.water_sun_dir.w;
  return vec4f(to_srgb(neutral(col)), edge * fade * (0.5 + 0.5 * s));
}

// ---------------------------------------------------------------- billboards (spray, fireflies, glow halos)

struct BoardOut {
  @builtin(position) pos: vec4f,
  @location(0) uv: vec2f,
  @location(1) col: vec4f,
  @location(2) depth: f32,
}

@vertex
fn vs_board(@builtin(vertex_index) vid: u32, @builtin(instance_index) iid: u32) -> BoardOut {
  let b = boards[iid + u32(object.flags.y)];
  let c = quad(vid);
  let w = b.pos + (view.cam_right.xyz * c.x + view.cam_up.xyz * c.y) * (b.size * 0.5);
  var o: BoardOut;
  o.pos = view.view_proj * vec4f(w, 1.0);
  o.uv = c * 0.5 + 0.5;
  o.col = b.col;
  o.depth = o.pos.w;
  return o;
}

@fragment
fn fs_point(i: BoardOut) -> @location(0) vec4f {
  let a = ss(0.5, 0.0, length(i.uv - 0.5)) * i.col.a;
  return vec4f(to_srgb(neutral(i.col.rgb)), a);
}

@fragment
fn fs_glow(i: BoardOut) -> @location(0) vec4f {
  // the canvas gradient of the web version: 1 -> 0.55 -> 0.14 -> 0 at r = 0, 0.18, 0.45, 1
  let r = length(i.uv - 0.5) * 2.0;
  var g = mix(0.14, 0.0, saturate((r - 0.45) / 0.55));
  if (r < 0.18) {
    g = mix(1.0, 0.55, r / 0.18);
  } else if (r < 0.45) {
    g = mix(0.55, 0.14, (r - 0.18) / 0.27);
  }
  return vec4f(finish(i.col.rgb, i.depth, 1.0), g * i.col.a);
}
