// The last step of every frame, ported from PSPost in src/shaders.hlsl: scales a rendered view,
// picture or video frame onto a screen with fit, FXAA (Lottes, console variant) and the vignette.

struct Post {
  info: vec4f, // xy: 1 / source size, z: FXAA on, w: vignette strength
  uv: vec4f,   // xy: scale, zw: offset (output uv -> source uv, for cover / contain)
}

@group(0) @binding(0) var<uniform> post: Post;
@group(0) @binding(1) var post_src: texture_2d<f32>;
@group(0) @binding(2) var linear_smp: sampler;

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

fn fetch(uv: vec2f) -> vec3f {
  return textureSampleLevel(post_src, linear_smp, uv, 0.0).rgb;
}

@fragment
fn fs_post(i: FsOut) -> @location(0) vec4f {
  let screen = vec2f(i.ndc.x * 0.5 + 0.5, 0.5 - i.ndc.y * 0.5);
  let uv = screen * post.uv.xy + post.uv.zw;
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
  let px = post.info.xy;
  var color = fetch(uv);
  if (post.info.z > 0.5) {
    let w = vec3f(0.299, 0.587, 0.114);
    let nw = fetch(uv + vec2f(-1.0, -1.0) * px);
    let ne = fetch(uv + vec2f(1.0, -1.0) * px);
    let sw = fetch(uv + vec2f(-1.0, 1.0) * px);
    let se = fetch(uv + vec2f(1.0, 1.0) * px);
    let l_nw = dot(nw, w);
    let l_ne = dot(ne, w);
    let l_sw = dot(sw, w);
    let l_se = dot(se, w);
    let l_m = dot(color, w);
    let l_min = min(l_m, min(min(l_nw, l_ne), min(l_sw, l_se)));
    let l_max = max(l_m, max(max(l_nw, l_ne), max(l_sw, l_se)));
    if (l_max - l_min > max(0.0312, l_max * 0.125)) {
      var dir = vec2f(-((l_nw + l_ne) - (l_sw + l_se)), (l_nw + l_sw) - (l_ne + l_se));
      let reduce = max((l_nw + l_ne + l_sw + l_se) * (0.25 / 8.0), 1.0 / 128.0);
      dir = clamp(dir / (min(abs(dir.x), abs(dir.y)) + reduce), vec2f(-8.0), vec2f(8.0)) * px;
      let a = 0.5 * (fetch(uv + dir * (1.0 / 3.0 - 0.5)) + fetch(uv + dir * (2.0 / 3.0 - 0.5)));
      let b = a * 0.5 + 0.25 * (fetch(uv - dir * 0.5) + fetch(uv + dir * 0.5));
      let l_b = dot(b, w);
      color = select(b, a, l_b < l_min || l_b > l_max);
    }
  }
  // CSS: radial-gradient(ellipse at 50% 45%, transparent 55%, rgba(6, 8, 20, 0.28) 100%)
  let q = (screen - vec2f(0.5, 0.45)) / vec2f(0.5, 0.45);
  let v = saturate((length(q) / 1.57918 - 0.55) / 0.45) * post.info.w;
  color = mix(color, vec3f(6.0, 8.0, 20.0) / 255.0, v);
  return vec4f(color, 1.0);
}
