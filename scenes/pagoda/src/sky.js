import * as THREE from 'three';

// Time-of-day keyframes (hour -> look). Colours are sRGB hex, converted to linear once.
//  zen/hor/gnd : sky zenith, horizon, below-horizon haze (also fog colour)
//  key/keyC    : sun (day) or moon (night) light intensity + colour
//  hemiS/hemiG : hemisphere fill light sky/ground colour, hemiI its intensity
//  glow        : lanterns & windows, stars : star visibility, exp : exposure
export const RAW = [
  [0.0, { zen: 0x050b1f, hor: 0x18264d, gnd: 0x101a38, key: 0.75, keyC: 0x9db4ff, hemiS: 0x40528a, hemiG: 0x151a2a, hemiI: 0.85, glow: 1, stars: 1, exp: 1.15 }],
  [4.5, { zen: 0x08122e, hor: 0x24315e, gnd: 0x141e40, key: 0.65, keyC: 0x9db4ff, hemiS: 0x44568e, hemiG: 0x171c2c, hemiI: 0.85, glow: 1, stars: 0.9, exp: 1.15 }],
  [5.4, { zen: 0x23356c, hor: 0xd88473, gnd: 0x5b4f72, key: 0.5, keyC: 0xff9a70, hemiS: 0x7d84b8, hemiG: 0x2f2a3c, hemiI: 0.85, glow: 0.75, stars: 0.25, exp: 1.08 }],
  [6.3, { zen: 0x4a78bf, hor: 0xffbd8c, gnd: 0xb3a0b6, key: 1.9, keyC: 0xffb47c, hemiS: 0xb6c4e2, hemiG: 0x5b4f42, hemiI: 0.9, glow: 0.2, stars: 0, exp: 1.0 }],
  [8.0, { zen: 0x3b82d6, hor: 0xbcdef6, gnd: 0x96bde3, key: 2.7, keyC: 0xfff0d8, hemiS: 0xcfe3ff, hemiG: 0x5d6a46, hemiI: 1.05, glow: 0, stars: 0, exp: 1.0 }],
  [12.5, { zen: 0x2e7ce0, hor: 0xcae6ff, gnd: 0x9cc4e9, key: 3.0, keyC: 0xfff8ef, hemiS: 0xd6e9ff, hemiG: 0x627048, hemiI: 1.1, glow: 0, stars: 0, exp: 1.0 }],
  [16.2, { zen: 0x3a7fd0, hor: 0xd8e8f4, gnd: 0xa6c6e2, key: 2.8, keyC: 0xffeccf, hemiS: 0xd3e3f6, hemiG: 0x606a44, hemiI: 1.05, glow: 0, stars: 0, exp: 1.0 }],
  [17.8, { zen: 0x4a6cb3, hor: 0xffc58b, gnd: 0xb9a7b9, key: 2.3, keyC: 0xffb066, hemiS: 0xd9c6c0, hemiG: 0x5a4a38, hemiI: 0.95, glow: 0.12, stars: 0, exp: 1.0 }],
  [18.7, { zen: 0x34407f, hor: 0xff9065, gnd: 0x5e4b7c, key: 1.5, keyC: 0xff7a48, hemiS: 0xa58cb4, hemiG: 0x3a2a30, hemiI: 0.9, glow: 0.6, stars: 0.05, exp: 1.06 }],
  [19.4, { zen: 0x1a2252, hor: 0x7a4a7c, gnd: 0x2b2448, key: 0.35, keyC: 0xa290ff, hemiS: 0x575490, hemiG: 0x1c1a2a, hemiI: 0.8, glow: 0.95, stars: 0.5, exp: 1.1 }],
  [20.5, { zen: 0x060d25, hor: 0x1c2a56, gnd: 0x111b3a, key: 0.75, keyC: 0x9db4ff, hemiS: 0x40528a, hemiG: 0x151a2a, hemiI: 0.85, glow: 1, stars: 1, exp: 1.15 }],
];
const COLOR_KEYS = ['zen', 'hor', 'gnd', 'keyC', 'hemiS', 'hemiG'];
const NUM_KEYS = ['key', 'hemiI', 'glow', 'stars', 'exp'];

const KEYS = RAW.map(([h, k]) => {
  const o = { h };
  for (const c of COLOR_KEYS) o[c] = new THREE.Color(k[c]);
  for (const n of NUM_KEYS) o[n] = k[n];
  return o;
});
KEYS.push({ ...KEYS[0], h: 24 });

const env = {};
for (const c of COLOR_KEYS) env[c] = new THREE.Color();

export function sampleEnv(hour) {
  hour = ((hour % 24) + 24) % 24;
  let k = 0;
  while (k < KEYS.length - 2 && KEYS[k + 1].h <= hour) k++;
  const a = KEYS[k], b = KEYS[k + 1];
  let t = (hour - a.h) / (b.h - a.h);
  t = t * t * (3 - 2 * t);
  for (const c of COLOR_KEYS) env[c].copy(a[c]).lerp(b[c], t);
  for (const n of NUM_KEYS) env[n] = a[n] + (b[n] - a[n]) * t;
  return env;
}

// Sun: rises 06:00 in the east (+x), sets 19:00 in the west (-x), path tilted towards +z.
export function sunAngle(hour) {
  hour = ((hour % 24) + 24) % 24;
  if (hour >= 6 && hour <= 19) return ((hour - 6) / 13) * Math.PI;
  const hh = (hour - 19 + 24) % 24;
  return Math.PI + (hh / 11) * Math.PI;
}

function bodyDir(angle, tilt, yaw, out) {
  out.set(Math.cos(angle), Math.sin(angle) * Math.sin(tilt), Math.sin(angle) * Math.cos(tilt));
  out.applyAxisAngle(new THREE.Vector3(0, 1, 0), yaw);
  return out.normalize();
}

export function sunDirection(hour, out) {
  return bodyDir(sunAngle(hour), 0.95, 0.35, out);
}

export function moonDirection(hour, out) {
  return bodyDir(sunAngle(hour) + Math.PI, 0.75, -0.25, out);
}

// ---------------------------------------------------------------- sky dome

const SKY_VERT = /* glsl */ `
varying vec3 vDir;
void main() {
  vDir = position;
  vec4 p = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  gl_Position = p.xyww;
}
`;

const SKY_FRAG = /* glsl */ `
uniform vec3 uZenith, uHorizon, uGround, uSunDir, uMoonDir, uSunColor;
uniform float uSunVis, uMoonVis, uStars, uTime;
varying vec3 vDir;

float hash(vec3 p) {
  p = fract(p * 0.3183099 + 0.1);
  p *= 17.0;
  return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}

void main() {
  vec3 d = normalize(vDir);
  float h = d.y;
  vec3 col = mix(uHorizon, uZenith, pow(smoothstep(-0.02, 0.85, h), 0.6));
  col = mix(col, uGround, smoothstep(0.0, -0.3, h));

  // sun glow + disc
  float sd = max(dot(d, uSunDir), 0.0);
  col += uSunColor * (pow(sd, 6.0) * 0.22 + pow(sd, 48.0) * 0.45) * uSunVis;
  col += uSunColor * smoothstep(0.99935, 0.9996, sd) * 3.0 * uSunVis;
  // warm band on the horizon around the sun
  col += uSunColor * pow(sd, 2.0) * exp(-abs(h) * 9.0) * 0.18 * uSunVis;

  // moon
  float md = dot(d, uMoonDir);
  float disc = smoothstep(0.99955, 0.9997, md);
  // crescent: subtract an offset disc
  vec3 off = normalize(uMoonDir + vec3(0.012, 0.006, 0.0));
  float shadow = smoothstep(0.99955, 0.9997, dot(d, off));
  col += vec3(0.95, 0.97, 1.0) * disc * (1.0 - shadow * 0.85) * 1.8 * uMoonVis;
  col += vec3(0.45, 0.55, 0.9) * pow(max(md, 0.0), 300.0) * 0.35 * uMoonVis;

  // stars
  // (a floating world: the starfield continues below the horizon, fading into the haze)
  if (uStars > 0.001 && h > -0.7) {
    vec3 p = d * 230.0;
    vec3 id = floor(p);
    float s = hash(id);
    if (s > 0.972) {
      vec3 c = id + 0.5 + (vec3(hash(id + 3.1), hash(id + 7.7), hash(id + 1.9)) - 0.5) * 0.6;
      float dist = length(p - c);
      float tw = 0.55 + 0.45 * sin(uTime * (0.8 + s * 3.0) + s * 91.0);
      float b = smoothstep(0.22, 0.0, dist) * tw * (s - 0.972) / 0.028;
      col += mix(vec3(0.8, 0.85, 1.0), vec3(1.0, 0.9, 0.75), hash(id + 5.0)) * b * 2.0 * uStars * mix(0.25, 1.0, smoothstep(-0.55, 0.1, h));
    }
  }
  gl_FragColor = vec4(col, 1.0);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

export function createSky() {
  const uniforms = {
    uZenith: { value: new THREE.Color() },
    uHorizon: { value: new THREE.Color() },
    uGround: { value: new THREE.Color() },
    uSunColor: { value: new THREE.Color() },
    uSunDir: { value: new THREE.Vector3(0, 1, 0) },
    uMoonDir: { value: new THREE.Vector3(0, -1, 0) },
    uSunVis: { value: 1 },
    uMoonVis: { value: 0 },
    uStars: { value: 0 },
    uTime: { value: 0 },
  };
  const mat = new THREE.ShaderMaterial({
    uniforms,
    vertexShader: SKY_VERT,
    fragmentShader: SKY_FRAG,
    side: THREE.BackSide,
    depthWrite: false,
    depthTest: false,
    fog: false,
  });
  const mesh = new THREE.Mesh(new THREE.SphereGeometry(900, 48, 24), mat);
  mesh.renderOrder = -10;
  mesh.frustumCulled = false;
  return { mesh, uniforms };
}
